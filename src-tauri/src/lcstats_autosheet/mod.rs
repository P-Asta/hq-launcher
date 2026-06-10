mod layouts;
mod sheets;
mod stats;

use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const LCSTATS_SSE_URL: &str = "http://localhost:2145/";
const LCSTATS_RETRY_DELAY: Duration = Duration::from_secs(3);
const LCSTATS_WRITE_TIMEOUT: Duration = Duration::from_secs(45);

#[derive(Clone, Default)]
pub struct LcStatsAutosheetState {
    running: Arc<AtomicBool>,
    listener_running: Arc<AtomicBool>,
    pending_stats: Arc<Mutex<Vec<PendingStatsEntry>>>,
    latest_payload: Arc<Mutex<Option<LatestLcStatsPayload>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LatestLcStatsPayload {
    pub received_at: u64,
    pub raw: String,
    pub stats: Value,
}

pub fn start_for_launch(
    app: tauri::AppHandle,
    enabled: bool,
    state: &tauri::State<'_, LcStatsAutosheetState>,
) {
    if !enabled {
        return;
    }
    match crate::google_oauth::auth_status(app.clone()) {
        Ok(status) if status.authenticated => {}
        Ok(_) => {
            log::info!("LCStatsTracker AutoSheet listener skipped: Google login is not connected");
            return;
        }
        Err(e) => {
            log::error!(
                "LCStatsTracker AutoSheet listener skipped: failed to check Google login: {e}"
            );
            return;
        }
    }
    let settings = match crate::google_oauth::get_settings(app.clone()) {
        Ok(settings) => settings,
        Err(e) => {
            log::error!("LCStatsTracker AutoSheet listener skipped: failed to read settings: {e}");
            return;
        }
    };
    if settings.spreadsheet_id.trim().is_empty() || settings.active_sheet_name.trim().is_empty() {
        log::info!("LCStatsTracker AutoSheet listener skipped: spreadsheet or sheet is not set");
        return;
    }
    if !layouts::is_supported_layout(&settings.layout) {
        log::info!(
            "LCStatsTracker AutoSheet listener skipped: layout {} has no writer yet",
            settings.layout
        );
        return;
    }
    state.running.store(true, Ordering::Release);
    ensure_listener(app, state);
}

pub fn start_manual(
    app: tauri::AppHandle,
    state: &tauri::State<'_, LcStatsAutosheetState>,
) -> Result<bool, String> {
    if !crate::google_oauth::auth_status(app.clone())?.authenticated {
        return Err("Google login is required to track LCStatsTracker.".to_string());
    }
    let settings = crate::google_oauth::get_settings(app.clone())?;
    if settings.spreadsheet_id.trim().is_empty() || settings.active_sheet_name.trim().is_empty() {
        return Err("Spreadsheet and sheet are required to track LCStatsTracker.".to_string());
    }
    if !layouts::is_supported_layout(&settings.layout) {
        return Err(format!("Layout {} has no writer yet.", settings.layout));
    }

    start_for_launch(app, true, state);
    Ok(is_running(state))
}

pub fn stop(state: &tauri::State<'_, LcStatsAutosheetState>) {
    state.running.store(false, Ordering::Release);
}

pub fn is_running(state: &tauri::State<'_, LcStatsAutosheetState>) -> bool {
    state.running.load(Ordering::Acquire)
}

pub fn latest_payload(
    state: &tauri::State<'_, LcStatsAutosheetState>,
) -> Result<Option<LatestLcStatsPayload>, String> {
    state
        .latest_payload
        .lock()
        .map(|payload| payload.clone())
        .map_err(|e| format!("LCStatsTracker latest payload lock failed: {e}"))
}

#[derive(Debug, Clone)]
struct PendingStatsEntry {
    id: u64,
    attempts: u32,
    settings: crate::google_oauth::LcStatsSettings,
    stats: Value,
}

fn ensure_listener(app: tauri::AppHandle, state: &tauri::State<'_, LcStatsAutosheetState>) {
    if state
        .listener_running
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return;
    }

    let state = state.inner().clone();
    tauri::async_runtime::spawn(async move {
        let result = run_listener(app, state.clone()).await;
        if let Err(e) = result {
            log::error!("LCStatsTracker AutoSheet listener stopped: {e}");
        }
        state.listener_running.store(false, Ordering::Release);
    });
}

async fn run_listener(app: tauri::AppHandle, state: LcStatsAutosheetState) -> Result<(), String> {
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(2))
        .build()
        .map_err(|e| e.to_string())?;

    loop {
        if !state.running.load(Ordering::Acquire) {
            tokio::time::sleep(LCSTATS_RETRY_DELAY).await;
            continue;
        }

        let payload = match receive_lcstats_payload(&client).await {
            Ok(payload) => payload.trim().to_string(),
            Err(e) => {
                log::debug!("LCStatsTracker SSE not ready: {e}");
                tokio::time::sleep(LCSTATS_RETRY_DELAY).await;
                continue;
            }
        };
        if payload.is_empty() {
            log::warn!("LCStatsTracker AutoSheet payload ignored: empty payload from local server");
            continue;
        }
        if !state.running.load(Ordering::Acquire) {
            log::debug!("LCStatsTracker AutoSheet payload ignored: tracking is stopped");
            continue;
        }

        let stats: Value = match serde_json::from_str(&payload) {
            Ok(stats) => stats,
            Err(e) => {
                log::error!(
                    "LCStatsTracker AutoSheet payload ignored: failed to parse payload: {e}"
                );
                continue;
            }
        };
        remember_latest_payload(&state, payload, stats.clone())?;
        let settings = match crate::google_oauth::get_settings(app.clone()) {
            Ok(settings) => settings,
            Err(e) => {
                log::error!(
                    "LCStatsTracker AutoSheet payload ignored: failed to read settings: {e}"
                );
                tokio::time::sleep(LCSTATS_RETRY_DELAY).await;
                continue;
            }
        };
        if settings.spreadsheet_id.trim().is_empty()
            || settings.active_sheet_name.trim().is_empty()
            || !layouts::is_supported_layout(&settings.layout)
        {
            log::error!(
                "LCStatsTracker AutoSheet payload ignored: invalid settings for layout {}",
                settings.layout
            );
            continue;
        }

        if let Err(e) = flush_pending_stats(app.clone(), &client, &state).await {
            log::debug!("Failed to flush pending LCStatsTracker AutoSheet writes: {e}");
        }

        match write_stats_with_timeout(app.clone(), &client, &settings, &stats).await {
            Ok(()) => {}
            Err(e) => {
                log::warn!(
                    "Failed to write LCStatsTracker stats to Google Sheets; queued for retry: {e}"
                );
                if let Err(queue_error) = enqueue_pending_stats(&state, settings, stats, e.clone())
                {
                    log::error!(
                        "Failed to keep LCStatsTracker stats in memory for later retry: {queue_error}"
                    );
                } else {
                    log::info!("Kept LCStatsTracker stats in memory fallback queue for retry");
                }
            }
        }
    }
}

fn remember_latest_payload(
    state: &LcStatsAutosheetState,
    raw: String,
    stats: Value,
) -> Result<(), String> {
    let mut latest = state
        .latest_payload
        .lock()
        .map_err(|e| format!("LCStatsTracker latest payload lock failed: {e}"))?;
    *latest = Some(LatestLcStatsPayload {
        received_at: now_epoch_secs(),
        raw,
        stats,
    });
    Ok(())
}

async fn write_stats_with_timeout(
    app: tauri::AppHandle,
    client: &reqwest::Client,
    settings: &crate::google_oauth::LcStatsSettings,
    stats: &Value,
) -> Result<(), String> {
    match tokio::time::timeout(
        LCSTATS_WRITE_TIMEOUT,
        layouts::write_stats(app, client, settings, stats),
    )
    .await
    {
        Ok(result) => result,
        Err(_) => Err("Timed out writing LCStatsTracker stats to Google Sheets".to_string()),
    }
}

async fn flush_pending_stats(
    app: tauri::AppHandle,
    client: &reqwest::Client,
    state: &LcStatsAutosheetState,
) -> Result<(), String> {
    let mut entries = take_pending_stats(state)?;
    if entries.is_empty() {
        return Ok(());
    }

    let mut remaining = Vec::new();
    let total = entries.len();
    while !entries.is_empty() {
        let mut entry = entries.remove(0);
        entry.attempts = entry.attempts.saturating_add(1);
        match write_stats_with_timeout(app.clone(), client, &entry.settings, &entry.stats).await {
            Ok(()) => {
                log::info!(
                    "Retried pending LCStatsTracker AutoSheet write {} successfully",
                    entry.id
                );
            }
            Err(e) => {
                if entry.attempts == 1 {
                    log::error!(
                        "Failed to write queued LCStatsTracker stats to Google Sheets after retry {}: {e}",
                        entry.id
                    );
                } else {
                    log::warn!(
                        "Queued LCStatsTracker AutoSheet write {} still could not be retried: {e}",
                        entry.id
                    );
                }
                remaining.push(entry);
                remaining.extend(entries);
                restore_pending_stats(state, remaining)?;
                return Err(e);
            }
        }
    }

    log::info!("Flushed {total} pending LCStatsTracker AutoSheet writes");
    Ok(())
}

fn enqueue_pending_stats(
    state: &LcStatsAutosheetState,
    settings: crate::google_oauth::LcStatsSettings,
    stats: Value,
    error: String,
) -> Result<(), String> {
    let mut entries = state
        .pending_stats
        .lock()
        .map_err(|e| format!("LCStatsTracker fallback queue lock failed: {e}"))?;
    let queue_len = entries.len() as u64;
    entries.push(PendingStatsEntry {
        id: now_epoch_secs()
            .saturating_mul(1000)
            .saturating_add(queue_len),
        attempts: 0,
        settings,
        stats,
    });
    log::debug!("Queued LCStatsTracker AutoSheet fallback write in memory: {error}");
    Ok(())
}

fn take_pending_stats(state: &LcStatsAutosheetState) -> Result<Vec<PendingStatsEntry>, String> {
    let mut entries = state
        .pending_stats
        .lock()
        .map_err(|e| format!("LCStatsTracker fallback queue lock failed: {e}"))?;
    Ok(std::mem::take(&mut *entries))
}

fn restore_pending_stats(
    state: &LcStatsAutosheetState,
    mut remaining: Vec<PendingStatsEntry>,
) -> Result<(), String> {
    let mut entries = state
        .pending_stats
        .lock()
        .map_err(|e| format!("LCStatsTracker fallback queue lock failed: {e}"))?;
    if entries.is_empty() {
        *entries = remaining;
    } else {
        remaining.extend(std::mem::take(&mut *entries));
        *entries = remaining;
    }
    Ok(())
}

fn now_epoch_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

async fn receive_lcstats_payload(client: &reqwest::Client) -> Result<String, String> {
    let response = client
        .get(LCSTATS_SSE_URL)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !response.status().is_success() {
        return Err(format!("LCStatsTracker SSE returned {}", response.status()));
    }

    let mut buffer = String::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| e.to_string())?;
        buffer.push_str(&String::from_utf8_lossy(&chunk));
        if let Some(payload) = first_complete_sse_payload(&buffer) {
            return Ok(payload);
        }
    }

    Ok(first_sse_payload(&buffer).unwrap_or_default())
}

fn first_complete_sse_payload(text: &str) -> Option<String> {
    let normalized = normalize_sse_text(text);
    let mut rest = normalized.as_str();
    while let Some((event, next)) = rest.split_once("\n\n") {
        if let Some(payload) = event_payload(event) {
            return Some(payload);
        }
        rest = next;
    }
    None
}

fn first_sse_payload(text: &str) -> Option<String> {
    let normalized = normalize_sse_text(text);
    if let Some(payload) = normalized
        .split("\n\n")
        .find_map(|event| event_payload(event))
    {
        return Some(payload);
    }

    let trimmed = normalized.trim();
    if trimmed.starts_with('{') || trimmed.starts_with('[') {
        Some(trimmed.to_string())
    } else {
        None
    }
}

fn event_payload(event: &str) -> Option<String> {
    let data = event
        .lines()
        .filter_map(|line| line.strip_prefix("data:"))
        .map(|line| line.strip_prefix(' ').unwrap_or(line).trim_end())
        .collect::<Vec<_>>()
        .join("\n");
    if data.trim().is_empty() {
        None
    } else {
        Some(data)
    }
}

fn normalize_sse_text(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_complete_sse_event_before_connection_closes() {
        let payload = first_complete_sse_payload("event: stats\r\ndata: {\"quota\":130}\r\n\r\n");

        assert_eq!(payload.as_deref(), Some("{\"quota\":130}"));
    }

    #[test]
    fn does_not_parse_incomplete_streaming_event() {
        let payload = first_complete_sse_payload("data: {\"quota\":130}");

        assert_eq!(payload, None);
    }

    #[test]
    fn parses_final_sse_event_when_server_closes_without_blank_line() {
        let payload = first_sse_payload("data: {\"quota\":130}");

        assert_eq!(payload.as_deref(), Some("{\"quota\":130}"));
    }

    #[test]
    fn accepts_raw_json_payloads_from_non_sse_responses() {
        let payload = first_sse_payload("  {\"quota\":130}\n");

        assert_eq!(payload.as_deref(), Some("{\"quota\":130}"));
    }
}

mod layouts;
mod sheets;
mod stats;

use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::hash_map::DefaultHasher;
use std::collections::VecDeque;
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::Emitter;

const LCSTATS_SSE_URL: &str = "http://localhost:2145/";
const LCSTATS_RETRY_DELAY: Duration = Duration::from_secs(3);
const LCSTATS_WRITE_TIMEOUT: Duration = Duration::from_secs(90);
const LCSTATS_CONFIRM_TIMEOUT: Duration = Duration::from_secs(30);
/// Upper bound for the exponential retry backoff. Once the delay grows past
/// this it stays capped here, so a persistently failing write is retried
/// indefinitely at a steady cadence instead of being dropped.
const LCSTATS_RETRY_BACKOFF_CAP: Duration = Duration::from_secs(300);
const RECENT_WRITTEN_PAYLOAD_LIMIT: usize = 64;
const LCSTATS_WRITE_TIMEOUT_ERROR: &str = "Timed out writing LCStatsTracker stats to Google Sheets";

#[derive(Clone, Default)]
pub struct LcStatsAutosheetState {
    running: Arc<AtomicBool>,
    listener_running: Arc<AtomicBool>,
    next_request_id: Arc<AtomicU64>,
    pending_stats: Arc<Mutex<Vec<PendingStatsEntry>>>,
    latest_payload: Arc<Mutex<Option<LatestLcStatsPayload>>>,
    recent_written_payloads: Arc<Mutex<VecDeque<u64>>>,
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
        log::info!("LCStatsTracker AutoSheet listener skipped: LCStatsTracker mod is disabled");
        return;
    }
    let settings = match crate::google_oauth::get_settings(app.clone()) {
        Ok(settings) => settings,
        Err(e) => {
            log::error!("LCStatsTracker AutoSheet listener skipped: failed to read settings: {e}");
            return;
        }
    };
    if !settings.use_lcstats_api {
        log::info!("LCStatsTracker AutoSheet listener skipped: LCStatsTracker API use is disabled");
        state.running.store(false, Ordering::Release);
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
    log::info!(
        "LCStatsTracker AutoSheet tracking enabled for layout {} on sheet {}",
        settings.layout,
        settings.active_sheet_name
    );
    state.running.store(true, Ordering::Release);
    ensure_listener(app, state);
}

pub fn start_manual(
    app: tauri::AppHandle,
    state: &tauri::State<'_, LcStatsAutosheetState>,
) -> Result<bool, String> {
    let settings = crate::google_oauth::get_settings(app.clone())?;
    if !settings.use_lcstats_api {
        stop(state);
        return Err("LCStatsTracker API use is disabled in launcher settings.".to_string());
    }
    if !crate::google_oauth::auth_status(app.clone())?.authenticated {
        return Err("Google login is required to track LCStatsTracker.".to_string());
    }
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
    if state.running.swap(false, Ordering::AcqRel) {
        log::info!("LCStatsTracker AutoSheet tracking stopped");
    }
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
    request_id: u64,
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
        log::info!("LCStatsTracker AutoSheet listener already running");
        return;
    }

    log::info!("Starting LCStatsTracker AutoSheet listener");
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

        let request_id = next_request_id(&state);
        let stats: Value = match serde_json::from_str(&payload) {
            Ok(stats) => stats,
            Err(e) => {
                log::error!(
                    "LCStatsTracker AutoSheet request {request_id} input error: failed to parse payload: {e}"
                );
                continue;
            }
        };
        let summary = stats::lcstats(&stats);
        log::info!(
            "LCStatsTracker AutoSheet request {request_id} received: payload_bytes={}, seed={}, moon={}",
            payload.len(),
            summary.seed_text(),
            summary.moon_name()
        );
        remember_latest_payload(&state, payload.clone(), stats.clone())?;
        emit_overlay_lcstats_update(&app, payload.clone(), stats.clone());
        let settings = match crate::google_oauth::get_settings(app.clone()) {
            Ok(settings) => settings,
            Err(e) => {
                log::error!(
                    "LCStatsTracker AutoSheet request {request_id} input error: failed to read settings: {e}"
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
                "LCStatsTracker AutoSheet request {request_id} input error: invalid settings for layout {}",
                settings.layout
            );
            continue;
        }
        log::info!(
            "LCStatsTracker AutoSheet request {request_id} input ready: layout={}, sheet={}",
            settings.layout,
            settings.active_sheet_name
        );

        if let Err(e) = flush_pending_stats(app.clone(), &client, &state).await {
            log::debug!("Failed to flush pending LCStatsTracker AutoSheet writes: {e}");
        }

        if !mark_payload_for_write(&state, &payload)? {
            log::info!(
                "LCStatsTracker AutoSheet request {request_id} input ignored: duplicate payload"
            );
            continue;
        }

        let write_outcome = write_and_confirm(app.clone(), &client, &settings, &stats).await;
        match write_outcome {
            WriteOutcome::Confirmed => {
                log::info!(
                    "LCStatsTracker AutoSheet request {request_id} input complete: layout={}, sheet={}",
                    settings.layout,
                    settings.active_sheet_name
                );
            }
            WriteOutcome::NoReceipt => {
                // No check cell to verify (e.g. economy-only payload). The write
                // call itself returned Ok, so treat it as confirmed.
                log::info!(
                    "LCStatsTracker AutoSheet request {request_id} input complete (no receipt): layout={}, sheet={}",
                    settings.layout,
                    settings.active_sheet_name
                );
            }
            WriteOutcome::Unconfirmed(error) => {
                // The write either errored or could not be read back, so the
                // payload is not safely committed. Roll the fingerprint back so
                // a re-delivery is retried, and queue the write for retry with
                // backoff. Retrying is safe because the next attempt re-scans
                // for the first empty row: if the write did land, the scan
                // moves past it and the retry becomes a no-op confirmation.
                let _ = unmark_payload_for_write(&state, &payload);
                log::warn!(
                    "LCStatsTracker AutoSheet request {request_id} input error: write not confirmed; queued for retry: {error}"
                );
                if let Err(queue_error) =
                    enqueue_pending_stats(&state, request_id, settings, stats, error)
                {
                    log::error!(
                        "LCStatsTracker AutoSheet request {request_id} input error: failed to keep in memory retry queue: {queue_error}"
                    );
                } else {
                    log::info!(
                        "LCStatsTracker AutoSheet request {request_id} queued for retry"
                    );
                }
            }
        }
    }
}

/// Outcome of a write + read-back confirmation cycle for one payload.
enum WriteOutcome {
    /// The write landed and was verified by reading the check cell back.
    Confirmed,
    /// The write returned Ok but produced no check cell (e.g. economy-only
    /// payload). Nothing to verify, so it is treated as complete.
    NoReceipt,
    /// The write failed or could not be confirmed. The payload should be
    /// retried. Carries the underlying error message.
    Unconfirmed(String),
}

/// Write a payload and confirm it landed via a read-back of the layout's check
/// cell. The confirmation makes retries safe: if the write committed, the next
/// `first_empty_row_from` scan moves past it, so re-running the same layout
/// becomes a no-op rather than a duplicate row.
async fn write_and_confirm(
    app: tauri::AppHandle,
    client: &reqwest::Client,
    settings: &crate::google_oauth::LcStatsSettings,
    stats: &Value,
) -> WriteOutcome {
    match write_stats_with_timeout(app.clone(), client, settings, stats).await {
        Ok(Some(receipt)) => match confirm_write(app, client, settings, &receipt).await {
            Ok(true) => WriteOutcome::Confirmed,
            Ok(false) => {
                WriteOutcome::Unconfirmed("write check cell was empty after write".to_string())
            }
            Err(error) => WriteOutcome::Unconfirmed(format!("failed to confirm write: {error}")),
        },
        Ok(None) => WriteOutcome::NoReceipt,
        Err(error) => WriteOutcome::Unconfirmed(error),
    }
}

fn emit_overlay_lcstats_update(app: &tauri::AppHandle, raw: String, stats: Value) {
    let payload = serde_json::json!({
        "source": "lcstatstracker",
        "receivedAt": now_epoch_secs(),
        "raw": raw,
        "stats": stats,
    });

    let _ = app.emit("overlay://lcstats-updated", payload);
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
) -> Result<Option<layouts::WriteReceipt>, String> {
    match tokio::time::timeout(
        LCSTATS_WRITE_TIMEOUT,
        layouts::write_stats(app, client, settings, stats),
    )
    .await
    {
        Ok(result) => result,
        Err(_) => Err(LCSTATS_WRITE_TIMEOUT_ERROR.to_string()),
    }
}

/// Read back the check cell identified by `receipt` to confirm the write
/// actually landed on the sheet. Returns `Ok(true)` when the cell is populated
/// (write confirmed), `Ok(false)` when the cell is still empty (write did not
/// commit). An HTTP/parse error is surfaced as `Err`.
async fn confirm_write(
    app: tauri::AppHandle,
    client: &reqwest::Client,
    settings: &crate::google_oauth::LcStatsSettings,
    receipt: &layouts::WriteReceipt,
) -> Result<bool, String> {
    let confirm_result = tokio::time::timeout(
        LCSTATS_CONFIRM_TIMEOUT,
        async {
            let token = crate::google_oauth::access_token(app.clone()).await?;
            let spreadsheet_id = settings.spreadsheet_id.trim();
            let sheet_name = settings.active_sheet_name.trim();
            let cell = format!("{}{}", receipt.column, receipt.row);
            let value = sheets::read_number(client, &token, spreadsheet_id, sheet_name, &cell)
                .await
                .or_else(|_| Ok::<f64, String>(0.0))?;
            // A populated check cell (non-zero numeric) confirms the write. We
            // intentionally also treat read failures as "not confirmed" rather
            // than hard-failing, so the caller queues a safe retry.
            Ok(value != 0.0)
        },
    )
    .await;
    match confirm_result {
        Ok(inner) => inner,
        Err(_) => Ok(false),
    }
}

fn mark_payload_for_write(
    state: &LcStatsAutosheetState,
    raw_payload: &str,
) -> Result<bool, String> {
    let fingerprint = payload_fingerprint(raw_payload);
    let mut recent = state
        .recent_written_payloads
        .lock()
        .map_err(|e| format!("LCStatsTracker duplicate payload lock failed: {e}"))?;
    if recent.contains(&fingerprint) {
        return Ok(false);
    }
    recent.push_back(fingerprint);
    while recent.len() > RECENT_WRITTEN_PAYLOAD_LIMIT {
        recent.pop_front();
    }
    Ok(true)
}

/// Remove a payload's fingerprint from the dedupe ring. Used when a write
/// could not be confirmed so that a future re-delivery of the same payload is
/// retried instead of silently suppressed.
fn unmark_payload_for_write(
    state: &LcStatsAutosheetState,
    raw_payload: &str,
) -> Result<(), String> {
    let fingerprint = payload_fingerprint(raw_payload);
    let mut recent = state
        .recent_written_payloads
        .lock()
        .map_err(|e| format!("LCStatsTracker duplicate payload lock failed: {e}"))?;
    recent.retain(|item| *item != fingerprint);
    Ok(())
}

fn payload_fingerprint(raw_payload: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    raw_payload.trim().hash(&mut hasher);
    hasher.finish()
}

/// Exponential backoff delay for a given retry attempt (1-based). Produces a
/// 3s, 6s, 12s, 24s, 48s, 96s, ... sequence that caps at
/// `LCSTATS_RETRY_BACKOFF_CAP` so a persistently failing write is retried
/// indefinitely at a steady cadence rather than being dropped.
fn retry_backoff_delay(attempt: u32) -> Duration {
    let exponent = attempt.saturating_sub(1).min(7) as u32;
    let base_secs: u64 = LCSTATS_RETRY_DELAY.as_secs();
    let delay_secs = base_secs.saturating_mul(1u64 << exponent);
    Duration::from_secs(delay_secs).min(LCSTATS_RETRY_BACKOFF_CAP)
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

        // Back off before a retry so a transient network/Google issue has time
        // to recover. The first attempt has no delay (attempt == 1). The delay
        // grows exponentially up to LCSTATS_RETRY_BACKOFF_CAP, after which the
        // entry keeps retrying at that capped interval indefinitely — a write
        // is never dropped while the launcher is running.
        if entry.attempts > 1 {
            let delay = retry_backoff_delay(entry.attempts);
            tokio::time::sleep(delay).await;
        }

        log::info!(
            "LCStatsTracker AutoSheet request {} retry {} input ready: layout={}, sheet={}",
            entry.request_id,
            entry.attempts,
            entry.settings.layout,
            entry.settings.active_sheet_name
        );
        match write_and_confirm(app.clone(), client, &entry.settings, &entry.stats).await {
            WriteOutcome::Confirmed | WriteOutcome::NoReceipt => {
                log::info!(
                    "LCStatsTracker AutoSheet request {} retry {} input complete",
                    entry.request_id,
                    entry.attempts
                );
            }
            WriteOutcome::Unconfirmed(error) => {
                if entry.attempts == 1 {
                    log::error!(
                        "LCStatsTracker AutoSheet request {} retry {} input error: {error}",
                        entry.request_id,
                        entry.attempts
                    );
                } else {
                    log::warn!(
                        "LCStatsTracker AutoSheet request {} retry {} still could not be completed: {error}",
                        entry.request_id,
                        entry.attempts
                    );
                }
                remaining.push(entry);
                remaining.extend(entries);
                restore_pending_stats(state, remaining)?;
                return Err(error);
            }
        }
    }

    log::info!("Flushed {total} pending LCStatsTracker AutoSheet writes");
    Ok(())
}

fn enqueue_pending_stats(
    state: &LcStatsAutosheetState,
    request_id: u64,
    settings: crate::google_oauth::LcStatsSettings,
    stats: Value,
    error: String,
) -> Result<(), String> {
    let mut entries = state
        .pending_stats
        .lock()
        .map_err(|e| format!("LCStatsTracker fallback queue lock failed: {e}"))?;
    entries.push(PendingStatsEntry {
        request_id,
        attempts: 0,
        settings,
        stats,
    });
    log::debug!(
        "Queued LCStatsTracker AutoSheet request {request_id} fallback write in memory: {error}"
    );
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

fn next_request_id(state: &LcStatsAutosheetState) -> u64 {
    state.next_request_id.fetch_add(1, Ordering::AcqRel) + 1
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

    #[test]
    fn duplicate_payloads_are_marked_only_once() {
        let state = LcStatsAutosheetState::default();

        assert_eq!(
            mark_payload_for_write(&state, " {\"quota\":130}\n").unwrap(),
            true
        );
        assert_eq!(
            mark_payload_for_write(&state, "{\"quota\":130}").unwrap(),
            false
        );
        assert_eq!(
            mark_payload_for_write(&state, "{\"quota\":160}").unwrap(),
            true
        );
    }
}

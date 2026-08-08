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
use tauri::{Emitter, Manager};

const LCSTATS_SSE_URL: &str = "http://localhost:2145/";
const LCSTATS_RETRY_DELAY: Duration = Duration::from_secs(3);
const LCSTATS_RECONNECT_BASE_DELAY: Duration = Duration::from_secs(1);
const LCSTATS_RECONNECT_BACKOFF_CAP: Duration = Duration::from_secs(3);
const LCSTATS_TCP_KEEPALIVE: Duration = Duration::from_secs(15);
const LCSTATS_TCP_KEEPALIVE_INTERVAL: Duration = Duration::from_secs(5);
const LCSTATS_WRITE_TIMEOUT: Duration = Duration::from_secs(90);
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
        loop {
            let result = run_listener(app.clone(), state.clone()).await;
            if let Err(e) = result {
                log::error!("LCStatsTracker AutoSheet listener stopped unexpectedly: {e}");
            }
            if !state.running.load(Ordering::Acquire) {
                break;
            }
            log::info!("Restarting LCStatsTracker AutoSheet listener after an unexpected stop");
            tokio::time::sleep(LCSTATS_RETRY_DELAY).await;
        }
        state.listener_running.store(false, Ordering::Release);
    });
}

async fn run_listener(app: tauri::AppHandle, state: LcStatsAutosheetState) -> Result<(), String> {
    let mut client = build_lcstats_client()?;
    let mut reconnect_attempt = 0u32;

    loop {
        if !state.running.load(Ordering::Acquire) {
            tokio::time::sleep(LCSTATS_RETRY_DELAY).await;
            continue;
        }

        let payload = match receive_lcstats_payload(&client).await {
            Ok(payload) if !payload.trim().is_empty() => {
                reconnect_attempt = 0;
                payload.trim().to_string()
            }
            Ok(_) => {
                reconnect_attempt = reconnect_attempt.saturating_add(1);
                if reconnect_attempt == 1 {
                    log::warn!(
                        "LCStatsTracker SSE connection closed without a payload; rebuilding connection immediately"
                    );
                } else {
                    log::debug!(
                        "LCStatsTracker SSE reconnect attempt {reconnect_attempt} returned no payload; retrying immediately"
                    );
                }
                // An empty response can be a transient race in the mod's SSE
                // handler. Reconnect before the pending payload is reset or
                // another fast practice-day event is published.
                client = build_lcstats_client()?;
                continue;
            }
            Err(e) => {
                reconnect_attempt = reconnect_attempt.saturating_add(1);
                let delay = reconnect_backoff_delay(reconnect_attempt);
                if reconnect_attempt == 1 {
                    log::warn!(
                        "LCStatsTracker SSE connection lost: {e}; rebuilding connection in {}s",
                        delay.as_secs()
                    );
                } else {
                    log::debug!(
                        "LCStatsTracker SSE reconnect attempt {reconnect_attempt} failed: {e}; retrying in {}s",
                        delay.as_secs()
                    );
                }
                tokio::time::sleep(delay).await;
                client = build_lcstats_client()?;
                continue;
            }
        };
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
        write_overlay_lcstats_file(&app, &payload);
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
            WriteOutcome::TimedOut => {
                log::warn!(
                    "LCStatsTracker AutoSheet request {request_id} input timed out; queued for retry because the row may not have been written"
                );
                if let Err(queue_error) = enqueue_pending_stats(
                    &state,
                    request_id,
                    settings,
                    stats,
                    LCSTATS_WRITE_TIMEOUT_ERROR.to_string(),
                ) {
                    log::error!(
                        "LCStatsTracker AutoSheet request {request_id} input error: failed to keep timed-out write in memory retry queue: {queue_error}"
                    );
                } else if let Err(retry_error) =
                    flush_pending_stats(app.clone(), &client, &state).await
                {
                    log::warn!(
                        "LCStatsTracker AutoSheet request {request_id} timed-out write retry is pending: {retry_error}"
                    );
                }
            }
            WriteOutcome::Failed(error) => {
                // Unambiguous failure: the write did not commit. It is safe to
                // retry because the next attempt re-scans for the first empty
                // row and skips any row that already has content.
                log::warn!(
                    "LCStatsTracker AutoSheet request {request_id} input error: failed to write Google Sheets; queued for retry: {error}"
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

fn build_lcstats_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(2))
        // Preserve the server's intentional day-long HTTP response while
        // letting the OS detect a genuinely dead local socket. Unlike an HTTP
        // read timeout, TCP keepalive does not abandon a healthy long poll.
        .tcp_keepalive(LCSTATS_TCP_KEEPALIVE)
        .tcp_keepalive_interval(LCSTATS_TCP_KEEPALIVE_INTERVAL)
        .tcp_keepalive_retries(3)
        .build()
        .map_err(|e| format!("failed to build LCStatsTracker HTTP client: {e}"))
}

fn reconnect_backoff_delay(attempt: u32) -> Duration {
    let exponent = attempt.saturating_sub(1).min(2);
    let delay_secs = LCSTATS_RECONNECT_BASE_DELAY
        .as_secs()
        .saturating_mul(1u64 << exponent);
    Duration::from_secs(delay_secs).min(LCSTATS_RECONNECT_BACKOFF_CAP)
}

/// Outcome of a write attempt for one payload.
enum WriteOutcome {
    /// The write call returned Ok — treat the payload as committed.
    Confirmed,
    /// The write timed out. The payload may or may not have landed. We keep the
    /// fingerprint committed (so a replay does not create a duplicate row) and
    /// do not retry, accepting a rare miss over a guaranteed duplicate.
    TimedOut,
    /// The write failed with a clear error. Safe to retry because
    /// `first_empty_row_from` will re-scan and skip any committed row.
    Failed(String),
}

/// Write a payload. Read-back verification is intentionally NOT used: Google
/// Sheets has propagation delay, several fields legitimately write 0, and a
/// false "empty" verdict causes the retry to land on the next row — producing
/// the cumulative duplicate-row symptom. Instead we trust the HTTP result and
/// only retry on unambiguous failures.
async fn write_and_confirm(
    app: tauri::AppHandle,
    client: &reqwest::Client,
    settings: &crate::google_oauth::LcStatsSettings,
    stats: &Value,
) -> WriteOutcome {
    match write_stats_with_timeout(app, client, settings, stats).await {
        Ok(_) => WriteOutcome::Confirmed,
        Err(error) if is_write_timeout_error(&error) => WriteOutcome::TimedOut,
        Err(error) => WriteOutcome::Failed(error),
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

/// Persist the latest LCStatsTracker raw payload as a relay file so the native
/// injected overlay can pick it up via its 1-second config poll. The C# mod's
/// SSE server (localhost:2145) is a lossy request-per-packet channel that the
/// launcher reliably wins; this file is the overlay's dependable data source
/// on top of (not instead of) the SSE stream. The payload is the exact raw
/// stats JSON the mod emitted over SSE, so no serialization is needed.
fn write_overlay_lcstats_file(app: &tauri::AppHandle, raw: &str) {
    let Some(dir) = app
        .path()
        .app_data_dir()
        .ok()
        .map(|data_dir| data_dir.join("config").join("overlay"))
    else {
        return;
    };
    if let Err(e) = std::fs::create_dir_all(&dir) {
        log::warn!("Failed to create overlay lcstats relay dir: {e}");
        return;
    }
    let path = dir.join("lcstats.json");
    if let Err(e) = std::fs::write(&path, raw) {
        log::warn!("Failed to write overlay lcstats relay file: {e}");
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

fn payload_fingerprint(raw_payload: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    raw_payload.trim().hash(&mut hasher);
    hasher.finish()
}

fn is_write_timeout_error(error: &str) -> bool {
    error == LCSTATS_WRITE_TIMEOUT_ERROR
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
            WriteOutcome::Confirmed => {
                log::info!(
                    "LCStatsTracker AutoSheet request {} retry {} input complete",
                    entry.request_id,
                    entry.attempts
                );
            }
            WriteOutcome::TimedOut => {
                // A retry that times out is treated like the initial timeout:
                // the row may have landed, so we must not retry again (would
                // create a duplicate). Drop the entry from the queue.
                log::warn!(
                    "LCStatsTracker AutoSheet request {} retry {} timed out; dropped to avoid duplicate rows",
                    entry.request_id,
                    entry.attempts
                );
            }
            WriteOutcome::Failed(error) => {
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

    // Accumulate raw bytes rather than decoding each chunk. A chunk boundary
    // can fall in the middle of a multi-byte UTF-8 sequence (Korean player
    // names, emoji); decoding per-chunk would replace the split halves with
    // U+FFFD and corrupt the JSON, causing serde_json::from_str to fail.
    // Decoding only the final, complete payload avoids that.
    let mut buffer: Vec<u8> = Vec::new();
    let mut stream = response.bytes_stream();
    // This endpoint is a long poll, not a continuously-chatty SSE feed. The
    // server intentionally sends no bytes until the in-game day ends. Do not
    // impose an application-level read timeout here: abandoning a request does
    // not cancel its server-side waiter, and repeated reconnects leave multiple
    // waiters racing to consume/reset the next day payload. TCP keepalive on
    // reqwest's connection still detects genuinely broken sockets.
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| e.to_string())?;
        buffer.extend_from_slice(&chunk);
        if let Some(payload) = first_complete_sse_payload(&buffer) {
            return Ok(payload);
        }
    }

    Ok(first_sse_payload(&buffer).unwrap_or_default())
}

fn first_complete_sse_payload(bytes: &[u8]) -> Option<String> {
    let normalized = normalize_sse_bytes(bytes);
    let mut rest = normalized.as_slice();
    while let Some(split) = find_event_boundary(rest) {
        let event = &rest[..split.start];
        let next = &rest[split.end..];
        if let Some(payload) = event_payload_bytes(event) {
            return Some(payload);
        }
        rest = next;
    }
    None
}

fn first_sse_payload(bytes: &[u8]) -> Option<String> {
    let normalized = normalize_sse_bytes(bytes);
    if let Some(payload) = iterate_sse_events(&normalized).find_map(|event| event_payload_bytes(event))
    {
        return Some(payload);
    }

    let trimmed = trim_bytes(&normalized);
    if trimmed.first().is_some_and(|b| matches!(b, b'{' | b'[')) {
        String::from_utf8(trimmed.to_vec()).ok()
    } else {
        None
    }
}

fn event_payload_bytes(event: &[u8]) -> Option<String> {
    let mut data_parts: Vec<&[u8]> = Vec::new();
    for line in split_lines(event) {
        if let Some(rest) = line.strip_prefix(b"data:") {
            let rest = rest.strip_prefix(b" ").unwrap_or(rest);
            data_parts.push(trim_end_bytes(rest));
        }
    }
    if data_parts.is_empty() {
        return None;
    }
    if data_parts.iter().all(|part| part.is_empty()) {
        return None;
    }
    let mut joined = Vec::with_capacity(
        data_parts
            .iter()
            .map(|part| part.len() + 1)
            .sum::<usize>(),
    );
    for (index, part) in data_parts.iter().enumerate() {
        if index > 0 {
            joined.push(b'\n');
        }
        joined.extend_from_slice(part);
    }
    String::from_utf8(joined).ok()
}

/// Start/end byte offsets of the first `"\n\n"` (or `"\r\n\r\n"`) SSE event
/// delimiter in `bytes`, normalized so both CRLF and LF are recognized.
struct EventBoundary {
    start: usize,
    end: usize,
}

fn find_event_boundary(bytes: &[u8]) -> Option<EventBoundary> {
    let mut index = 0;
    while index + 1 < bytes.len() {
        if bytes[index] == b'\n' {
            if bytes[index + 1] == b'\n' {
                return Some(EventBoundary {
                    start: index,
                    end: index + 2,
                });
            }
        }
        index += 1;
    }
    None
}

fn iterate_sse_events(bytes: &[u8]) -> impl Iterator<Item = &[u8]> {
    struct EventIter<'a> {
        remaining: &'a [u8],
    }
    impl<'a> Iterator for EventIter<'a> {
        type Item = &'a [u8];
        fn next(&mut self) -> Option<&'a [u8]> {
            if self.remaining.is_empty() {
                return None;
            }
            match find_event_boundary(self.remaining) {
                Some(split) => {
                    let event = &self.remaining[..split.start];
                    self.remaining = &self.remaining[split.end..];
                    Some(event)
                }
                None => {
                    let event = self.remaining;
                    self.remaining = &[];
                    if event.is_empty() {
                        None
                    } else {
                        Some(event)
                    }
                }
            }
        }
    }
    EventIter {
        remaining: bytes,
    }
}

fn split_lines(bytes: &[u8]) -> impl Iterator<Item = &[u8]> {
    struct LineIter<'a> {
        remaining: &'a [u8],
    }
    impl<'a> Iterator for LineIter<'a> {
        type Item = &'a [u8];
        fn next(&mut self) -> Option<&'a [u8]> {
            if self.remaining.is_empty() {
                return None;
            }
            match self.remaining.iter().position(|b| *b == b'\n') {
                Some(pos) => {
                    let (line, rest) = self.remaining.split_at(pos);
                    self.remaining = if rest.len() > 1 { &rest[1..] } else { &[] };
                    Some(line)
                }
                None => {
                    let line = self.remaining;
                    self.remaining = &[];
                    Some(line)
                }
            }
        }
    }
    LineIter { remaining: bytes }
}

fn trim_end_bytes(bytes: &[u8]) -> &[u8] {
    let mut end = bytes.len();
    while end > 0 && matches!(bytes[end - 1], b' ' | b'\t' | b'\r') {
        end -= 1;
    }
    &bytes[..end]
}

fn trim_bytes(bytes: &[u8]) -> &[u8] {
    let mut start = 0;
    while start < bytes.len() && matches!(bytes[start], b' ' | b'\t' | b'\n' | b'\r') {
        start += 1;
    }
    let mut end = bytes.len();
    while end > start && matches!(bytes[end - 1], b' ' | b'\t' | b'\n' | b'\r') {
        end -= 1;
    }
    &bytes[start..end]
}

fn normalize_sse_bytes(bytes: &[u8]) -> Vec<u8> {
    // Normalize CRLF and lone CR to LF, operating on raw bytes so a multi-byte
    // UTF-8 sequence split across chunks is never partially decoded.
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'\r' {
            out.push(b'\n');
            if index + 1 < bytes.len() && bytes[index + 1] == b'\n' {
                index += 2;
            } else {
                index += 1;
            }
        } else {
            out.push(bytes[index]);
            index += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_complete_sse_event_before_connection_closes() {
        let payload = first_complete_sse_payload(b"event: stats\r\ndata: {\"quota\":130}\r\n\r\n");

        assert_eq!(payload.as_deref(), Some("{\"quota\":130}"));
    }

    #[test]
    fn does_not_parse_incomplete_streaming_event() {
        let payload = first_complete_sse_payload(b"data: {\"quota\":130}");

        assert_eq!(payload, None);
    }

    #[test]
    fn parses_final_sse_event_when_server_closes_without_blank_line() {
        let payload = first_sse_payload(b"data: {\"quota\":130}");

        assert_eq!(payload.as_deref(), Some("{\"quota\":130}"));
    }

    #[test]
    fn accepts_raw_json_payloads_from_non_sse_responses() {
        let payload = first_sse_payload(b"  {\"quota\":130}\n");

        assert_eq!(payload.as_deref(), Some("{\"quota\":130}"));
    }

    #[test]
    fn parses_multibyte_payload_split_across_chunk_boundary() {
        // "Asta":"멍늅잉" — the Korean name is 9 UTF-8 bytes (3 per syllable).
        // Splitting the JSON in the middle of the name must still decode to the
        // original string rather than producing U+FFFD replacement characters
        // that would make serde_json::from_str fail.
        let name = "멍늅잉";
        let full: Vec<u8> = format!("data: {{\"Asta\":\"{}\"}}\n\n", name).into_bytes();
        // Cut inside the first 3-byte syllable (after 1 byte of it).
        let split_at = full
            .windows(name.len())
            .position(|w| w == name.as_bytes())
            .unwrap()
            + 1;
        let part1 = &full[..split_at];
        let part2 = &full[split_at..];

        let mut combined = Vec::with_capacity(full.len());
        combined.extend_from_slice(part1);
        // The first chunk alone (cut mid-codepoint) must NOT yield a payload,
        // because the event is incomplete (\n\n not reached yet) — and even if
        // it were, we must never decode a partial buffer.
        assert_eq!(first_complete_sse_payload(&combined), None);

        combined.extend_from_slice(part2);
        let payload = first_complete_sse_payload(&combined).expect("joined payload parses");
        let parsed: serde_json::Value = serde_json::from_str(&payload).unwrap();
        assert_eq!(parsed["Asta"].as_str(), Some(name));
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

    #[test]
    fn reconnect_backoff_is_bounded() {
        assert_eq!(reconnect_backoff_delay(1), Duration::from_secs(1));
        assert_eq!(reconnect_backoff_delay(2), Duration::from_secs(2));
        assert_eq!(reconnect_backoff_delay(3), Duration::from_secs(3));
        assert_eq!(reconnect_backoff_delay(4), Duration::from_secs(3));
        assert_eq!(reconnect_backoff_delay(u32::MAX), Duration::from_secs(3));
    }
}

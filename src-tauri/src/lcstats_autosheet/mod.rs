mod layouts;
mod sheets;
mod stats;

use serde_json::Value;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

const LCSTATS_SSE_URL: &str = "http://localhost:2145/";

#[derive(Clone, Default)]
pub struct LcStatsAutosheetState {
    running: Arc<AtomicBool>,
}

pub fn start_for_launch(
    app: tauri::AppHandle,
    enabled: bool,
    state: &tauri::State<'_, LcStatsAutosheetState>,
) {
    if !enabled {
        return;
    }
    if state
        .running
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return;
    }

    let state = state.inner().clone();
    tauri::async_runtime::spawn(async move {
        let result = run_listener(app).await;
        if let Err(e) = result {
            log::warn!("LCStatsTracker AutoSheet listener stopped: {e}");
        }
        state.running.store(false, Ordering::Release);
    });
}

async fn run_listener(app: tauri::AppHandle) -> Result<(), String> {
    let settings = crate::google_oauth::get_settings(app.clone())?;
    if settings.spreadsheet_id.trim().is_empty() || settings.active_sheet_name.trim().is_empty() {
        log::info!("LCStatsTracker AutoSheet listener skipped: spreadsheet or sheet is not set");
        return Ok(());
    }
    if !layouts::is_supported_layout(&settings.layout) {
        log::info!(
            "LCStatsTracker AutoSheet listener skipped: layout {} has no writer yet",
            settings.layout
        );
        return Ok(());
    }

    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(2))
        .build()
        .map_err(|e| e.to_string())?;
    let started = Instant::now();
    let max_runtime = Duration::from_secs(12 * 60 * 60);

    while started.elapsed() < max_runtime {
        match receive_lcstats_payload(&client).await {
            Ok(payload) => {
                let payload = payload.trim().to_string();
                if payload.is_empty() {
                    continue;
                }
                let stats: Value = serde_json::from_str(&payload)
                    .map_err(|e| format!("failed to parse LCStatsTracker payload: {e}"))?;
                let settings = crate::google_oauth::get_settings(app.clone())?;
                if settings.spreadsheet_id.trim().is_empty()
                    || settings.active_sheet_name.trim().is_empty()
                    || !layouts::is_supported_layout(&settings.layout)
                {
                    log::warn!(
                        "LCStatsTracker AutoSheet payload ignored: invalid settings for layout {}",
                        settings.layout
                    );
                    continue;
                }
                if let Err(e) = layouts::write_stats(app.clone(), &client, &settings, &stats).await
                {
                    log::warn!("Failed to write LCStatsTracker stats to Google Sheets: {e}");
                }
            }
            Err(e) => {
                log::debug!("LCStatsTracker SSE not ready: {e}");
                tokio::time::sleep(Duration::from_secs(3)).await;
            }
        }
    }

    Ok(())
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
    let text = response.text().await.map_err(|e| e.to_string())?;
    Ok(text
        .lines()
        .filter_map(|line| line.strip_prefix("data:"))
        .map(str::trim)
        .collect::<Vec<_>>()
        .join("\n"))
}

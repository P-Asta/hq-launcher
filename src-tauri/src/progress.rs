use serde::Serialize;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

/// Frontend-facing progress event payload for long-running tasks.
///
/// Event name: `download://progress`
#[derive(Debug, Clone, Serialize)]
pub struct TaskProgressPayload {
    pub version: u32,

    // Generic "multi-step task" progress
    pub steps_total: u32,
    pub step: u32, // 1-based
    pub step_name: String,
    pub step_progress: f64,   // 0.0..=1.0
    pub overall_percent: f64, // 0.0..=100.0

    // Optional details (used by download/unzip/install phases)
    pub detail: Option<String>,
    pub downloaded_bytes: Option<u64>,
    pub total_bytes: Option<u64>,
    pub extracted_files: Option<u64>,
    pub total_files: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TaskUpdatableProgressPayload {
    pub version: u32,
    pub run_mode: Option<String>,
    pub total: u64,
    pub checked: u64,
    pub updatable_mods: Vec<String>,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TaskFinishedPayload {
    pub version: u32,
    pub run_mode: Option<String>,
    pub path: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct TaskErrorPayload {
    pub version: u32,
    pub run_mode: Option<String>,
    pub message: String,
}

/// Minimum spacing between two `download://progress` events for the *same*
/// step. Downloads report per network chunk, which can be hundreds of times a
/// second; every event is an IPC round-trip plus a React re-render, so
/// intermediate updates above ~20/s are pure overhead.
const PROGRESS_EMIT_MIN_INTERVAL: Duration = Duration::from_millis(50);

struct LastProgressEmit {
    at: Instant,
    version: u32,
    step: u32,
    step_name: String,
}

static LAST_PROGRESS_EMIT: Mutex<Option<LastProgressEmit>> = Mutex::new(None);

/// Decides whether a progress event may be dropped.
///
/// Never drops boundary events — the first event, any change of version, step
/// or step name, and any payload that reports completion — so the UI can only
/// ever miss a sub-100% update of a step it is already showing, and only for
/// [`PROGRESS_EMIT_MIN_INTERVAL`].
fn should_emit(last: &Option<LastProgressEmit>, now: Instant, payload: &TaskProgressPayload) -> bool {
    let Some(last) = last.as_ref() else {
        return true;
    };
    if payload.version != last.version
        || payload.step != last.step
        || payload.step_name != last.step_name
        || payload.step_progress >= 1.0
        || payload.overall_percent >= 100.0
    {
        return true;
    }
    now.duration_since(last.at) >= PROGRESS_EMIT_MIN_INTERVAL
}

pub fn emit_progress(app: &AppHandle, payload: TaskProgressPayload) {
    let now = Instant::now();
    match LAST_PROGRESS_EMIT.lock() {
        Ok(mut last) => {
            if !should_emit(&last, now, &payload) {
                return;
            }
            *last = Some(LastProgressEmit {
                at: now,
                version: payload.version,
                step: payload.step,
                step_name: payload.step_name.clone(),
            });
        }
        // A poisoned lock must never cost us an event.
        Err(_) => {}
    }
    let _ = app.emit("download://progress", payload);
}

pub fn emit_finished(app: &AppHandle, payload: TaskFinishedPayload) {
    let _ = app.emit("download://finished", payload);
}

pub fn emit_error(app: &AppHandle, payload: TaskErrorPayload) {
    let _ = app.emit("download://error", payload);
}

pub fn emit_updatable_progress(app: &AppHandle, payload: TaskUpdatableProgressPayload) {
    let _ = app.emit("updatable://progress", payload);
}

pub fn emit_updatable_finished(app: &AppHandle, payload: TaskFinishedPayload) {
    let _ = app.emit("updatable://finished", payload);
}

pub fn emit_updatable_error(app: &AppHandle, payload: TaskErrorPayload) {
    let _ = app.emit("updatable://error", payload);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn payload(version: u32, step: u32, name: &str, progress: f64, overall: f64) -> TaskProgressPayload {
        TaskProgressPayload {
            version,
            steps_total: 3,
            step,
            step_name: name.to_string(),
            step_progress: progress,
            overall_percent: overall,
            detail: None,
            downloaded_bytes: None,
            total_bytes: None,
            extracted_files: None,
            total_files: None,
        }
    }

    fn last(at: Instant, version: u32, step: u32, name: &str) -> Option<LastProgressEmit> {
        Some(LastProgressEmit { at, version, step, step_name: name.to_string() })
    }

    #[test]
    fn first_event_always_emits() {
        assert!(should_emit(&None, Instant::now(), &payload(70, 1, "Download", 0.0, 0.0)));
    }

    #[test]
    fn same_step_within_interval_is_dropped() {
        let now = Instant::now();
        let l = last(now, 70, 1, "Download");
        assert!(!should_emit(&l, now + Duration::from_millis(10), &payload(70, 1, "Download", 0.4, 13.0)));
    }

    #[test]
    fn same_step_after_interval_emits() {
        let now = Instant::now();
        let l = last(now, 70, 1, "Download");
        assert!(should_emit(&l, now + PROGRESS_EMIT_MIN_INTERVAL, &payload(70, 1, "Download", 0.4, 13.0)));
    }

    #[test]
    fn boundary_events_are_never_dropped() {
        let now = Instant::now();
        let l = last(now, 70, 1, "Download");
        let soon = now + Duration::from_millis(1);
        // step change
        assert!(should_emit(&l, soon, &payload(70, 2, "Extract", 0.0, 33.4)));
        // step name change
        assert!(should_emit(&l, soon, &payload(70, 1, "Verify", 0.1, 3.0)));
        // version change
        assert!(should_emit(&l, soon, &payload(71, 1, "Download", 0.1, 3.0)));
        // step completion
        assert!(should_emit(&l, soon, &payload(70, 1, "Download", 1.0, 33.3)));
        // overall completion
        assert!(should_emit(&l, soon, &payload(70, 1, "Download", 0.99, 100.0)));
    }
}

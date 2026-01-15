use serde::Serialize;
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
pub struct TaskFinishedPayload {
    pub version: u32,
    pub path: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct TaskErrorPayload {
    pub version: u32,
    pub message: String,
}

pub fn emit_progress(app: &AppHandle, payload: TaskProgressPayload) {
    let _ = app.emit("download://progress", payload);
}

pub fn emit_finished(app: &AppHandle, payload: TaskFinishedPayload) {
    let _ = app.emit("download://finished", payload);
}

pub fn emit_error(app: &AppHandle, payload: TaskErrorPayload) {
    let _ = app.emit("download://error", payload);
}


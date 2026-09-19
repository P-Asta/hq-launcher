//! Small helpers that were duplicated verbatim across modules.

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) fn url_encode(value: &str) -> String {
    let mut out = String::new();
    for b in value.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            b' ' => out.push_str("%20"),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

pub(crate) fn now_epoch_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

pub(crate) fn dir_has_any_entries(path: &Path) -> bool {
    std::fs::read_dir(path)
        .ok()
        .and_then(|mut rd| rd.next())
        .is_some()
}

pub(crate) fn has_legacy_complete_files(path: &Path) -> bool {
    path.join("Lethal Company.exe").is_file()
        && path.join("UnityPlayer.dll").is_file()
        && path.join("Lethal Company_Data").is_dir()
        && path.join("winhttp.dll").is_file()
        && path.join("BepInEx").join("core").is_dir()
}

pub(crate) fn overall_from_step(step: u32, step_progress: f64, steps_total: u32) -> f64 {
    let s = step.max(1).min(steps_total) as f64;
    let sp = step_progress.clamp(0.0, 1.0);
    (((s - 1.0) + sp) / (steps_total as f64)) * 100.0
}

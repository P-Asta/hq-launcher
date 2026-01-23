mod bepinex_cfg;
mod downloader;
mod installer;
mod logger;
mod mod_config;
mod mods;
mod progress;
mod thunderstore;
mod zip_utils;
mod variable;

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::sync::Mutex;
use tauri::{Manager, State};

use crate::bepinex_cfg::read_manifest;
use crate::progress::{TaskErrorPayload, TaskProgressPayload};
use crate::{
    mod_config::ModsConfig,
    progress::{TaskFinishedPayload, TaskUpdatableProgressPayload},
};

fn overall_from_step(step: u32, step_progress: f64, steps_total: u32) -> f64 {
    if steps_total == 0 {
        return 0.0;
    }
    let step0 = step.saturating_sub(1) as f64;
    let steps = steps_total as f64;
    (((step0 + step_progress.clamp(0.0, 1.0)) / steps) * 100.0).clamp(0.0, 100.0)
}

#[derive(Debug, Clone, Serialize)]
struct ManifestDto {
    version: u32,
    chain_config: Vec<Vec<String>>,
    mods: Vec<mod_config::ModEntry>,
    manifests: BTreeMap<u32, String>,
}

fn shared_config_dir(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    Ok(app
        .path()
        .app_data_dir()
        .map_err(|e| format!("failed to resolve app data dir: {e}"))?
        .join("config")
        .join("shared"))
}

fn is_safe_rel_path(rel: &std::path::Path) -> bool {
    use std::path::Component;
    rel.components().all(|c| match c {
        Component::Normal(_) => true,
        _ => false, // reject Prefix/RootDir/CurDir/ParentDir
    })
}

fn version_dir(app: &tauri::AppHandle, version: u32) -> Result<std::path::PathBuf, String> {
    Ok(app
        .path()
        .app_data_dir()
        .map_err(|e| format!("failed to resolve app data dir: {e}"))?
        .join("versions")
        .join(format!("v{version}")))
}

fn find_file_named(
    root: &std::path::Path,
    target_name: &str,
    max_depth: usize,
) -> Option<std::path::PathBuf> {
    let target_lower = target_name.to_lowercase();
    let mut stack: Vec<(std::path::PathBuf, usize)> = vec![(root.to_path_buf(), 0)];

    while let Some((dir, depth)) = stack.pop() {
        if depth > max_depth {
            continue;
        }
        let rd = std::fs::read_dir(&dir).ok()?;
        for e in rd.flatten() {
            let path = e.path();
            let ft = e.file_type().ok()?;
            if ft.is_dir() {
                stack.push((path, depth + 1));
                continue;
            }
            if !ft.is_file() {
                continue;
            }
            let name = path.file_name().and_then(|s| s.to_str())?;
            if name.to_lowercase() == target_lower {
                return Some(path);
            }
        }
    }
    None
}

fn plugins_dir(app: &tauri::AppHandle, version: u32) -> Result<std::path::PathBuf, String> {
    Ok(version_dir(app, version)?.join("BepInEx").join("plugins"))
}

fn mod_folder_name(dev: &str, name: &str) -> String {
    format!("{dev}-{name}")
}

fn mod_dir_for(
    plugins_dir: &std::path::Path,
    dev: &str,
    name: &str,
) -> Option<std::path::PathBuf> {
    // Fast paths (common on Windows; case-insensitive FS).
    let direct = plugins_dir.join(mod_folder_name(dev, name));
    if direct.exists() {
        return Some(direct);
    }

    let dev_l = dev.trim().to_lowercase();
    let name_l = name.trim().to_lowercase();
    let lowered = plugins_dir.join(mod_folder_name(&dev_l, &name_l));
    if lowered.exists() {
        return Some(lowered);
    }

    // Fallback: scan directories and match case-insensitively.
    let target = mod_folder_name(&dev_l, &name_l);
    let Ok(rd) = std::fs::read_dir(plugins_dir) else {
        return None;
    };
    for e in rd.flatten() {
        let path = e.path();
        if !path.is_dir() {
            continue;
        }
        let Some(folder) = path.file_name().and_then(|s| s.to_str()) else {
            continue;
        };
        if folder.to_lowercase() == target {
            return Some(path);
        }
    }
    None
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct DisabledMod {
    dev: String,
    name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct InstalledModVersion {
    dev: String,
    name: String,
    version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DisableModFile {
    version: u32,
    mods: Vec<DisabledMod>,
}

fn disablemod_path(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    Ok(app
        .path()
        .app_data_dir()
        .map_err(|e| format!("failed to resolve app data dir: {e}"))?
        .join("config")
        .join("disablemod.json"))
}

pub(crate) fn thunderstore_cache_path(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    Ok(app
        .path()
        .app_data_dir()
        .map_err(|e| format!("failed to resolve app data dir: {e}"))?
        .join("cache")
        .join("thunderstore.json"))
}

fn read_disablemod(app: &tauri::AppHandle) -> Result<DisableModFile, String> {
    let path = disablemod_path(app)?;
    let default_mod = normalize_mod_id("SlushyRH", "FreeeeeeMoooooons");
    if !path.exists() {
        // v2 (migration): include default disabled mod entry.
        let f = DisableModFile {
            version: 2,
            mods: vec![default_mod],
        };
        // best-effort persist so frontend sees stable state
        let _ = write_disablemod(app, &f);
        return Ok(f);
    }
    let text = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let mut f = match serde_json::from_str::<DisableModFile>(&text) {
        Ok(v) => v,
        Err(e) => {
            // If the file is corrupted, recover with defaults rather than breaking the UI.
            log::warn!("Failed to parse disablemod.json, resetting: {e}");
            let f = DisableModFile {
                version: 2,
                mods: vec![default_mod],
            };
            let _ = write_disablemod(app, &f);
            return Ok(f);
        }
    };

    // Migration: v1 -> v2
    if f.version == 1 {
        f.version = 2;
        f.mods.push(default_mod);
        f.mods
            .sort_by(|a, b| a.dev.cmp(&b.dev).then(a.name.cmp(&b.name)));
        f.mods.dedup();
        let _ = write_disablemod(app, &f);
    }

    Ok(f)
}

fn write_disablemod(app: &tauri::AppHandle, f: &DisableModFile) -> Result<(), String> {
    let path = disablemod_path(app)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(f).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| e.to_string())
}

fn normalize_mod_id(dev: &str, name: &str) -> DisabledMod {
    DisabledMod {
        dev: dev.trim().to_lowercase(),
        name: name.trim().to_lowercase(),
    }
}

fn for_each_file_recursive(
    root: &std::path::Path,
    mut f: impl FnMut(&std::path::Path) -> Result<(), String>,
) -> Result<(), String> {
    let mut stack: Vec<std::path::PathBuf> = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for e in std::fs::read_dir(&dir).map_err(|e| e.to_string())? {
            let e = e.map_err(|e| e.to_string())?;
            let path = e.path();
            let ty = e.file_type().map_err(|e| e.to_string())?;
            if ty.is_dir() {
                stack.push(path);
            } else if ty.is_file() {
                f(&path)?;
            }
        }
    }
    Ok(())
}

fn set_mod_files_old_suffix(mod_dir: &std::path::Path, enabled: bool) -> Result<(), String> {
    if !mod_dir.exists() {
        return Ok(());
    }

    if enabled {
        // Remove .old suffix
        for_each_file_recursive(mod_dir, |path| {
            let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
            if !name.to_lowercase().ends_with(".old") {
                return Ok(());
            }
            let mut new_name = name.to_string();
            new_name.truncate(new_name.len().saturating_sub(4));
            let new_path = path.with_file_name(new_name);
            if new_path.exists() {
                // Don't overwrite; keep the .old file.
                return Ok(());
            }
            std::fs::rename(path, new_path).map_err(|e| e.to_string())
        })
    } else {
        // Add .old suffix
        for_each_file_recursive(mod_dir, |path| {
            let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
            if name.to_lowercase().ends_with(".old") {
                return Ok(());
            }
            let new_path = path.with_file_name(format!("{name}.old"));
            if new_path.exists() {
                return Ok(());
            }
            std::fs::rename(path, new_path).map_err(|e| e.to_string())
        })
    }
}

// (intentionally no "is_disabled"/"is_mod_enabled" helpers; frontend uses disablemod list as source of truth)

fn apply_disabled_mods_for_version(app: &tauri::AppHandle, version: u32) -> Result<(), String> {
    let list = read_disablemod(app)?;
    let plugins = plugins_dir(app, version)?;
    for m in list.mods {
        if let Some(dir) = mod_dir_for(&plugins, &m.dev, &m.name) {
            let _ = set_mod_files_old_suffix(&dir, false);
        }
    }
    Ok(())
}

fn hqol_mod_dir(plugins_dir: &std::path::Path) -> Option<std::path::PathBuf> {
    mod_dir_for(plugins_dir, "HQHQTeam", "HQoL")
        .or_else(|| mod_dir_for(plugins_dir, "HQHQTeam", "HQOL"))
}

fn sync_hqol_with_disablemod_for_version(app: &tauri::AppHandle, version: u32) -> Result<(), String> {
    let list = read_disablemod(app)?;
    let id1 = normalize_mod_id("HQHQTeam", "HQoL");
    let id2 = normalize_mod_id("HQHQTeam", "HQOL");
    let disabled = list.mods.iter().any(|m| m == &id1 || m == &id2);

    let plugins = plugins_dir(app, version)?;
    if let Some(dir) = hqol_mod_dir(&plugins) {
        let _ = set_mod_files_old_suffix(&dir, !disabled);
    }
    Ok(())
}

fn ensure_practice_mods_disabled_for_version(app: &tauri::AppHandle, version: u32) -> Result<(), String> {
    let practice = variable::get_practice_mod_list();
    let mut list = read_disablemod(app)?;

    // Force-disable all practice mods globally (source of truth for the UI).
    for m in &practice {
        let id = normalize_mod_id(&m.dev, &m.name);
        list.mods.retain(|x| x != &id);
        list.mods.push(id);
    }
    list.mods
        .sort_by(|a, b| a.dev.cmp(&b.dev).then(a.name.cmp(&b.name)));
    list.mods.dedup();
    write_disablemod(app, &list)?;

    // Apply for this version immediately.
    let plugins = plugins_dir(app, version)?;
    for m in practice {
        if let Some(dir) = mod_dir_for(&plugins, &m.dev, &m.name) {
            let _ = set_mod_files_old_suffix(&dir, false);
        }
    }

    Ok(())
}

async fn prepare_practice_mods_for_version(app: &tauri::AppHandle, version: u32) -> Result<(), String> {
    let game_root = version_dir(app, version)?;
    if !game_root.exists() {
        return Err(format!(
            "version folder not found: {}",
            game_root.to_string_lossy()
        ));
    }

    let practice_all = variable::get_practice_mod_list();
    let practice_enabled: Vec<mod_config::ModEntry> = practice_all
        .iter()
        .cloned()
        .filter(|m| m.is_compatible(version))
        .collect();

    // Emit progress so the UI can show work (practice installs can be slow).
    const STEPS_TOTAL: u32 = 1;
    progress::emit_progress(
        app,
        TaskProgressPayload {
            version,
            steps_total: STEPS_TOTAL,
            step: 1,
            step_name: "Practice Mods".to_string(),
            step_progress: 0.0,
            overall_percent: 0.0,
            detail: Some("Preparing practice mods...".to_string()),
            downloaded_bytes: None,
            total_bytes: None,
            extracted_files: Some(0),
            total_files: Some(practice_enabled.len() as u64),
        },
    );

    // Install enabled practice mods additively (no overwrite).
    let cfg = ModsConfig {
        mods: practice_enabled.clone(),
    };

    let install_res: Result<(), String> = mods::install_mods_with_progress(
        app,
        &game_root,
        version,
        &cfg,
        |done, total, detail| {
            let step_progress = if total == 0 {
                1.0
            } else {
                (done as f64 / total as f64).clamp(0.0, 1.0)
            };
            progress::emit_progress(
                app,
                TaskProgressPayload {
                    version,
                    steps_total: STEPS_TOTAL,
                    step: 1,
                    step_name: "Practice Mods".to_string(),
                    step_progress,
                    overall_percent: overall_from_step(1, step_progress, STEPS_TOTAL),
                    detail,
                    downloaded_bytes: None,
                    total_bytes: None,
                    extracted_files: Some(done),
                    total_files: Some(total),
                },
            );
        },
    )
    .await;

    if let Err(e) = &install_res {
        progress::emit_error(
            app,
            TaskErrorPayload {
                version,
                message: e.clone(),
            },
        );
        return Err(e.clone());
    }

    // Update disable list: practice mods are disabled by default, except compatible ones for this version.
    let mut list = read_disablemod(app)?;
    let all_ids: Vec<DisabledMod> = practice_all
        .iter()
        .map(|m| normalize_mod_id(&m.dev, &m.name))
        .collect();
    let enabled_ids: Vec<DisabledMod> = practice_enabled
        .iter()
        .map(|m| normalize_mod_id(&m.dev, &m.name))
        .collect();

    // Remove any existing entries for practice mods.
    list.mods.retain(|m| !all_ids.contains(m));
    // Add all practice mods as disabled, then remove the enabled subset.
    for id in &all_ids {
        list.mods.push(id.clone());
    }
    list.mods.retain(|m| !enabled_ids.contains(m));
    list.mods
        .sort_by(|a, b| a.dev.cmp(&b.dev).then(a.name.cmp(&b.name)));
    list.mods.dedup();
    write_disablemod(app, &list)?;

    // Apply filesystem state for this version: disable all practice mods, then enable compatible subset.
    let plugins = plugins_dir(app, version)?;
    for m in &practice_all {
        if let Some(dir) = mod_dir_for(&plugins, &m.dev, &m.name) {
            let _ = set_mod_files_old_suffix(&dir, false);
        }
    }
    for m in &practice_enabled {
        if let Some(dir) = mod_dir_for(&plugins, &m.dev, &m.name) {
            let _ = set_mod_files_old_suffix(&dir, true);
        }
    }

    // Special rule: when running Practice and Imperium is installed, force-disable HQoL (HQHQTeam).
    // Otherwise, HQoL should follow disablemod.json state.
    let imperium_installed = mod_dir_for(&plugins, "giosuel", "Imperium").is_some();
    if let Some(hqol_dir) = hqol_mod_dir(&plugins) {
        if imperium_installed {
            let _ = set_mod_files_old_suffix(&hqol_dir, false);
        } else {
            // Re-sync HQoL to user's config.
            let _ = sync_hqol_with_disablemod_for_version(app, version);
        }
    }

    progress::emit_finished(
        app,
        TaskFinishedPayload {
            version,
            path: game_root.to_string_lossy().to_string(),
        },
    );

    Ok(())
}

#[derive(Default)]
struct GameState {
    child: Mutex<Option<std::process::Child>>,
}

#[derive(Default)]
struct DownloadState {
    active: Mutex<Option<ActiveDownload>>,
}

struct ActiveDownload {
    version: u32,
    cancel: Arc<AtomicBool>,
}

#[derive(Debug, Clone, Serialize)]
struct GameStatus {
    running: bool,
    pid: Option<u32>,
}

#[tauri::command]
async fn download(
    app: tauri::AppHandle,
    version: u32,
    state: State<'_, DownloadState>,
) -> Result<bool, String> {
    // Only allow one active download at a time (simplifies cancel + UI state).
    let cancel = Arc::new(AtomicBool::new(false));
    {
        let mut guard = state
            .active
            .lock()
            .map_err(|_| "download state lock poisoned".to_string())?;
        if let Some(active) = guard.as_ref() {
            if !active.cancel.load(Ordering::Relaxed) {
                return Err(format!(
                    "download already in progress (v{}). Please cancel it first.",
                    active.version
                ));
            }
        }
        *guard = Some(ActiveDownload {
            version,
            cancel: cancel.clone(),
        });
    }

    let res = installer::download_and_setup(app.clone(), version, cancel.clone()).await;

    // Clear active download state (best-effort).
    {
        let mut guard = state
            .active
            .lock()
            .map_err(|_| "download state lock poisoned".to_string())?;
        if guard.as_ref().is_some_and(|a| a.version == version) {
            *guard = None;
        }
    }
    res
}

#[tauri::command]
fn cancel_download(
    app: tauri::AppHandle,
    version: u32,
    state: State<'_, DownloadState>,
) -> Result<bool, String> {
    let mut did_signal = false;
    {
        let guard = state
            .active
            .lock()
            .map_err(|_| "download state lock poisoned".to_string())?;
        if let Some(active) = guard.as_ref() {
            if active.version == version {
                active.cancel.store(true, Ordering::Relaxed);
                did_signal = true;
            }
        }
    }

    // Best-effort cleanup: delete the partial version folder.
    // If files are still in use, the installer will retry cleanup on exit.
    if did_signal {
        if let Ok(dir) = version_dir(&app, version) {
            let _ = std::fs::remove_dir_all(&dir);
        }
    }

    Ok(did_signal)
}

#[tauri::command]
async fn sync_latest_install_from_manifest(app: tauri::AppHandle) -> Result<bool, String> {
    installer::sync_latest_install_from_manifest(app).await?;
    Ok(true)
}

#[tauri::command]
async fn open_version_folder(app: tauri::AppHandle) -> Result<bool, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("failed to resolve app data dir: {e}"))?
        .join("versions");
    let _ = opener::open(dir).map_err(|e| e.to_string())?;
    Ok(true)
}

#[tauri::command]
async fn check_mod_updates(app: tauri::AppHandle, version: u32) -> Result<bool, String> {
    let client = reqwest::Client::new();

    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("failed to resolve app data dir: {e}"))?
        .join("versions");
    let extract_dir = dir.join(format!("v{version}"));
    let (_, mods_cfg, _, _) = ModsConfig::fetch_manifest(&client).await?;

    let mut updatable_mods: Vec<String> = vec![];

    let res = mods::updatable_mods_with_progress(
        &app,
        &extract_dir,
        version,
        &mods_cfg,
        |checked, total, detail, mod_name| {
            if let Some(mod_name) = mod_name {
                if !updatable_mods.contains(&mod_name) {
                    updatable_mods.push(mod_name.clone());
                }
            }

            progress::emit_updatable_progress(
                &app,
                TaskUpdatableProgressPayload {
                    version,
                    total,
                    checked,
                    updatable_mods: updatable_mods.clone(),
                    detail,
                },
            );
        },
    )
    .await;

    if let Err(e) = res {
        progress::emit_updatable_error(
            &app,
            TaskErrorPayload {
                version,
                message: e.clone(),
            },
        );
        return Err(e);
    }

    progress::emit_updatable_finished(
        &app,
        TaskFinishedPayload {
            version,
            path: extract_dir.to_string_lossy().to_string(),
        },
    );
    Ok(true)
}

#[tauri::command]
async fn apply_mod_updates(app: tauri::AppHandle, version: u32) -> Result<bool, String> {
    let res: Result<(), String> = async {
        let client = reqwest::Client::new();

        let dir = app
            .path()
            .app_data_dir()
            .map_err(|e| format!("failed to resolve app data dir: {e}"))?
            .join("versions");
        let game_root = dir.join(format!("v{version}"));
        if !game_root.exists() {
            return Err(format!(
                "version folder not found: {}",
                game_root.to_string_lossy()
            ));
        }

        let (_, mods_cfg, _, _) = ModsConfig::fetch_manifest(&client).await?;

        const STEPS_TOTAL: u32 = 2;
        progress::emit_progress(
            &app,
            TaskProgressPayload {
                version,
                steps_total: STEPS_TOTAL,
                step: 1,
                step_name: "Check Updates".to_string(),
                step_progress: 0.0,
                overall_percent: overall_from_step(1, 0.0, STEPS_TOTAL),
                detail: Some("Checking updatable mods...".to_string()),
                downloaded_bytes: None,
                total_bytes: None,
                extracted_files: None,
                total_files: None,
            },
        );

        let mut updatable: Vec<String> = vec![];
        mods::updatable_mods_with_progress(
            &app,
            &game_root,
            version,
            &mods_cfg,
            |checked, total, detail, mod_name| {
                if let Some(m) = mod_name {
                    if !updatable.contains(&m) {
                        updatable.push(m);
                    }
                }
                let step_progress = if total == 0 {
                    1.0
                } else {
                    (checked as f64 / total as f64).clamp(0.0, 1.0)
                };
                progress::emit_progress(
                    &app,
                    TaskProgressPayload {
                        version,
                        steps_total: STEPS_TOTAL,
                        step: 1,
                        step_name: "Check Updates".to_string(),
                        step_progress,
                        overall_percent: overall_from_step(1, step_progress, STEPS_TOTAL),
                        detail,
                        downloaded_bytes: None,
                        total_bytes: None,
                        extracted_files: Some(checked),
                        total_files: Some(total),
                    },
                );
            },
        )
        .await?;

        if updatable.is_empty() {
            progress::emit_progress(
                &app,
                TaskProgressPayload {
                    version,
                    steps_total: STEPS_TOTAL,
                    step: 2,
                    step_name: "Update Mods".to_string(),
                    step_progress: 1.0,
                    overall_percent: 100.0,
                    detail: Some("No updates available".to_string()),
                    downloaded_bytes: None,
                    total_bytes: None,
                    extracted_files: None,
                    total_files: None,
                },
            );
            return Ok(());
        }

        progress::emit_progress(
            &app,
            TaskProgressPayload {
                version,
                steps_total: STEPS_TOTAL,
                step: 2,
                step_name: "Update Mods".to_string(),
                step_progress: 0.0,
                overall_percent: overall_from_step(2, 0.0, STEPS_TOTAL),
                detail: Some(format!("Updating {} mods...", updatable.len())),
                downloaded_bytes: None,
                total_bytes: None,
                extracted_files: Some(0),
                total_files: Some(updatable.len() as u64),
            },
        );

        mods::update_mods_with_progress(
            &app,
            &game_root,
            version,
            &mods_cfg,
            updatable.clone(),
            |done, total, detail| {
                let step_progress = if total == 0 {
                    1.0
                } else {
                    (done as f64 / total as f64).clamp(0.0, 1.0)
                };
                progress::emit_progress(
                    &app,
                    TaskProgressPayload {
                        version,
                        steps_total: STEPS_TOTAL,
                        step: 2,
                        step_name: "Update Mods".to_string(),
                        step_progress,
                        overall_percent: overall_from_step(2, step_progress, STEPS_TOTAL),
                        detail,
                        downloaded_bytes: None,
                        total_bytes: None,
                        extracted_files: Some(done),
                        total_files: Some(total),
                    },
                );
            },
        )
        .await?;

        Ok(())
    }
    .await;

    match res {
        Ok(()) => {
            progress::emit_finished(
                &app,
                TaskFinishedPayload {
                    version,
                    path: version_dir(&app, version)?.to_string_lossy().to_string(),
                },
            );
            Ok(true)
        }
        Err(e) => {
            progress::emit_error(
                &app,
                TaskErrorPayload {
                    version,
                    message: e.clone(),
                },
            );
            Err(e)
        }
    }
}

#[cfg(target_os = "linux")]
fn get_steam_client_path(launcher_root: &std::path::Path) -> std::path::PathBuf {
    if let Some(home_dir) = dirs::home_dir() {
        let steam_paths = [
            home_dir.join(".steam/steam"),
            home_dir.join(".local/share/Steam"),
            home_dir.join(".var/app/com.valvesoftware.Steam/.local/share/Steam"),
        ];
        for path in steam_paths.iter() {
            if path.exists() && path.join("steamapps").exists() {
                println!("Found real Steam installation at: {:?}", path);
                return path.clone();
            }
        }
    }

    println!("Steam not found. Mocking client path.");
    launcher_root.to_path_buf()
}

#[tauri::command]
fn launch_game(
    app: tauri::AppHandle,
    version: u32,
    state: State<'_, GameState>,
) -> Result<u32, String> {
    let dir = version_dir(&app, version)?;
    if !dir.exists() {
        return Err(format!(
            "version folder not found: {}",
            dir.to_string_lossy()
        ));
    }

    let _app_path = app.path().app_data_dir().map_err(|e| format!("app path not found: {e}"))?;
    let exe_name = "Lethal Company.exe";
    let exe_path = dir.join(exe_name);
    let exe_path = if exe_path.exists() {
        exe_path
    } else {
        find_file_named(&dir, exe_name, 3)
            .ok_or_else(|| format!("{exe_name} not found under {}", dir.to_string_lossy()))?
    };

    let exe_dir = exe_path
        .parent()
        .ok_or_else(|| "invalid exe path".to_string())?;

    // If already running, return an error.
    {
        let mut guard = state
            .child
            .lock()
            .map_err(|_| "game state lock poisoned".to_string())?;
        if let Some(child) = guard.as_mut() {
            if child.try_wait().map_err(|e| e.to_string())?.is_none() {
                return Err("game is already running".to_string());
            }
        }
        *guard = None;
    }

    // Non-practice run: force-disable practice mods.
    ensure_practice_mods_disabled_for_version(&app, version)?;

    // Ensure disabled mods are applied for this version before launch.
    let _ = apply_disabled_mods_for_version(&app, version);
    // For HQoL specifically, also ensure `.old` matches disablemod.json on normal runs.
    let _ = sync_hqol_with_disablemod_for_version(&app, version);

    #[cfg(target_os = "windows")]
    let mut command = std::process::Command::new(&exe_path);
    
    #[cfg(target_os = "macos")]
    let mut command = {
        let mut cmd = std::process::Command::new("open");
        cmd.arg("-a");
        cmd.arg(&exe_path);
        cmd
    };

    #[cfg(target_os = "linux")]
    let (proton_binary, compat_data_path) = {
        let proton_env_path = installer::proton_env_dir(&app).map_err(|e| format!("proton_env path not found: {e}"))?;
        let proton_bin_path = installer::get_current_proton_dir_impl(&app)
            .map_err(|e| format!("proton path not found: {e}"))?
            .ok_or("found proton path but is None")?;
        let compat_pre_path = proton_env_path.join("wine_prefix");
        if !compat_pre_path.exists() {
            std::fs::create_dir(&compat_pre_path).map_err(|e| format!("could not make prefix: {e}"))?;
        }
        (
            proton_bin_path.join("proton"),
            compat_pre_path
        )
    };

    #[cfg(target_os = "linux")]
    let mut command = {
        let steam_path = get_steam_client_path(&_app_path);
        let mut cmd = std::process::Command::new(&proton_binary);
        cmd.arg("run");
        cmd.arg(&exe_path);
        cmd.env("STEAM_COMPAT_DATA_PATH", &compat_data_path);
        cmd.env("STEAM_COMPAT_CLIENT_INSTALL_PATH", &steam_path);
        cmd.env("WINEDLLOVERRIDES", "winhttp=n,b");
        println!("{:?}", cmd);
        cmd
    };

    let child = command
        .current_dir(exe_dir)
        .spawn()
        .map_err(|e| format!("failed to launch: {e}"))?;

    let pid = child.id();
    let mut guard = state
        .child
        .lock()
        .map_err(|_| "game state lock poisoned".to_string())?;
    *guard = Some(child);
    Ok(pid)
}

#[tauri::command]
async fn launch_game_practice(
    app: tauri::AppHandle,
    version: u32,
    state: State<'_, GameState>,
) -> Result<u32, String> {
    let dir = version_dir(&app, version)?;
    if !dir.exists() {
        return Err(format!(
            "version folder not found: {}",
            dir.to_string_lossy()
        ));
    }

    let _app_path = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("app path not found: {e}"))?;
    let exe_name = "Lethal Company.exe";
    let exe_path = dir.join(exe_name);
    let exe_path = if exe_path.exists() {
        exe_path
    } else {
        find_file_named(&dir, exe_name, 3)
            .ok_or_else(|| format!("{exe_name} not found under {}", dir.to_string_lossy()))?
    };

    let exe_dir = exe_path
        .parent()
        .ok_or_else(|| "invalid exe path".to_string())?;

    // If already running, return an error.
    {
        let mut guard = state
            .child
            .lock()
            .map_err(|_| "game state lock poisoned".to_string())?;
        if let Some(child) = guard.as_mut() {
            if child.try_wait().map_err(|e| e.to_string())?.is_none() {
                return Err("game is already running".to_string());
            }
        }
        *guard = None;
    }

    // Practice run: install + enable practice mods (compatible with this game version).
    prepare_practice_mods_for_version(&app, version).await?;

    // Ensure disabled mods are applied for this version before launch.
    let _ = apply_disabled_mods_for_version(&app, version);

    #[cfg(target_os = "windows")]
    let mut command = std::process::Command::new(&exe_path);

    #[cfg(target_os = "macos")]
    let mut command = {
        let mut cmd = std::process::Command::new("open");
        cmd.arg("-a");
        cmd.arg(&exe_path);
        cmd
    };

    #[cfg(target_os = "linux")]
    let (proton_binary, compat_data_path) = {
        let proton_env_path = installer::proton_env_dir(&app)
            .map_err(|e| format!("proton_env path not found: {e}"))?;
        let proton_bin_path = installer::get_current_proton_dir_impl(&app)
            .map_err(|e| format!("proton path not found: {e}"))?
            .ok_or("found proton path but is None")?;
        let compat_pre_path = proton_env_path.join("wine_prefix");
        if !compat_pre_path.exists() {
            std::fs::create_dir(&compat_pre_path).map_err(|e| format!("could not make prefix: {e}"))?;
        }
        (
            proton_bin_path.join("proton"),
            compat_pre_path
        )
    };

    #[cfg(target_os = "linux")]
    let mut command = {
        let steam_path = get_steam_client_path(&_app_path);
        let mut cmd = std::process::Command::new(&proton_binary);
        cmd.arg("run");
        cmd.arg(&exe_path);
        cmd.env("STEAM_COMPAT_DATA_PATH", &compat_data_path);
        cmd.env("STEAM_COMPAT_CLIENT_INSTALL_PATH", &steam_path);
        cmd.env("WINEDLLOVERRIDES", "winhttp=n,b");
        println!("{:?}", cmd);
        cmd
    };

    let child = command
        .current_dir(exe_dir)
        .spawn()
        .map_err(|e| format!("failed to launch: {e}"))?;

    let pid = child.id();
    let mut guard = state
        .child
        .lock()
        .map_err(|_| "game state lock poisoned".to_string())?;
    *guard = Some(child);
    Ok(pid)
}

#[tauri::command]
fn get_game_status(state: State<'_, GameState>) -> Result<GameStatus, String> {
    let mut guard = state
        .child
        .lock()
        .map_err(|_| "game state lock poisoned".to_string())?;
    if let Some(child) = guard.as_mut() {
        match child.try_wait().map_err(|e| e.to_string())? {
            None => Ok(GameStatus {
                running: true,
                pid: Some(child.id()),
            }),
            Some(_) => {
                *guard = None;
                Ok(GameStatus {
                    running: false,
                    pid: None,
                })
            }
        }
    } else {
        Ok(GameStatus {
            running: false,
            pid: None,
        })
    }
}

#[tauri::command]
fn stop_game(state: State<'_, GameState>) -> Result<bool, String> {
    let mut guard = state
        .child
        .lock()
        .map_err(|_| "game state lock poisoned".to_string())?;
    if let Some(mut child) = guard.take() {
        let _ = child.kill();
        let _ = child.wait();
        Ok(true)
    } else {
        Ok(false)
    }
}

#[tauri::command]
fn get_disabled_mods(app: tauri::AppHandle) -> Result<Vec<DisabledMod>, String> {
    Ok(read_disablemod(&app)?.mods)
}

#[tauri::command]
fn apply_disabled_mods(app: tauri::AppHandle, version: u32) -> Result<bool, String> {
    apply_disabled_mods_for_version(&app, version)?;
    Ok(true)
}

#[tauri::command]
fn set_mod_enabled(
    app: tauri::AppHandle,
    version: u32,
    dev: String,
    name: String,
    enabled: bool,
) -> Result<bool, String> {
    let mut list = read_disablemod(&app)?;

    // Use normalized ids in the file.
    let id = normalize_mod_id(&dev, &name);
    list.mods.retain(|m| m != &id);
    if !enabled {
        list.mods.push(id);
        list.mods
            .sort_by(|a, b| a.dev.cmp(&b.dev).then(a.name.cmp(&b.name)));
        list.mods.dedup();
    }
    write_disablemod(&app, &list)?;

    // Apply to current version immediately (still add-only / no overwrite).
    let plugins = plugins_dir(&app, version)?;
    if let Some(dir) = mod_dir_for(&plugins, &dev, &name) {
        let _ = set_mod_files_old_suffix(&dir, enabled);
    }
    Ok(true)
}

#[tauri::command]
fn list_installed_mod_versions(
    app: tauri::AppHandle,
    version: u32,
) -> Result<Vec<InstalledModVersion>, String> {
    let plugins = plugins_dir(&app, version)?;
    if !plugins.exists() {
        return Ok(vec![]);
    }

    let mut out: Vec<InstalledModVersion> = vec![];
    let Ok(rd) = std::fs::read_dir(&plugins) else {
        return Ok(out);
    };

    for e in rd.flatten() {
        let path = e.path();
        if !path.is_dir() {
            continue;
        }

        // Plugin folder naming is deterministic: "{dev}-{name}"
        let folder = e.file_name().to_string_lossy().to_string();
        let Some((dev, name)) = folder.split_once('-') else {
            continue;
        };

        // When disabled, we suffix every file with `.old`, including manifest.json.
        let manifest_path = path.join("manifest.json");
        let manifest_old_path = path.join("manifest.json.old");

        let manifest = if manifest_path.exists() {
            read_manifest(&manifest_path)
        } else if manifest_old_path.exists() {
            read_manifest(&manifest_old_path)
        } else {
            continue;
        };

        match manifest {
            Ok(m) => {
                out.push(InstalledModVersion {
                    dev: dev.to_string(),
                    name: name.to_string(),
                    version: m.version_number,
                });
            }
            Err(err) => {
                log::warn!(
                    "Failed to read plugin manifest for {} (v{}): {}",
                    folder,
                    version,
                    err
                );
            }
        }
    }

    out.sort_by(|a, b| a.dev.cmp(&b.dev).then(a.name.cmp(&b.name)));
    Ok(out)
}

#[tauri::command]
async fn get_manifest() -> Result<ManifestDto, String> {
    let client = reqwest::Client::new();
    let (version, cfg, chain_config, manifests) =
        mod_config::ModsConfig::fetch_manifest(&client).await?;
    Ok(ManifestDto {
        version,
        chain_config,
        mods: cfg.mods,
        manifests,
    })
}

#[tauri::command]
fn list_installed_versions(app: tauri::AppHandle) -> Result<Vec<u32>, String> {
    let base = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("failed to resolve app data dir: {e}"))?
        .join("versions");

    let mut out: Vec<u32> = vec![];
    let Ok(rd) = std::fs::read_dir(&base) else {
        return Ok(out);
    };

    for e in rd {
        let Ok(e) = e else { continue };
        let path = e.path();
        if !path.is_dir() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
            continue;
        };
        let Some(num) = name.strip_prefix('v') else {
            continue;
        };
        if let Ok(v) = num.parse::<u32>() {
            out.push(v);
        }
    }
    out.sort_unstable();
    Ok(out)
}

#[tauri::command]
fn list_config_files(app: tauri::AppHandle) -> Result<Vec<String>, String> {
    let base = shared_config_dir(&app)?;
    if !base.exists() {
        return Ok(vec![]);
    }

    let mut out: Vec<String> = vec![];
    let base_canon = std::fs::canonicalize(&base).map_err(|e| e.to_string())?;

    let mut stack: Vec<std::path::PathBuf> = vec![base.clone()];
    while let Some(dir) = stack.pop() {
        for e in std::fs::read_dir(&dir).map_err(|e| e.to_string())? {
            let e = e.map_err(|e| e.to_string())?;
            let path = e.path();
            let ty = e.file_type().map_err(|e| e.to_string())?;
            if ty.is_dir() {
                stack.push(path);
                continue;
            }
            if !ty.is_file() {
                continue;
            }
            let canon = std::fs::canonicalize(&path).map_err(|e| e.to_string())?;
            if !canon.starts_with(&base_canon) {
                continue;
            }
            let rel = canon
                .strip_prefix(&base_canon)
                .unwrap_or(&canon)
                .to_string_lossy()
                .replace('\\', "/");
            out.push(rel.trim_start_matches('/').to_string());
        }
    }

    out.sort();
    Ok(out)
}

#[tauri::command]
fn list_config_files_for_mod(
    app: tauri::AppHandle,
    dev: String,
    name: String,
) -> Result<Vec<String>, String> {
    let all = list_config_files(app)?;
    let d = dev.to_lowercase();
    let n = name.to_lowercase();
    Ok(all
        .into_iter()
        .filter(|p| {
            let lp = p.to_lowercase();
            lp.contains(&d) || lp.contains(&n)
        })
        .collect())
}

#[tauri::command]
fn read_config_file(app: tauri::AppHandle, rel_path: String) -> Result<String, String> {
    let base = shared_config_dir(&app)?;
    let rel = std::path::Path::new(&rel_path);
    if !is_safe_rel_path(rel) {
        return Err("invalid path".to_string());
    }
    let path = base.join(rel);
    std::fs::read_to_string(&path).map_err(|e| e.to_string())
}

#[tauri::command]
fn read_bepinex_cfg(
    app: tauri::AppHandle,
    rel_path: String,
) -> Result<bepinex_cfg::FileData, String> {
    let base = shared_config_dir(&app)?;
    let rel = std::path::Path::new(&rel_path);
    if !is_safe_rel_path(rel) {
        return Err("invalid path".to_string());
    }
    let path = base.join(rel);
    let text = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    bepinex_cfg::parse(&text)
}

#[derive(Debug, Clone, Deserialize)]
struct SetBepInExEntryArgs {
    rel_path: String,
    section: String,
    entry: String,
    value: bepinex_cfg::Value,
}

#[tauri::command]
fn set_bepinex_cfg_entry(app: tauri::AppHandle, args: SetBepInExEntryArgs) -> Result<bool, String> {
    let base = shared_config_dir(&app)?;
    let rel = std::path::Path::new(&args.rel_path);

    log::info!("set_bepinex_cfg_entry: {:?}", args);

    if !is_safe_rel_path(rel) {
        return Err("invalid path".to_string());
    }
    let path = base.join(rel);

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    // If the cfg doesn't exist yet, start from an empty file and create the
    // requested section/entry.
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(e.to_string()),
    };
    let mut file = bepinex_cfg::parse(&text)?;

    let section = match file.sections.iter_mut().find(|s| s.name == args.section) {
        Some(s) => s,
        None => {
            file.sections.push(bepinex_cfg::Section {
                name: args.section.clone(),
                entries: vec![],
            });
            file.sections
                .iter_mut()
                .find(|s| s.name == args.section)
                .ok_or("failed to create section".to_string())?
        }
    };

    match section.entries.iter_mut().find(|e| e.name == args.entry) {
        Some(e) => {
            e.value = args.value;
        }
        None => {
            section.entries.push(bepinex_cfg::Entry {
                name: args.entry,
                description: None,
                default: None,
                value: args.value,
            });
        }
    }

    let new_text = bepinex_cfg::write(&file)?;
    std::fs::write(&path, new_text).map_err(|e| e.to_string())?;
    Ok(true)
}

#[derive(Debug, Clone, Deserialize)]
struct WriteConfigArgs {
    rel_path: String,
    contents: String,
}

#[tauri::command]
fn write_config_file(app: tauri::AppHandle, args: WriteConfigArgs) -> Result<bool, String> {
    let base = shared_config_dir(&app)?;
    let rel = std::path::Path::new(&args.rel_path);
    if !is_safe_rel_path(rel) {
        return Err("invalid path".to_string());
    }
    let path = base.join(rel);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(&path, args.contents).map_err(|e| e.to_string())?;
    Ok(true)
}

// =========================
// 🔹 AUTO-UPDATE COMMANDS
// =========================

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    name: String,
    published_at: String,
    body: Option<String>,
    assets: Vec<GitHubAsset>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GitHubAsset {
    name: String,
    browser_download_url: String,
    size: u64,
}

#[derive(Debug, Clone, Serialize)]
struct UpdateInfo {
    available: bool,
    current_version: String,
    version: Option<String>,
    date: Option<String>,
    body: Option<String>,
}

#[tauri::command]
async fn check_app_update(app: tauri::AppHandle) -> Result<UpdateInfo, String> {
    use semver::Version;

    let current_version_str = app.package_info().version.to_string();

    // GitHub Releases API에서 최신 릴리즈 가져오기
    let client = reqwest::Client::new();
    let github_release_url = "https://api.github.com/repos/p-asta/hq-launcher/releases/latest";

    let github_release: GitHubRelease = client
        .get(github_release_url)
        .header("User-Agent", "hq-launcher-updater")
        .send()
        .await
        .map_err(|e| format!("Failed to fetch GitHub release: {e}"))?
        .json()
        .await
        .map_err(|e| format!("Failed to parse GitHub release: {e}"))?;

    // 버전 비교 (tag_name에서 v 제거)
    let latest_version_str = github_release.tag_name.trim_start_matches('v').to_string();
    let current_version = Version::parse(&current_version_str)
        .map_err(|e| format!("Failed to parse current version: {e}"))?;
    let latest_version = Version::parse(&latest_version_str)
        .map_err(|e| format!("Failed to parse latest version: {e}"))?;

    let available = latest_version > current_version;

    Ok(UpdateInfo {
        available,
        current_version: current_version_str,
        version: if available {
            Some(latest_version_str.clone())
        } else {
            None
        },
        date: Some(github_release.published_at),
        body: github_release.body,
    })
}

#[derive(Debug, Clone, Serialize)]
struct UpdateProgress {
    downloaded: u64,
    total: u64,
    percent: f64,
}

#[tauri::command]
async fn download_app_update(app: tauri::AppHandle) -> Result<bool, String> {
    use tauri_plugin_updater::UpdaterExt;

    // Tauri updater 사용 (엔드포인트는 tauri.conf.json에서 설정, GitHub Releases latest.json)
    let updater = app
        .updater_builder()
        .build()
        .map_err(|e| format!("Failed to initialize updater: {e}"))?;

    let update = updater
        .check()
        .await
        .map_err(|e| format!("Failed to check for updates: {e}"))?
        .ok_or("No update available")?;

    // Download the update with progress tracking
    // on_chunk: FnMut(chunk_length: usize, content_length: Option<u64>)
    // on_download_finish: FnOnce()
    let mut downloaded = 0u64;
    update
        .download(
            |chunk_length, content_length| {
                downloaded += chunk_length as u64;
                if let Some(total) = content_length {
                    let percent = (downloaded as f64 / total as f64) * 100.0;
                    log::debug!(
                        "Update download progress: {:.2}% ({}/{} bytes)",
                        percent,
                        downloaded,
                        total
                    );
                } else {
                    log::debug!("Update download progress: {} bytes downloaded", downloaded);
                }
            },
            || {
                log::info!("Update download finished");
            },
        )
        .await
        .map_err(|e| format!("Failed to download update: {e}"))?;

    Ok(true)
}


#[tauri::command]
#[cfg(not(target_os = "macos"))]
async fn get_global_shortcut(_app: tauri::AppHandle, shortcut: String) -> Result<String, String> {
    let shortcut = shortcut.replace("CommandOrControl", "Ctrl");
    Ok(shortcut)
}

#[tauri::command]
#[cfg(target_os = "macos")]
async fn get_global_shortcut(_app: tauri::AppHandle, shortcut: String) -> Result<String, String> {
    let shortcut = shortcut.replace("CommandOrControl", "Cmd");
    Ok(shortcut)
}

#[tauri::command]
async fn install_app_update(app: tauri::AppHandle) -> Result<bool, String> {
    use tauri_plugin_updater::UpdaterExt;

    // Tauri updater 사용 (엔드포인트는 tauri.conf.json에서 설정, GitHub Releases latest.json)
    let updater = app
        .updater_builder()
        .build()
        .map_err(|e| format!("Failed to initialize updater: {e}"))?;

    let update = updater
        .check()
        .await
        .map_err(|e| format!("Failed to check for updates: {e}"))?
        .ok_or("No update available")?;

    // Download and install the update
    // on_chunk: FnMut(chunk_length: usize, content_length: Option<u64>)
    // on_download_finish: FnOnce()
    let mut downloaded = 0u64;
    update
        .download_and_install(
            |chunk_length, content_length| {
                downloaded += chunk_length as u64;
                if let Some(total) = content_length {
                    let percent = (downloaded as f64 / total as f64) * 100.0;
                    log::debug!(
                        "Update download progress: {:.2}% ({}/{} bytes)",
                        percent,
                        downloaded,
                        total
                    );
                } else {
                    log::debug!("Update download progress: {} bytes downloaded", downloaded);
                }
            },
            || {
                log::info!("Update download finished, installing...");
            },
        )
        .await
        .map_err(|e| format!("Failed to download and install update: {e}"))?;

    Ok(true)
}

#[tauri::command]
fn get_app_version(app: tauri::AppHandle) -> Result<String, String> {
    Ok(app.package_info().version.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(GameState::default())
        .manage(DownloadState::default())
        .manage(downloader::DepotLoginState::default())
        .setup(|app| {
            // File logging (AppDataDir/logs/hq-launcher.log)
            logger::init(&app.handle()).map_err(|e| tauri::Error::Setup(e.into()))?;

            // Startup housekeeping (best-effort, won't block UI):
            // - Purge mods that remote manifest marks as enabled=false (and their configs)
            // - Ensure default config is downloaded if shared config dir is empty
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = installer::purge_remote_disabled_mods_on_startup(app_handle.clone()).await
                {
                    log::warn!("Failed to purge remote-disabled mods on startup: {e}");
                }
                if let Err(e) = installer::ensure_default_config(app_handle.clone()).await {
                    log::warn!("Failed to ensure default config on startup: {e}");
                }
                #[cfg(target_os = "linux")]
                {
                    if let Err(e) = installer::install_proton_ge_impl(&app_handle).await {
                        log::warn!("Failed to install Proton-GE on startup: {e}");
                    }
                }
            });

            Ok(())
        })
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            download,
            cancel_download,
            sync_latest_install_from_manifest,
            check_mod_updates,
            apply_mod_updates,
            launch_game,
            launch_game_practice,
            get_game_status,
            stop_game,
            get_disabled_mods,
            apply_disabled_mods,
            set_mod_enabled,
            list_installed_mod_versions,
            get_manifest,
            list_installed_versions,
            list_config_files,
            list_config_files_for_mod,
            read_config_file,
            read_bepinex_cfg,
            set_bepinex_cfg_entry,
            write_config_file,
            downloader::depot_login,
            downloader::depot_login_start,
            downloader::depot_login_submit_code,
            downloader::depot_get_login_state,
            downloader::depot_logout,
            downloader::depot_download,
            downloader::depot_download_files,
            check_app_update,
            download_app_update,
            install_app_update,
            get_app_version,
            installer::install_proton_ge,
            installer::get_current_proton_dir,
            open_version_folder,
            get_global_shortcut
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

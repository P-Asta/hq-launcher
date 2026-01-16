mod installer;
mod bepinex_cfg;
mod logger;
mod mods;
mod mod_config;
mod progress;
mod thunderstore;
mod zip_utils;

use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use tauri::{Manager, State};

use crate::{mod_config::ModsConfig, progress::{TaskFinishedPayload, TaskUpdatableProgressPayload}};

#[derive(Debug, Clone, Serialize)]
struct ManifestDto {
    version: u32,
    chain_config: Vec<Vec<String>>,
    mods: Vec<mod_config::ModEntry>,
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

fn find_file_named(root: &std::path::Path, target_name: &str, max_depth: usize) -> Option<std::path::PathBuf> {
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct DisabledMod {
    dev: String,
    name: String,
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

fn thunderstore_cache_path(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    Ok(app
        .path()
        .app_data_dir()
        .map_err(|e| format!("failed to resolve app data dir: {e}"))?
        .join("config")
        .join("cache.json"))
}

fn read_disablemod(app: &tauri::AppHandle) -> Result<DisableModFile, String> {
    let path = disablemod_path(app)?;
    if !path.exists() {
        return Ok(DisableModFile {
            version: 1,
            mods: vec![],
        });
    }
    let text = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    serde_json::from_str(&text).map_err(|e| e.to_string())
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
        let dir = plugins.join(mod_folder_name(&m.dev, &m.name));
        let _ = set_mod_files_old_suffix(&dir, false);
    }
    Ok(())
}

#[derive(Default)]
struct GameState {
    child: Mutex<Option<std::process::Child>>,
}

#[derive(Debug, Clone, Serialize)]
struct GameStatus {
    running: bool,
    pid: Option<u32>,
}

#[tauri::command]
async fn download(app: tauri::AppHandle, version: u32) -> Result<bool, String> {
    installer::download_and_setup(app, version).await
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
    let (_, mods_cfg, _) = ModsConfig::fetch_manifest(&client).await?;

    let mut updatable_mods: Vec<String> = vec![];


    mods::updatable_mods_with_progress(
        &extract_dir,
        version,
        &mods_cfg,
        |checked, total, detail, mod_name| {
            if let Some(mod_name) = mod_name {
                updatable_mods.push(mod_name.clone());
            }
            
            progress::emit_updatable_progress(
                &app,
                TaskUpdatableProgressPayload { total,  checked, updatable_mods: updatable_mods.clone(), detail }
            );
        },
    )
    .await?;

    progress::emit_updatable_finished(
        &app,
        TaskFinishedPayload { version, path: extract_dir.to_string_lossy().to_string() }
    );
    Ok(true)
}

#[tauri::command]
fn launch_game(app: tauri::AppHandle, version: u32, state: State<'_, GameState>) -> Result<u32, String> {
    let dir = version_dir(&app, version)?;
    if !dir.exists() {
        return Err(format!("version folder not found: {}", dir.to_string_lossy()));
    }

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
        let mut guard = state.child.lock().map_err(|_| "game state lock poisoned".to_string())?;
        if let Some(child) = guard.as_mut() {
            if child.try_wait().map_err(|e| e.to_string())?.is_none() {
                return Err("game is already running".to_string());
            }
        }
        *guard = None;
    }

    // Ensure disabled mods are applied for this version before launch.
    let _ = apply_disabled_mods_for_version(&app, version);

    let child = std::process::Command::new(&exe_path)
        .current_dir(exe_dir)
        .spawn()
        .map_err(|e| format!("failed to launch: {e}"))?;

    let pid = child.id();
    let mut guard = state.child.lock().map_err(|_| "game state lock poisoned".to_string())?;
    *guard = Some(child);
    Ok(pid)
}

#[tauri::command]
fn get_game_status(state: State<'_, GameState>) -> Result<GameStatus, String> {
    let mut guard = state.child.lock().map_err(|_| "game state lock poisoned".to_string())?;
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
    let mut guard = state.child.lock().map_err(|_| "game state lock poisoned".to_string())?;
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
fn set_mod_enabled(app: tauri::AppHandle, version: u32, dev: String, name: String, enabled: bool) -> Result<bool, String> {
    let mut list = read_disablemod(&app)?;

    // Use normalized ids in the file.
    let id = normalize_mod_id(&dev, &name);
    list.mods.retain(|m| *m != id);
    if !enabled {
        list.mods.push(id);
        list.mods.sort_by(|a, b| a.dev.cmp(&b.dev).then(a.name.cmp(&b.name)));
        list.mods.dedup();
    }
    write_disablemod(&app, &list)?;

    // Apply to current version immediately (still add-only / no overwrite).
    let dir = plugins_dir(&app, version)?.join(mod_folder_name(&dev, &name));
    let _ = set_mod_files_old_suffix(&dir, enabled);
    Ok(true)
}

#[tauri::command]
async fn get_manifest() -> Result<ManifestDto, String> {
    let client = reqwest::Client::new();
    let (version, cfg, chain_config) = mod_config::ModsConfig::fetch_manifest(&client).await?;
    Ok(ManifestDto {
        version,
        chain_config,
        mods: cfg.mods,
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
fn read_bepinex_cfg(app: tauri::AppHandle, rel_path: String) -> Result<bepinex_cfg::FileData, String> {
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
    let text = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let mut file = bepinex_cfg::parse(&text)?;

    let section = file
        .sections
        .iter_mut()
        .find(|s| s.name == args.section)
        .ok_or("section not found".to_string())?;
    let entry = section
        .entries
        .iter_mut()
        .find(|e| e.name == args.entry)
        .ok_or("entry not found".to_string())?;

    entry.value = args.value;

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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(GameState::default())
        .setup(|app| {
            // File logging (AppDataDir/logs/hq-launcher.log)
            logger::init(&app.handle()).map_err(|e| tauri::Error::Setup(e.into()))?;

            // On first run (app startup), sync latest installed version with remote manifest.
            // This is additive-only: it won't overwrite existing config/mod files.
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = installer::sync_latest_install_from_manifest(handle).await {
                    log::error!("manifest sync failed: {e}");
                }
            });

            // tauri::async_runtime::spawn(async move {
            //     if let Err(e) = mods::update_mods_with_progress(&app, version).await {
            //         log::error!("mod update failed: {e}");
            //     }
            // });

            Ok(())
        })
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            download,
            check_mod_updates,
            launch_game,
            get_game_status,
            stop_game,
            get_disabled_mods,
            apply_disabled_mods,
            set_mod_enabled,
            get_manifest,
            list_installed_versions,
            list_config_files,
            list_config_files_for_mod,
            read_config_file,
            read_bepinex_cfg,
            set_bepinex_cfg_entry,
            write_config_file
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

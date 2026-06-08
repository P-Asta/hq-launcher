use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::Manager;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct StorageConfig {
    game_storage_dir: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GameStorageSettings {
    pub current_dir: String,
    pub default_dir: String,
    pub custom_dir: Option<String>,
    pub is_custom: bool,
}

fn config_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    Ok(app
        .path()
        .app_data_dir()
        .map_err(|e| format!("failed to resolve app data dir: {e}"))?
        .join("config")
        .join("storage.json"))
}

pub fn default_versions_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    Ok(app
        .path()
        .app_data_dir()
        .map_err(|e| format!("failed to resolve app data dir: {e}"))?
        .join("versions"))
}

fn read_config(app: &tauri::AppHandle) -> Result<StorageConfig, String> {
    let path = config_path(app)?;
    if !path.exists() {
        return Ok(StorageConfig::default());
    }
    let text = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    serde_json::from_str(&text).map_err(|e| e.to_string())
}

fn write_config(app: &tauri::AppHandle, config: &StorageConfig) -> Result<(), String> {
    let path = config_path(app)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(config).map_err(|e| e.to_string())?;
    std::fs::write(path, json).map_err(|e| e.to_string())
}

fn normalize_existing_or_creatable_dir(path: PathBuf) -> Result<PathBuf, String> {
    if path.exists() {
        if !path.is_dir() {
            return Err(format!("path is not a folder: {}", path.to_string_lossy()));
        }
        return std::fs::canonicalize(&path).map_err(|e| e.to_string());
    }

    if let Some(parent) = path.parent() {
        if parent.exists() {
            let parent = std::fs::canonicalize(parent).map_err(|e| e.to_string())?;
            let Some(name) = path.file_name() else {
                return Err("invalid folder path".to_string());
            };
            return Ok(parent.join(name));
        }
    }

    Ok(path)
}

pub fn versions_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let config = read_config(app)?;
    match config.game_storage_dir {
        Some(dir) => Ok(dir.join("versions")),
        None => default_versions_dir(app),
    }
}

pub fn versions_dir_for_custom(
    app: &tauri::AppHandle,
    custom_dir: Option<PathBuf>,
) -> Result<PathBuf, String> {
    match custom_dir {
        Some(path) => Ok(normalize_existing_or_creatable_dir(path)?.join("versions")),
        None => default_versions_dir(app),
    }
}

pub fn game_storage_settings(app: &tauri::AppHandle) -> Result<GameStorageSettings, String> {
    let default_dir = default_versions_dir(app)?;
    let config = read_config(app)?;
    let current_dir = versions_dir(app)?;
    Ok(GameStorageSettings {
        current_dir: current_dir.to_string_lossy().to_string(),
        default_dir: default_dir.to_string_lossy().to_string(),
        custom_dir: config
            .game_storage_dir
            .as_ref()
            .map(|p| p.to_string_lossy().to_string()),
        is_custom: config.game_storage_dir.is_some(),
    })
}

pub fn set_game_storage_dir(
    app: &tauri::AppHandle,
    custom_dir: Option<PathBuf>,
) -> Result<GameStorageSettings, String> {
    let normalized_custom = match custom_dir {
        Some(path) => {
            let path = normalize_existing_or_creatable_dir(path)?;
            Some(path)
        }
        None => None,
    };
    write_config(
        app,
        &StorageConfig {
            game_storage_dir: normalized_custom,
        },
    )?;
    game_storage_settings(app)
}

fn dir_has_any_entries(path: &Path) -> bool {
    std::fs::read_dir(path)
        .ok()
        .and_then(|mut rd| rd.next())
        .is_some()
}

fn count_copy_entries(path: &Path) -> Result<u64, String> {
    let mut count = 0u64;
    for entry in std::fs::read_dir(path).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let src = entry.path();
        let metadata = std::fs::symlink_metadata(&src).map_err(|e| e.to_string())?;
        let file_type = metadata.file_type();
        if file_type.is_symlink() {
            continue;
        }
        if file_type.is_dir() {
            count = count.saturating_add(1);
            count = count.saturating_add(count_copy_entries(&src)?);
        } else if file_type.is_file() {
            count = count.saturating_add(1);
        }
    }
    Ok(count)
}

fn copy_dir_contents<F>(
    from: &Path,
    to: &Path,
    copied: &mut u64,
    total: u64,
    on_progress: &mut F,
) -> Result<(), String>
where
    F: FnMut(u64, u64, Option<String>),
{
    std::fs::create_dir_all(to).map_err(|e| e.to_string())?;
    for entry in std::fs::read_dir(from).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let src = entry.path();
        let dest = to.join(entry.file_name());
        let metadata = std::fs::symlink_metadata(&src).map_err(|e| e.to_string())?;
        let file_type = metadata.file_type();

        if file_type.is_symlink() {
            continue;
        }

        if file_type.is_dir() {
            std::fs::create_dir_all(&dest).map_err(|e| e.to_string())?;
            *copied = copied.saturating_add(1);
            on_progress(
                *copied,
                total,
                dest.file_name()
                    .and_then(|name| name.to_str())
                    .map(|name| format!("Creating {name}")),
            );
            copy_dir_contents(&src, &dest, copied, total, on_progress)?;
            continue;
        }

        if file_type.is_file() {
            if let Some(parent) = dest.parent() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            std::fs::copy(&src, &dest).map_err(|e| {
                format!(
                    "failed to copy {} to {}: {e}",
                    src.to_string_lossy(),
                    dest.to_string_lossy()
                )
            })?;
            *copied = copied.saturating_add(1);
            on_progress(
                *copied,
                total,
                dest.file_name()
                    .and_then(|name| name.to_str())
                    .map(|name| format!("Copying {name}")),
            );
        }
    }
    Ok(())
}

pub fn move_versions_dir<F>(
    old_dir: &Path,
    new_dir: &Path,
    mut on_progress: F,
) -> Result<bool, String>
where
    F: FnMut(u64, u64, Option<String>),
{
    let old_exists = old_dir.exists();
    if !old_exists || !dir_has_any_entries(old_dir) {
        std::fs::create_dir_all(new_dir).map_err(|e| e.to_string())?;
        on_progress(1, 1, Some("Storage folder is ready".to_string()));
        return Ok(false);
    }

    if new_dir.exists() && dir_has_any_entries(new_dir) {
        return Err(format!(
            "target versions folder is not empty: {}",
            new_dir.to_string_lossy()
        ));
    }

    if let Some(parent) = new_dir.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    let total = count_copy_entries(old_dir)?.max(1);
    on_progress(0, total, Some("Preparing game files...".to_string()));

    match std::fs::rename(old_dir, new_dir) {
        Ok(()) => {
            on_progress(total, total, Some("Moved game files".to_string()));
            Ok(true)
        }
        Err(rename_err) => {
            let mut copied = 0u64;
            copy_dir_contents(old_dir, new_dir, &mut copied, total, &mut on_progress)
                .map_err(|copy_err| format!("{rename_err}; {copy_err}"))?;
            on_progress(total, total, Some("Removing old game storage...".to_string()));
            std::fs::remove_dir_all(old_dir).map_err(|e| e.to_string())?;
            Ok(true)
        }
    }
}

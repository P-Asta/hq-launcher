use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tauri::Manager;

pub const MOVABLE_DATA_DIRS: &[&str] = &[
    "versions",
    "config",
    "cache",
    "logs",
    "downloader",
    "depot_config",
    "temp",
    "proton_env",
];

pub const OPTIONAL_MOVE_DIRS: &[&str] = &["logs", "temp"];

const SETTINGS_FILE_NAME: &str = "launcher-settings.json";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LauncherSettings {
    #[serde(default)]
    pub data_dir: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DataDirectoryInfo {
    pub current_path: String,
    pub default_path: String,
    pub using_default: bool,
    pub has_existing_data: bool,
}

fn settings_dir() -> Result<PathBuf, String> {
    let base =
        dirs::config_dir().ok_or_else(|| "failed to resolve config directory".to_string())?;
    Ok(base.join("asta").join("hq-launcher"))
}

pub fn settings_path() -> Result<PathBuf, String> {
    Ok(settings_dir()?.join(SETTINGS_FILE_NAME))
}

pub fn default_data_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .map_err(|e| format!("failed to resolve app data dir: {e}"))
}

pub fn load_settings(_app: &tauri::AppHandle) -> Result<LauncherSettings, String> {
    let path = settings_path()?;
    if !path.exists() {
        return Ok(LauncherSettings::default());
    }

    let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    serde_json::from_str::<LauncherSettings>(&content).map_err(|e| e.to_string())
}

pub fn save_settings(_app: &tauri::AppHandle, settings: &LauncherSettings) -> Result<(), String> {
    let path = settings_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let content = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
    std::fs::write(path, content).map_err(|e| e.to_string())
}

fn parse_saved_data_dir(raw: &str) -> Option<PathBuf> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }

    let candidate = PathBuf::from(trimmed);
    if candidate.is_absolute() {
        Some(candidate)
    } else {
        None
    }
}

pub fn custom_data_dir(app: &tauri::AppHandle) -> Result<Option<PathBuf>, String> {
    let settings = load_settings(app)?;
    Ok(settings.data_dir.as_deref().and_then(parse_saved_data_dir))
}

pub fn app_data_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    Ok(match custom_data_dir(app)? {
        Some(custom) => custom,
        None => default_data_dir(app)?,
    })
}

pub fn versions_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    Ok(app_data_dir(app)?.join("versions"))
}

pub fn config_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    Ok(app_data_dir(app)?.join("config"))
}

pub fn shared_config_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    Ok(config_dir(app)?.join("shared"))
}

pub fn downloader_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    Ok(app_data_dir(app)?.join("downloader"))
}

pub fn depot_config_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    Ok(app_data_dir(app)?.join("depot_config"))
}

pub fn cache_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    Ok(app_data_dir(app)?.join("cache"))
}

pub fn logs_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    Ok(app_data_dir(app)?.join("logs"))
}

pub fn temp_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    Ok(app_data_dir(app)?.join("temp"))
}

pub fn proton_env_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    Ok(app_data_dir(app)?.join("proton_env"))
}

pub fn thunderstore_cache_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    Ok(cache_dir(app)?.join("thunderstore.json"))
}

pub fn disablemod_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    Ok(config_dir(app)?.join("disablemod.json"))
}

pub fn data_directory_info(app: &tauri::AppHandle) -> Result<DataDirectoryInfo, String> {
    let current = app_data_dir(app)?;
    let default = default_data_dir(app)?;
    let has_existing_data = MOVABLE_DATA_DIRS
        .iter()
        .any(|name| current.join(name).exists());

    Ok(DataDirectoryInfo {
        current_path: current.to_string_lossy().to_string(),
        default_path: default.to_string_lossy().to_string(),
        using_default: current == default,
        has_existing_data,
    })
}

pub fn movable_entries(root: &Path) -> Vec<PathBuf> {
    MOVABLE_DATA_DIRS
        .iter()
        .map(|name| root.join(name))
        .filter(|path| path.exists())
        .collect()
}

pub fn movable_entries_for_migration(root: &Path) -> Vec<PathBuf> {
    MOVABLE_DATA_DIRS
        .iter()
        .filter(|name| !OPTIONAL_MOVE_DIRS.contains(name))
        .map(|name| root.join(name))
        .filter(|path| path.exists())
        .collect()
}

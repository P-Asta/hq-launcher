use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU8, Ordering};
use tauri::Manager;

const RELEASE_CHANNEL_CONFIG_VERSION: u32 = 1;
static CURRENT_CHANNEL: AtomicU8 = AtomicU8::new(0);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReleaseChannel {
    Stable,
    Beta,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ReleaseChannelConfig {
    version: u32,
    channel: ReleaseChannel,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReleaseChannelDto {
    pub channel: ReleaseChannel,
    pub is_beta: bool,
    pub manifest_url: &'static str,
    pub updater_url: &'static str,
}

impl Default for ReleaseChannel {
    fn default() -> Self {
        Self::Stable
    }
}

impl ReleaseChannel {
    pub fn as_u8(self) -> u8 {
        match self {
            Self::Stable => 0,
            Self::Beta => 1,
        }
    }

    pub fn from_u8(value: u8) -> Self {
        match value {
            1 => Self::Beta,
            _ => Self::Stable,
        }
    }

    pub fn manifest_url(self) -> &'static str {
        match self {
            Self::Stable => "https://f.asta.rs/hq-launcher/manifest.json",
            Self::Beta => "https://f.asta.rs/hq-launcher/beta/manifest.json",
        }
    }

    pub fn updater_url(self) -> &'static str {
        match self {
            Self::Stable => {
                "https://github.com/p-asta/hq-launcher/releases/latest/download/latest.json"
            }
            Self::Beta => "https://f.asta.rs/hq-launcher/beta/latest.json",
        }
    }

    pub fn into_dto(self) -> ReleaseChannelDto {
        ReleaseChannelDto {
            channel: self,
            is_beta: self == Self::Beta,
            manifest_url: self.manifest_url(),
            updater_url: self.updater_url(),
        }
    }
}

pub fn current() -> ReleaseChannel {
    ReleaseChannel::from_u8(CURRENT_CHANNEL.load(Ordering::Relaxed))
}

pub fn set_current(channel: ReleaseChannel) {
    CURRENT_CHANNEL.store(channel.as_u8(), Ordering::Relaxed);
}

pub fn config_path(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    Ok(app
        .path()
        .app_data_dir()
        .map_err(|e| format!("failed to resolve app data dir: {e}"))?
        .join("config")
        .join("release_channel.json"))
}

pub fn load(app: &tauri::AppHandle) -> Result<ReleaseChannel, String> {
    let path = config_path(app)?;
    if !path.exists() {
        let channel = ReleaseChannel::default();
        let _ = save(app, channel);
        set_current(channel);
        return Ok(channel);
    }

    let text = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    match serde_json::from_str::<ReleaseChannelConfig>(&text) {
        Ok(cfg) => {
            set_current(cfg.channel);
            Ok(cfg.channel)
        }
        Err(e) => {
            log::warn!("Failed to parse release_channel.json, resetting: {e}");
            let channel = ReleaseChannel::default();
            let _ = save(app, channel);
            set_current(channel);
            Ok(channel)
        }
    }
}

pub fn save(app: &tauri::AppHandle, channel: ReleaseChannel) -> Result<(), String> {
    let path = config_path(app)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let cfg = ReleaseChannelConfig {
        version: RELEASE_CHANNEL_CONFIG_VERSION,
        channel,
    };
    let json = serde_json::to_string_pretty(&cfg).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| e.to_string())?;
    set_current(channel);
    Ok(())
}

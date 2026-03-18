use discord_rich_presence::{
    activity::{self, Button},
    DiscordIpc, DiscordIpcClient,
};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;

const DISCORD_CLIENT_ID: &str = "1481955684320018593";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PresencePayload {
    pub details: String,
    pub state: Option<String>,
    pub large_image: Option<String>,
    pub large_text: Option<String>,
    pub button_label: Option<String>,
    pub button_url: Option<String>,
}

#[derive(Default)]
pub struct DiscordPresenceState {
    client: Mutex<Option<DiscordIpcClient>>,
    last_payload: Mutex<Option<PresencePayload>>,
}

fn ensure_connected(state: &DiscordPresenceState) -> Result<(), String> {
    let mut guard = state
        .client
        .lock()
        .map_err(|_| "discord presence lock poisoned".to_string())?;

    if guard.is_some() {
        return Ok(());
    }

    let mut client = DiscordIpcClient::new(DISCORD_CLIENT_ID).map_err(|e| e.to_string())?;
    client.connect().map_err(|e| e.to_string())?;
    *guard = Some(client);
    Ok(())
}

fn build_activity<'a>(payload: &'a PresencePayload) -> activity::Activity<'a> {
    let mut activity = activity::Activity::new().details(payload.details.as_str());
    activity = activity.activity_type(activity::ActivityType::Competing);

    if let Some(state) = payload.state.as_deref() {
        activity = activity.state(state);
    }

    if payload.large_image.is_some() || payload.large_text.is_some() {
        let mut assets = activity::Assets::new();
        if let Some(image) = payload.large_image.as_deref() {
            assets = assets.large_image(image);
        }
        if let Some(text) = payload.large_text.as_deref() {
            assets = assets.large_text(text);
        }
        activity = activity.assets(assets);
    }

    if let (Some(label), Some(url)) = (
        payload.button_label.as_deref(),
        payload.button_url.as_deref(),
    ) {
        activity = activity.buttons(vec![Button::new(label, url)]);
    }

    activity
}

#[tauri::command]
pub fn set_discord_presence(
    payload: PresencePayload,
    state: tauri::State<'_, DiscordPresenceState>,
) -> Result<bool, String> {
    {
        let last = state
            .last_payload
            .lock()
            .map_err(|_| "discord presence cache lock poisoned".to_string())?;
        if last.as_ref() == Some(&payload) {
            return Ok(true);
        }
    }

    ensure_connected(&state)?;

    {
        let mut client_guard = state
            .client
            .lock()
            .map_err(|_| "discord presence lock poisoned".to_string())?;
        let client = client_guard
            .as_mut()
            .ok_or_else(|| "discord client not initialized".to_string())?;
        client
            .set_activity(build_activity(&payload))
            .map_err(|e| e.to_string())?;
    }

    let mut last = state
        .last_payload
        .lock()
        .map_err(|_| "discord presence cache lock poisoned".to_string())?;
    *last = Some(payload);

    Ok(true)
}

#[tauri::command]
pub fn clear_discord_presence(
    state: tauri::State<'_, DiscordPresenceState>,
) -> Result<bool, String> {
    {
        let mut last = state
            .last_payload
            .lock()
            .map_err(|_| "discord presence cache lock poisoned".to_string())?;
        *last = None;
    }

    let mut client_guard = state
        .client
        .lock()
        .map_err(|_| "discord presence lock poisoned".to_string())?;
    if let Some(client) = client_guard.as_mut() {
        let _ = client.clear_activity();
    }

    Ok(true)
}

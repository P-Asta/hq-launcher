use discord_rich_presence::{
    activity::{self, Button},
    DiscordIpc, DiscordIpcClient,
};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    net::TcpStream,
    path::PathBuf,
    sync::Mutex,
    time::{Duration, Instant},
};
use tungstenite::{client::IntoClientRequest, stream::MaybeTlsStream, Message, WebSocket};

const DISCORD_CLIENT_ID: &str = "1481955684320018593";
const DEFAULT_STREAM_OVERLAYS_WS_PORT: u16 = 8000;
const DEFAULT_PARTY_MAX_SIZE: i32 = 4;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PresencePayload {
    pub details: String,
    pub state: Option<String>,
    pub large_image: Option<String>,
    pub large_text: Option<String>,
    pub small_image: Option<String>,
    pub small_text: Option<String>,
    pub button_label: Option<String>,
    pub button_url: Option<String>,
    pub use_stream_overlays: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ResolvedPresencePayload {
    details: String,
    state: Option<String>,
    large_image: Option<String>,
    large_text: Option<String>,
    small_image: Option<String>,
    small_text: Option<String>,
    button_label: Option<String>,
    button_url: Option<String>,
    party: Option<[i32; 2]>,
}

#[derive(Debug, Default, Deserialize)]
struct StreamOverlaysEnvelope {
    #[serde(rename = "type")]
    message_type: Option<String>,
    #[serde(default, rename = "showOverlay")]
    _show_overlay: Option<bool>,
    #[serde(default, rename = "crewCount")]
    crew_count: Option<i32>,
    #[serde(default, rename = "moonName")]
    moon_name: Option<String>,
    #[serde(default, rename = "weatherName")]
    weather_name: Option<String>,
    #[serde(default, rename = "quotaValue")]
    quota_value: Option<i32>,
    #[serde(default, rename = "quotaIndex")]
    quota_index: Option<i32>,
    #[serde(default, rename = "lootValue")]
    loot_value: Option<i32>,
}

#[derive(Default)]
pub struct DiscordPresenceState {
    client: Mutex<Option<DiscordIpcClient>>,
    last_payload: Mutex<Option<ResolvedPresencePayload>>,
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

fn build_activity<'a>(payload: &'a ResolvedPresencePayload) -> activity::Activity<'a> {
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
        if let Some(image) = payload.small_image.as_deref() {
            assets = assets.small_image(image);
        }
        if let Some(text) = payload.small_text.as_deref() {
            assets = assets.small_text(text);
        }
        activity = activity.assets(assets);
    }

    if let Some([current, max]) = payload.party {
        activity = activity.party(
            activity::Party::new()
                .id("hq-launcher-stream-overlays")
                .size([current, max]),
        );
    }

    if let (Some(label), Some(url)) = (
        payload.button_label.as_deref(),
        payload.button_url.as_deref(),
    ) {
        activity = activity.buttons(vec![Button::new(label, url)]);
    }

    activity
}

fn stream_overlays_config_path() -> Option<PathBuf> {
    let local_app_data = std::env::var_os("LOCALAPPDATA")?;
    let mut path = PathBuf::from(local_app_data);
    path.push("..");
    path.push("LocalLow");
    path.push("ZeekerssRBLX");
    path.push("Lethal Company");
    path.push("StreamOverlays");
    path.push("global.cfg");
    Some(path)
}

fn stream_overlays_ws_port() -> u16 {
    let Some(config_path) = stream_overlays_config_path() else {
        return DEFAULT_STREAM_OVERLAYS_WS_PORT;
    };

    let Ok(contents) = fs::read_to_string(config_path) else {
        return DEFAULT_STREAM_OVERLAYS_WS_PORT;
    };

    contents
        .lines()
        .find_map(|line| {
            let trimmed = line.trim();
            trimmed
                .strip_prefix("WebSocketPort = ")
                .and_then(|value| value.trim().parse::<u16>().ok())
        })
        .unwrap_or(DEFAULT_STREAM_OVERLAYS_WS_PORT)
}

fn connect_stream_overlays_socket() -> Option<WebSocket<MaybeTlsStream<TcpStream>>> {
    let request = format!("ws://127.0.0.1:{}/overlay", stream_overlays_ws_port())
        .into_client_request()
        .ok()?;
    let (socket, _) = tungstenite::connect(request).ok()?;
    Some(socket)
}

fn receive_stream_overlays_data() -> Option<StreamOverlaysEnvelope> {
    let mut socket = connect_stream_overlays_socket()?;
    match socket.get_mut() {
        MaybeTlsStream::Plain(stream) => {
            let _ = stream.set_read_timeout(Some(Duration::from_millis(1200)));
        }
        MaybeTlsStream::NativeTls(stream) => {
            let _ = stream
                .get_mut()
                .set_read_timeout(Some(Duration::from_millis(1200)));
        }
        #[allow(unreachable_patterns)]
        _ => {}
    }
    let deadline = Instant::now() + Duration::from_millis(1500);

    while Instant::now() < deadline {
        let message = socket.read().ok()?;
        match message {
            Message::Text(text) => {
                let parsed = serde_json::from_str::<StreamOverlaysEnvelope>(&text).ok()?;
                if parsed.message_type.as_deref() == Some("data") {
                    return Some(parsed);
                }
            }
            Message::Binary(bytes) => {
                let text = String::from_utf8(bytes.to_vec()).ok()?;
                let parsed = serde_json::from_str::<StreamOverlaysEnvelope>(&text).ok()?;
                if parsed.message_type.as_deref() == Some("data") {
                    return Some(parsed);
                }
            }
            _ => {}
        }
    }

    None
}

fn truncate_text(value: String) -> String {
    let max_chars = 120;
    if value.chars().count() <= max_chars {
        value
    } else {
        let truncated = value
            .chars()
            .take(max_chars.saturating_sub(1))
            .collect::<String>();
        format!("{truncated}…")
    }
}

fn resolve_presence_payload(payload: &PresencePayload) -> ResolvedPresencePayload {
    let mut resolved = ResolvedPresencePayload {
        details: payload.details.clone(),
        state: payload.state.clone(),
        large_image: payload.large_image.clone(),
        large_text: payload.large_text.clone(),
        small_image: payload.small_image.clone(),
        small_text: payload.small_text.clone(),
        button_label: payload.button_label.clone(),
        button_url: payload.button_url.clone(),
        party: None,
    };

    if !payload.use_stream_overlays {
        return resolved;
    }

    let Some(overlay_data) = receive_stream_overlays_data() else {
        return resolved;
    };

    let Some(moon_name) = overlay_data
        .moon_name
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return resolved;
    };

    let mut extra_parts = Vec::new();
    if let Some(crew_count) = overlay_data.crew_count.filter(|value| *value > 0) {
        resolved.party = Some([crew_count, crew_count.max(DEFAULT_PARTY_MAX_SIZE)]);
        let base_large_text = resolved
            .large_text
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("grinding");
        resolved.large_text = Some(format!(
            "{base_large_text} ({crew_count}/{DEFAULT_PARTY_MAX_SIZE})"
        ));
    }

    resolved.state = Some(truncate_text(format!("Playing {moon_name}")));

    if let Some(quota_value) = overlay_data.quota_value {
        match overlay_data.quota_index {
            Some(quota_index) => extra_parts.push(format!("q{quota_index} {quota_value}")),
            None => extra_parts.push(format!("quota {quota_value}")),
        }
    }

    if let Some(loot_value) = overlay_data.loot_value {
        extra_parts.push(format!("shiploot: {loot_value}"));
    }

    if !extra_parts.is_empty() {
        resolved.details = truncate_text(match extra_parts.as_slice() {
            [quota_text, ship_loot_text] => format!("{quota_text} ({ship_loot_text})"),
            _ => extra_parts.join(" "),
        });
    } else {
        resolved.details = truncate_text(resolved.details);
    }

    if let Some(state) = resolved.state.take() {
        resolved.state = match payload.state.as_deref().map(str::trim) {
            Some(existing) if !existing.is_empty() && existing != state => {
                Some(truncate_text(format!("{existing} • {state}")))
            }
            _ => Some(state),
        };
    }

    resolved
}

#[tauri::command]
pub fn set_discord_presence(
    payload: PresencePayload,
    state: tauri::State<'_, DiscordPresenceState>,
) -> Result<bool, String> {
    let resolved_payload = resolve_presence_payload(&payload);

    {
        let last = state
            .last_payload
            .lock()
            .map_err(|_| "discord presence cache lock poisoned".to_string())?;
        if last.as_ref() == Some(&resolved_payload) {
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
            .set_activity(build_activity(&resolved_payload))
            .map_err(|e| e.to_string())?;
    }

    let mut last = state
        .last_payload
        .lock()
        .map_err(|_| "discord presence cache lock poisoned".to_string())?;
    *last = Some(resolved_payload);

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

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::Manager;

const SHEETS_SCOPE: &str = "https://www.googleapis.com/auth/spreadsheets";
const DRIVE_METADATA_SCOPE: &str = "https://www.googleapis.com/auth/drive.metadata.readonly";
const BUNDLED_OAUTH_CLIENT_ID: Option<&str> = option_env!("GOOGLE_LCSTATS_CLIENT_ID");
const BUNDLED_OAUTH_CLIENT_SECRET: Option<&str> = option_env!("GOOGLE_LCSTATS_CLIENT_SECRET");

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GoogleLcStatsAuthState {
    pub authenticated: bool,
    pub scope: Option<String>,
    pub expires_at: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredToken {
    access_token: String,
    #[serde(default)]
    client_id: Option<String>,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    scope: Option<String>,
    #[serde(default)]
    token_type: Option<String>,
    #[serde(default)]
    expires_at: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
struct TokenResponse {
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    expires_in: Option<u64>,
    #[serde(default)]
    scope: Option<String>,
    #[serde(default)]
    token_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LcStatsSettings {
    pub spreadsheet_id: String,
    pub active_sheet_name: String,
    pub start_column: String,
    pub quota_column: String,
    pub sell_column: String,
    pub layout: String,
    #[serde(default)]
    pub google_client_id: String,
    #[serde(default)]
    pub google_client_secret: String,
}

impl Default for LcStatsSettings {
    fn default() -> Self {
        Self {
            spreadsheet_id: String::new(),
            active_sheet_name: String::new(),
            start_column: "D".to_string(),
            quota_column: "B".to_string(),
            sell_column: "AB".to_string(),
            layout: "AutoSheetModel".to_string(),
            google_client_id: String::new(),
            google_client_secret: String::new(),
        }
    }
}

#[derive(Debug, Clone)]
struct OAuthCredentials {
    client_id: String,
    client_secret: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoogleSpreadsheetFile {
    pub id: String,
    pub name: String,
}

fn token_path(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("config");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join("google_lcstats_oauth.json"))
}

fn settings_path(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("config");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join("lcstats_settings.json"))
}

fn read_token(app: &tauri::AppHandle) -> Result<Option<StoredToken>, String> {
    let path = token_path(app)?;
    if !path.exists() {
        return Ok(None);
    }
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    serde_json::from_str::<StoredToken>(&text)
        .map(Some)
        .map_err(|e| e.to_string())
}

fn write_token(app: &tauri::AppHandle, token: &StoredToken) -> Result<(), String> {
    let path = token_path(app)?;
    let text = serde_json::to_string_pretty(token).map_err(|e| e.to_string())?;
    std::fs::write(path, text).map_err(|e| e.to_string())
}

fn token_body(params: &[(&str, &str)]) -> String {
    params
        .iter()
        .map(|(key, value)| format!("{}={}", url_encode(key), url_encode(value)))
        .collect::<Vec<_>>()
        .join("&")
}

fn token_body_vec(params: Vec<(&str, &str)>) -> String {
    token_body(&params)
}

fn now_epoch_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn has_required_scope(scope: Option<&str>) -> bool {
    let scopes: std::collections::HashSet<&str> = scope.unwrap_or("").split_whitespace().collect();
    scopes.contains(SHEETS_SCOPE) && scopes.contains(DRIVE_METADATA_SCOPE)
}

fn url_encode(value: &str) -> String {
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

fn url_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(hex) = std::str::from_utf8(&bytes[i + 1..i + 3]) {
                if let Ok(v) = u8::from_str_radix(hex, 16) {
                    out.push(v);
                    i += 3;
                    continue;
                }
            }
        }
        out.push(if bytes[i] == b'+' { b' ' } else { bytes[i] });
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

fn parse_query(query: &str) -> HashMap<String, String> {
    query
        .split('&')
        .filter_map(|part| {
            let (key, value) = part.split_once('=')?;
            Some((url_decode(key), url_decode(value)))
        })
        .collect()
}

fn base64_url_no_pad(bytes: &[u8]) -> String {
    URL_SAFE_NO_PAD.encode(bytes)
}

fn generate_code_verifier() -> Result<String, String> {
    let mut bytes = [0_u8; 64];
    getrandom::fill(&mut bytes).map_err(|e| e.to_string())?;
    Ok(base64_url_no_pad(&bytes))
}

fn code_challenge(verifier: &str) -> String {
    let digest = Sha256::digest(verifier.as_bytes());
    base64_url_no_pad(&digest)
}

fn oauth_credentials(app: &tauri::AppHandle) -> Result<OAuthCredentials, String> {
    let settings = get_settings(app.clone())?;
    let custom_client_id = settings.google_client_id.trim().to_string();
    let client_id = if custom_client_id.is_empty() {
        BUNDLED_OAUTH_CLIENT_ID
            .unwrap_or_default()
            .trim()
            .to_string()
    } else {
        custom_client_id
    };
    if client_id.is_empty() {
        return Err(
            "Google OAuth client ID is required. Add it in LCStatsTracker settings.".to_string(),
        );
    }
    let custom_client_secret = settings.google_client_secret.trim().to_string();
    let client_secret = if !custom_client_secret.is_empty() {
        Some(custom_client_secret)
    } else if settings.google_client_id.trim().is_empty() {
        BUNDLED_OAUTH_CLIENT_SECRET
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
    } else {
        None
    };
    Ok(OAuthCredentials {
        client_id,
        client_secret,
    })
}

fn listen_for_oauth_code(listener: TcpListener, expected_state: String) -> Result<String, String> {
    listener.set_nonblocking(true).map_err(|e| e.to_string())?;
    listener.set_ttl(64).map_err(|e| e.to_string())?;

    let started = Instant::now();
    let mut stream = loop {
        match listener.accept() {
            Ok((stream, _)) => break stream,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                if started.elapsed() > Duration::from_secs(180) {
                    return Err("Google login timed out.".to_string());
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(e) => return Err(e.to_string()),
        }
    };
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .map_err(|e| e.to_string())?;
    let mut buf = [0_u8; 4096];
    let n = stream.read(&mut buf).map_err(|e| e.to_string())?;
    let request = String::from_utf8_lossy(&buf[..n]);
    let first_line = request.lines().next().unwrap_or("");
    let target = first_line
        .split_whitespace()
        .nth(1)
        .ok_or_else(|| "invalid OAuth redirect request".to_string())?;
    let query = target.split_once('?').map(|(_, q)| q).unwrap_or_default();
    let params = parse_query(query);
    let state = params.get("state").cloned().unwrap_or_default();
    let mut body = "Google login completed. You can close this window.".to_string();

    let result = if state != expected_state {
        body = "Google login failed: invalid state.".to_string();
        Err("OAuth state mismatch".to_string())
    } else if let Some(error) = params.get("error") {
        body = format!("Google login failed: {error}");
        Err(format!("Google OAuth error: {error}"))
    } else {
        params
            .get("code")
            .cloned()
            .ok_or_else(|| "OAuth code was not returned".to_string())
    };

    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.as_bytes().len(),
        body
    );
    let _ = stream.write_all(response.as_bytes());
    result
}

pub fn auth_status(app: tauri::AppHandle) -> Result<GoogleLcStatsAuthState, String> {
    let Some(token) = read_token(&app)? else {
        return Ok(GoogleLcStatsAuthState::default());
    };
    let Ok(credentials) = oauth_credentials(&app) else {
        return Ok(GoogleLcStatsAuthState::default());
    };
    if token.client_id.as_deref() != Some(credentials.client_id.as_str()) {
        return Ok(GoogleLcStatsAuthState::default());
    }
    let has_scope = has_required_scope(token.scope.as_deref());
    let has_token = !token.access_token.trim().is_empty();
    Ok(GoogleLcStatsAuthState {
        authenticated: has_token && has_scope,
        scope: token.scope,
        expires_at: token.expires_at,
    })
}

async fn refresh_access_token(
    app: &tauri::AppHandle,
    token: &StoredToken,
) -> Result<StoredToken, String> {
    let credentials = oauth_credentials(app)?;
    let refresh_token = token
        .refresh_token
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| "Google login expired. Please sign in again.".to_string())?;
    if token.client_id.as_deref() != Some(credentials.client_id.as_str()) {
        return Err("Google OAuth client changed. Please sign in again.".to_string());
    }
    let mut params = vec![
        ("client_id", credentials.client_id.as_str()),
        ("refresh_token", refresh_token),
        ("grant_type", "refresh_token"),
    ];
    if let Some(client_secret) = credentials.client_secret.as_deref() {
        params.push(("client_secret", client_secret));
    }
    let body = token_body_vec(params);
    let response = reqwest::Client::new()
        .post("https://oauth2.googleapis.com/token")
        .header(
            reqwest::header::CONTENT_TYPE,
            "application/x-www-form-urlencoded",
        )
        .body(body)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?
        .json::<TokenResponse>()
        .await
        .map_err(|e| e.to_string())?;

    let next = StoredToken {
        access_token: response.access_token,
        client_id: Some(credentials.client_id),
        refresh_token: token.refresh_token.clone(),
        scope: response.scope.or_else(|| token.scope.clone()),
        token_type: response.token_type.or_else(|| token.token_type.clone()),
        expires_at: response
            .expires_in
            .map(|seconds| now_epoch_secs() + seconds),
    };
    write_token(app, &next)?;
    Ok(next)
}

pub async fn access_token(app: tauri::AppHandle) -> Result<String, String> {
    let token = read_token(&app)?.ok_or_else(|| "Google login is required.".to_string())?;
    let credentials = oauth_credentials(&app)?;
    if token.client_id.as_deref() != Some(credentials.client_id.as_str()) {
        return Err("Google OAuth client changed. Please sign in again.".to_string());
    }
    if !has_required_scope(token.scope.as_deref()) {
        return Err(
            "Google Sheets and Drive metadata permissions were not granted. Please login again."
                .to_string(),
        );
    }
    let expired = token
        .expires_at
        .is_some_and(|expires_at| expires_at <= now_epoch_secs().saturating_add(60));
    let token = if expired {
        refresh_access_token(&app, &token).await?
    } else {
        token
    };
    Ok(token.access_token)
}

pub async fn start_oauth(app: tauri::AppHandle) -> Result<GoogleLcStatsAuthState, String> {
    let credentials = oauth_credentials(&app)?;
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|e| e.to_string())?;
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();
    let redirect_uri = format!("http://127.0.0.1:{port}/oauth2redirect");
    let state = format!(
        "hq-launcher-lcstats-{}-{}",
        std::process::id(),
        now_epoch_secs()
    );
    let code_verifier = generate_code_verifier()?;
    let challenge = code_challenge(&code_verifier);

    let auth_url = format!(
        "https://accounts.google.com/o/oauth2/v2/auth?client_id={}&redirect_uri={}&response_type=code&scope={}&access_type=offline&prompt=consent&state={}&code_challenge={}&code_challenge_method=S256",
        url_encode(&credentials.client_id),
        url_encode(&redirect_uri),
        url_encode(&format!("{SHEETS_SCOPE} {DRIVE_METADATA_SCOPE}")),
        url_encode(&state),
        url_encode(&challenge)
    );
    opener::open(auth_url).map_err(|e| format!("failed to open Google login: {e}"))?;

    let expected_state = state.clone();
    let code = tauri::async_runtime::spawn_blocking(move || {
        listen_for_oauth_code(listener, expected_state)
    })
    .await
    .map_err(|e| e.to_string())??;

    let mut token_params = vec![
        ("client_id", credentials.client_id.as_str()),
        ("code", code.as_str()),
        ("code_verifier", code_verifier.as_str()),
        ("grant_type", "authorization_code"),
        ("redirect_uri", redirect_uri.as_str()),
    ];
    if let Some(client_secret) = credentials.client_secret.as_deref() {
        token_params.push(("client_secret", client_secret));
    }
    let token_body = token_body_vec(token_params);
    let response = reqwest::Client::new()
        .post("https://oauth2.googleapis.com/token")
        .header(
            reqwest::header::CONTENT_TYPE,
            "application/x-www-form-urlencoded",
        )
        .body(token_body)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?
        .json::<TokenResponse>()
        .await
        .map_err(|e| e.to_string())?;

    if !has_required_scope(response.scope.as_deref()) {
        return Err("Google Sheets and Drive metadata permissions were not granted.".to_string());
    }

    let token = StoredToken {
        access_token: response.access_token,
        client_id: Some(credentials.client_id),
        refresh_token: response.refresh_token,
        scope: response.scope,
        token_type: response.token_type,
        expires_at: response
            .expires_in
            .map(|seconds| now_epoch_secs() + seconds),
    };
    write_token(&app, &token)?;
    auth_status(app)
}

pub fn logout(app: tauri::AppHandle) -> Result<bool, String> {
    let path = token_path(&app)?;
    if path.exists() {
        std::fs::remove_file(path).map_err(|e| e.to_string())?;
    }
    Ok(true)
}

pub fn get_settings(app: tauri::AppHandle) -> Result<LcStatsSettings, String> {
    let path = settings_path(&app)?;
    if !path.exists() {
        return Ok(LcStatsSettings::default());
    }
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    serde_json::from_str::<LcStatsSettings>(&text).map_err(|e| e.to_string())
}

pub fn set_settings(app: tauri::AppHandle, settings: LcStatsSettings) -> Result<bool, String> {
    let current = get_settings(app.clone()).unwrap_or_default();
    let credentials_changed = current.google_client_id.trim() != settings.google_client_id.trim()
        || current.google_client_secret.trim() != settings.google_client_secret.trim();
    let path = settings_path(&app)?;
    let text = serde_json::to_string_pretty(&settings).map_err(|e| e.to_string())?;
    std::fs::write(path, text).map_err(|e| e.to_string())?;
    if credentials_changed {
        let token_path = token_path(&app)?;
        if token_path.exists() {
            std::fs::remove_file(token_path).map_err(|e| e.to_string())?;
        }
    }
    Ok(true)
}

#[derive(Debug, Deserialize)]
struct SpreadsheetMetadata {
    #[serde(default)]
    sheets: Vec<SheetMetadata>,
}

#[derive(Debug, Deserialize)]
struct SheetMetadata {
    properties: Option<SheetProperties>,
}

#[derive(Debug, Deserialize)]
struct SheetProperties {
    title: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DriveFilesResponse {
    #[serde(default)]
    files: Vec<DriveFile>,
}

#[derive(Debug, Deserialize)]
struct DriveFile {
    id: Option<String>,
    name: Option<String>,
    #[serde(default)]
    capabilities: Option<DriveFileCapabilities>,
}

#[derive(Debug, Deserialize)]
struct DriveFileCapabilities {
    #[serde(rename = "canEdit")]
    can_edit: Option<bool>,
}

pub async fn list_spreadsheets(
    app: tauri::AppHandle,
) -> Result<Vec<GoogleSpreadsheetFile>, String> {
    let token = access_token(app).await?;
    let query =
        "mimeType='application/vnd.google-apps.spreadsheet' and trashed=false and 'me' in writers";
    let url = format!(
        "https://www.googleapis.com/drive/v3/files?q={}&fields=files(id,name,capabilities/canEdit)&orderBy=modifiedTime desc&pageSize=100",
        url_encode(query)
    );
    let data = reqwest::Client::new()
        .get(url)
        .header(reqwest::header::AUTHORIZATION, format!("Bearer {token}"))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !data.status().is_success() {
        let status = data.status();
        let body = data.text().await.unwrap_or_default();
        return Err(format!("Google Drive API error ({status}): {body}"));
    }
    let data = data
        .json::<DriveFilesResponse>()
        .await
        .map_err(|e| e.to_string())?;
    Ok(data
        .files
        .into_iter()
        .filter(|file| {
            file.capabilities
                .as_ref()
                .and_then(|capabilities| capabilities.can_edit)
                .unwrap_or(false)
        })
        .filter_map(|file| {
            Some(GoogleSpreadsheetFile {
                id: file.id?,
                name: file.name?,
            })
        })
        .collect())
}

pub async fn list_sheet_names(
    app: tauri::AppHandle,
    spreadsheet_id: String,
) -> Result<Vec<String>, String> {
    let spreadsheet_id = spreadsheet_id.trim();
    if spreadsheet_id.is_empty() {
        return Ok(vec![]);
    }
    let token = access_token(app).await?;
    let url = format!(
        "https://sheets.googleapis.com/v4/spreadsheets/{}?fields=sheets.properties.title",
        url_encode(spreadsheet_id)
    );
    let data = reqwest::Client::new()
        .get(url)
        .header(reqwest::header::AUTHORIZATION, format!("Bearer {token}"))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !data.status().is_success() {
        let status = data.status();
        let body = data.text().await.unwrap_or_default();
        return Err(format!("Google Sheets API error ({status}): {body}"));
    }
    let data = data
        .json::<SpreadsheetMetadata>()
        .await
        .map_err(|e| e.to_string())?;
    Ok(data
        .sheets
        .into_iter()
        .filter_map(|sheet| sheet.properties.and_then(|props| props.title))
        .collect())
}

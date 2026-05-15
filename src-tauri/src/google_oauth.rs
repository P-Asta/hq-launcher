use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::Manager;

const DRIVE_FILE_SCOPE: &str = "https://www.googleapis.com/auth/drive.file";
const SHEETS_SCOPE: &str = "https://www.googleapis.com/auth/spreadsheets";
const DRIVE_METADATA_SCOPE: &str = "https://www.googleapis.com/auth/drive.metadata.readonly";
const BUNDLED_OAUTH_CLIENT_ID: Option<&str> = option_env!("GOOGLE_LCSTATS_CLIENT_ID");
const BUNDLED_OAUTH_CLIENT_SECRET: Option<&str> = option_env!("GOOGLE_LCSTATS_CLIENT_SECRET");
const BUNDLED_PICKER_API_KEY: Option<&str> = option_env!("GOOGLE_LCSTATS_PICKER_API_KEY");
const BUNDLED_PICKER_APP_ID: Option<&str> = option_env!("GOOGLE_LCSTATS_PICKER_APP_ID");

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
    #[serde(default)]
    pub google_picker_api_key: String,
    #[serde(default)]
    pub google_picker_app_id: String,
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
            google_picker_api_key: String::new(),
            google_picker_app_id: String::new(),
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoogleSheetInfo {
    pub sheet_id: i64,
    pub title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GooglePickerConfig {
    pub api_key: String,
    pub app_id: String,
    pub scope: String,
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
    scopes.contains(DRIVE_FILE_SCOPE) || scopes.contains(SHEETS_SCOPE)
}

fn has_scope(scope: Option<&str>, required: &str) -> bool {
    scope
        .unwrap_or("")
        .split_whitespace()
        .any(|scope| scope == required)
}

fn requested_oauth_scope(credentials: &OAuthCredentials) -> String {
    let bundled_client_id = BUNDLED_OAUTH_CLIENT_ID.unwrap_or_default().trim();
    if !bundled_client_id.is_empty() && credentials.client_id == bundled_client_id {
        DRIVE_FILE_SCOPE.to_string()
    } else {
        format!("{SHEETS_SCOPE} {DRIVE_METADATA_SCOPE}")
    }
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

fn project_number_from_client_id(client_id: &str) -> Option<String> {
    let (prefix, _) = client_id.split_once('-')?;
    if !prefix.is_empty() && prefix.bytes().all(|b| b.is_ascii_digit()) {
        Some(prefix.to_string())
    } else {
        None
    }
}

pub fn picker_config(app: tauri::AppHandle) -> Result<GooglePickerConfig, String> {
    let settings = get_settings(app.clone())?;
    let credentials = oauth_credentials(&app)?;
    let custom_oauth = !settings.google_client_id.trim().is_empty();
    let custom_picker_settings = !settings.google_picker_api_key.trim().is_empty()
        || !settings.google_picker_app_id.trim().is_empty();
    let api_key = if settings.google_picker_api_key.trim().is_empty() {
        if custom_oauth && !custom_picker_settings {
            String::new()
        } else {
            BUNDLED_PICKER_API_KEY
                .unwrap_or_default()
                .trim()
                .to_string()
        }
    } else {
        settings.google_picker_api_key.trim().to_string()
    };
    if api_key.is_empty() {
        return Err(
            "Google Picker API key is required. Add it in LCStatsTracker settings.".to_string(),
        );
    }
    let app_id = if settings.google_picker_app_id.trim().is_empty() {
        if custom_oauth && !custom_picker_settings {
            String::new()
        } else {
            BUNDLED_PICKER_APP_ID
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned)
                .or_else(|| project_number_from_client_id(&credentials.client_id))
                .unwrap_or_default()
        }
    } else {
        settings.google_picker_app_id.trim().to_string()
    };
    if app_id.is_empty() {
        return Err(
            "Google Picker App ID is required. Add the Cloud project number in LCStatsTracker settings."
                .to_string(),
        );
    }
    Ok(GooglePickerConfig {
        api_key,
        app_id,
        scope: DRIVE_FILE_SCOPE.to_string(),
    })
}

fn http_response(status: &str, content_type: &str, body: &str) -> String {
    format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.as_bytes().len(),
        body
    )
}

fn picker_page(
    access_token: &str,
    api_key: &str,
    app_id: &str,
    state: &str,
) -> Result<String, String> {
    let access_token = serde_json::to_string(access_token).map_err(|e| e.to_string())?;
    let api_key = serde_json::to_string(api_key).map_err(|e| e.to_string())?;
    let app_id = serde_json::to_string(app_id).map_err(|e| e.to_string())?;
    let state = serde_json::to_string(state).map_err(|e| e.to_string())?;
    Ok(format!(
        r#"<!doctype html>
<html>
<head>
  <meta charset="utf-8">
  <title>Google Sheets Picker</title>
  <style>
    html, body {{ height: 100%; margin: 0; font-family: system-ui, sans-serif; color: #202124; }}
    body {{ display: grid; place-items: center; }}
    .box {{ max-width: 520px; padding: 24px; text-align: center; }}
    button {{ border: 0; border-radius: 999px; background: #1a73e8; color: white; padding: 12px 18px; font-weight: 600; }}
    p {{ color: #5f6368; }}
  </style>
</head>
<body>
  <div class="box">
    <h1>Select a Google Sheets file</h1>
    <p>If the picker does not open automatically, click the button below.</p>
    <button id="open">Open Google Picker</button>
  </div>
  <script>
    const accessToken = {access_token};
    const apiKey = {api_key};
    const appId = {app_id};
    const state = {state};

    function finish(path, params = {{}}) {{
      const url = new URL(path, window.location.origin);
      url.searchParams.set("state", state);
      for (const [key, value] of Object.entries(params)) {{
        url.searchParams.set(key, value || "");
      }}
      window.location.href = url.toString();
    }}

    function openPicker() {{
      const picker = window.google && window.google.picker;
      if (!picker) return;
      const viewId = picker.ViewId.SPREADSHEETS || picker.ViewId.DOCS;
      const view = new picker.DocsView(viewId);
      if (view.setMimeTypes) view.setMimeTypes("application/vnd.google-apps.spreadsheet");
      if (view.setMode && picker.DocsViewMode && picker.DocsViewMode.LIST) {{
        view.setMode(picker.DocsViewMode.LIST);
      }}
      if (view.setIncludeFolders) view.setIncludeFolders(false);
      if (view.setSelectFolderEnabled) view.setSelectFolderEnabled(false);
      new picker.PickerBuilder()
        .addView(view)
        .setOAuthToken(accessToken)
        .setDeveloperKey(apiKey)
        .setAppId(appId)
        .setCallback((data) => {{
          const action = data[picker.Response.ACTION] || data.action;
          if (action === picker.Action.CANCEL) {{
            finish("/cancel");
            return;
          }}
          if (action !== picker.Action.PICKED) return;
          const docs = data[picker.Response.DOCUMENTS] || data.docs || [];
          const doc = docs[0];
          if (!doc) {{
            finish("/cancel");
            return;
          }}
          finish("/picked", {{
            id: doc[picker.Document.ID] || doc.id || "",
            name: doc[picker.Document.NAME] || doc.name || "",
            url: doc[picker.Document.URL] || doc.url || ""
          }});
        }})
        .build()
        .setVisible(true);
    }}

    function loadPicker() {{
      gapi.load("picker", {{ callback: openPicker }});
    }}

    document.getElementById("open").addEventListener("click", openPicker);
  </script>
  <script async defer src="https://apis.google.com/js/api.js" onload="loadPicker()"></script>
</body>
</html>"#
    ))
}

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn spreadsheet_list_page(files: &[GoogleSpreadsheetFile], state: &str) -> String {
    let state = html_escape(state);
    let items = if files.is_empty() {
        "<p>No editable Google Sheets files were found.</p>".to_string()
    } else {
        files
            .iter()
            .map(|file| {
                format!(
                    r#"<form method="get" action="/picked">
      <input type="hidden" name="state" value="{state}">
      <input type="hidden" name="id" value="{id}">
      <input type="hidden" name="name" value="{name}">
      <button type="submit">{name}<span>{id}</span></button>
    </form>"#,
                    state = state,
                    id = html_escape(&file.id),
                    name = html_escape(&file.name),
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };

    format!(
        r#"<!doctype html>
<html>
<head>
  <meta charset="utf-8">
  <title>Select a Google Sheets file</title>
  <style>
    * {{ box-sizing: border-box; }}
    html, body {{ min-height: 100%; margin: 0; font-family: system-ui, sans-serif; color: #f8fafc; background: #111827; }}
    body {{ display: grid; place-items: center; padding: 32px; }}
    main {{ width: min(720px, 100%); }}
    h1 {{ margin: 0 0 8px; font-size: 24px; }}
    p {{ color: #cbd5e1; }}
    .list {{ display: grid; gap: 10px; margin-top: 20px; }}
    button {{ width: 100%; border: 1px solid rgba(148,163,184,.35); border-radius: 8px; background: rgba(255,255,255,.06); color: inherit; padding: 12px 14px; text-align: left; font: inherit; cursor: pointer; }}
    button:hover, button:focus {{ border-color: rgba(248,250,252,.7); background: rgba(255,255,255,.1); outline: none; }}
    span {{ display: block; margin-top: 4px; color: #94a3b8; font-size: 12px; overflow-wrap: anywhere; }}
    a {{ display: inline-block; margin-top: 20px; color: #93c5fd; }}
  </style>
</head>
<body>
  <main>
    <h1>Select one Google Sheets file</h1>
    <p>Only one spreadsheet can be selected for LCStatsTracker.</p>
    <div class="list">
      {items}
    </div>
    <a href="/cancel?state={state}">Cancel</a>
  </main>
</body>
</html>"#
    )
}

enum SpreadsheetPickerUi {
    GooglePicker {
        access_token: String,
        api_key: String,
        app_id: String,
    },
    SpreadsheetList {
        files: Vec<GoogleSpreadsheetFile>,
    },
}

fn listen_for_picker_selection(
    listener: TcpListener,
    ui: SpreadsheetPickerUi,
    expected_state: String,
) -> Result<Option<GoogleSpreadsheetFile>, String> {
    listener.set_nonblocking(true).map_err(|e| e.to_string())?;
    listener.set_ttl(64).map_err(|e| e.to_string())?;

    let started = Instant::now();
    while started.elapsed() <= Duration::from_secs(180) {
        let (mut stream, _) = match listener.accept() {
            Ok(value) => value,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(100));
                continue;
            }
            Err(e) => return Err(e.to_string()),
        };
        stream.set_nonblocking(false).map_err(|e| e.to_string())?;
        stream
            .set_read_timeout(Some(Duration::from_secs(10)))
            .map_err(|e| e.to_string())?;
        let mut buf = [0_u8; 8192];
        let n = stream.read(&mut buf).map_err(|e| e.to_string())?;
        let request = String::from_utf8_lossy(&buf[..n]);
        let first_line = request.lines().next().unwrap_or("");
        let target = first_line.split_whitespace().nth(1).unwrap_or("/");
        let path = target
            .split_once('?')
            .map(|(path, _)| path)
            .unwrap_or(target);
        let query = target.split_once('?').map(|(_, q)| q).unwrap_or_default();
        let params = parse_query(query);

        match path {
            "/" => {
                let body = match &ui {
                    SpreadsheetPickerUi::GooglePicker {
                        access_token,
                        api_key,
                        app_id,
                    } => picker_page(access_token, api_key, app_id, &expected_state)?,
                    SpreadsheetPickerUi::SpreadsheetList { files } => {
                        spreadsheet_list_page(files, &expected_state)
                    }
                };
                let _ = stream.write_all(http_response("200 OK", "text/html", &body).as_bytes());
            }
            "/picked" => {
                if params.get("state") != Some(&expected_state) {
                    let body = "Google Picker failed: invalid state.";
                    let _ = stream
                        .write_all(http_response("400 Bad Request", "text/plain", body).as_bytes());
                    return Err("Google Picker state mismatch".to_string());
                }
                let id = params.get("id").cloned().unwrap_or_default();
                let name = params.get("name").cloned().unwrap_or_default();
                let body = "Google Sheets file selected. You can close this window.";
                let _ = stream.write_all(http_response("200 OK", "text/plain", body).as_bytes());
                if id.trim().is_empty() {
                    return Ok(None);
                }
                return Ok(Some(GoogleSpreadsheetFile { id, name }));
            }
            "/cancel" => {
                let body = "Google Picker was cancelled. You can close this window.";
                let _ = stream.write_all(http_response("200 OK", "text/plain", body).as_bytes());
                return Ok(None);
            }
            "/favicon.ico" => {
                let _ = stream.write_all(
                    "HTTP/1.1 204 No Content\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                        .as_bytes(),
                );
            }
            _ => {
                let _ = stream.write_all(
                    http_response("404 Not Found", "text/plain", "Not found").as_bytes(),
                );
            }
        }
    }

    Err("Google Picker timed out.".to_string())
}

async fn open_spreadsheet_picker_ui(
    ui: SpreadsheetPickerUi,
) -> Result<Option<GoogleSpreadsheetFile>, String> {
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|e| e.to_string())?;
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();
    let state = format!(
        "hq-launcher-picker-{}-{}",
        std::process::id(),
        now_epoch_secs()
    );
    opener::open(format!("http://127.0.0.1:{port}/"))
        .map_err(|e| format!("failed to open Google Picker: {e}"))?;

    tauri::async_runtime::spawn_blocking(move || listen_for_picker_selection(listener, ui, state))
        .await
        .map_err(|e| e.to_string())?
}

pub async fn pick_spreadsheet(
    app: tauri::AppHandle,
    _spreadsheet_id: String,
) -> Result<Option<GoogleSpreadsheetFile>, String> {
    let token = access_token(app.clone()).await?;
    let selected = match picker_config(app.clone()) {
        Ok(picker) => {
            open_spreadsheet_picker_ui(SpreadsheetPickerUi::GooglePicker {
                access_token: token.clone(),
                api_key: picker.api_key,
                app_id: picker.app_id,
            })
            .await
        }
        Err(picker_error) => {
            let files = list_spreadsheets(app).await.map_err(|list_error| {
                format!("{picker_error} Spreadsheet list unavailable: {list_error}")
            })?;
            open_spreadsheet_picker_ui(SpreadsheetPickerUi::SpreadsheetList { files }).await
        }
    }?;

    if let Some(file) = &selected {
        assert_spreadsheet_can_edit_with_token(&reqwest::Client::new(), &token, &file.id).await?;
    }
    Ok(selected)
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
    stream.set_nonblocking(false).map_err(|e| e.to_string())?;
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
            "Google Sheets file permission was not granted. Please login again.".to_string(),
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

pub async fn assert_spreadsheet_can_edit(
    app: tauri::AppHandle,
    client: &reqwest::Client,
    spreadsheet_id: &str,
) -> Result<(), String> {
    let token = read_token(&app)?.ok_or_else(|| "Google login is required.".to_string())?;
    if !has_scope(token.scope.as_deref(), DRIVE_FILE_SCOPE)
        && !has_scope(token.scope.as_deref(), DRIVE_METADATA_SCOPE)
    {
        return Ok(());
    }
    let access_token = access_token(app).await?;
    assert_spreadsheet_can_edit_with_token(client, &access_token, spreadsheet_id).await
}

async fn assert_spreadsheet_can_edit_with_token(
    client: &reqwest::Client,
    access_token: &str,
    spreadsheet_id: &str,
) -> Result<(), String> {
    let url = format!(
        "https://www.googleapis.com/drive/v3/files/{}?fields=id,name,capabilities/canEdit",
        url_encode(spreadsheet_id.trim())
    );
    let response = client
        .get(url)
        .header(
            reqwest::header::AUTHORIZATION,
            format!("Bearer {access_token}"),
        )
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!(
            "Google Drive file grant is missing for this spreadsheet ({status}): {body}. Select this spreadsheet again using the same Google account."
        ));
    }
    let file = response
        .json::<DriveFile>()
        .await
        .map_err(|e| e.to_string())?;
    let can_edit = file
        .capabilities
        .as_ref()
        .and_then(|capabilities| capabilities.can_edit)
        .unwrap_or(false);
    if !can_edit {
        return Err(format!(
            "The selected Google account cannot edit this spreadsheet{}. Share the sheet with edit access or choose an editable copy.",
            file.name
                .as_deref()
                .filter(|name| !name.trim().is_empty())
                .map(|name| format!(" ({name})"))
                .unwrap_or_default()
        ));
    }
    Ok(())
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
        url_encode(&requested_oauth_scope(&credentials)),
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
        return Err("Google Sheets file permission was not granted.".to_string());
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
    #[serde(rename = "sheetId")]
    sheet_id: Option<i64>,
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
    Ok(list_sheet_infos(app, spreadsheet_id)
        .await?
        .into_iter()
        .map(|sheet| sheet.title)
        .collect())
}

pub async fn list_sheet_infos(
    app: tauri::AppHandle,
    spreadsheet_id: String,
) -> Result<Vec<GoogleSheetInfo>, String> {
    let spreadsheet_id = spreadsheet_id.trim();
    if spreadsheet_id.is_empty() {
        return Ok(vec![]);
    }
    let token = access_token(app).await?;
    let url = format!(
        "https://sheets.googleapis.com/v4/spreadsheets/{}?fields=sheets.properties(sheetId,title)",
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
        .filter_map(|sheet| {
            let props = sheet.properties?;
            Some(GoogleSheetInfo {
                sheet_id: props.sheet_id?,
                title: props.title?,
            })
        })
        .collect())
}

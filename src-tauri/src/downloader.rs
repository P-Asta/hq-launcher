use expectrl::{ControlCode, Regex, Session};
use log::info;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command as StdCommand;
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::Emitter;
use tauri::Manager;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;

use crate::progress::{self, TaskProgressPayload};

fn strip_ansi(s: &str) -> String {
    // Minimal ANSI stripper for log display.
    // Removes common CSI/OSC sequences and carriage returns.
    let bytes = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0usize;
    while i < bytes.len() {
        let b = bytes[i];
        if b == 0x1b {
            // ESC sequence
            if i + 1 >= bytes.len() {
                break;
            }
            let next = bytes[i + 1];
            // CSI: ESC [
            if next == b'[' {
                i += 2;
                while i < bytes.len() {
                    let c = bytes[i];
                    i += 1;
                    // final byte of CSI is in 0x40..0x7E
                    if (0x40..=0x7E).contains(&c) {
                        break;
                    }
                }
                continue;
            }
            // OSC: ESC ]
            if next == b']' {
                i += 2;
                while i < bytes.len() {
                    // BEL terminator
                    if bytes[i] == 0x07 {
                        i += 1;
                        break;
                    }
                    // ST terminator: ESC \
                    if bytes[i] == 0x1b && i + 1 < bytes.len() && bytes[i + 1] == b'\\' {
                        i += 2;
                        break;
                    }
                    i += 1;
                }
                continue;
            }

            // Other ESC sequences: skip ESC + next byte (best-effort)
            i += 2;
            continue;
        }
        if b == b'\r' {
            i += 1;
            continue;
        }
        out.push(b);
        i += 1;
    }

    String::from_utf8_lossy(&out).to_string()
}

fn looks_like_twofactor_needed(text: &str) -> bool {
    let l = text.to_lowercase();
    // Patched IPC tokens
    if l.contains("steam_guard_device_code_required")
        || l.contains("steam_guard_email_code_required")
        || l.contains("steam_guard_code_required")
        || l.contains("auth_polling_wait")
    {
        return true;
    }

    // Heuristics (covering many DepotDownloader/SteamKit2 variants)
    l.contains("steam guard")
        || l.contains("steamguard")
        || l.contains("two-factor")
        || l.contains("two factor")
        || l.contains("2fa")
        || (l.contains("auth")
            && (l.contains("code") || l.contains("steam") || l.contains("guard")))
        || (l.contains("enter") && l.contains("code"))
        || l.contains("authentication code")
        || l.contains("security code")
        || l.contains("emailed")
        || (l.contains("email") && l.contains("code"))
        || (l.contains("sent") && l.contains("code"))
}

#[cfg(all(target_os = "windows", target_arch = "x86_64"))]
const DEPOT_DOWNLOADER_NAME: &str = "DepotDownloader-windows-x64";

#[cfg(all(target_os = "windows", target_arch = "aarch64"))]
const DEPOT_DOWNLOADER_NAME: &str = "DepotDownloader-windows-arm64";

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
const DEPOT_DOWNLOADER_NAME: &str = "DepotDownloader-macos-arm64";

#[cfg(all(target_os = "macos", target_arch = "x86_64"))]
const DEPOT_DOWNLOADER_NAME: &str = "DepotDownloader-macos-x64";

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
const DEPOT_DOWNLOADER_NAME: &str = "DepotDownloader-linux-x64";

#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
const DEPOT_DOWNLOADER_NAME: &str = "DepotDownloader-linux-arm64";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginCredentials {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginState {
    pub is_logged_in: bool,
    pub username: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", content = "data")]
pub enum DepotDownloaderEvent {
    Output(String),
    Progress {
        current: u64,
        total: u64,
    },
    NeedsTwoFactor {
        session_id: u64,
        message: Option<String>,
    },
    NeedsMobileConfirmation {
        session_id: u64,
    },
    LoginSuccess,
    LoginFailed(String),
    DownloadComplete,
    Error(String),
}

#[derive(Debug, Clone)]
pub struct DownloadTaskContext {
    pub version: u32,
    pub steps_total: u32,
    pub step: u32, // 1-based
    pub step_name: String,
}

fn overall_from_step(step: u32, step_progress: f64, steps_total: u32) -> f64 {
    let s = step.max(1).min(steps_total) as f64;
    let sp = step_progress.clamp(0.0, 1.0);
    (((s - 1.0) + sp) / (steps_total as f64)) * 100.0
}

#[derive(Default)]
pub struct DepotLoginState {
    next_id: AtomicU64,
    sessions: Mutex<HashMap<u64, mpsc::UnboundedSender<String>>>,
}

pub struct DepotDownloader {
    app: tauri::AppHandle,
    executable_path: PathBuf,
    config_dir: PathBuf,
    ipc_mode: bool,
}

impl DepotDownloader {
    const APP_ID: &'static str = "1966720";
    const DEPOT_ID: &'static str = "1966721";
    const PATCH_MARKER: &'static str = ".hq_launcher_ipc";

    pub fn new(app: &tauri::AppHandle) -> Result<Self, String> {
        let app_data = app
            .path()
            .app_data_dir()
            .map_err(|e| format!("failed to resolve app data dir: {e}"))?;

        let downloader_dir = app_data.join("downloader");
        let ipc_mode = downloader_dir.join(Self::PATCH_MARKER).exists();

        #[cfg(target_os = "windows")]
        let executable_path = downloader_dir.join("DepotDownloader.exe");

        #[cfg(not(target_os = "windows"))]
        let executable_path = downloader_dir.join("DepotDownloader");

        if !executable_path.exists() {
            return Err("DepotDownloader not installed. Please install it first.".to_string());
        }

        let config_dir = app_data.join("depot_config");
        std::fs::create_dir_all(&config_dir).map_err(|e| e.to_string())?;

        Ok(Self {
            app: app.clone(),
            executable_path,
            config_dir,
            ipc_mode,
        })
    }

    /// 로그인 상태 파일 경로
    fn login_state_path(&self) -> PathBuf {
        self.config_dir.join("login_state.json")
    }

    /// 저장된 로그인 상태 확인
    pub fn get_login_state(&self) -> LoginState {
        if let Ok(content) = std::fs::read_to_string(self.login_state_path()) {
            if let Ok(state) = serde_json::from_str::<LoginState>(&content) {
                return state;
            }
        }
        LoginState {
            is_logged_in: false,
            username: None,
        }
    }

    /// 로그인 상태 저장
    fn save_login_state(&self, state: &LoginState) -> Result<(), String> {
        let content = serde_json::to_string(state).map_err(|e| e.to_string())?;
        std::fs::write(self.login_state_path(), content).map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Steam 로그인
    pub async fn login(
        &self,
        credentials: LoginCredentials,
        two_factor_code: Option<String>,
    ) -> Result<(), String> {
        // DepotDownloader requires `-app` in some versions even for auth flows.
        // To avoid downloading the full depot during login, we use `-manifest-only`
        // against a single known depot.
        let login_tmp_dir = self.config_dir.join("_login_check");
        let _ = std::fs::create_dir_all(&login_tmp_dir);

        let args = vec![
            // Our patched build supports `-ipc` for machine-friendly prompts/logs.
            // (Official release won't have marker file, so we won't pass it.)
            // NOTE: keep this first so it is easy to spot in logs.
            // (No-op unless ipc_mode=true)
            // We push conditionally below.
            "-app".to_string(),
            Self::APP_ID.to_string(),
            "-depot".to_string(),
            Self::DEPOT_ID.to_string(),
            "-manifest-only".to_string(),
            // "-no-mobile".to_string(),
            "-dir".to_string(),
            login_tmp_dir.to_string_lossy().to_string(),
            "-username".to_string(),
            credentials.username.clone(),
            "-password".to_string(),
            credentials.password.clone(),
            // "-remember-password".to_string(),
        ];
        let mut args = args;
        if self.ipc_mode {
            args.insert(0, "-ipc".to_string());
        }

        log::info!("Attempting login for user: {}", credentials.username);

        let code_present = two_factor_code
            .as_ref()
            .map(|c| !c.trim().is_empty())
            .unwrap_or(false);

        log::info!(
            "Depot login: 2FA code provided? {}",
            if code_present { "yes" } else { "no" }
        );

        let mut child = Command::new(&self.executable_path)
            .args(&args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(Stdio::piped())
            .current_dir(&self.config_dir)
            .spawn()
            .map_err(|e| format!("Failed to spawn DepotDownloader: {e}"))?;

        let mut stdin = child.stdin.take();
        let stdout = child.stdout.take().ok_or("Failed to capture stdout")?;
        let stderr = child.stderr.take().ok_or("Failed to capture stderr")?;

        let (tx, mut rx) = mpsc::unbounded_channel::<(bool, String)>(); // (is_stderr, line)

        {
            let tx = tx.clone();
            tokio::spawn(async move {
                let mut r = BufReader::new(stdout).lines();
                while let Ok(Some(line)) = r.next_line().await {
                    let _ = tx.send((false, line));
                }
            });
        }
        {
            let tx = tx.clone();
            tokio::spawn(async move {
                let mut r = BufReader::new(stderr).lines();
                while let Ok(Some(line)) = r.next_line().await {
                    let _ = tx.send((true, line));
                }
            });
        }

        let mut needs_2fa = false;
        let mut auth_code_sent = false;
        let mut guard_prompt_seen = false;
        let mut last_output_at = Instant::now();
        let mut idle_ticks = tokio::time::interval(Duration::from_millis(500));

        let status = loop {
            tokio::select! {
                s = child.wait() => {
                    break s.map_err(|e| e.to_string())?;
                }
                _ = idle_ticks.tick() => {
                    // DepotDownloader sometimes prints Steam Guard prompt without a newline,
                    // so our line-based readers won't see it. If output stalls around login,
                    // assume it's waiting for Steam Guard and either request a code or send it.
                    let idle_for = last_output_at.elapsed();
                    let send_after = if code_present {
                        Duration::from_secs(2)
                    } else {
                        Duration::from_secs(8)
                    };

                    if idle_for >= send_after && !auth_code_sent {
                        // If we don't have a code yet, treat this as "Steam Guard code requested"
                        // and stop the process so UI can ask the user for the code.
                        if !code_present {
                            needs_2fa = true;
                            self.emit_event(DepotDownloaderEvent::NeedsTwoFactor { session_id: 0, message: None });
                            self.emit_event(DepotDownloaderEvent::Output(
                                "Steam Guard code requested. Check your email/Steam app, then enter the code and try again.".to_string(),
                            ));
                            let _ = child.kill().await;
                            let _ = child.wait().await;
                            let _ = std::fs::remove_dir_all(&login_tmp_dir);
                            return Err("Two-factor authentication required".to_string());
                        }

                        // Code is present: assume prompt exists (even without newline) and submit to stdin.
                        needs_2fa = true;
                        self.emit_event(DepotDownloaderEvent::NeedsTwoFactor { session_id: 0, message: None });
                        if let Some(code) = two_factor_code.as_ref() {
                            if let Some(input) = stdin.as_mut() {
                                self.emit_event(DepotDownloaderEvent::Output(
                                    "Submitting Steam Guard code...".to_string(),
                                ));
                                let _ = input.write_all(format!("{code}\n").as_bytes()).await;
                                let _ = input.flush().await;
                                auth_code_sent = true;
                                last_output_at = Instant::now();
                            }
                        }
                    }

                    // Hard timeout to avoid indefinite hangs.
                    if idle_for >= Duration::from_secs(90) {
                        let _ = child.kill().await;
                        let _ = child.wait().await;
                        let _ = std::fs::remove_dir_all(&login_tmp_dir);
                        return Err("Login timed out (Steam Guard / network). Please try again.".to_string());
                    }
                }
                msg = rx.recv() => {
                    let Some((is_stderr, line)) = msg else {
                        // no more output
                        continue;
                    };

                    if is_stderr {
                        log::error!("DepotDownloader error: {}", line);
                        self.emit_event(DepotDownloaderEvent::Output(format!("ERROR: {}", line)));
                    } else {
                        log::info!("DepotDownloader: {}", line);
                        self.emit_event(DepotDownloaderEvent::Output(line.clone()));
                    }

                    last_output_at = Instant::now();

                    let l = line.to_lowercase();

                    if l.contains("use the steam mobile app to confirm your sign in") {
                        let _ = child.kill().await;
                        let _ = child.wait().await;
                        let _ = std::fs::remove_dir_all(&login_tmp_dir);
                        return Err("Steam mobile confirmation required. Approve the login in Steam app and try again.".to_string());
                    }

                    if l.contains("previous 2-factor auth code") && l.contains("incorrect") {
                        let _ = child.kill().await;
                        let _ = child.wait().await;
                        let _ = std::fs::remove_dir_all(&login_tmp_dir);
                        return Err("Steam Guard code incorrect. Please try again.".to_string());
                    }

                    if l.contains("failed to authenticate with steam:")
                        && l.contains("no code was provided")
                    {
                        let _ = child.kill().await;
                        let _ = child.wait().await;
                        let _ = std::fs::remove_dir_all(&login_tmp_dir);
                        if code_present {
                            return Err("Steam Guard code was not accepted. Please try again.".to_string());
                        } else {
                            return Err("Two-factor authentication required".to_string());
                        }
                    }

                    // Common Steam Guard / 2FA prompts.
                    let asks_for_code =
                        l.contains("steam guard")
                        || l.contains("two-factor")
                        || l.contains("two factor")
                        || l.contains("2fa")
                        || (l.contains("enter") && l.contains("code"))
                        || l.contains("auth code")
                        || l.contains("emailed");

                    if asks_for_code {
                        needs_2fa = true;
                        guard_prompt_seen = true;
                        self.emit_event(DepotDownloaderEvent::NeedsTwoFactor { session_id: 0, message: None });

                        // If no code was provided, stop here so UI can ask user for the code.
                        if !code_present {
                            self.emit_event(DepotDownloaderEvent::Output(
                                "Steam Guard code requested. Check your email/Steam app, then enter the code and try again.".to_string(),
                            ));
                            let _ = child.kill().await;
                            let _ = child.wait().await;
                            let _ = std::fs::remove_dir_all(&login_tmp_dir);
                            return Err("Two-factor authentication required".to_string());
                        }

                        // Code present: DepotDownloader reads it from stdin.
                        if !auth_code_sent {
                            if let Some(code) = two_factor_code.as_ref() {
                                if let Some(input) = stdin.as_mut() {
                                    self.emit_event(DepotDownloaderEvent::Output(
                                        "Submitting Steam Guard code...".to_string(),
                                    ));
                                    let _ = input.write_all(format!("{code}\n").as_bytes()).await;
                                    let _ = input.flush().await;
                                    auth_code_sent = true;
                                    last_output_at = Instant::now();
                                }
                            }
                        } else if guard_prompt_seen {
                            // Prompt again after we already sent a code -> treat as invalid/expired.
                            let _ = child.kill().await;
                            let _ = child.wait().await;
                            let _ = std::fs::remove_dir_all(&login_tmp_dir);
                            return Err("Steam Guard code was rejected or expired. Please request a new code and try again.".to_string());
                        }
                    }
                }
            }
        };

        if !status.success() {
            if needs_2fa && two_factor_code.is_none() {
                let _ = std::fs::remove_dir_all(&login_tmp_dir);
                return Err("Two-factor authentication required".to_string());
            }
            let _ = std::fs::remove_dir_all(&login_tmp_dir);
            return Err(format!("Login failed with status: {}", status));
        }

        // If the process exited successfully, treat it as a successful login.
        // Some DepotDownloader flows won't emit a consistent "logged in" line.
        if needs_2fa && two_factor_code.is_none() {
            let _ = std::fs::remove_dir_all(&login_tmp_dir);
            return Err("Two-factor authentication required".to_string());
        }

        let state = LoginState {
            is_logged_in: true,
            username: Some(credentials.username),
        };
        self.save_login_state(&state)?;
        self.emit_event(DepotDownloaderEvent::LoginSuccess);
        log::info!("Login successful");

        // Best-effort cleanup of the temp output directory.
        let _ = std::fs::remove_dir_all(&login_tmp_dir);
        Ok(())
    }

    /// Steam 로그인 (interactive): monitors output, emits code request, waits for code via channel, then writes to stdin.
    pub async fn login_interactive(
        &self,
        session_id: u64,
        credentials: LoginCredentials,
        two_factor_code: Option<String>,
        rx_code: &mut mpsc::UnboundedReceiver<String>,
    ) -> Result<(), String> {
        // Expect-style login using a PTY on Windows (ConPTY) via expectrl.
        // This avoids the "no newline prompt" problem entirely.
        // Use a persistent cache dir for login. Do NOT delete it, because some DepotDownloader
        // versions may store useful state under `-dir` (and deleting it would break "remembered login").
        let login_cache_dir = self.config_dir.join("_login_cache");
        let _ = std::fs::create_dir_all(&login_cache_dir);

        // IMPORTANT: do not include password in logs / errors.
        // Also: ensure `current_dir` is the same config dir used for downloads,
        // otherwise remembered credentials won't be found later.
        let mut cmd = StdCommand::new(&self.executable_path);
        cmd.current_dir(&self.config_dir);
        if self.ipc_mode {
            cmd.arg("-ipc");
        }
        cmd.arg("-app")
            .arg(Self::APP_ID)
            .arg("-depot")
            .arg(Self::DEPOT_ID)
            .arg("-manifest-only")
            // .arg("-no-mobile")
            .arg("-dir")
            .arg(login_cache_dir.to_string_lossy().to_string())
            .arg("-username")
            .arg(credentials.username.clone())
            .arg("-password")
            .arg(credentials.password.clone())
            .arg("-remember-password");

        let mut p =
            Session::spawn(cmd).map_err(|_| "Failed to start DepotDownloader".to_string())?;
        // Use non-blocking `check()` loop instead of blocking `expect()` to ensure we keep
        // draining submitted codes and never hang on reads.

        // If user pre-provided a code, hold it; otherwise wait for submit.
        let mut pending_code: Option<String> = two_factor_code.and_then(|c| {
            let t = c.trim().to_string();
            if t.is_empty() {
                None
            } else {
                Some(t)
            }
        });

        let start = Instant::now();
        let mut last_output_at = Instant::now();
        let mut saw_login_progress = false;
        let mut requested_2fa = false;
        let mut saw_mobile_confirm = false;
        loop {
            // Drain submitted codes.
            while let Ok(code) = rx_code.try_recv() {
                let t = code.trim().to_string();
                if !t.is_empty() {
                    self.emit_event(DepotDownloaderEvent::Output(format!(
                        "Steam Guard code received (len={}).",
                        t.len()
                    )));
                    pending_code = Some(t);
                }
            }

            // If we already have a code, try sending it (idempotent).
            if let Some(code) = pending_code.take() {
                self.emit_event(DepotDownloaderEvent::Output(
                    "Submitting Steam Guard code...".to_string(),
                ));
                if let Err(e) = p.send_line(&code) {
                    let _ = p.send(ControlCode::EndOfText);
                    return Err(format!("Failed to send code to DepotDownloader: {e}"));
                }
            }

            // Read any available output (non-blocking).
            let m = p.check(Regex("(?s).+"));
            match m {
                Ok(caps) => {
                    let out_bytes = caps.get(0).unwrap_or(&[]);
                    if !out_bytes.is_empty() {
                        let out = strip_ansi(&String::from_utf8_lossy(out_bytes).to_string());
                        if !out.trim().is_empty() {
                            last_output_at = Instant::now();
                            for line in out.replace("\r\n", "\n").replace('\r', "\n").split('\n') {
                                let line = line.trim_end();
                                if !line.trim().is_empty() {
                                    self.emit_event(DepotDownloaderEvent::Output(line.to_string()));
                                }
                            }
                        }

                        let l = out.to_lowercase();
                        if l.contains("connecting to steam3")
                            || l.contains("logging")
                            || l.contains("steam3")
                        {
                            saw_login_progress = true;
                        }

                        // Mobile confirmation
                        if (l.contains("confirm") && l.contains("sign in"))
                            || l.contains("steam mobile app")
                        {
                            if !saw_mobile_confirm {
                                saw_mobile_confirm = true;
                                self.emit_event(DepotDownloaderEvent::NeedsMobileConfirmation {
                                    session_id,
                                });
                            }
                        }

                        // 2FA / auth detection (more aggressive)
                        if looks_like_twofactor_needed(&l)
                            || (start.elapsed() < Duration::from_secs(45)
                                && (l.contains("auth") || l.contains("authentication")))
                        {
                            if !requested_2fa {
                                requested_2fa = true;
                                self.emit_event(DepotDownloaderEvent::NeedsTwoFactor {
                                    session_id,
                                    message: Some(
                                        "Steam Guard code required. Enter code then submit."
                                            .to_string(),
                                    ),
                                });
                            }
                        }
                    }
                }
                Err(e) => {
                    // EOF means process exited.
                    if matches!(e, expectrl::Error::Eof) {
                        break;
                    }
                }
            }

            // If output stalls around login, assume it's waiting for Steam Guard.
            let idle_for = last_output_at.elapsed();
            let threshold = if saw_login_progress {
                Duration::from_secs(6)
            } else {
                Duration::from_secs(10)
            };
            if !requested_2fa && idle_for >= threshold && start.elapsed() > Duration::from_secs(5) {
                requested_2fa = true;
                self.emit_event(DepotDownloaderEvent::NeedsTwoFactor {
                    session_id,
                    message: Some("Steam Guard code required. Enter code then submit.".to_string()),
                });
            }

            // Hard timeout
            if start.elapsed() > Duration::from_secs(180) {
                let _ = p.send(ControlCode::EndOfText);
                return Err("Login timed out.".to_string());
            }

            // If the underlying process exited, finish (EOF isn't always reliable on ConPTY).
            #[cfg(windows)]
            {
                if !p.get_process_mut().is_alive() {
                    break;
                }
            }

            // Small sleep to avoid busy loop (no async await here; Session is not Send).
            std::thread::sleep(Duration::from_millis(120));
        }

        // On Windows, expectrl uses ConPTY (conpty::Process) under the hood.
        // We can obtain an exit code from the underlying process handle.
        #[cfg(windows)]
        {
            let exit_code = p
                .get_process_mut()
                .wait(None)
                .map_err(|_| "Failed to wait for DepotDownloader".to_string())?;

            if exit_code != 0 {
                return Err(format!("Login failed (exit code: {exit_code})."));
            }
        }

        #[cfg(unix)]
        {
            // On unix we'd normally use WaitStatus, but this app currently targets Windows first.
            // If needed, we can add unix exit-status handling later.
        }

        let state = LoginState {
            is_logged_in: true,
            username: Some(credentials.username),
        };
        self.save_login_state(&state)?;
        log::info!(
            "Saved login state: {}",
            self.login_state_path().to_string_lossy()
        );
        self.emit_event(DepotDownloaderEvent::LoginSuccess);
        Ok(())
    }

    /// Depot 다운로드
    pub async fn download_depot(
        &self,
        manifest_id: Option<String>,
        output_dir: PathBuf,
        task: Option<DownloadTaskContext>,
    ) -> Result<(), String> {
        let login_state = self.get_login_state();
        if !login_state.is_logged_in {
            return Err("Not logged in. Please login first.".to_string());
        }
        let username = login_state.username.clone().ok_or_else(|| {
            "Missing username for remembered login. Please login again.".to_string()
        })?;

        std::fs::create_dir_all(&output_dir).map_err(|e| e.to_string())?;

        let mut args = vec![
            // Enable IPC mode if patched.
            "-app".to_string(),
            Self::APP_ID.to_string(),
            "-depot".to_string(),
            Self::DEPOT_ID.to_string(),
            "-dir".to_string(),
            output_dir.to_string_lossy().to_string(),
            "-username".to_string(),
            username,
            "-remember-password".to_string(),
        ];
        if self.ipc_mode {
            args.insert(0, "-ipc".to_string());
        }

        if let Some(manifest) = manifest_id {
            args.push("-manifest".to_string());
            args.push(manifest);
        }

        let mut child = Command::new(&self.executable_path)
            .args(&args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .current_dir(&self.config_dir)
            .spawn()
            .map_err(|e| format!("Failed to spawn DepotDownloader: {e}"))?;

        let stdout = child.stdout.take().ok_or("Failed to capture stdout")?;
        let stderr = child.stderr.take().ok_or("Failed to capture stderr")?;

        let (tx, mut rx) = mpsc::unbounded_channel::<(bool, String)>(); // (is_stderr, line)
        {
            let tx = tx.clone();
            tokio::spawn(async move {
                let mut r = BufReader::new(stdout).lines();
                while let Ok(Some(line)) = r.next_line().await {
                    let _ = tx.send((false, line));
                }
            });
        }
        {
            let tx = tx.clone();
            tokio::spawn(async move {
                let mut r = BufReader::new(stderr).lines();
                while let Ok(Some(line)) = r.next_line().await {
                    let _ = tx.send((true, line));
                }
            });
        }

        let mut last_task_progress_bp: Option<u64> = None;
        // If we have seen any progress >= 0.01% (basis point >= 1),
        // do NOT treat "no output for 15s" as an auth prompt.
        let mut last_progress_bp: u64 = 0;
        let mut last_output_at = Instant::now();
        let mut idle_ticks = tokio::time::interval(Duration::from_millis(500));
        let status = loop {
            tokio::select! {
                s = child.wait() => break s.map_err(|e| e.to_string())?,
                _ = idle_ticks.tick() => {
                    if last_output_at.elapsed() > Duration::from_secs(15) {
                        // After progress has started, DepotDownloader may go quiet for a while
                        // (large files, disk I/O). Only fail if it stays silent for a long time.
                        if last_progress_bp >= 1 {
                            if last_output_at.elapsed() > Duration::from_secs(300) {
                                let _ = child.kill().await;
                                let err = "Download stalled (no output for 5 minutes). Please retry.".to_string();
                                self.emit_event(DepotDownloaderEvent::Error(err.clone()));
                                return Err(err);
                            }
                        } else {
                            let _ = child.kill().await;
                            let err = "Steam Guard / login required. Please login and try again.".to_string();
                            self.emit_event(DepotDownloaderEvent::Error(err.clone()));
                            return Err(err);
                        }
                    }
                }
                msg = rx.recv() => {
                    let Some((is_stderr, line)) = msg else { continue; };
                    last_output_at = Instant::now();
                    let l = line.to_lowercase();
                    let auth_prompt =
                        l.contains("steam guard")
                        || l.contains("two-factor")
                        || l.contains("two factor")
                        || l.contains("2fa")
                        || (l.contains("enter") && l.contains("code"))
                        || (l.contains("enter") && l.contains("password"))
                        || l.contains("authentication code")
                        || l.contains("emailed")
                        || l.contains("use the steam mobile app to confirm");
                    if auth_prompt {
                        // Downloads are non-interactive. If Steam auth is required here,
                        // instruct the UI to run an interactive login first.
                        let _ = child.kill().await;
                        let err = "Steam Guard / login required. Please login and try again.".to_string();
                        self.emit_event(DepotDownloaderEvent::Error(err.clone()));
                        return Err(err);
                    }
                    if is_stderr {
                        let line = strip_ansi(&line);
                        log::error!("DepotDownloader error: {}", line);
                        self.emit_event(DepotDownloaderEvent::Output(format!("ERROR: {}", line)));
                    } else {
                        let line = strip_ansi(&line);
                        log::info!("DepotDownloader: {}", line);
                        if let Some(progress) = self.parse_progress(&line) {
                            // Track last seen progress so we can distinguish auth prompts from stalls.
                            last_progress_bp = progress.0;
                            self.emit_event(DepotDownloaderEvent::Progress {
                                current: progress.0,
                                total: progress.1,
                            });

                            // Bridge DepotDownloader progress into the frontend-wide task progress
                            // so install UI doesn't stay stuck at the step's initial percent.
                            if let Some(task) = task.as_ref() {
                                if last_task_progress_bp != Some(progress.0) && progress.1 > 0 {
                                    last_task_progress_bp = Some(progress.0);
                                    let step_progress = (progress.0 as f64) / (progress.1 as f64);

                                    // Use remaining text after the % token as a small detail (file path).
                                    let s = line.trim_start();
                                    let pct_part = s.split_whitespace().next().unwrap_or("");
                                    let detail = s
                                        .get(pct_part.len()..)
                                        .unwrap_or("")
                                        .trim_start()
                                        .to_string();

                                    progress::emit_progress(
                                        &self.app,
                                        TaskProgressPayload {
                                            version: task.version,
                                            steps_total: task.steps_total,
                                            step: task.step,
                                            step_name: task.step_name.clone(),
                                            step_progress,
                                            overall_percent: overall_from_step(
                                                task.step,
                                                step_progress,
                                                task.steps_total,
                                            ),
                                            detail: if detail.is_empty() { None } else { Some(detail) },
                                            downloaded_bytes: None,
                                            total_bytes: None,
                                            extracted_files: None,
                                            total_files: None,
                                        },
                                    );
                                }
                            }
                        }
                        self.emit_event(DepotDownloaderEvent::Output(line));
                    }
                }
            }
        };

        if status.success() {
            log::info!("Download completed successfully");
            self.emit_event(DepotDownloaderEvent::DownloadComplete);
            Ok(())
        } else {
            let err = "Steam Guard / login required. Please login and try again.".to_string();
            self.emit_event(DepotDownloaderEvent::Error(err.clone()));
            Err(err)
        }
    }

    /// 특정 파일만 다운로드
    pub async fn download_files(
        &self,
        file_list: Vec<String>,
        output_dir: PathBuf,
    ) -> Result<(), String> {
        let login_state = self.get_login_state();
        if !login_state.is_logged_in {
            return Err("Not logged in. Please login first.".to_string());
        }
        let username = login_state.username.clone().ok_or_else(|| {
            "Missing username for remembered login. Please login again.".to_string()
        })?;

        std::fs::create_dir_all(&output_dir).map_err(|e| e.to_string())?;

        // 파일 목록을 임시 파일로 저장
        let filelist_path = self.config_dir.join("filelist.txt");
        std::fs::write(&filelist_path, file_list.join("\n")).map_err(|e| e.to_string())?;

        let args = vec![
            // Enable IPC mode if patched.
            "-app".to_string(),
            Self::APP_ID.to_string(),
            "-depot".to_string(),
            Self::DEPOT_ID.to_string(),
            "-dir".to_string(),
            output_dir.to_string_lossy().to_string(),
            "-filelist".to_string(),
            filelist_path.to_string_lossy().to_string(),
            "-username".to_string(),
            username,
            "-remember-password".to_string(),
        ];
        let mut args = args;
        if self.ipc_mode {
            args.insert(0, "-ipc".to_string());
        }

        log::info!("Downloading {} files from depot", file_list.len());

        let mut child = Command::new(&self.executable_path)
            .args(&args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .current_dir(&self.config_dir)
            .spawn()
            .map_err(|e| format!("Failed to spawn DepotDownloader: {e}"))?;

        let stdout = child.stdout.take().ok_or("Failed to capture stdout")?;
        let stderr = child.stderr.take().ok_or("Failed to capture stderr")?;

        let (tx, mut rx) = mpsc::unbounded_channel::<(bool, String)>(); // (is_stderr, line)
        {
            let tx = tx.clone();
            tokio::spawn(async move {
                let mut r = BufReader::new(stdout).lines();
                while let Ok(Some(line)) = r.next_line().await {
                    let _ = tx.send((false, line));
                }
            });
        }
        {
            let tx = tx.clone();
            tokio::spawn(async move {
                let mut r = BufReader::new(stderr).lines();
                while let Ok(Some(line)) = r.next_line().await {
                    let _ = tx.send((true, line));
                }
            });
        }

        // Same logic as download(): once we've seen progress, don't treat short silence as auth.
        let mut last_progress_bp: u64 = 0;
        let mut last_output_at = Instant::now();
        let mut idle_ticks = tokio::time::interval(Duration::from_millis(500));
        let status = loop {
            tokio::select! {
                s = child.wait() => break s.map_err(|e| e.to_string())?,
                _ = idle_ticks.tick() => {
                    if last_output_at.elapsed() > Duration::from_secs(15) {
                        if last_progress_bp >= 1 {
                            if last_output_at.elapsed() > Duration::from_secs(300) {
                                let _ = child.kill().await;
                                // 임시 파일 정리
                                let _ = std::fs::remove_file(&filelist_path);
                                return Err("Download stalled (no output for 5 minutes). Please retry.".to_string());
                            }
                        } else {
                            let _ = child.kill().await;
                            // 임시 파일 정리
                            let _ = std::fs::remove_file(&filelist_path);
                            return Err("Steam Guard / login required. Please login and try again.".to_string());
                        }
                    }
                }
                msg = rx.recv() => {
                    let Some((is_stderr, line)) = msg else { continue; };
                    last_output_at = Instant::now();
                    let l = line.to_lowercase();
                    let auth_prompt =
                        l.contains("steam guard")
                        || l.contains("two-factor")
                        || l.contains("two factor")
                        || l.contains("2fa")
                        || (l.contains("enter") && l.contains("code"))
                        || (l.contains("enter") && l.contains("password"))
                        || l.contains("authentication code")
                        || l.contains("emailed")
                        || l.contains("use the steam mobile app to confirm");
                    if auth_prompt {
                        let _ = child.kill().await;
                        // 임시 파일 정리
                        let _ = std::fs::remove_file(&filelist_path);
                        return Err("Steam Guard / login required. Please login and try again.".to_string());
                    }
                    if is_stderr {
                        let line = strip_ansi(&line);
                        log::error!("DepotDownloader error: {}", line);
                        self.emit_event(DepotDownloaderEvent::Output(format!("ERROR: {}", line)));
                    } else {
                        let line = strip_ansi(&line);
                        log::info!("DepotDownloader: {}", line);
                        if let Some(progress) = self.parse_progress(&line) {
                            last_progress_bp = progress.0;
                            self.emit_event(DepotDownloaderEvent::Progress {
                                current: progress.0,
                                total: progress.1,
                            });
                        }
                        self.emit_event(DepotDownloaderEvent::Output(line));
                    }
                }
            }
        };

        // 임시 파일 정리
        let _ = std::fs::remove_file(&filelist_path);

        if status.success() {
            log::info!("File download completed");
            self.emit_event(DepotDownloaderEvent::DownloadComplete);
            Ok(())
        } else {
            Err("Steam Guard / login required. Please login and try again.".to_string())
        }
    }

    /// 로그아웃
    pub fn logout(&self) -> Result<(), String> {
        let state = LoginState {
            is_logged_in: false,
            username: None,
        };
        self.save_login_state(&state)?;

        // 저장된 인증 정보 삭제
        let config_files = ["config.vdf", ".DepotDownloader"];
        for filename in &config_files {
            let path = self.config_dir.join(filename);
            if path.exists() {
                let _ = std::fs::remove_file(path);
            }
        }

        // ssfn* 패턴 파일들 삭제
        if let Ok(entries) = std::fs::read_dir(&self.config_dir) {
            for entry in entries.flatten() {
                if let Ok(name) = entry.file_name().into_string() {
                    if name.starts_with("ssfn") {
                        let _ = std::fs::remove_file(entry.path());
                    }
                }
            }
        }

        log::info!("Logged out successfully");
        Ok(())
    }

    /// 진행률 파싱 (DepotDownloader 출력 형식에 맞게 조정 필요)
    fn parse_progress(&self, line: &str) -> Option<(u64, u64)> {
        // DepotDownloader prints progress lines like:
        // " 28.91% C:\path\to\file"
        // Use basis points (0.01%) to preserve decimals:
        // current=2891, total=10000 → UI can compute percent = current/total*100
        let s = line.trim_start();
        if !s.contains('%') {
            return None;
        }
        let pct_part = s.split_whitespace().next()?;
        let pct_str = pct_part.strip_suffix('%')?;
        let pct: f64 = pct_str.parse().ok()?;
        if !pct.is_finite() {
            return None;
        }
        let clamped = pct.clamp(0.0, 100.0);
        let basis_points = (clamped * 100.0).round() as u64;
        Some((basis_points, 10_000))
    }

    /// 이벤트 발생
    fn emit_event(&self, event: DepotDownloaderEvent) {
        // Also mirror to backend logs to help debugging when UI misses events.
        match &event {
            DepotDownloaderEvent::Output(s) => {
                let preview = if s.len() > 500 {
                    format!("{}…", &s[..500])
                } else {
                    s.clone()
                };
                log::info!("DepotDownloader: {}", preview.replace('\n', "\\n"));
            }
            DepotDownloaderEvent::Progress { current, total } => {
                log::info!("DepotDownloader progress: {current}/{total}");
            }
            DepotDownloaderEvent::Error(e) => log::error!("DepotDownloader error: {e}"),
            DepotDownloaderEvent::LoginFailed(e) => {
                log::error!("DepotDownloader login failed: {e}")
            }
            _ => {}
        }
        let _ = self.app.emit("depot-downloader", event);
    }
}

fn depot_config_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let app_data = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("failed to resolve app data dir: {e}"))?;
    let config_dir = app_data.join("depot_config");
    std::fs::create_dir_all(&config_dir).map_err(|e| e.to_string())?;
    Ok(config_dir)
}

fn depot_login_state_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    Ok(depot_config_dir(app)?.join("login_state.json"))
}

fn read_saved_login_state(app: &tauri::AppHandle) -> Result<LoginState, String> {
    let path = depot_login_state_path(app)?;
    if let Ok(content) = std::fs::read_to_string(path) {
        if let Ok(state) = serde_json::from_str::<LoginState>(&content) {
            return Ok(state);
        }
    }
    Ok(LoginState {
        is_logged_in: false,
        username: None,
    })
}

fn write_saved_login_state(app: &tauri::AppHandle, state: &LoginState) -> Result<(), String> {
    let path = depot_login_state_path(app)?;
    let content = serde_json::to_string(state).map_err(|e| e.to_string())?;
    std::fs::write(path, content).map_err(|e| e.to_string())?;
    Ok(())
}

pub async fn install_downloader(app: &tauri::AppHandle) -> Result<bool, String> {
    let download_url = format!("https://github.com/SteamRE/DepotDownloader/releases/download/DepotDownloader_3.4.0/{DEPOT_DOWNLOADER_NAME}.zip");

    let install_path = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("failed to resolve app data dir: {e}"))?
        .join("downloader");
    let marker_path = install_path.join(DepotDownloader::PATCH_MARKER);

    // If patched build already installed, skip.
    if install_path.exists() && marker_path.exists() {
        info!(
            "Patched DepotDownloader already installed at {}",
            install_path.display()
        );
        return Ok(true);
    }

    // Dev convenience: if DepotDownloader source exists next to repo, build patched binary and install it.
    // This makes Steam Guard prompts newline-terminated and machine-friendly via `-ipc`.
    if let Ok(exe) = std::env::current_exe() {
        let repo_root = exe
            .parent() // .../target/debug
            .and_then(|p| p.parent()) // .../target
            .and_then(|p| p.parent()) // .../src-tauri
            .and_then(|p| p.parent()) // .../repo root
            .map(|p| p.to_path_buf());

        if let Some(root) = repo_root {
            let src = root
                .join(".depotdownloader")
                .join("DepotDownloader")
                .join("DepotDownloader.csproj");
            if src.exists() {
                info!("Building patched DepotDownloader from {}", src.display());
                std::fs::create_dir_all(&install_path).map_err(|e| e.to_string())?;

                let out_dir = install_path.clone();
                let src_s = src.to_string_lossy().to_string();
                let out_s = out_dir.to_string_lossy().to_string();

                // Build in blocking thread.
                tauri::async_runtime::spawn_blocking(move || -> Result<(), String> {
                    let out = std::process::Command::new("dotnet")
                        .args([
                            "publish",
                            &src_s,
                            "-c",
                            "Release",
                            "-r",
                            "win-x64",
                            "--self-contained",
                            "true",
                            "-p:PublishSingleFile=true",
                            "-p:PublishTrimmed=false",
                            "-o",
                            &out_s,
                        ])
                        .output()
                        .map_err(|e| e.to_string())?;

                    if !out.status.success() {
                        let stdout = String::from_utf8_lossy(&out.stdout);
                        let stderr = String::from_utf8_lossy(&out.stderr);
                        return Err(format!("dotnet publish failed: {stdout}{stderr}"));
                    }
                    Ok(())
                })
                .await
                .map_err(|e| e.to_string())??;

                std::fs::write(&marker_path, b"ipc").map_err(|e| e.to_string())?;
                info!(
                    "Patched DepotDownloader installed at {}",
                    install_path.display()
                );
                return Ok(true);
            }
        }
    }

    info!(
        "Downloading DepotDownloader from {download_url} to {}",
        install_path.display()
    );

    std::fs::create_dir_all(&install_path).map_err(|e| e.to_string())?;

    let client = reqwest::Client::new();
    let response = client
        .get(&download_url)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?;

    // ZIP 파일 다운로드
    let bytes = response.bytes().await.map_err(|e| e.to_string())?;
    let zip_path = install_path.join("downloader.zip");
    std::fs::write(&zip_path, &bytes).map_err(|e| e.to_string())?;

    info!("Extracting DepotDownloader to {}", install_path.display());

    // ZIP 압축 해제 (blocking IO)
    let zip_path_clone = zip_path.clone();
    let install_path_clone = install_path.clone();

    tauri::async_runtime::spawn_blocking(move || -> Result<(), String> {
        let file = std::fs::File::open(&zip_path_clone).map_err(|e| e.to_string())?;
        let mut archive = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;

        for i in 0..archive.len() {
            let mut file = archive.by_index(i).map_err(|e| e.to_string())?;
            let outpath = match file.enclosed_name() {
                Some(path) => install_path_clone.join(path),
                None => continue,
            };

            if file.name().ends_with('/') {
                std::fs::create_dir_all(&outpath).map_err(|e| e.to_string())?;
            } else {
                if let Some(p) = outpath.parent() {
                    if !p.exists() {
                        std::fs::create_dir_all(p).map_err(|e| e.to_string())?;
                    }
                }
                let mut outfile = std::fs::File::create(&outpath).map_err(|e| e.to_string())?;
                std::io::copy(&mut file, &mut outfile).map_err(|e| e.to_string())?;
            }

            // Unix 실행 권한 설정
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if let Some(mode) = file.unix_mode() {
                    std::fs::set_permissions(&outpath, std::fs::Permissions::from_mode(mode))
                        .map_err(|e| e.to_string())?;
                }
            }
        }

        // ZIP 파일 삭제
        std::fs::remove_file(&zip_path_clone).map_err(|e| e.to_string())?;

        Ok(())
    })
    .await
    .map_err(|e| e.to_string())??;

    info!("DepotDownloader installed successfully");

    Ok(true)
}

// Tauri 커맨드들
#[tauri::command]
pub async fn depot_login(
    app: tauri::AppHandle,
    login_state: tauri::State<'_, DepotLoginState>,
    username: String,
    password: String,
    two_factor_code: Option<String>,
) -> Result<(), String> {
    let downloader = DepotDownloader::new(&app)?;

    // NOTE: Never log passwords or 2FA codes.
    let session_id = login_state.next_id.fetch_add(1, Ordering::Relaxed) + 1;
    let (tx, mut rx) = mpsc::unbounded_channel::<String>();
    {
        let mut map = login_state
            .sessions
            .lock()
            .map_err(|_| "login state lock poisoned".to_string())?;
        map.insert(session_id, tx);
    }

    // UX decision: after starting a login attempt, always ask user to check Steam Guard (email/app)
    // and allow submitting a code into this *same running* process via `depot_login_submit_code`.
    // This avoids relying on prompt/log detection which can be unreliable across DD versions.
    downloader.emit_event(DepotDownloaderEvent::NeedsTwoFactor {
        session_id,
        message: Some("Steam Guard (email/app) 코드를 확인한 뒤 입력해주세요.".to_string()),
    });
    downloader.emit_event(DepotDownloaderEvent::Output(
        "로그인 시도 시작됨. Steam Guard 코드가 오면 입력 후 Submit code를 눌러주세요.".to_string(),
    ));

    let res = downloader
        .login_interactive(
            session_id,
            LoginCredentials { username, password },
            two_factor_code,
            &mut rx,
        )
        .await;

    // Cleanup session sender.
    {
        let mut map = login_state
            .sessions
            .lock()
            .map_err(|_| "login state lock poisoned".to_string())?;
        map.remove(&session_id);
    }

    res
}

/// Start an interactive login session and return session_id immediately.
/// The running process will emit `LoginSuccess`/`Error` events, and accept codes via `depot_login_submit_code`.
#[tauri::command]
pub async fn depot_login_start(
    app: tauri::AppHandle,
    login_state: tauri::State<'_, DepotLoginState>,
    username: String,
    password: String,
) -> Result<u64, String> {
    let downloader = DepotDownloader::new(&app)?;

    // NOTE: Never log passwords or 2FA codes.
    let session_id = login_state.next_id.fetch_add(1, Ordering::Relaxed) + 1;
    let (tx, mut rx) = mpsc::unbounded_channel::<String>();
    {
        let mut map = login_state
            .sessions
            .lock()
            .map_err(|_| "login state lock poisoned".to_string())?;
        map.insert(session_id, tx);
    }

    // Prompt UI immediately (no reliance on log detection).
    downloader.emit_event(DepotDownloaderEvent::NeedsTwoFactor {
        session_id,
        message: Some("Steam Guard (email/app) 코드를 확인한 뒤 입력해주세요.".to_string()),
    });
    downloader.emit_event(DepotDownloaderEvent::Output(
        "로그인 시도 시작됨. Steam Guard 코드가 오면 입력 후 Submit code를 눌러주세요.".to_string(),
    ));

    let app2 = app.clone();
    tauri::async_runtime::spawn(async move {
        let downloader = match DepotDownloader::new(&app2) {
            Ok(d) => d,
            Err(e) => {
                let _ = app2.emit("depot-downloader", DepotDownloaderEvent::Error(e));
                return;
            }
        };

        let res = downloader
            .login_interactive(
                session_id,
                LoginCredentials { username, password },
                None,
                &mut rx,
            )
            .await;

        // Cleanup session sender.
        // IMPORTANT: don't capture `tauri::State<'_ , _>` into the spawned task (not 'static).
        // Re-acquire state from the AppHandle instead.
        {
            let state = app2.state::<DepotLoginState>();
            if let Ok(mut map) = state.sessions.lock() {
                map.remove(&session_id);
            };
        }

        if let Err(err) = res {
            downloader.emit_event(DepotDownloaderEvent::Error(err));
        }
    });

    Ok(session_id)
}

#[tauri::command]
pub fn depot_login_submit_code(
    login_state: tauri::State<'_, DepotLoginState>,
    session_id: u64,
    code: String,
) -> Result<bool, String> {
    let code = code.trim().to_string();
    if code.is_empty() {
        return Err("empty code".to_string());
    }
    let map = login_state
        .sessions
        .lock()
        .map_err(|_| "login state lock poisoned".to_string())?;
    let tx = map
        .get(&session_id)
        .ok_or_else(|| "login session not found (expired?)".to_string())?;
    // Do not log the code itself; only acknowledge receipt.
    log::info!(
        "Steam Guard code received for session_id={session_id} (len={})",
        code.len()
    );
    tx.send(code)
        .map_err(|_| "failed to send code to login session".to_string())?;
    Ok(true)
}
#[tauri::command]
pub async fn depot_download(
    app: tauri::AppHandle,
    manifest_id: Option<String>,
    output_dir: String,
) -> Result<(), String> {
    let downloader = DepotDownloader::new(&app)?;
    downloader
        .download_depot(manifest_id, PathBuf::from(output_dir), None)
        .await
}

#[tauri::command]
pub fn depot_get_login_state(app: tauri::AppHandle) -> Result<LoginState, String> {
    // Allow reading login state even if DepotDownloader isn't installed yet.
    read_saved_login_state(&app)
}

#[tauri::command]
pub fn depot_logout(app: tauri::AppHandle) -> Result<(), String> {
    // Allow logout even if DepotDownloader isn't installed yet (state-only cleanup).
    write_saved_login_state(
        &app,
        &LoginState {
            is_logged_in: false,
            username: None,
        },
    )?;

    // Best-effort cleanup of remembered files in config dir.
    let config_dir = depot_config_dir(&app)?;
    let config_files = ["config.vdf", ".DepotDownloader"];
    for filename in &config_files {
        let path = config_dir.join(filename);
        if path.exists() {
            let _ = std::fs::remove_file(path);
        }
    }

    // ssfn* pattern files
    if let Ok(entries) = std::fs::read_dir(&config_dir) {
        for entry in entries.flatten() {
            if let Ok(name) = entry.file_name().into_string() {
                if name.starts_with("ssfn") {
                    let _ = std::fs::remove_file(entry.path());
                }
            }
        }
    }

    Ok(())
}

#[tauri::command]
pub async fn depot_download_files(
    app: tauri::AppHandle,
    files: Vec<String>,
    output_dir: String,
) -> Result<(), String> {
    let downloader = DepotDownloader::new(&app)?;
    downloader
        .download_files(files, PathBuf::from(output_dir))
        .await
}

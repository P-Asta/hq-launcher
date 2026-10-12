//! Windows game execution on macOS. Keep Steam and the game in the same Wine prefix.
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;
#[cfg(target_os = "macos")]
use std::sync::atomic::{AtomicBool, Ordering};

use serde::{Deserialize, Serialize};
use tauri::Manager;

const STEAM_SETUP_URL: &str = "https://cdn.akamai.steamstatic.com/client/installer/SteamSetup.exe";
#[cfg(target_os = "macos")]
static STEAM_SETUP_BUSY: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Settings {
    #[serde(default)]
    graphics_backend: Option<String>,
    runtime_path: Option<PathBuf>,
    prefix_path: Option<PathBuf>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeChoice {
    path: String,
    label: String,
    kind: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PrefixChoice {
    path: String,
    label: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeStatus {
    graphics_backend: String,
    metal_supported: bool,
    runtime_path: Option<String>,
    runtime_installed: bool,
    prefix_path: Option<String>,
    runtime_kind: Option<String>,
    detected_runtimes: Vec<RuntimeChoice>,
    detected_prefixes: Vec<PrefixChoice>,
    steam_installed: bool,
    ready: bool,
}

fn app_data(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    app.path().app_data_dir().map_err(|e| e.to_string())
}

fn settings_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    Ok(app_data(app)?.join("config/macos_runtime.json"))
}

fn read_settings(app: &tauri::AppHandle) -> Result<Settings, String> {
    let path = settings_path(app)?;
    if !path.exists() {
        return Ok(Settings::default());
    }
    serde_json::from_slice(&std::fs::read(path).map_err(|e| e.to_string())?)
        .map_err(|e| format!("Failed to read macOS runtime settings: {e}"))
}

fn runtime_kind(path: &Path) -> &'static str {
    if path.to_string_lossy().contains("/CrossOver/") {
        "crossover"
    } else {
        "wine"
    }
}

fn resolve_runtime_path(path: &Path) -> PathBuf {
    if path.is_dir() {
        for relative in [
            "Contents/SharedSupport/CrossOver/bin/wine",
            "Contents/Resources/wine/bin/wine",
            "bin/wine64",
            "bin/wine",
        ] {
            let candidate = path.join(relative);
            if candidate.is_file() {
                return candidate;
            }
        }
    }
    path.to_path_buf()
}

fn validate_runtime(path: &Path) -> Result<(), String> {
    if !path.is_absolute() || !path.is_file() {
        return Err("Select an installed Wine executable or CrossOver/Wine app.".into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if std::fs::metadata(path)
            .map_err(|e| e.to_string())?
            .permissions()
            .mode()
            & 0o111
            == 0
        {
            return Err("The selected Wine runtime is not executable.".into());
        }
    }
    Ok(())
}

fn steam_executable(prefix: &Path) -> Option<PathBuf> {
    [
        "drive_c/Program Files (x86)/Steam/steam.exe",
        "drive_c/Program Files/Steam/steam.exe",
    ]
    .into_iter()
    .map(|relative| prefix.join(relative))
    .find(|path| path.is_file())
}

fn resolved_paths(app: &tauri::AppHandle) -> Result<(Option<PathBuf>, PathBuf), String> {
    let settings = read_settings(app)?;
    #[cfg(target_os = "macos")]
    let managed = crate::macos_wine_install::installed_wine(app)?;
    #[cfg(not(target_os = "macos"))]
    let managed = None;
    let runtime = settings.runtime_path.or(managed);
    let prefix = settings
        .prefix_path
        .unwrap_or(app_data(app)?.join("macos_env/wine_prefix"));
    Ok((runtime, prefix))
}

fn status(app: &tauri::AppHandle) -> Result<RuntimeStatus, String> {
    let (runtime, prefix) = resolved_paths(app)?;
    let mut detected_runtimes = Vec::new();
    let mut candidates = Vec::new();
    if let Some(path) = runtime.as_ref() {
        candidates.push((path.clone(), "Current runtime".to_string()));
    }
    for root in [
        Some(PathBuf::from("/Applications")),
        dirs::home_dir().map(|home| home.join("Applications")),
    ]
    .into_iter()
    .flatten()
    {
        for name in [
            "CrossOver.app",
            "Wine Stable.app",
            "Wine Staging.app",
            "Wine Devel.app",
        ] {
            candidates.push((
                resolve_runtime_path(&root.join(name)),
                name.trim_end_matches(".app").to_string(),
            ));
        }
    }
    for (path, label) in candidates {
        if validate_runtime(&path).is_ok()
            && !detected_runtimes
                .iter()
                .any(|item: &RuntimeChoice| item.path == path.to_string_lossy())
        {
            detected_runtimes.push(RuntimeChoice {
                path: path.to_string_lossy().into_owned(),
                label,
                kind: runtime_kind(&path).into(),
            });
        }
    }
    let mut detected_prefixes = vec![PrefixChoice {
        path: prefix.to_string_lossy().into_owned(),
        label: "HQ Launcher prefix".into(),
    }];
    if let Some(home) = dirs::home_dir() {
        let default_prefix = home.join(".wine");
        if default_prefix.join("system.reg").is_file() && default_prefix != prefix {
            detected_prefixes.push(PrefixChoice {
                path: default_prefix.to_string_lossy().into_owned(),
                label: "Default Wine prefix".into(),
            });
        }
        for relative in [
            "Library/Application Support/CrossOver/Bottles",
            "Library/Containers/com.isaacmarovitz.Whisky/Bottles",
        ] {
            if let Ok(entries) = std::fs::read_dir(home.join(relative)) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.join("system.reg").is_file() && path != prefix {
                        detected_prefixes.push(PrefixChoice {
                            path: path.to_string_lossy().into_owned(),
                            label: entry.file_name().to_string_lossy().into_owned(),
                        });
                    }
                }
            }
        }
    }
    let steam_installed = steam_executable(&prefix).is_some();
    let runtime_valid = runtime
        .as_ref()
        .is_some_and(|path| validate_runtime(path).is_ok());
    Ok(RuntimeStatus {
        graphics_backend: read_settings(app)?
            .graphics_backend
            .unwrap_or_else(|| "auto".into()),
        metal_supported: {
            #[cfg(target_os = "macos")]
            {
                crate::macos_wine_install::metal_supported()
            }
            #[cfg(not(target_os = "macos"))]
            {
                false
            }
        },
        runtime_installed: runtime_valid,
        runtime_kind: runtime.as_ref().map(|path| runtime_kind(path).into()),
        runtime_path: runtime.map(|path| path.to_string_lossy().into_owned()),
        prefix_path: Some(prefix.to_string_lossy().into_owned()),
        detected_runtimes,
        detected_prefixes,
        steam_installed,
        ready: runtime_valid && steam_installed,
    })
}

#[tauri::command]
pub fn get_macos_runtime_status(app: tauri::AppHandle) -> Result<Option<RuntimeStatus>, String> {
    if cfg!(target_os = "macos") {
        status(&app).map(Some)
    } else {
        Ok(None)
    }
}

#[tauri::command]
pub fn set_macos_runtime_settings(
    app: tauri::AppHandle,
    runtime_path: String,
    prefix_path: String,
    graphics_backend: Option<String>,
) -> Result<RuntimeStatus, String> {
    if graphics_backend
        .as_deref()
        .is_some_and(|value| !matches!(value, "auto" | "dxvk"))
    {
        return Err("Unknown graphics backend.".into());
    }
    let graphics_backend = graphics_backend.or(read_settings(&app)?.graphics_backend);
    let runtime = if runtime_path.trim().is_empty() {
        None
    } else {
        let path = resolve_runtime_path(Path::new(runtime_path.trim()));
        validate_runtime(&path)?;
        Some(path)
    };
    // The settings UI displays the resolved managed path. Saving a prefix-only
    // change must keep automatic runtime updates enabled.
    #[cfg(target_os = "macos")]
    let runtime = {
        let managed = crate::macos_wine_install::installed_wine(&app)?;
        runtime.filter(|path| Some(path) != managed.as_ref())
    };
    let prefix = if prefix_path.trim().is_empty() {
        None
    } else {
        let path = PathBuf::from(prefix_path.trim());
        if !path.is_absolute() || (path.exists() && !path.is_dir()) {
            return Err("The Wine prefix must be an absolute folder path.".into());
        }
        if runtime
            .as_ref()
            .is_some_and(|path| runtime_kind(path) == "crossover")
            && !path.join("cxbottle.conf").is_file()
        {
            return Err("Choose an existing CrossOver bottle containing Windows Steam.".into());
        }
        Some(path)
    };
    if runtime
        .as_ref()
        .is_some_and(|path| runtime_kind(path) == "crossover")
        && prefix.is_none()
    {
        return Err("Choose a CrossOver bottle.".into());
    }
    let path = settings_path(&app)?;
    std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
    std::fs::write(
        path,
        serde_json::to_vec_pretty(&Settings {
            runtime_path: runtime,
            prefix_path: prefix,
            graphics_backend,
        })
        .map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    status(&app)
}

#[tauri::command]
pub async fn choose_macos_runtime_path(kind: String) -> Result<Option<String>, String> {
    let dialog = rfd::AsyncFileDialog::new();
    let selected = match kind.as_str() {
        "runtime" => dialog.set_title("Choose Wine executable").pick_file().await,
        "prefix" => {
            dialog
                .set_title("Choose Wine prefix or CrossOver bottle")
                .pick_folder()
                .await
        }
        _ => return Err("Unknown runtime path kind".into()),
    };
    Ok(selected.map(|file| file.path().to_string_lossy().into_owned()))
}

#[tauri::command]
pub async fn install_macos_runtime(app: tauri::AppHandle) -> Result<RuntimeStatus, String> {
    #[cfg(target_os = "macos")]
    {
        crate::macos_wine_install::ensure_installed(&app).await?;
        status(&app)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = app;
        Err("Wine for macOS is only installed on macOS.".into())
    }
}

pub fn wine_command(runtime: &Path, prefix: &Path, executable: &Path) -> Command {
    let mut command = Command::new(runtime);
    if runtime_kind(runtime) == "crossover" {
        command.env("CX_BOTTLE_PATH", prefix.parent().unwrap_or(prefix));
        command
            .arg("--bottle")
            .arg(prefix.file_name().unwrap_or_default())
            .arg("--cx-app");
    }
    command.arg(executable);
    command.env("WINEPREFIX", prefix).env("WINEDEBUG", "-all");
    command
}

pub fn game_command(
    app: &tauri::AppHandle,
    executable: &Path,
    template: Option<&str>,
    load_bepinex: bool,
) -> Result<Command, String> {
    let (runtime, prefix) = resolved_paths(app)?;
    let runtime =
        runtime.ok_or("macOS runtime is not installed. Open Launch settings to install it.")?;
    validate_runtime(&runtime)?;
    #[cfg(target_os = "macos")]
    let runtime = if use_metal(app, &runtime)? {
        crate::macos_wine_install::installed_metal_wine(app)?
            .ok_or("Metal renderer is not prepared. Press Run again to install it.")?
    } else {
        runtime
    };
    let mut base = wine_command(&runtime, &prefix, executable);
    // Media codecs belong to the game, not Steam's installer/updater. Extra
    // dylib search paths can interfere with the updater's 32-bit dependencies.
    #[cfg(target_os = "macos")]
    crate::macos_wine_install::configure_command(&mut base, &runtime);
    #[cfg(target_os = "macos")]
    if crate::macos_wine_install::installed_metal_wine(app)?.as_deref() == Some(runtime.as_path()) {
        base.env("DXMT_SHADER_CACHE", "1");
        // Unity's default worker pool caused long stalls under Wine/Rosetta.
        // Four workers reduced those stalls in the same-scene M1 comparison.
        base.args(["-job-worker-count", "4"]);
    }
    let args: Vec<OsString> = base.get_args().map(|value| value.to_owned()).collect();
    let (program, args) = crate::build_wrapped_launch_command(template, base.get_program(), &args)?;
    let mut command = Command::new(program);
    command.args(args);
    for (key, value) in base.get_envs() {
        if let Some(value) = value {
            command.env(key, value);
        } else {
            command.env_remove(key);
        }
    }
    // BepInEx is a Windows winhttp proxy. The native Windows overlay is not supported here.
    command.env(
        "WINEDLLOVERRIDES",
        if load_bepinex {
            "winhttp=n,b;version=b"
        } else {
            "winhttp=b;version=b"
        },
    );
    command
        .env("SteamAppId", "1966720")
        .env("SteamGameId", "1966720");
    Ok(command)
}

#[cfg(target_os = "macos")]
pub async fn ensure_runtime(app: &tauri::AppHandle) -> Result<(PathBuf, PathBuf), String> {
    if read_settings(app)?.runtime_path.is_none() {
        crate::macos_wine_install::ensure_installed(app).await?;
    }
    let (runtime, prefix) = resolved_paths(app)?;
    let runtime = runtime.ok_or("Wine installation did not produce an executable.")?;
    validate_runtime(&runtime)?;
    std::fs::create_dir_all(&prefix).map_err(|e| e.to_string())?;
    Ok((runtime, prefix))
}

#[cfg(target_os = "macos")]
async fn open_steam(app: &tauri::AppHandle, runtime: &Path, prefix: &Path) -> Result<bool, String> {
    use tauri::Emitter;
    let result = open_steam_inner(app, runtime, prefix).await;
    if let Err(message) = &result {
        let _ = app.emit(
            "macos-runtime-progress",
            serde_json::json!({
                "stage": "error", "percent": null, "message": message,
            }),
        );
    }
    result
}

#[cfg(target_os = "macos")]
async fn open_steam_inner(
    app: &tauri::AppHandle,
    runtime: &Path,
    prefix: &Path,
) -> Result<bool, String> {
    use futures_util::StreamExt;
    use std::io::{Read, Write};
    if STEAM_SETUP_BUSY.swap(true, Ordering::AcqRel) {
        return Err("Windows Steam setup is already in progress.".into());
    }
    struct SetupGuard;
    impl Drop for SetupGuard {
        fn drop(&mut self) {
            STEAM_SETUP_BUSY.store(false, Ordering::Release);
        }
    }
    let guard = SetupGuard;
    let existing = steam_executable(prefix);
    let installing = existing.is_none();
    let executable = if let Some(path) = existing {
        path
    } else {
        let download = app_data(app)?.join("macos_env/SteamSetup.exe");
        std::fs::create_dir_all(download.parent().unwrap()).map_err(|e| e.to_string())?;
        let partial = download.with_extension("exe.part");
        let response = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(180))
            .build()
            .map_err(|e| e.to_string())?
            .get(STEAM_SETUP_URL)
            .send()
            .await
            .map_err(|e| format!("Failed to download Windows Steam: {e}"))?
            .error_for_status()
            .map_err(|e| e.to_string())?;
        let mut file = std::fs::File::create(&partial).map_err(|e| e.to_string())?;
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            file.write_all(&chunk.map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
        }
        drop(file);
        let mut header = [0; 2];
        std::fs::File::open(&partial)
            .map_err(|e| e.to_string())?
            .read_exact(&mut header)
            .map_err(|e| e.to_string())?;
        if header != *b"MZ" {
            return Err(
                "Downloaded Steam installer is not a Windows executable. Please retry.".into(),
            );
        }
        std::fs::rename(partial, &download).map_err(|e| e.to_string())?;
        download
    };
    let log_path = app_data(app)?.join("logs/macos-steam.log");
    if installing {
        use tauri::Emitter;
        let _ = app.emit(
            "macos-runtime-progress",
            serde_json::json!({
                "stage": "installing-steam", "percent": null,
                "message": "Installing Windows Steam automatically...",
            }),
        );
        let mut installer =
            spawn_steam_process(runtime, prefix, &executable, true, false, &log_path)?;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(180);
        loop {
            if let Some(exit) = installer.try_wait().map_err(|e| e.to_string())? {
                if let Some(message) = steam_exit_error(
                    exit.success(),
                    true,
                    steam_executable(prefix).is_some(),
                    &log_path,
                ) {
                    log::warn!("{message} (exit {exit})");
                    let _ = app.emit(
                        "macos-runtime-progress",
                        serde_json::json!({
                            "stage": "error", "percent": null, "message": message,
                        }),
                    );
                    return Err(message);
                }
                break;
            }
            if std::time::Instant::now() >= deadline {
                let _ = installer.kill();
                let _ = installer.wait();
                return Err(format!(
                    "Windows Steam installation timed out. Diagnostic log: {}",
                    log_path.display()
                ));
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
        let _ = app.emit(
            "macos-runtime-progress",
            serde_json::json!({
                "stage": "ready", "percent": 100,
            }),
        );
    }
    let steam =
        steam_executable(prefix).ok_or("Windows Steam installation did not create steam.exe.")?;
    let managed = crate::macos_wine_install::installed_wine(app)?.as_deref() == Some(runtime);
    if managed {
        crate::macos_steam::ensure_helpers(steam.parent().unwrap())?;
    }
    let mut child = spawn_steam_process(runtime, prefix, &steam, false, managed, &log_path)?;
    if managed {
        monitor_steam_helpers(prefix.to_path_buf(), steam.parent().unwrap().to_path_buf());
    }
    // Catch immediate startup failures before telling the user a window opened.
    for _ in 0..15 {
        if let Some(exit) = child.try_wait().map_err(|e| e.to_string())? {
            if let Some(message) = steam_exit_error(exit.success(), false, true, &log_path) {
                log::warn!("{message} (exit {exit})");
                return Err(message);
            }
            return Ok(installing);
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    drop(guard);
    let app = app.clone();
    std::thread::spawn(move || {
        use tauri::Emitter;
        if let Ok(exit) = child.wait() {
            if let Some(message) = steam_exit_error(exit.success(), false, true, &log_path) {
                log::warn!("{message} (exit {exit})");
                let _ = app.emit(
                    "macos-runtime-progress",
                    serde_json::json!({
                        "stage": "error", "percent": null, "message": message,
                    }),
                );
            }
        }
    });
    Ok(installing)
}

#[cfg(target_os = "macos")]
fn spawn_steam_process(
    runtime: &Path,
    prefix: &Path,
    executable: &Path,
    installing: bool,
    managed: bool,
    log_path: &Path,
) -> Result<std::process::Child, String> {
    use std::io::Write;
    std::fs::create_dir_all(log_path.parent().unwrap()).map_err(|e| e.to_string())?;
    let mut log_file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)
        .map_err(|e| e.to_string())?;
    writeln!(log_file, "\nStarting {}", executable.display()).map_err(|e| e.to_string())?;
    let mut command = wine_command(runtime, prefix, executable);
    if installing {
        command.arg("/S");
    } else if managed {
        command.args([
            "-cef-disable-gpu",
            "-cef-disable-gpu-compositing",
            "-cef-force-opaque-backgrounds",
            "-noverifyfiles",
        ]);
        // Keep normal Steam update checks enabled. Only skip startup repair of
        // the intentional helper shim; the monitor reapplies it after updates.
    }
    // Open the client visibly so first-time users can sign in. Keep Steam
    // independent of game process groups and mod DLL overrides.
    command
        .current_dir(executable.parent().unwrap())
        .env_remove("WINEDLLOVERRIDES")
        .env("WINEDEBUG", "fixme-all")
        .stdout(log_file.try_clone().map_err(|e| e.to_string())?)
        .stderr(log_file)
        .spawn()
        .map_err(|e| format!("Failed to open Windows Steam: {e}"))
}

#[cfg(target_os = "macos")]
fn monitor_steam_helpers(prefix: PathBuf, steam: PathBuf) {
    static MONITOR_BUSY: AtomicBool = AtomicBool::new(false);
    if MONITOR_BUSY.swap(true, Ordering::AcqRel) {
        return;
    }
    std::thread::spawn(move || {
        let mut absent = 0;
        let mut last_error = None;
        loop {
            std::thread::sleep(std::time::Duration::from_secs(2));
            if !steam_is_running(&prefix) {
                absent += 1;
                if absent >= 5 {
                    break;
                }
                continue;
            }
            absent = 0;
            match crate::macos_steam::ensure_helpers(&steam) {
                Ok(true) => {
                    // Valve's updater replaced the helper. Let Steam restart
                    // only its CEF processes with the refreshed shim; never
                    // terminate Steam itself or the game sharing this prefix.
                    for line in process_lines() {
                        let lower = line.to_ascii_lowercase();
                        if !(lower.contains("\\steamwebhelper.exe")
                            || lower.contains("\\steamwebhelper_real.exe"))
                        {
                            continue;
                        }
                        if let Some(pid) = line
                            .split_whitespace()
                            .next()
                            .and_then(|p| p.parse::<u32>().ok())
                        {
                            if process_has_wine_prefix(pid, &prefix) {
                                unsafe {
                                    libc::kill(pid as libc::pid_t, libc::SIGTERM);
                                }
                            }
                        }
                    }
                    last_error = None;
                }
                Ok(false) => {
                    last_error = None;
                }
                Err(error) => {
                    if last_error.as_ref() != Some(&error) {
                        log::warn!("Steam helper preparation: {error}");
                        last_error = Some(error);
                    }
                }
            }
        }
        MONITOR_BUSY.store(false, Ordering::Release);
    });
}

fn steam_exit_error(
    success: bool,
    installing: bool,
    installed: bool,
    log_path: &Path,
) -> Option<String> {
    if !success || (installing && !installed) {
        Some(format!(
            "Windows Steam {} failed or installation was cancelled. Retry in Launch settings. Diagnostic log: {}",
            if installing { "installer" } else { "client" },
            log_path.display(),
        ))
    } else {
        None
    }
}

#[tauri::command]
pub async fn start_macos_steam(app: tauri::AppHandle) -> Result<bool, String> {
    #[cfg(target_os = "macos")]
    {
        let (runtime, prefix) = ensure_runtime(&app).await?;
        open_steam(&app, &runtime, &prefix).await?;
        Ok(true)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = app;
        Err("Windows Steam setup is only needed on macOS.".into())
    }
}

#[cfg(target_os = "macos")]
fn use_metal(app: &tauri::AppHandle, runtime: &Path) -> Result<bool, String> {
    Ok(
        read_settings(app)?.graphics_backend.as_deref() != Some("dxvk")
            && crate::macos_wine_install::metal_supported()
            && crate::macos_wine_install::installed_wine(app)?.as_deref() == Some(runtime),
    )
}

/// Called before every run mode, before acquiring the synchronous launch lock.
#[cfg(target_os = "macos")]
pub async fn prepare_launch(app: &tauri::AppHandle) -> Result<(), String> {
    let (runtime, prefix) = ensure_runtime(app).await?;
    if !steam_is_running(&prefix) {
        let installing = open_steam(app, &runtime, &prefix).await?;
        return Err(if installing {
            "Windows Steam is installed and has opened. Wait for its update and sign in, then press Run again."
        } else {
            "Windows Steam has opened. Wait for it to finish updating and sign in, then press Run again."
        }.into());
    }
    if use_metal(app, &runtime)? {
        crate::macos_wine_install::ensure_metal_wine(app, &prefix).await?;
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn process_lines() -> Vec<String> {
    // Same-user process arguments only. Environment values are never formatted
    // as text or logged; Steam prefix detection reads its NUL-delimited data.
    Command::new("/bin/ps")
        .args([
            "-ww",
            "-U",
            &unsafe { libc::getuid() }.to_string(),
            "-o",
            "pid=,command=",
        ])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| {
            String::from_utf8_lossy(&output.stdout)
                .lines()
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

fn parse_procargs_environment(data: &[u8]) -> Option<Vec<&[u8]>> {
    let argc = i32::from_ne_bytes(data.get(..4)?.try_into().ok()?);
    if argc <= 0 || argc as usize > data.len().saturating_sub(4) {
        return None;
    }
    let mut remaining = data.get(4..)?;
    // KERN_PROCARGS2 stores argc, the saved executable path, NUL padding,
    // exactly argc argument strings, and then the environment strings.
    let executable_end = remaining.iter().position(|byte| *byte == 0)?;
    remaining = remaining.get(executable_end + 1..)?;
    let argv_start = remaining.iter().position(|byte| *byte != 0)?;
    remaining = remaining.get(argv_start..)?;
    for _ in 0..argc {
        let end = remaining.iter().position(|byte| *byte == 0)?;
        remaining = remaining.get(end + 1..)?;
    }
    // Wine rewrites argv and leaves NUL padding before the environment.
    // The saved argc still includes its original arguments.
    remaining = remaining
        .iter()
        .position(|byte| *byte != 0)
        .map(|start| &remaining[start..])
        .unwrap_or_default();
    let mut environment = Vec::new();
    while !remaining.is_empty() {
        let end = remaining.iter().position(|byte| *byte == 0)?;
        if end == 0 {
            break;
        }
        environment.push(&remaining[..end]);
        remaining = &remaining[end + 1..];
    }
    Some(environment)
}

#[cfg(target_os = "macos")]
fn process_has_wine_prefix(pid: u32, prefix: &Path) -> bool {
    use std::os::unix::ffi::OsStrExt;

    let Ok(pid) = libc::pid_t::try_from(pid) else {
        return false;
    };
    if pid <= 1 {
        return false;
    }
    let mut argmax: libc::c_int = 0;
    let mut size = std::mem::size_of_val(&argmax);
    let mut mib = [libc::CTL_KERN, libc::KERN_ARGMAX];
    let result = unsafe {
        libc::sysctl(
            mib.as_mut_ptr(),
            mib.len() as libc::c_uint,
            (&mut argmax as *mut libc::c_int).cast(),
            &mut size,
            std::ptr::null_mut(),
            0,
        )
    };
    if result != 0 || !(1..=16 * 1024 * 1024).contains(&argmax) {
        return false;
    }
    let mut data = vec![0u8; argmax as usize];
    size = data.len();
    let mut mib = [libc::CTL_KERN, libc::KERN_PROCARGS2, pid];
    let result = unsafe {
        libc::sysctl(
            mib.as_mut_ptr(),
            mib.len() as libc::c_uint,
            data.as_mut_ptr().cast(),
            &mut size,
            std::ptr::null_mut(),
            0,
        )
    };
    if result != 0 || size > data.len() {
        return false;
    }
    data.truncate(size);
    parse_procargs_environment(&data).is_some_and(|environment| {
        environment
            .into_iter()
            .any(|entry| entry.strip_prefix(b"WINEPREFIX=") == Some(prefix.as_os_str().as_bytes()))
    })
}

#[cfg(target_os = "macos")]
fn steam_is_running(prefix: &Path) -> bool {
    process_lines().iter().any(|line| {
        if !line.to_ascii_lowercase().contains("steam.exe") {
            return false;
        }
        line.split_whitespace()
            .next()
            .and_then(|pid| pid.parse().ok())
            .is_some_and(|pid| process_has_wine_prefix(pid, prefix))
    })
}

fn game_pid_from_line(line: &str, game_dir: &Path) -> Option<u32> {
    let line = line.trim();
    let (pid, command) = line.split_once(char::is_whitespace)?;
    let command = command.replace('\\', "/").to_lowercase();
    let game_path = game_dir
        .join("Lethal Company.exe")
        .to_string_lossy()
        .replace('\\', "/")
        .to_lowercase();
    if command.contains(&game_path) {
        pid.parse::<u32>().ok().filter(|pid| *pid > 1)
    } else {
        None
    }
}

#[cfg(target_os = "macos")]
pub fn game_pids(game_dir: &Path) -> Vec<u32> {
    process_lines()
        .iter()
        .filter_map(|line| game_pid_from_line(line, game_dir))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn older_runtime_settings_keep_automatic_graphics_and_custom_paths() {
        let settings: Settings =
            serde_json::from_str(r#"{"runtimePath":"/custom/wine","prefixPath":"/custom/prefix"}"#)
                .unwrap();
        assert_eq!(
            settings.runtime_path.as_deref(),
            Some(Path::new("/custom/wine"))
        );
        assert!(settings.graphics_backend.is_none());
        let settings: Settings = serde_json::from_str(r#"{"graphicsBackend":"dxvk"}"#).unwrap();
        assert_eq!(settings.graphics_backend.as_deref(), Some("dxvk"));
    }

    #[test]
    fn steam_exit_reports_failure_and_incomplete_installation() {
        let log = Path::new("/logs/macos-steam.log");
        assert!(steam_exit_error(false, true, false, log).is_some());
        assert!(steam_exit_error(true, true, false, log).is_some());
        assert!(steam_exit_error(true, true, true, log).is_none());
        assert!(steam_exit_error(true, false, true, log).is_none());
    }

    fn procargs_fixture(arguments: &[&[u8]], environment: &[&[u8]]) -> Vec<u8> {
        let mut data = (arguments.len() as i32).to_ne_bytes().to_vec();
        data.extend_from_slice(b"/Runtime/wine\0\0\0");
        for value in arguments.iter().chain(environment) {
            data.extend_from_slice(value);
            data.push(0);
        }
        data.push(0);
        data
    }

    #[test]
    fn procargs_keeps_prefix_spaces_and_exact_environment_boundaries() {
        let data = procargs_fixture(
            &[b"wine", b"", b"WINEPREFIX=/Bottles/Steam"],
            &[b"WINEPREFIX=/Bottles/Steam 2", b"OTHER=with spaces"],
        );
        let environment = parse_procargs_environment(&data).unwrap();
        assert_eq!(
            environment,
            vec![
                b"WINEPREFIX=/Bottles/Steam 2".as_slice(),
                b"OTHER=with spaces".as_slice()
            ]
        );
        assert!(!environment
            .iter()
            .any(|value| value.strip_prefix(b"WINEPREFIX=") == Some(b"/Bottles/Steam".as_slice())));
        assert!(environment.iter().any(
            |value| value.strip_prefix(b"WINEPREFIX=") == Some(b"/Bottles/Steam 2".as_slice())
        ));
    }

    #[test]
    fn procargs_reads_environment_after_wine_rewrites_argv() {
        let mut data = procargs_fixture(&[b"steam.exe", b"-noverifyfiles", b""], &[]);
        data.extend_from_slice(&[0; 180]);
        data.extend_from_slice(b"WINEPREFIX=/Bottles/Steam 2\0OTHER=value\0\0");
        assert_eq!(
            parse_procargs_environment(&data).unwrap(),
            vec![b"WINEPREFIX=/Bottles/Steam 2".as_slice(), b"OTHER=value".as_slice()]
        );
    }

    #[test]
    fn procargs_rejects_invalid_argc_and_truncated_data() {
        assert!(parse_procargs_environment(&[0, 1, 2]).is_none());
        for argc in [-1i32, 0, i32::MAX] {
            let mut data = procargs_fixture(&[b"wine"], &[]);
            data[..4].copy_from_slice(&argc.to_ne_bytes());
            assert!(parse_procargs_environment(&data).is_none());
        }
        let mut data = procargs_fixture(&[b"wine"], &[b"WINEPREFIX=/Bottles/Steam"]);
        data.truncate(data.len() - 2);
        assert!(parse_procargs_environment(&data).is_none());
        assert_eq!(
            parse_procargs_environment(&procargs_fixture(&[b"wine"], &[])),
            Some(vec![])
        );
    }

    #[test]
    fn wine_preserves_paths_with_spaces_as_single_arguments() {
        let prefix = Path::new("/Users/test/Library/Application Support/HQ/wine_prefix");
        let exe = Path::new("/Games/v73/Lethal Company.exe");
        let command = wine_command(Path::new("/Runtime/Wine Stable.app/bin/wine"), prefix, exe);
        assert_eq!(
            command.get_args().collect::<Vec<_>>(),
            vec![exe.as_os_str()]
        );
        assert!(command
            .get_envs()
            .any(|(key, value)| key == "WINEPREFIX" && value == Some(prefix.as_os_str())));
    }

    #[test]
    fn crossover_selects_the_matching_bottle() {
        let command = wine_command(
            Path::new("/Applications/CrossOver.app/Contents/SharedSupport/CrossOver/bin/wine"),
            Path::new("/Bottles/Lethal Company"),
            Path::new("/Games/Lethal Company.exe"),
        );
        assert_eq!(
            command.get_args().collect::<Vec<_>>(),
            vec![
                "--bottle",
                "Lethal Company",
                "--cx-app",
                "/Games/Lethal Company.exe"
            ]
        );
        assert!(command
            .get_envs()
            .any(|(key, value)| key == "CX_BOTTLE_PATH"
                && value == Some(std::ffi::OsStr::new("/Bottles"))));
    }

    #[test]
    fn process_matching_does_not_stop_steam_or_other_game_versions() {
        let dir = Path::new("/Games/v73");
        assert_eq!(
            game_pid_from_line("123 Z:\\Games\\v73\\Lethal Company.exe", dir),
            Some(123)
        );
        assert_eq!(
            game_pid_from_line("124 /Games/v730/Lethal Company.exe", dir),
            None
        );
        assert_eq!(
            game_pid_from_line("125 steam.exe WINEPREFIX=/Games/v73", dir),
            None
        );
    }
}

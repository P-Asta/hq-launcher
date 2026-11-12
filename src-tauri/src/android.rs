use serde::Deserialize;
use tauri::{plugin::PluginHandle, AppHandle, Manager, Wry};

struct InternalRuntime(PluginHandle<Wry>);

fn native(app: &AppHandle, command: &str) -> Result<serde_json::Value, String> {
    native_args(app, command, serde_json::json!({}))
}

fn native_args(app: &AppHandle, command: &str, args: serde_json::Value) -> Result<serde_json::Value, String> {
    app.state::<InternalRuntime>()
        .0
        .run_mobile_plugin(command, args)
        .map_err(|error| error.to_string())
}

pub(crate) fn cache_game_controls_version(app: &AppHandle, depot: &str, version: u32) -> Result<(), String> {
    native_args(app, "cacheGameVersion", serde_json::json!({ "depotManifest": depot, "gameVersion": version })).map(|_| ())
}

#[derive(Deserialize)]
pub(crate) struct InternalGamePaths {
    pub private_root: String,
    pub runtime_root: String,
    pub game_dir: String,
    pub steam_manifest: String,
    #[serde(default)]
    pub source: String,
    pub receipt: Option<String>,
    pub version: Option<u32>,
    pub depot_manifest: Option<String>,
}

pub(crate) fn internal_game_paths(app: &AppHandle) -> Result<InternalGamePaths, String> {
    serde_json::from_value(native(app, "gamePaths")?).map_err(|error| error.to_string())
}

#[tauri::command]
async fn get_android_versions(app: AppHandle) -> Result<serde_json::Value, String> {
    let (_, _, _, catalog, _) = crate::mod_config::ModsConfig::fetch_manifest(&crate::android_mods::http_client()?).await?;
    let state = native(&app, "versions")?;
    let steam_depot = internal_game_paths(&app).ok().and_then(|paths| {
        std::fs::read_to_string(paths.steam_manifest).ok()
            .and_then(|text| crate::android_mod_support::parse_steam_manifest(&text).ok())
            .filter(|manifest| manifest.state_flags == 4).map(|manifest| manifest.depot_manifest)
    });
    let selected = state["selected_version"].as_u64().unwrap_or(0);
    let installed = state["installed"].as_array().cloned().unwrap_or_default();
    let versions: Vec<_> = catalog.iter().rev().map(|(version, depot)| {
        let steam = steam_depot.as_deref() == Some(depot.as_str());
        let managed = installed.iter().any(|record| record["version"].as_u64() == Some(*version as u64) && record["depot_manifest"].as_str() == Some(depot.as_str()));
        serde_json::json!({"version":version,"installed":steam || managed,"selected": if selected == 0 {steam} else {selected == *version as u64},"source":if steam {"steam"} else {"managed"}})
    }).collect();
    Ok(serde_json::json!({"versions":versions,"install":state["install"]}))
}

#[tauri::command]
async fn install_android_version(app: AppHandle, version: u32) -> Result<(), String> {
    let _reservation = crate::android_mods::reserve_runtime_operation()?;
    ensure_android_session_stopped(&app)?;
    let (_, _, _, catalog, _) = crate::mod_config::ModsConfig::fetch_manifest(&crate::android_mods::http_client()?).await?;
    let depot = catalog.get(&version).ok_or("This game version is not in the HQ catalog")?;
    native_args(&app, "installVersion", serde_json::json!({"gameVersion":version,"depotManifest":depot})).map(|_| ())
}

#[tauri::command]
async fn select_android_version(app: AppHandle, version: u32) -> Result<(), String> {
    let _reservation = crate::android_mods::reserve_runtime_operation()?;
    ensure_android_session_stopped(&app)?;
    let (_, _, _, catalog, _) = crate::mod_config::ModsConfig::fetch_manifest(&crate::android_mods::http_client()?).await?;
    let depot = catalog.get(&version).ok_or("This game version is not in the HQ catalog")?;
    let steam = internal_game_paths(&app).ok().and_then(|paths| std::fs::read_to_string(paths.steam_manifest).ok())
        .and_then(|text| crate::android_mod_support::parse_steam_manifest(&text).ok())
        .is_some_and(|manifest| manifest.state_flags == 4 && manifest.depot_manifest == *depot);
    native_args(&app, "selectVersion", serde_json::json!({"gameVersion":if steam {0} else {version},"depotManifest":depot})).map(|_| ())
}

pub(crate) fn ensure_android_session_stopped(app: &AppHandle) -> Result<(), String> {
    native(app, "ensureStopped").map(|_| ())
}

pub(crate) fn enable_bepinex_override(app: &AppHandle) -> Result<(), String> {
    ensure_android_session_stopped(app)?;
    native(app, "configureBepinex").map(|_| ())
}

#[tauri::command]
async fn get_android_runtime_status(app: AppHandle) -> Result<serde_json::Value, String> {
    native(&app, "status")
}

#[tauri::command]
async fn install_android_runtime(app: AppHandle) -> Result<(), String> {
    native(&app, "installRuntime").map(|_| ())
}

#[tauri::command]
async fn open_android_steam(app: AppHandle) -> Result<(), String> {
    let _reservation = crate::android_mods::reserve_runtime_operation()
        .map_err(|_| "Wait for mod installation to finish before opening Steam.".to_string())?;
    native(&app, "openSteam").map(|_| ())
}

#[tauri::command]
async fn install_android_game(app: AppHandle) -> Result<(), String> {
    open_android_steam(app).await
}

#[tauri::command]
async fn launch_android_game(app: AppHandle) -> Result<(), String> {
    let _reservation = crate::android_mods::reserve_runtime_operation()
        .map_err(|_| "Wait for mod installation to finish before launching the game.".to_string())?;
    crate::android_mods::prepare_android_compatibility(&app).await?;
    let depot = crate::android_mods::ensure_mods_ready_for_launch(&app)?;
    native_args(&app, "launchGame", serde_json::json!({ "depotManifest": depot })).map(|_| ())
}

#[tauri::command]
async fn stop_android_session(app: AppHandle) -> Result<(), String> {
    native(&app, "stopSession").map(|_| ())
}

#[tauri::command]
async fn open_android_developer_settings(app: AppHandle) -> Result<(), String> {
    native(&app, "openDeveloperSettings").map(|_| ())
}

#[tauri::command]
async fn set_android_microphone(app: AppHandle, enabled: bool) -> Result<(), String> {
    app.state::<InternalRuntime>().0
        .run_mobile_plugin::<serde_json::Value>("setMicrophone", serde_json::json!({ "enabled": enabled }))
        .map(|_| ())
        .map_err(|error| error.to_string())
}

#[tauri::mobile_entry_point]
pub fn run() {
    let runtime = tauri::plugin::Builder::<Wry>::new("hq-runtime")
        .setup(|app, api| {
            let handle = api.register_android_plugin("asta.hq_launcher", "HqRuntimePlugin")?;
            app.manage(InternalRuntime(handle));
            Ok(())
        })
        .build();

    tauri::Builder::default()
        .plugin(runtime)
        .invoke_handler(tauri::generate_handler![
            get_android_runtime_status,
            install_android_runtime,
            open_android_steam,
            install_android_game,
            launch_android_game,
            stop_android_session,
            open_android_developer_settings,
            set_android_microphone,
            get_android_versions,
            install_android_version,
            select_android_version,
            crate::android_mods::get_android_mod_state,
            crate::android_mods::install_android_mods,
            crate::android_mods::set_android_mod_enabled
        ])
        .run(tauri::generate_context!())
        .expect("error while running HQ Launcher for Android");
}

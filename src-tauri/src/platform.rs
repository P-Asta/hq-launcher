// Keep desktop commands and their public module paths intact. Android has its
// own entry point: desktop process, overlay and Wine APIs do not exist there.
#[cfg(not(target_os = "android"))]
include!("lib.rs");

#[cfg(target_os = "android")]
mod android;
#[cfg(target_os = "android")]
mod android_mods;
#[cfg(target_os = "android")]
mod android_mod_support;
#[cfg(target_os = "android")]
mod bepinex_cfg;
#[cfg(target_os = "android")]
mod mod_config;
#[cfg(target_os = "android")]
mod mod_files;
#[cfg(target_os = "android")]
mod mods;
#[cfg(target_os = "android")]
mod release_channel;
#[cfg(target_os = "android")]
mod runtime_operation;
#[cfg(target_os = "android")]
mod thunderstore;
#[cfg(target_os = "android")]
mod zip_utils;
#[cfg(target_os = "android")]
pub(crate) fn thunderstore_cache_path(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    use tauri::Manager;
    Ok(app.path().app_data_dir().map_err(|error| error.to_string())?.join("cache/thunderstore.json"))
}
#[cfg(target_os = "android")]
pub use android::run;

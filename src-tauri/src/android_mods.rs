//! Installs the existing HQ package selection into HQ's own Android Steam library.
use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::Emitter;

use crate::android_mod_support::{self as support, GamePaths, ModPolicy};
use crate::mod_config::{ModEntry, ModsConfig};

const BEPINEX_URL: &str = "https://thunderstore.io/package/download/BepInEx/BepInExPack/5.4.2304/";
const CONFIG_URL: &str = "https://f.asta.rs/hq-launcher/default_config.zip";
pub(crate) use crate::runtime_operation::reserve_runtime_operation;

pub fn is_applying() -> bool {
    crate::runtime_operation::is_reserved()
}

/// Reqwest 0.13 defaults to a platform verifier which needs a JVM on Android.
/// An explicit Mozilla CA store uses rustls/webpki directly and keeps normal
/// certificate-chain and hostname verification without any JNI initialization.
pub(crate) fn http_client() -> Result<reqwest::Client, String> {
    let roots = rustls::RootCertStore::from_iter(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let tls = rustls::ClientConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .map_err(|error| error.to_string())?
    .with_root_certificates(roots)
    .with_no_client_auth();
    reqwest::Client::builder()
        .tls_backend_preconfigured(tls)
        .connect_timeout(std::time::Duration::from_secs(30))
        .user_agent("HQ-Launcher-Android/1.5.1")
        .build()
        .map_err(|error| error.to_string())
}

pub(crate) fn validate_archive(path: &Path) -> Result<(), String> {
    let file = std::fs::File::open(path).map_err(|error| error.to_string())?;
    let mut archive = zip::ZipArchive::new(file).map_err(|error| error.to_string())?;
    for index in 0..archive.len() {
        let entry = archive.by_index(index).map_err(|error| error.to_string())?;
        if !support::safe_archive_entry(entry.name())
            || entry
                .unix_mode()
                .is_some_and(|mode| mode & 0o170000 == 0o120000)
        {
            return Err("Archive contains an unsafe path or symbolic link".into());
        }
    }
    Ok(())
}

fn paths(app: &tauri::AppHandle) -> Result<GamePaths, String> {
    let native = crate::android::internal_game_paths(app)?;
    if native.source == "managed" {
        return support::validate_managed_game_paths(
            Path::new(&native.private_root), Path::new(&native.runtime_root), Path::new(&native.game_dir),
            Path::new(&native.steam_manifest), Path::new(native.receipt.as_deref().ok_or("Missing install receipt")?),
            native.version.ok_or("Missing managed version")?, native.depot_manifest.as_deref().ok_or("Missing depot manifest")?,
        );
    }
    support::validate_game_paths(
        Path::new(&native.private_root),
        Path::new(&native.runtime_root),
        Path::new(&native.game_dir),
        Path::new(&native.steam_manifest),
    )
}

fn writable(app: &tauri::AppHandle, paths: &GamePaths) -> Result<(), String> {
    crate::android::ensure_android_session_stopped(app)?;
    support::checked_internal_path(&paths.private_root, &paths.game_dir)?;
    support::checked_internal_path(&paths.private_root, &paths.steam_manifest)?;
    let current = support::parse_steam_manifest(
        &std::fs::read_to_string(&paths.steam_manifest).map_err(|error| error.to_string())?,
    )?;
    if current.state_flags != 4
        || current.depot_manifest != paths.manifest.depot_manifest
        || current.install_dir != paths.manifest.install_dir
    {
        return Err(
            "Steam must finish installing or updating Lethal Company before applying HQ mods"
                .into(),
        );
    }
    if let Some(install) = &paths.managed {
        support::validate_managed_game_paths(
            &paths.private_root, &paths.private_root.join("linuxfs"), &paths.game_dir,
            &paths.steam_manifest, &install.receipt, install.version, &install.depot_manifest,
        )?;
    }
    // Archive extraction and toggles must never follow an existing link out of
    // the installation, including dangling links in BepInEx or the workspace.
    support::reject_symlink_tree(&paths.game_dir)
}

fn key(dev: &str, name: &str) -> String {
    format!("{}::{}", dev.to_lowercase(), name.to_lowercase())
}

fn base_config(cfg: ModsConfig, version: u32) -> Result<ModsConfig, String> {
    for spec in &cfg.mods {
        if !support::safe_mod_id(&spec.dev) || !support::safe_mod_id(&spec.name) {
            return Err("HQ manifest contains an unsafe package identifier".into());
        }
    }
    Ok(ModsConfig {
        mods: cfg
            .mods
            .into_iter()
            .filter(|spec| {
                let affinity = spec
                    .tags
                    .iter()
                    .chain(spec.tag_constraints.keys())
                    .any(|tag| support::is_run_mode_tag(tag));
                (!affinity || spec.tags.is_empty()) && spec.is_install_compatible(version)
            })
            .collect(),
    })
}

fn policies(cfg: &ModsConfig, version: u32) -> Vec<ModPolicy> {
    cfg.mods
        .iter()
        .map(|spec| ModPolicy {
            key: key(&spec.dev, &spec.name),
            remote_enabled: spec.enabled && support::android_mod_block_reason(&spec.dev, &spec.name, version).is_none(),
            switch_group: spec.switch_group.clone(),
        })
        .collect()
}

fn install_config(cfg: &ModsConfig, enabled: &BTreeMap<String, bool>) -> ModsConfig {
    ModsConfig {
        mods: cfg
            .mods
            .iter()
            .filter(|spec| {
                spec.switch_group
                    .as_deref()
                    .is_none_or(|group| group.trim().is_empty())
                    || enabled.get(&key(&spec.dev, &spec.name)) == Some(&true)
            })
            .cloned()
            .collect(),
    }
}

#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
struct SavedState {
    format: u32,
    depot_manifest: String,
    complete: bool,
    bepinex_version: String,
    enabled: BTreeMap<String, bool>,
    android_compatibility: u32,
}

fn state_path(paths: &GamePaths, temporary: bool) -> Result<PathBuf, String> {
    let destination = paths.mod_state_path();
    let path = if temporary { destination.with_extension("json.tmp") } else { destination };
    match std::fs::symlink_metadata(&path) {
        Ok(_) => {
            support::checked_internal_path(&paths.private_root, &path)?;
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.to_string()),
    }
    Ok(path)
}

fn read_state(paths: &GamePaths) -> Result<SavedState, String> {
    let path = state_path(paths, false)?;
    if !path.exists() {
        return Ok(SavedState {
            format: 1,
            ..SavedState::default()
        });
    }
    if std::fs::metadata(&path)
        .map_err(|error| error.to_string())?
        .len()
        > 1024 * 1024
    {
        return Err("HQ mod settings are too large".into());
    }
    let saved: SavedState =
        serde_json::from_str(&std::fs::read_to_string(path).map_err(|error| error.to_string())?)
            .map_err(|error| error.to_string())?;
    if saved.format != 1 {
        return Err("Unsupported HQ mod settings format".into());
    }
    Ok(saved)
}

/// Offline preflight for HQ's launch command. The caller holds the shared
/// runtime-operation reservation until the native launch is accepted.
pub(crate) fn ensure_mods_ready_for_launch(app: &tauri::AppHandle) -> Result<String, String> {
    let paths = paths(app)?;
    let saved = if state_path(&paths, false)?.exists() {
        Some(read_state(&paths)?)
    } else {
        None
    };
    support::validate_launch_state(
        saved
            .as_ref()
            .map(|saved| (saved.complete, saved.depot_manifest.as_str())),
        paths.depot_manifest(),
        paths.manifest.state_flags,
    )?;
    Ok(paths.depot_manifest().to_string())
}

/// Reconcile an existing installation once, keeping packages and unrelated choices.
pub(crate) async fn prepare_android_compatibility(app: &tauri::AppHandle) -> Result<(), String> {
    let paths = paths(app)?;
    if !state_path(&paths, false)?.exists() { return Ok(()); }
    let mut saved = read_state(&paths)?;
    if saved.android_compatibility >= 1 || !saved.complete || saved.depot_manifest != paths.depot_manifest() { return Ok(()); }
    writable(app, &paths)?;
    let (remote, version) = manifest_and_version(&paths).await?;
    let version = version.ok_or("Connect to verify this game version before applying Android compatibility settings")?;
    let cfg = base_config(remote.clone(), version)?;
    let choices = support::effective_choices(&policies(&cfg, version), &saved.enabled);
    writable(app, &paths)?;
    apply_choices(&paths, &managed_keys(&remote, &saved), &choices)?;
    saved.enabled.extend(choices);
    saved.android_compatibility = 1;
    save_state(&paths, &saved)
}

fn save_state(paths: &GamePaths, state: &SavedState) -> Result<(), String> {
    let destination = state_path(paths, false)?;
    let temporary = state_path(paths, true)?;
    let mut file = std::fs::File::create(&temporary).map_err(|error| error.to_string())?;
    serde_json::to_writer_pretty(&mut file, state).map_err(|error| error.to_string())?;
    file.sync_all().map_err(|error| error.to_string())?;
    drop(file);
    std::fs::rename(temporary, destination).map_err(|error| error.to_string())
}

fn mod_dir(game: &Path, section: &str, spec: &ModEntry) -> Option<PathBuf> {
    let folder = format!("{}-{}", spec.dev, spec.name);
    std::fs::read_dir(game.join("BepInEx").join(section))
        .ok()?
        .filter_map(Result::ok)
        .find(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .eq_ignore_ascii_case(&folder)
                && entry.file_type().is_ok_and(|kind| kind.is_dir())
        })
        .map(|entry| entry.path())
}

fn mod_installed(paths: &GamePaths, spec: &ModEntry) -> bool {
    ["plugins", "patchers"]
        .iter()
        .filter_map(|section| mod_dir(&paths.game_dir, section, spec))
        .any(|dir| {
            ["manifest.json", "manifest.json.old"].iter().any(|name| {
                let manifest = dir.join(name);
                support::checked_internal_path(&paths.game_dir, &manifest).is_ok()
                    && crate::bepinex_cfg::read_manifest(&manifest).is_ok()
            })
        })
}

fn bepinex_installed(game: &Path) -> bool {
    [
        "winhttp.dll",
        "doorstop_config.ini",
        "BepInEx/core/BepInEx.dll",
    ]
    .iter()
    .all(|name| {
        let path = game.join(name);
        path.is_file() && support::checked_internal_path(game, &path).is_ok()
    })
}

fn progress(app: &tauri::AppHandle, phase: &str, detail: &str, percent: Option<f64>) {
    let _ = app.emit("android-mod-progress", json!({ "phase": phase, "detail": detail, "progress": percent.map(|value| value.clamp(0.0, 100.0)) }));
}

async fn download(
    app: &tauri::AppHandle,
    client: &reqwest::Client,
    url: &str,
    target: &Path,
    phase: &str,
) -> Result<(), String> {
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|error| error.to_string())?
        .error_for_status()
        .map_err(|error| error.to_string())?;
    let total = response.content_length();
    let mut stream = response.bytes_stream();
    let mut file = std::fs::File::create(target).map_err(|error| error.to_string())?;
    let mut downloaded = 0u64;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|error| error.to_string())?;
        file.write_all(&chunk).map_err(|error| error.to_string())?;
        downloaded += chunk.len() as u64;
        progress(
            app,
            phase,
            "Downloading files",
            total
                .filter(|total| *total > 0)
                .map(|total| downloaded as f64 / total as f64 * 100.0),
        );
    }
    Ok(())
}

fn managed_keys(remote: &ModsConfig, saved: &SavedState) -> BTreeSet<String> {
    saved
        .enabled
        .keys()
        .cloned()
        .chain(remote.mods.iter().map(|spec| key(&spec.dev, &spec.name)))
        .collect()
}

fn apply_choices(
    paths: &GamePaths,
    managed: &BTreeSet<String>,
    enabled: &BTreeMap<String, bool>,
) -> Result<(), String> {
    crate::mod_files::reconcile_managed_mods(&paths.game_dir, managed, enabled)?;
    if !crate::mod_files::excluded_mods_are_disabled(&paths.game_dir, managed, enabled)? {
        return Err("Could not disable outdated or incompatible HQ mods.".into());
    }
    Ok(())
}

async fn manifest_and_version(paths: &GamePaths) -> Result<(ModsConfig, Option<u32>), String> {
    let (_, cfg, _, versions, _) = ModsConfig::fetch_manifest(&http_client()?).await?;
    let version = support::game_version(&versions, paths.depot_manifest());
    Ok((cfg, version))
}

#[tauri::command]
pub async fn get_android_mod_state(app: tauri::AppHandle) -> Result<Value, String> {
    let paths = match paths(&app) {
        Ok(paths) => paths,
        Err(reason) => {
            return Ok(
                json!({ "game_version": null, "supported": false, "installed": false, "applying": is_applying(), "mods": [], "reason": reason }),
            )
        }
    };
    let (remote, version) = manifest_and_version(&paths).await?;
    let Some(version) = version else {
        return Ok(
            json!({ "game_version": null, "supported": false, "installed": false, "applying": is_applying(), "mods": [], "depot_manifest": paths.depot_manifest(),
            "reason": "This Steam game version is not in the HQ compatibility list." }),
        );
    };
    let cfg = base_config(remote.clone(), version)?;
    if paths.manifest.state_flags == 4 {
        // Cosmetic native control metadata is keyed by the installed depot, so
        // updates cannot reuse a previous version's Tab availability.
        if let Err(error) = crate::android::cache_game_controls_version(&app, paths.depot_manifest(), version) {
            log::warn!("Could not cache Android control version: {error}");
        }
    }
    let saved = read_state(&paths)?;
    let enabled = support::effective_choices(&policies(&cfg, version), &saved.enabled);
    let required = install_config(&cfg, &enabled);
    let excluded_disabled = crate::mod_files::excluded_mods_are_disabled(
        &paths.game_dir,
        &managed_keys(&remote, &saved),
        &enabled,
    )?;
    let installed = support::install_is_current(
        saved.complete,
        &saved.depot_manifest,
        paths.depot_manifest(),
        saved.bepinex_version == "5.4.2304"
            && bepinex_installed(&paths.game_dir)
            && required.mods.iter().all(|spec| mod_installed(&paths, spec))
            && excluded_disabled,
    ) && paths.manifest.state_flags == 4;
    let mods: Vec<_> = cfg.mods.iter().filter(|spec| !spec.tags.iter().any(|tag| tag.eq_ignore_ascii_case("ui_hidden"))).map(|spec| json!({
        "dev": spec.dev, "name": spec.name, "enabled": enabled.get(&key(&spec.dev, &spec.name)).copied().unwrap_or(false),
        "installed": mod_installed(&paths, spec), "switch_group": spec.switch_group,
        "blocked_reason": support::android_mod_block_reason(&spec.dev, &spec.name, version),
    })).collect();
    Ok(
        json!({ "game_version": version, "supported": paths.manifest.state_flags == 4, "installed": installed,
        "applying": is_applying(), "mods": mods, "depot_manifest": paths.depot_manifest(),
        "reason": if paths.manifest.state_flags != 4 { Some("Wait for Steam to finish installing or updating the game.") }
            else if !excluded_disabled { Some("Outdated or incompatible HQ mods remain. Reinstall HQ mods.") } else { None } }),
    )
}

async fn install_inner(app: &tauri::AppHandle) -> Result<(), String> {
    let paths = paths(app)?;
    writable(app, &paths)?;
    progress(app, "manifest", "Checking HQ mod compatibility", None);
    let (remote, version) = manifest_and_version(&paths).await?;
    let version = version
        .ok_or("This Steam depot is not supported by HQ. No mods were installed.")?;
    let cfg = base_config(remote.clone(), version)?;
    if cfg.mods.is_empty() {
        return Err("No HQ mods are compatible with this game version.".into());
    }
    let mut saved = read_state(&paths)?;
    let enabled = support::effective_choices(&policies(&cfg, version), &saved.enabled);
    let managed = managed_keys(&remote, &saved);
    saved.enabled.extend(enabled.clone());
    saved.complete = false;
    save_state(&paths, &saved)?;
    let workspace = paths.game_dir.join(".hq-launcher/android");
    std::fs::create_dir_all(&workspace).map_err(|error| error.to_string())?;
    let client = http_client()?;
    if saved.bepinex_version != "5.4.2304" || !bepinex_installed(&paths.game_dir) {
        let archive = workspace.join("bepinex.zip");
        download(app, &client, BEPINEX_URL, &archive, "bepinex").await?;
        writable(app, &paths)?;
        crate::zip_utils::extract_thunderstore_package_with_progress(
            &archive,
            &paths.game_dir,
            |done, total, detail| {
                progress(
                    app,
                    "bepinex",
                    detail.as_deref().unwrap_or("Installing BepInEx"),
                    (total > 0).then(|| done as f64 / total as f64 * 100.0),
                );
            },
        )?;
        std::fs::remove_file(archive).map_err(|error| error.to_string())?;
        if !bepinex_installed(&paths.game_dir) {
            return Err("BepInExPack archive did not contain the required runtime files".into());
        }
    }
    let archive = workspace.join("default-config.zip");
    download(app, &client, CONFIG_URL, &archive, "config").await?;
    writable(app, &paths)?;
    crate::zip_utils::extract_config_zip_into_bepinex_config_with_progress(
        &archive,
        &paths.game_dir.join("BepInEx/config"),
        |_, _, _| {},
    )?;
    std::fs::remove_file(archive).map_err(|error| error.to_string())?;
    let required = install_config(&cfg, &enabled);
    writable(app, &paths)?;
    crate::mods::install_mods_with_progress(
        app,
        &paths.game_dir,
        version,
        &required,
        &[],
        None,
        |done, total, info| {
            progress(
                app,
                "mods",
                info.detail.as_deref().unwrap_or("Installing HQ mods"),
                (total > 0).then(|| done as f64 / total as f64 * 100.0),
            );
        },
    )
    .await?;
    if !required.mods.iter().all(|spec| mod_installed(&paths, spec)) {
        return Err("Some HQ mods did not finish installing. Installation remains incomplete.".into());
    }
    writable(app, &paths)?;
    apply_choices(&paths, &managed, &enabled)?;
    crate::android::enable_bepinex_override(app)?;
    saved.depot_manifest = paths.depot_manifest().to_string();
    saved.android_compatibility = 1;
    saved.bepinex_version = "5.4.2304".into();
    saved.complete = true;
    save_state(&paths, &saved)?;
    progress(app, "complete", "HQ mods installed", Some(100.0));
    Ok(())
}

#[tauri::command]
pub async fn install_android_mods(app: tauri::AppHandle) -> Result<(), String> {
    let _applying = reserve_runtime_operation()?;
    let result = install_inner(&app).await;
    if let Err(error) = &result {
        progress(&app, "error", error, None);
    }
    result
}

#[tauri::command]
pub async fn set_android_mod_enabled(
    app: tauri::AppHandle,
    dev: String,
    name: String,
    enabled: bool,
) -> Result<(), String> {
    let _applying = reserve_runtime_operation()?;
    if !support::safe_mod_id(&dev) || !support::safe_mod_id(&name) {
        return Err("Unsafe mod identifier".into());
    }
    let paths = paths(&app)?;
    writable(&app, &paths)?;
    let (remote, version) = manifest_and_version(&paths).await?;
    let version = version.ok_or("Unsupported Steam game version")?;
    let cfg = base_config(remote.clone(), version)?;
    let mut saved = read_state(&paths)?;
    let managed = managed_keys(&remote, &saved);
    if !support::install_is_current(
        saved.complete,
        &saved.depot_manifest,
        paths.depot_manifest(),
        bepinex_installed(&paths.game_dir),
    ) {
        return Err("Install HQ mods first.".into());
    }
    let choices =
        support::toggle_choice(&policies(&cfg, version), &saved.enabled, &key(&dev, &name), enabled)?;
    let active = support::effective_choices(&policies(&cfg, version), &choices);
    saved.complete = false;
    save_state(&paths, &saved)?;
    if enabled {
        let spec = cfg
            .mods
            .iter()
            .find(|spec| key(&spec.dev, &spec.name) == key(&dev, &name))
            .ok_or("Unknown HQ mod")?;
        if !mod_installed(&paths, spec) {
            let single = ModsConfig {
                mods: vec![spec.clone()],
            };
            crate::mods::install_mods_with_progress(
                &app,
                &paths.game_dir,
                version,
                &single,
                &[],
                None,
                |_, _, info| {
                    progress(
                        &app,
                        "mods",
                        info.detail.as_deref().unwrap_or("Installing mods"),
                        None,
                    );
                },
            )
            .await?;
            if !mod_installed(&paths, spec) {
                return Err("Mod installation did not finish".into());
            }
        }
    }
    writable(&app, &paths)?;
    apply_choices(&paths, &managed, &active)?;
    saved.enabled = choices;
    saved.android_compatibility = 1;
    saved.complete = true;
    save_state(&paths, &saved)?;
    progress(&app, "complete", "Mod settings saved", Some(100.0));
    Ok(())
}

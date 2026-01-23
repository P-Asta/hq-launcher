use std::cmp::Ordering;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::bepinex_cfg::read_manifest;
use crate::mod_config::{ModEntry, ModsConfig};
use crate::thunderstore::{self, PackageListing};
use crate::zip_utils::extract_thunderstore_into_plugins_with_progress;
use semver::Version;

fn read_manifest_allow_old(mod_dir: &Path) -> Result<crate::bepinex_cfg::BepInExManifest, String> {
    let manifest = mod_dir.join("manifest.json");
    if manifest.exists() {
        return read_manifest(&manifest);
    }
    let manifest_old = mod_dir.join("manifest.json.old");
    if manifest_old.exists() {
        return read_manifest(&manifest_old);
    }
    Err(format!(
        "manifest.json not found under {}",
        mod_dir.to_string_lossy()
    ))
}

fn parse_semver_loose(s: &str) -> Option<Version> {
    let s = s.trim().trim_start_matches('v');
    if let Ok(v) = Version::parse(s) {
        return Some(v);
    }
    // Allow "1.2" or "1" by padding.
    let parts: Vec<&str> = s.split('.').collect();
    let padded = match parts.len() {
        1 => format!("{}.0.0", s),
        2 => format!("{}.0", s),
        _ => s.to_string(),
    };
    Version::parse(&padded).ok()
}

fn cmp_version_str(a: &str, b: &str) -> Ordering {
    match (parse_semver_loose(a), parse_semver_loose(b)) {
        (Some(va), Some(vb)) => va.cmp(&vb),
        // Prefer parsable semver over non-parsable.
        (Some(_), None) => Ordering::Greater,
        (None, Some(_)) => Ordering::Less,
        (None, None) => a.cmp(b),
    }
}

fn latest_pkg_version<'a>(
    versions: &'a [thunderstore::PackageVersion],
) -> Option<&'a thunderstore::PackageVersion> {
    versions
        .iter()
        .max_by(|a, b| cmp_version_str(&a.version_number, &b.version_number))
}

fn thunderstore_download_url(dev: &str, name: &str, version: &str) -> String {
    // Direct download endpoint (zip):
    // https://thunderstore.io/package/download/{dev}/{modname}/{version}/
    format!(
        "https://thunderstore.io/package/download/{}/{}/{}/",
        dev, name, version
    )
}

pub fn plugins_dir(game_root: &Path) -> PathBuf {
    game_root.join("BepInEx").join("plugins")
}

/// Downloads and installs a list of Thunderstore packages into `BepInEx/plugins`.
///
/// Progress callback reports `(installed_mods, total_mods, detail)`.
pub async fn install_mods_with_progress<F>(
    app: &tauri::AppHandle,
    game_root: &Path,
    game_version: u32,
    cfg: &ModsConfig,
    mut on_progress: F,
) -> Result<(), String>
where
    F: FnMut(u64, u64, Option<String>),
{
    let client = reqwest::Client::new();

    // Fetch Thunderstore package list once (per-package API is unreliable/404).
    let cache_path = crate::thunderstore_cache_path(app)?;
    let packages = thunderstore::fetch_community_packages(&client, &cache_path).await?;
    log::info!("Fetched {} packages", packages.len());
    let mut package_map: HashMap<(String, String), PackageListing> = HashMap::new();
    for p in packages.clone() {
        package_map.insert((p.owner.to_lowercase(), p.name.to_lowercase()), p);
    }

    let target_plugins = plugins_dir(game_root);
    std::fs::create_dir_all(&target_plugins).map_err(|e| e.to_string())?;
    log::info!("Target plugins dir: {}", target_plugins.to_string_lossy());

    // Temp workspace inside game folder (keeps things simple and visible for debugging).
    let temp_root = game_root.join(".hq-launcher").join("tmp").join("mods");
    if temp_root.exists() {
        std::fs::remove_dir_all(&temp_root).map_err(|e| e.to_string())?;
    }
    std::fs::create_dir_all(&temp_root).map_err(|e| e.to_string())?;

    let total_mods = cfg.mods.len() as u64;
    let mut installed: u64 = 0;
    on_progress(0, total_mods, Some("Starting...".to_string()));

    for (idx, spec) in cfg.mods.iter().enumerate() {
        // Add-only: if a plugin folder already exists for this mod, skip it.
        // Folder name is deterministic (does not include the mod version).
        let already_dir = target_plugins.join(format!("{}-{}", spec.dev, spec.name));
        if already_dir.exists() {
            log::info!(
                "{}/{}  |  {}-{} Start Check",
                idx + 1,
                cfg.mods.len(),
                spec.dev,
                spec.name
            );

            let manifest = read_manifest_allow_old(&already_dir)?;

            let version_limit = spec
                .version_config
                .get(&game_version)
                .unwrap_or(&"0.0.0".to_string())
                .clone();
            installed = installed.saturating_add(1);
            if version_limit == "0.0.0" {
                let new_version = packages
                    .clone()
                    .iter()
                    .find(|p| {
                        p.owner.to_lowercase() == spec.dev.to_lowercase()
                            && p.name.to_lowercase() == spec.name.to_lowercase()
                    })
                    .map(|p| {
                        p.versions
                            .first()
                            .map(|v| v.version_number.clone())
                            .unwrap_or_else(|| "0.0.0".to_string())
                    })
                    .unwrap_or_else(|| "0.0.0".to_string());

                if manifest.version_number == new_version {
                    continue;
                }
                log::info!(
                    "Updating {}-{} from {old_version} to {new_version}",
                    spec.dev,
                    spec.name,
                    old_version = manifest.version_number
                );
            } else if manifest.version_number != version_limit {
                log::info!(
                    "Updating {}-{} from {old_version} to {version_limit}",
                    spec.dev,
                    spec.name,
                    old_version = manifest.version_number
                );
            } else {
                on_progress(
                    installed,
                    total_mods,
                    Some(format!(
                        "Skipped {}/{}  |  {}-{} (version equal)",
                        idx + 1,
                        cfg.mods.len(),
                        spec.dev,
                        spec.name
                    )),
                );
                continue;
            }

            // log::info!("\tcurrent_version: {:#?}", current_version);
        }

        let mod_label = format!("{}-{}", spec.dev, spec.name);

        if !spec.is_compatible(game_version) {
            installed = installed.saturating_add(1);
            let why = incompatible_reason(spec, game_version);
            log::warn!("Skipping {mod_label}{why}");
            on_progress(
                installed,
                total_mods,
                Some(format!("Skipped {mod_label}{why}")),
            );
            continue;
        }

        on_progress(
            installed,
            total_mods,
            Some(format!("Resolving {mod_label}")),
        );

        let key = (spec.dev.to_lowercase(), spec.name.to_lowercase());
        let Some(pkg) = package_map.get(&key) else {
            installed = installed.saturating_add(1);
            log::error!("Package not found in list: {}-{}", spec.dev, spec.name);
            on_progress(
                installed,
                total_mods,
                Some(format!(
                    "Failed to resolve {mod_label} (not found in package list)"
                )),
            );
            continue;
        };

        let pinned = spec.pinned_version_for(game_version);
        let ver = if let Some(pin) = pinned {
            // Prefer the pinned version only if it exists in the listing.
            if pkg.versions.iter().any(|v| v.version_number == pin) {
                pin.to_string()
            } else {
                log::warn!(
                    "Pinned version not found for {mod_label}: {pin} (falling back to latest)"
                );
                latest_pkg_version(&pkg.versions)
                    .map(|v| v.version_number.clone())
                    .unwrap_or_else(|| "0.0.0".to_string())
            }
        } else {
            latest_pkg_version(&pkg.versions)
                .map(|v| v.version_number.clone())
                .unwrap_or_else(|| "0.0.0".to_string())
        };

        if ver == "0.0.0" {
            installed = installed.saturating_add(1);
            log::error!("No versions for {}-{}", spec.dev, spec.name);
            on_progress(
                installed,
                total_mods,
                Some(format!("Failed to resolve {mod_label} (no versions)")),
            );
            continue;
        }

        let download_url = thunderstore_download_url(&spec.dev, &spec.name, &ver);
        log::info!("Resolved {mod_label} => v{ver}");

        let zip_path = temp_root.join(format!("{}-{}-{}.zip", spec.dev, spec.name, ver));

        // Download zip
        on_progress(
            installed,
            total_mods,
            Some(format!("Downloading {mod_label}")),
        );
        log::info!("Downloading {mod_label} from {download_url}");
        let bytes = client
            .get(&download_url)
            .send()
            .await
            .map_err(|e| e.to_string())?
            .error_for_status()
            .map_err(|e| e.to_string())?
            .bytes()
            .await
            .map_err(|e| e.to_string())?;

        std::fs::write(&zip_path, &bytes).map_err(|e| e.to_string())?;

        // Extract directly into BepInEx/plugins, then delete the zip.
        on_progress(
            installed,
            total_mods,
            Some(format!("Extracting {mod_label}")),
        );
        let folder_name = format!("{}-{}", spec.dev, spec.name);

        if let Err(e) = extract_thunderstore_into_plugins_with_progress(
            &zip_path,
            &target_plugins,
            &folder_name,
            |_d, _t, _n| {},
        ) {
            installed = installed.saturating_add(1);
            log::error!("Failed to extract into plugins {mod_label}: {e}");
            on_progress(
                installed,
                total_mods,
                Some(format!("Failed to extract {mod_label} ({e})")),
            );
            let _ = std::fs::remove_file(&zip_path);
            continue;
        }

        // Cleanup per-mod artifacts
        if let Err(e) = std::fs::remove_file(&zip_path) {
            log::warn!("Failed to delete zip {}: {}", zip_path.to_string_lossy(), e);
        }

        installed = installed.saturating_add(1);
        on_progress(
            installed,
            total_mods,
            Some(format!("Installed {mod_label}")),
        );
    }

    // Best-effort cleanup of temp workspace.
    let _ = std::fs::remove_dir_all(&temp_root);

    Ok(())
}

pub async fn updatable_mods_with_progress<F>(
    app: &tauri::AppHandle,
    game_root: &Path,
    game_version: u32,
    cfg: &ModsConfig,
    mut on_progress: F,
) -> Result<(), String>
where
    F: FnMut(u64, u64, Option<String>, Option<String>),
{
    let client = reqwest::Client::new();

    let total_mods = cfg.mods.len() as u64;
    on_progress(0, total_mods, Some("Starting...".to_string()), None);

    // Fetch Thunderstore package list once (per-package API is unreliable/404).
    log::info!("Fetching Thunderstore package list for Lethal Company");
    let cache_path = crate::thunderstore_cache_path(app)?;
    let packages = thunderstore::fetch_community_packages(&client, &cache_path).await?;
    log::info!("Fetched {} packages", packages.len());
    let mut package_map: HashMap<(String, String), PackageListing> = HashMap::new();
    for p in packages.clone() {
        package_map.insert((p.owner.to_lowercase(), p.name.to_lowercase()), p);
    }

    let target_plugins = plugins_dir(game_root);
    std::fs::create_dir_all(&target_plugins).map_err(|e| e.to_string())?;
    log::info!("Target plugins dir: {}", target_plugins.to_string_lossy());

    // Temp workspace inside game folder (keeps things simple and visible for debugging).
    let temp_root = game_root.join(".hq-launcher").join("tmp").join("mods");
    if temp_root.exists() {
        std::fs::remove_dir_all(&temp_root).map_err(|e| e.to_string())?;
    }
    std::fs::create_dir_all(&temp_root).map_err(|e| e.to_string())?;

    on_progress(0, total_mods, Some("Starting...".to_string()), None);

    for (idx, spec) in cfg.mods.iter().enumerate() {
        // Add-only: if a plugin folder already exists for this mod, skip it.
        // Folder name is deterministic (does not include the mod version).
        let idx = idx as u64 + 1;
        let already_dir = target_plugins.join(format!("{}-{}", spec.dev, spec.name));
        let mod_label = format!("{}-{}", spec.dev, spec.name);
        if already_dir.exists() {
            let manifest = match read_manifest_allow_old(&already_dir) {
                Ok(m) => m,
                Err(e) => {
                    // Don't fail the entire check if one plugin has a broken/edited manifest.
                    // Treat it as "unknown version" so the user can still see other updates.
                    log::warn!("Failed to read {mod_label} manifest.json: {e}");
                    on_progress(
                        idx,
                        total_mods,
                        Some(format!("{mod_label}: failed to read manifest ({e})")),
                        None,
                    );
                    continue;
                }
            };

            // Use the SAME pinning semantics as install/update:
            // - If pinned_version_for(game_version) exists: compare against that pinned version.
            // - Else: compare against latest available version (semver max).
            let desired_version = if let Some(pin) = spec.pinned_version_for(game_version) {
                pin.to_string()
            } else {
                let key = (spec.dev.to_lowercase(), spec.name.to_lowercase());
                package_map
                    .get(&key)
                    .and_then(|p| latest_pkg_version(&p.versions).map(|v| v.version_number.clone()))
                    .unwrap_or_else(|| "0.0.0".to_string())
            };

            if desired_version == "0.0.0" {
                log::warn!("Could not resolve desired version for {mod_label} (no versions)");
                on_progress(
                    idx,
                    total_mods,
                    Some(format!("{mod_label}: failed to resolve latest version")),
                    None,
                );
                continue;
            }

            match cmp_version_str(&manifest.version_number, &desired_version) {
                Ordering::Equal => {
                    log::info!("{} is already the latest version", mod_label.clone());
                    on_progress(
                        idx,
                        total_mods,
                        Some(format!(
                            "{} is already the latest version",
                            mod_label.clone()
                        )),
                        None,
                    );
                }
                Ordering::Less => {
                    log::info!(
                        "{} mod can update ({} -> {})",
                        mod_label.clone(),
                        manifest.version_number,
                        desired_version
                    );
                    on_progress(
                        idx,
                        total_mods,
                        Some(format!("{} mod can update", mod_label.clone())),
                        Some(mod_label.clone()),
                    );
                }
                Ordering::Greater => {
                    log::info!(
                        "{} is newer than desired ({} > {})",
                        mod_label.clone(),
                        manifest.version_number,
                        desired_version
                    );
                    on_progress(
                        idx,
                        total_mods,
                        Some(format!("{} is newer than desired", mod_label.clone())),
                        None,
                    );
                }
            }
        } else {
            // Plugin folder doesn't exist, but mod is in remote manifest - mark as updatable (installable)
            if spec.is_compatible(game_version) {
                log::info!(
                    "{} is missing but available in manifest - can install",
                    mod_label.clone()
                );
                on_progress(
                    idx,
                    total_mods,
                    Some(format!(
                        "{} is missing but available - can install",
                        mod_label.clone()
                    )),
                    Some(mod_label.clone()),
                );
            } else {
                let why = incompatible_reason(spec, game_version);
                log::info!("{} is missing but incompatible{}", mod_label.clone(), why);
                on_progress(
                    idx,
                    total_mods,
                    Some(format!(
                        "{} is missing but incompatible{}",
                        mod_label.clone(),
                        why
                    )),
                    None,
                );
            }
        }
    }

    on_progress(total_mods, total_mods, Some("Finished".to_string()), None);

    Ok(())
}


pub async fn update_mods_with_progress<F>(
    app: &tauri::AppHandle,
    game_root: &Path,
    game_version: u32,
    cfg: &ModsConfig,
    updatable_mods: Vec<String>,
    mut on_progress: F,
) -> Result<(), String>
where
    F: FnMut(u64, u64, Option<String>),
{
    let client = reqwest::Client::new();

    // Fetch Thunderstore package list once (per-package API is unreliable/404).
    let cache_path = crate::thunderstore_cache_path(app)?;
    let packages = thunderstore::fetch_community_packages(&client, &cache_path).await?;
    log::info!("Fetched {} packages", packages.len());
    let mut package_map: HashMap<(String, String), PackageListing> = HashMap::new();
    for p in packages.clone() {
        package_map.insert((p.owner.to_lowercase(), p.name.to_lowercase()), p);
    }

    let target_plugins = plugins_dir(game_root);
    std::fs::create_dir_all(&target_plugins).map_err(|e| e.to_string())?;
    log::info!("Target plugins dir: {}", target_plugins.to_string_lossy());

    // Temp workspace inside game folder (keeps things simple and visible for debugging).
    let temp_root = game_root.join(".hq-launcher").join("tmp").join("mods");
    if temp_root.exists() {
        std::fs::remove_dir_all(&temp_root).map_err(|e| e.to_string())?;
    }
    std::fs::create_dir_all(&temp_root).map_err(|e| e.to_string())?;

    let total_mods = updatable_mods.len() as u64;
    let mut installed: u64 = 0;
    on_progress(0, total_mods, Some("Starting...".to_string()));

    for (_idx, spec) in cfg.mods.iter().enumerate() {
        // Add-only: if a plugin folder already exists for this mod, skip it.
        // Folder name is deterministic (does not include the mod version).

        let mod_label = format!("{}-{}", spec.dev, spec.name);

        if !updatable_mods.contains(&mod_label) {
            continue;
        }

        on_progress(
            installed,
            total_mods,
            Some(format!("Resolving {mod_label}")),
        );

        let key = (spec.dev.to_lowercase(), spec.name.to_lowercase());
        let Some(pkg) = package_map.get(&key) else {
            installed = installed.saturating_add(1);
            log::error!("Package not found in list: {}-{}", spec.dev, spec.name);
            on_progress(
                installed,
                total_mods,
                Some(format!(
                    "Failed to resolve {mod_label} (not found in package list)"
                )),
            );
            continue;
        };

        let pinned = spec.pinned_version_for(game_version);
        let ver = if let Some(pin) = pinned {
            if pkg.versions.iter().any(|v| v.version_number == pin) {
                pin.to_string()
            } else {
                log::warn!(
                    "Pinned version not found for {mod_label}: {pin} (falling back to latest)"
                );
                latest_pkg_version(&pkg.versions)
                    .map(|v| v.version_number.clone())
                    .unwrap_or_else(|| "0.0.0".to_string())
            }
        } else {
            latest_pkg_version(&pkg.versions)
                .map(|v| v.version_number.clone())
                .unwrap_or_else(|| "0.0.0".to_string())
        };

        if ver == "0.0.0" {
            installed = installed.saturating_add(1);
            log::error!("No versions for {}-{}", spec.dev, spec.name);
            on_progress(
                installed,
                total_mods,
                Some(format!("Failed to resolve {mod_label} (no versions)")),
            );
            continue;
        }

        let download_url = thunderstore_download_url(&spec.dev, &spec.name, &ver);
        log::info!("Resolved {mod_label} => v{ver}");

        let zip_path = temp_root.join(format!("{}-{}-{}.zip", spec.dev, spec.name, ver));

        // Download zip
        on_progress(
            installed,
            total_mods,
            Some(format!("Downloading {mod_label}")),
        );
        log::info!("Downloading {mod_label} from {download_url}");
        let bytes = client
            .get(&download_url)
            .send()
            .await
            .map_err(|e| e.to_string())?
            .error_for_status()
            .map_err(|e| e.to_string())?
            .bytes()
            .await
            .map_err(|e| e.to_string())?;

        std::fs::write(&zip_path, &bytes).map_err(|e| e.to_string())?;

        // Extract directly into BepInEx/plugins, then delete the zip.
        on_progress(
            installed,
            total_mods,
            Some(format!("Extracting {mod_label}")),
        );
        let folder_name = format!("{}-{}", spec.dev, spec.name);
        let existing = target_plugins.join(&folder_name);
        if existing.exists() {
            if let Err(e) = std::fs::remove_dir_all(&existing) {
                log::warn!(
                    "Failed to remove existing mod folder {}: {}",
                    existing.to_string_lossy(),
                    e
                );
            }
        }

        if let Err(e) = extract_thunderstore_into_plugins_with_progress(
            &zip_path,
            &target_plugins,
            &folder_name,
            |_d, _t, _n| {},
        ) {
            installed = installed.saturating_add(1);
            log::error!("Failed to extract into plugins {mod_label}: {e}");
            on_progress(
                installed,
                total_mods,
                Some(format!("Failed to extract {mod_label} ({e})")),
            );
            let _ = std::fs::remove_file(&zip_path);
            continue;
        }

        // Cleanup per-mod artifacts
        if let Err(e) = std::fs::remove_file(&zip_path) {
            log::warn!("Failed to delete zip {}: {}", zip_path.to_string_lossy(), e);
        }

        installed = installed.saturating_add(1);
        on_progress(
            installed,
            total_mods,
            Some(format!("Installed {mod_label}")),
        );
    }

    // Best-effort cleanup of temp workspace.
    let _ = std::fs::remove_dir_all(&temp_root);

    Ok(())
}

fn incompatible_reason(spec: &ModEntry, game_version: u32) -> String {
    let mut parts: Vec<String> = vec![];
    if let Some(min) = spec.low_cap {
        if game_version < min {
            parts.push(format!(" (requires >= {min})"));
        }
    }
    if let Some(max) = spec.high_cap {
        if game_version > max {
            parts.push(format!(" (requires <= {max})"));
        }
    }
    parts.join("")
}

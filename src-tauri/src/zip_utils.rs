use std::fs::File;

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};
use zip::ZipArchive;

fn is_bepinex_section_name(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "bepinex" | "plugins" | "plugin" | "patchers" | "patcher" | "config"
    )
}

fn strip_prefix_components<'a>(
    comps: &'a [std::path::Component<'a>],
    prefix: &[&str],
) -> Option<usize> {
    if comps.len() < prefix.len() {
        return None;
    }
    for (i, p) in prefix.iter().enumerate() {
        if comps[i].as_os_str() != *p {
            return None;
        }
    }
    Some(prefix.len())
}

/// Extracts a zip to `dest_dir`, emitting progress as `(done_entries, total_entries, detail)`.
///
/// This uses `enclosed_name()` to prevent Zip Slip (path traversal).
pub fn extract_zip_with_progress<F>(
    zip_path: &std::path::Path,
    dest_dir: &std::path::Path,
    mut on_progress: F,
) -> Result<(), String>
where
    F: FnMut(u64, u64, Option<String>),
{
    let file = File::open(zip_path).map_err(|e| e.to_string())?;
    let mut archive = ZipArchive::new(file).map_err(|e| e.to_string())?;

    let total_files = archive.len() as u64;
    let mut extracted: u64 = 0;
    on_progress(0, total_files, Some("Starting...".to_string()));

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
        let entry_name = Some(entry.name().to_string());

        // Prevent Zip Slip (path traversal). Skip unsafe paths.
        let Some(safe_rel) = entry.enclosed_name().map(|p| p.to_owned()) else {
            extracted = extracted.saturating_add(1);
            on_progress(
                extracted,
                total_files,
                Some("Skipped unsafe path".to_string()),
            );
            continue;
        };

        let out_path = dest_dir.join(safe_rel);

        if entry.is_dir() {
            std::fs::create_dir_all(&out_path).map_err(|e| e.to_string())?;
            extracted = extracted.saturating_add(1);
            on_progress(extracted, total_files, entry_name);
            continue;
        }

        if let Some(parent) = out_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }

        let mut out_file = File::create(&out_path).map_err(|e| e.to_string())?;
        std::io::copy(&mut entry, &mut out_file).map_err(|e| e.to_string())?;

        extracted = extracted.saturating_add(1);
        on_progress(extracted, total_files, entry_name);
    }

    Ok(())
}

/// Extract a config zip into a `BepInEx/config` directory.
///
/// The zip may contain:
/// - plain config files (directly)
/// - `config/**`
/// - `BepInEx/config/**`
///
/// This function strips those prefixes if present to avoid nesting like
/// `BepInEx/config/BepInEx/config/...`.
pub fn extract_config_zip_into_bepinex_config_with_progress<F>(
    zip_path: &Path,
    config_dir: &Path,
    mut on_progress: F,
) -> Result<(), String>
where
    F: FnMut(u64, u64, Option<String>),
{
    #[cfg(target_os = "android")]
    crate::android_mods::validate_archive(zip_path)?;
    let file = File::open(zip_path).map_err(|e| e.to_string())?;
    let mut archive = ZipArchive::new(file).map_err(|e| e.to_string())?;

    let total_entries = archive.len() as u64;
    let mut processed: u64 = 0;
    on_progress(0, total_entries, Some("Starting...".to_string()));

    std::fs::create_dir_all(config_dir).map_err(|e| e.to_string())?;

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
        let entry_name = Some(entry.name().to_string());

        let Some(safe_rel) = entry.enclosed_name().map(|p| p.to_owned()) else {
            processed = processed.saturating_add(1);
            on_progress(
                processed,
                total_entries,
                Some("Skipped unsafe path".to_string()),
            );
            continue;
        };

        // Strip optional prefixes.
        let comps: Vec<_> = safe_rel.components().collect();
        let mut start = 0usize;
        if let Some(s) = strip_prefix_components(&comps, &["BepInEx", "config"]) {
            start = s;
        } else if let Some(s) = strip_prefix_components(&comps, &["config"]) {
            start = s;
        }

        let rel_path: PathBuf = comps[start..].iter().collect();
        if rel_path.as_os_str().is_empty() {
            processed = processed.saturating_add(1);
            on_progress(processed, total_entries, entry_name);
            continue;
        }

        let out_path = config_dir.join(rel_path);

        if entry.is_dir() {
            std::fs::create_dir_all(&out_path).map_err(|e| e.to_string())?;
            processed = processed.saturating_add(1);
            on_progress(processed, total_entries, entry_name);
            continue;
        }

        // Add-only: do not overwrite existing config files.
        if out_path.exists() {
            processed = processed.saturating_add(1);
            on_progress(
                processed,
                total_entries,
                Some("Skipped existing file".to_string()),
            );
            continue;
        }

        if let Some(parent) = out_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }

        let mut out_file = File::create(&out_path).map_err(|e| e.to_string())?;
        std::io::copy(&mut entry, &mut out_file).map_err(|e| e.to_string())?;

        processed = processed.saturating_add(1);
        on_progress(processed, total_entries, entry_name);
    }

    Ok(())
}

/// Extracts a Thunderstore package zip into `dest_dir`.
///
/// Thunderstore packages usually include a single top-level folder (like `BepInExPack/`),
/// and also contain top-level files like `manifest.json` and `icon.png`. This function:
///
/// - ignores top-level files
/// - strips the top-level directory
/// - prevents Zip Slip via `enclosed_name()`
#[allow(dead_code)]
pub fn extract_thunderstore_package_with_progress<F>(
    zip_path: &std::path::Path,
    dest_dir: &std::path::Path,
    mut on_progress: F,
) -> Result<(), String>
where
    F: FnMut(u64, u64, Option<String>),
{
    #[allow(dead_code)]
    #[cfg(target_os = "android")]
    crate::android_mods::validate_archive(zip_path)?;
    let file = File::open(zip_path).map_err(|e| e.to_string())?;
    let mut archive = ZipArchive::new(file).map_err(|e| e.to_string())?;

    let total_entries = archive.len() as u64;
    let mut processed: u64 = 0;
    on_progress(0, total_entries, Some("Starting...".to_string()));

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
        let entry_name = Some(entry.name().to_string());

        let Some(safe_rel) = entry.enclosed_name().map(|p| p.to_owned()) else {
            processed = processed.saturating_add(1);
            on_progress(
                processed,
                total_entries,
                Some("Skipped unsafe path".to_string()),
            );
            continue;
        };

        // Ignore top-level files (manifest.json, icon.png, README, etc)
        if safe_rel.components().count() == 1 {
            processed = processed.saturating_add(1);
            on_progress(processed, total_entries, entry_name);
            continue;
        }

        // Strip the first component (top-level dir)
        let mut components = safe_rel.components();
        components.next();
        let relative = components.as_path();

        let out_path = dest_dir.join(relative);

        // An updated runtime must preserve launcher/user configuration.
        if out_path.is_file() && out_path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("ini") || ext.eq_ignore_ascii_case("cfg")) {
            processed = processed.saturating_add(1);
            on_progress(processed, total_entries, entry_name);
            continue;
        }

        if entry.is_dir() {
            std::fs::create_dir_all(&out_path).map_err(|e| e.to_string())?;
            processed = processed.saturating_add(1);
            on_progress(processed, total_entries, entry_name);
            continue;
        }

        if let Some(parent) = out_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }

        let mut out_file = File::create(&out_path).map_err(|e| e.to_string())?;
        std::io::copy(&mut entry, &mut out_file).map_err(|e| e.to_string())?;

        processed = processed.saturating_add(1);
        on_progress(processed, total_entries, entry_name);
    }

    Ok(())
}

/// Extract a Thunderstore mod zip into the correct BepInEx locations.
///
/// Behavior (requested):
/// - If the zip contains `plugins/**` (or `BepInEx/plugins/**`), extract those into `BepInEx/plugins/{folder_name}/...`
/// - If the zip contains `patchers/**` (or `BepInEx/patchers/**`), extract those into `BepInEx/patchers/{folder_name}/...`
/// - `config/**` entries are ignored (not extracted)
///
/// Notes:
/// - Uses `enclosed_name()` to prevent Zip Slip.
/// - Flattens redundant top-level directories common in Thunderstore zips.
pub fn extract_thunderstore_into_bepinex_with_progress<F>(
    zip_path: &Path,
    game_root: &Path,
    folder_name: &str,
    cancel: Option<&AtomicBool>,
    mut on_progress: F,
) -> Result<(), String>
where
    F: FnMut(u64, u64, Option<String>),
{
    use std::collections::BTreeSet;

    if folder_name.eq_ignore_ascii_case("BepInEx-BepInExPack") {
        return extract_bepinex_pack(zip_path, game_root, cancel, on_progress);
    }

    let plugins_root = game_root.join("BepInEx").join("plugins");
    let patchers_root = game_root.join("BepInEx").join("patchers");
    let config_root = game_root.join("BepInEx").join("config");
    let allow_config_payload = folder_name.eq_ignore_ascii_case("LeKAKiD-FontPatcher");

    std::fs::create_dir_all(&plugins_root).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&patchers_root).map_err(|e| e.to_string())?;
    if allow_config_payload {
        std::fs::create_dir_all(&config_root).map_err(|e| e.to_string())?;
    }

    #[cfg(target_os = "android")]
    crate::android_mods::validate_archive(zip_path)?;
    let file = File::open(zip_path).map_err(|e| e.to_string())?;
    let mut archive = ZipArchive::new(file).map_err(|e| e.to_string())?;

    let total_entries = archive.len() as u64;
    let mut processed: u64 = 0;
    on_progress(0, total_entries, Some("Starting...".to_string()));

    // Decide whether to strip a single wrapper directory (common in zips).
    let mut first_components: BTreeSet<String> = BTreeSet::new();
    let mut has_root_file = false;
    let mut has_patchers_payload = false;
    for i in 0..archive.len() {
        let Ok(entry) = archive.by_index(i) else {
            continue;
        };
        let Some(safe_rel) = entry.enclosed_name().map(|p| p.to_owned()) else {
            continue;
        };
        let comps: Vec<_> = safe_rel.components().collect();
        if comps.is_empty() {
            continue;
        }
        if comps.len() == 1 && !entry.is_dir() {
            has_root_file = true;
        }
        first_components.insert(comps[0].as_os_str().to_string_lossy().to_string());
        // Quick detect patchers payload to decide whether to reset patchers folder.
        let lower = safe_rel.to_string_lossy().to_lowercase();
        if lower.starts_with("patchers/")
            || lower.starts_with("patcher/")
            || lower.starts_with("bepinex/patchers/")
            || lower.starts_with("bepinex/patcher/")
            || lower.contains("/patchers/")
            || lower.contains("\\patchers\\")
            || lower.contains("/patcher/")
            || lower.contains("\\patcher\\")
        {
            has_patchers_payload = true;
        }
    }
    let flatten_prefix: Option<String> = if !has_root_file && first_components.len() == 1 {
        let only = first_components.iter().next().cloned();
        only.filter(|name| !is_bepinex_section_name(name))
    } else {
        None
    };

    if cancel.is_some_and(|c| c.load(AtomicOrdering::Relaxed)) {
        return Err("Cancelled".to_string());
    }

    // Reset per-mod plugin folder always (install/update semantics).
    let plugin_dest_dir = plugins_root.join(folder_name);
    if plugin_dest_dir.exists() {
        std::fs::remove_dir_all(&plugin_dest_dir).map_err(|e| e.to_string())?;
    }
    std::fs::create_dir_all(&plugin_dest_dir).map_err(|e| e.to_string())?;

    // Replace the whole owned payload, including patchers removed by a newer release.
    let patcher_dest_dir = patchers_root.join(folder_name);
    if patcher_dest_dir.exists() {
        std::fs::remove_dir_all(&patcher_dest_dir).map_err(|e| e.to_string())?;
    }
    if has_patchers_payload {
        if cancel.is_some_and(|c| c.load(AtomicOrdering::Relaxed)) {
            return Err("Cancelled".to_string());
        }
        let _ = std::fs::create_dir_all(&patcher_dest_dir);
    }

    let folder_lower = folder_name.to_lowercase();

    for i in 0..archive.len() {
        if cancel.is_some_and(|c| c.load(AtomicOrdering::Relaxed)) {
            return Err("Cancelled".to_string());
        }
        let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
        let entry_name = Some(entry.name().to_string());

        let Some(safe_rel0) = entry.enclosed_name().map(|p| p.to_owned()) else {
            processed = processed.saturating_add(1);
            on_progress(
                processed,
                total_entries,
                Some("Skipped unsafe path".to_string()),
            );
            continue;
        };

        // Flatten wrapper dir if needed.
        let comps0: Vec<_> = safe_rel0.components().collect();
        let safe_rel: PathBuf = if let Some(prefix) = flatten_prefix.as_deref() {
            if comps0.len() > 1
                && comps0
                    .first()
                    .is_some_and(|c| c.as_os_str().to_string_lossy().eq(prefix))
            {
                comps0[1..].iter().collect()
            } else {
                safe_rel0.clone()
            }
        } else {
            safe_rel0.clone()
        };

        let comps: Vec<_> = safe_rel.components().collect();
        if comps.is_empty() {
            processed = processed.saturating_add(1);
            on_progress(processed, total_entries, entry_name);
            continue;
        }

        // Strip optional leading `BepInEx/`.
        let mut start = 0usize;
        if comps.len() >= 2 && comps[0].as_os_str() == "BepInEx" {
            start = 1;
        }

        // Determine section (plugins/config/patchers) by first component after optional strip.
        let head = comps
            .get(start)
            .map(|c| c.as_os_str().to_string_lossy().to_lowercase());
        let section = head.as_deref().unwrap_or("");

        let dest_base = match section {
            "plugins" | "plugin" => plugin_dest_dir.clone(),
            "patchers" | "patcher" => patcher_dest_dir.clone(),
            "config" => {
                if allow_config_payload {
                    config_root.clone()
                } else {
                    // Explicitly ignore config payloads.
                    processed = processed.saturating_add(1);
                    on_progress(
                        processed,
                        total_entries,
                        Some("Skipped config entry".to_string()),
                    );
                    continue;
                }
            }
            _ => plugin_dest_dir.clone(), // fallback
        };

        // Build relative path after stripping section prefix when present.
        let mut rel_comps: Vec<_> = comps[start..].to_vec();
        if matches!(
            section,
            "plugins" | "plugin" | "patchers" | "patcher" | "config"
        ) {
            rel_comps = rel_comps[1..].to_vec();
        }

        // If the payload includes the mod folder name again, strip it.
        if rel_comps.len() > 1
            && rel_comps
                .first()
                .is_some_and(|c| c.as_os_str().to_string_lossy().to_lowercase() == folder_lower)
        {
            rel_comps = rel_comps[1..].to_vec();
        }

        let rel_path: PathBuf = rel_comps.iter().collect();
        if rel_path.as_os_str().is_empty() {
            processed = processed.saturating_add(1);
            on_progress(processed, total_entries, entry_name);
            continue;
        }

        let mut out_path = dest_base.join(rel_path);
        if out_path == plugin_dest_dir.join("manifest.json") {
            // A current manifest is the install receipt: publish it only after every payload succeeds.
            out_path = plugin_dest_dir.join("manifest.json.installing");
        }

        if entry.is_dir() {
            std::fs::create_dir_all(&out_path).map_err(|e| e.to_string())?;
            processed = processed.saturating_add(1);
            on_progress(processed, total_entries, entry_name);
            continue;
        }

        if let Some(parent) = out_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }

        // Write via a temporary sibling file so concurrent manifest scans never
        // observe an empty or partially-written JSON file mid-extraction.
        let file_name = out_path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| format!("invalid output path: {}", out_path.to_string_lossy()))?;
        let tmp_path = out_path.with_file_name(format!("{file_name}.hqtmp"));

        let mut out_file = File::create(&tmp_path).map_err(|e| e.to_string())?;
        let mut buf = [0u8; 64 * 1024];
        loop {
            if cancel.is_some_and(|c| c.load(AtomicOrdering::Relaxed)) {
                return Err("Cancelled".to_string());
            }
            let n = entry.read(&mut buf).map_err(|e| e.to_string())?;
            if n == 0 {
                break;
            }
            out_file.write_all(&buf[..n]).map_err(|e| e.to_string())?;
        }
        drop(out_file);
        std::fs::rename(&tmp_path, &out_path).map_err(|e| e.to_string())?;

        processed = processed.saturating_add(1);
        on_progress(processed, total_entries, entry_name);
    }

    if cancel.is_some_and(|c| c.load(AtomicOrdering::Relaxed)) { return Err("Cancelled".into()); }
    let pending_manifest = plugin_dest_dir.join("manifest.json.installing");
    if pending_manifest.exists() {
        std::fs::rename(pending_manifest, plugin_dest_dir.join("manifest.json")).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn extract_bepinex_pack<F>(zip_path: &Path, game_root: &Path, cancel: Option<&AtomicBool>, on_progress: F) -> Result<(), String>
where F: FnMut(u64, u64, Option<String>) {
    if cancel.is_some_and(|c| c.load(AtomicOrdering::Relaxed)) { return Err("Cancelled".into()); }
    let mut archive = ZipArchive::new(File::open(zip_path).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    let mut current = Vec::<PathBuf>::new();
    for i in 0..archive.len() {
        let entry = archive.by_index(i).map_err(|e| e.to_string())?;
        let Some(path) = entry.enclosed_name() else { continue; };
        if entry.is_dir() { continue; }
        let mut parts = path.components();
        if parts.next().is_some_and(|p| p.as_os_str() == "BepInExPack") {
            let relative = parts.as_path();
            if relative.starts_with("BepInEx/core") && relative.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("dll")) {
                current.push(relative.to_owned());
            }
        }
    }
    let tracking = game_root.join(".hq-launcher").join("bepinexpack-core-files.json");
    let previous: Vec<PathBuf> = std::fs::read(&tracking).ok().and_then(|bytes| serde_json::from_slice(&bytes).ok()).unwrap_or_default();
    let metadata = game_root.join("BepInEx/plugins/BepInEx-BepInExPack");
    for name in ["manifest.json", "manifest.json.old"] {
        let marker = metadata.join(name);
        if marker.is_file() { std::fs::remove_file(marker).map_err(|e| e.to_string())?; }
    }
    extract_thunderstore_package_with_progress(zip_path, game_root, on_progress)?;
    if cancel.is_some_and(|c| c.load(AtomicOrdering::Relaxed)) { return Err("Cancelled".into()); }
    // Only recorded core DLLs belong to this runtime; never remove game/config/save files.
    for old in previous {
        if old.components().all(|part| matches!(part, std::path::Component::Normal(_)))
            && old.starts_with("BepInEx/core") && old.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("dll")) && !current.contains(&old) {
            let path = game_root.join(old);
            if path.is_file() { std::fs::remove_file(path).map_err(|e| e.to_string())?; }
        }
    }
    std::fs::create_dir_all(tracking.parent().unwrap()).map_err(|e| e.to_string())?;
    std::fs::write(tracking, serde_json::to_vec(&current).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    // The regular package installer uses per-package manifest metadata for version checks.
    if let Ok(mut manifest) = archive.by_name("manifest.json") {
        std::fs::create_dir_all(&metadata).map_err(|e| e.to_string())?;
        let mut out = File::create(metadata.join("manifest.json")).map_err(|e| e.to_string())?;
        std::io::copy(&mut manifest, &mut out).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod preset_archive_tests {
    use super::*;
    use std::io::Write;
    fn workspace() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("hq-preset-{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir_all(&dir).unwrap(); dir
    }
    fn zip_at(path: &Path, entries: &[(&str, &str)]) {
        let mut zip = zip::ZipWriter::new(File::create(path).unwrap());
        for (name, contents) in entries {
            zip.start_file(*name, zip::write::SimpleFileOptions::default()).unwrap();
            zip.write_all(contents.as_bytes()).unwrap();
        }
        zip.finish().unwrap();
    }
    #[test]
    fn cancelled_install_never_commits_latest_metadata_and_retry_installs_payload() {
        let dir = workspace(); let archive = dir.join("mod.zip");
        zip_at(&archive, &[("manifest.json", "{\"version_number\":\"2.0.0\"}"), ("plugins/new.dll", "new")]);
        let cancel = AtomicBool::new(false);
        let result = extract_thunderstore_into_bepinex_with_progress(&archive, &dir, "Owner-Mod", Some(&cancel), |_,_,name| {
            if name.as_deref() == Some("manifest.json") { cancel.store(true, AtomicOrdering::Relaxed); }
        });
        assert!(result.is_err());
        assert!(!dir.join("BepInEx/plugins/Owner-Mod/manifest.json").exists(), "partial install cannot look current");
        extract_thunderstore_into_bepinex_with_progress(&archive, &dir, "Owner-Mod", None, |_,_,_| {}).unwrap();
        assert!(dir.join("BepInEx/plugins/Owner-Mod/manifest.json").exists());
        assert!(dir.join("BepInEx/plugins/Owner-Mod/new.dll").exists());
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn update_removes_old_plugins_and_patchers_without_touching_configs_or_saves() {
        let dir = workspace(); let archive = dir.join("mod.zip");
        zip_at(&archive, &[("plugins/old.dll", "old"), ("patchers/old.dll", "old")]);
        extract_thunderstore_into_bepinex_with_progress(&archive, &dir, "Owner-Mod", None, |_,_,_| {}).unwrap();
        std::fs::create_dir_all(dir.join("BepInEx/config")).unwrap();
        std::fs::write(dir.join("BepInEx/config/user.cfg"), "custom").unwrap();
        std::fs::write(dir.join("save.dat"), "saved").unwrap();
        zip_at(&archive, &[("plugins/new.dll", "new")]);
        extract_thunderstore_into_bepinex_with_progress(&archive, &dir, "Owner-Mod", None, |_,_,_| {}).unwrap();
        assert!(!dir.join("BepInEx/plugins/Owner-Mod/old.dll").exists());
        assert!(!dir.join("BepInEx/patchers/Owner-Mod/old.dll").exists());
        assert_eq!(std::fs::read_to_string(dir.join("BepInEx/config/user.cfg")).unwrap(), "custom");
        assert_eq!(std::fs::read_to_string(dir.join("save.dat")).unwrap(), "saved");
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn bepinex_pack_installs_into_game_root_preserving_settings_and_removing_obsolete_core() {
        let dir = workspace(); let archive = dir.join("pack.zip");
        zip_at(&archive, &[("manifest.json", "{}"), ("BepInExPack/BepInEx/core/old.dll", "old"), ("BepInExPack/winhttp.dll", "proxy"), ("BepInExPack/doorstop_config.ini", "default")]);
        extract_thunderstore_into_bepinex_with_progress(&archive, &dir, "BepInEx-BepInExPack", None, |_,_,_| {}).unwrap();
        assert!(dir.join("BepInEx/core/old.dll").exists());
        assert!(dir.join("winhttp.dll").exists());
        std::fs::write(dir.join("doorstop_config.ini"), "user settings").unwrap();
        zip_at(&archive, &[("manifest.json", "{}"), ("BepInExPack/BepInEx/core/new.dll", "new"), ("BepInExPack/doorstop_config.ini", "changed")]);
        extract_thunderstore_into_bepinex_with_progress(&archive, &dir, "BepInEx-BepInExPack", None, |_,_,_| {}).unwrap();
        assert!(!dir.join("BepInEx/core/old.dll").exists());
        assert!(dir.join("BepInEx/core/new.dll").exists());
        assert_eq!(std::fs::read_to_string(dir.join("doorstop_config.ini")).unwrap(), "user settings");
        std::fs::remove_dir_all(dir).unwrap();
    }
}

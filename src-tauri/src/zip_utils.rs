use std::fs::File;

use std::path::{Path, PathBuf};
use zip::ZipArchive;

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

/// Extract a Thunderstore mod zip into a subfolder under `BepInEx/plugins`.
///
/// User-requested behavior:
/// - create `BepInEx/plugins/{folder_name}/`
/// - extract the zip into that folder (so you get `.../{folder_name}/<zip contents...>`)
/// - BUT if the zip contains `BepInEx/plugins/**` or `plugins/**` anywhere in its path,
///   strip that prefix so the actual plugin payload lands under `{folder_name}/`.
/// - prevents Zip Slip via `enclosed_name()`
pub fn extract_thunderstore_into_plugins_with_progress<F>(
    zip_path: &Path,
    plugins_dir: &Path,
    folder_name: &str,
    mut on_progress: F,
) -> Result<(), String>
where
    F: FnMut(u64, u64, Option<String>),
{
    let file = File::open(zip_path).map_err(|e| e.to_string())?;
    let mut archive = ZipArchive::new(file).map_err(|e| e.to_string())?;

    let total_entries = archive.len() as u64;
    let mut processed: u64 = 0;
    on_progress(0, total_entries, Some("Starting...".to_string()));

    let base_dir = plugins_dir.join(folder_name);
    let _ = std::fs::remove_dir_all(&base_dir).map_err(|e| e.to_string());

    std::fs::create_dir_all(&base_dir).map_err(|e| e.to_string())?;

    log::info!(
        "Extracting Thunderstore mod zip into: {}",
        base_dir.to_string_lossy()
    );

    // Many Thunderstore zips contain a single top-level directory (or even
    // `BepInEx/plugins/<mod_folder>/...`). Since `base_dir` is already the mod folder,
    // we should flatten that redundant top-level directory to avoid nesting like:
    // `BepInEx/plugins/Dev-Mod/Dev-Mod/manifest.json`.
    let mut first_components: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    let mut has_root_file = false;
    for i in 0..archive.len() {
        let Ok(entry) = archive.by_index(i) else { continue };
        let Some(safe_rel) = entry.enclosed_name().map(|p| p.to_owned()) else {
            continue;
        };
        let comps: Vec<_> = safe_rel.components().collect();
        let mut start_at: Option<usize> = None;
        for idx in 0..comps.len().saturating_sub(1) {
            if comps[idx].as_os_str() == "BepInEx" && comps[idx + 1].as_os_str() == "plugins" {
                start_at = Some(idx + 2);
                break;
            }
        }
        if start_at.is_none() {
            for idx in 0..comps.len() {
                if comps[idx].as_os_str() == "plugins" {
                    start_at = Some(idx + 1);
                    break;
                }
            }
        }
        let rel_path: PathBuf = if let Some(start) = start_at {
            comps[start..].iter().collect()
        } else {
            safe_rel.clone()
        };
        let rel_comps: Vec<_> = rel_path.components().collect();
        if rel_comps.is_empty() {
            continue;
        }
        if rel_comps.len() == 1 && !entry.is_dir() {
            has_root_file = true;
        }
        if let Some(first) = rel_comps.first() {
            let s = first.as_os_str().to_string_lossy().to_string();
            if !s.is_empty() {
                first_components.insert(s);
            }
        }
    }
    let flatten_prefix: Option<String> = if !has_root_file && first_components.len() == 1 {
        first_components.iter().next().cloned()
    } else {
        None
    };

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
        let entry_name = Some(entry.name().to_string());

        let Some(safe_rel) = entry.enclosed_name().map(|p| p.to_owned()) else {
            log::error!("Skipped unsafe path: {}", entry.name());
            processed = processed.saturating_add(1);
            on_progress(
                processed,
                total_entries,
                Some("Skipped unsafe path".to_string()),
            );
            continue;
        };

        // Build mapped path under base_dir.
        // If the path contains ".../BepInEx/plugins/..." or ".../plugins/...",
        // strip everything up to that point.
        let comps: Vec<_> = safe_rel.components().collect();
        let mut start_at: Option<usize> = None;

        // Find "BepInEx/plugins" sequence anywhere in the path.
        for idx in 0..comps.len().saturating_sub(1) {
            if comps[idx].as_os_str() == "BepInEx" && comps[idx + 1].as_os_str() == "plugins" {
                start_at = Some(idx + 2);
                break;
            }
        }

        // If not found, find "plugins" component anywhere and strip up to it.
        if start_at.is_none() {
            for idx in 0..comps.len() {
                if comps[idx].as_os_str() == "plugins" {
                    start_at = Some(idx + 1);
                    break;
                }
            }
        }

        let rel_path: PathBuf = if let Some(start) = start_at {
            comps[start..].iter().collect()
        } else {
            // Preserve original relative path (including its top-level folder),
            // but nest it under the requested base dir.
            safe_rel.clone()
        };

        // Flatten redundant top-level directory (common in Thunderstore zips).
        let mut rel_comps: Vec<_> = rel_path.components().collect();
        if let Some(prefix) = flatten_prefix.as_deref() {
            if rel_comps.len() > 1
                && rel_comps
                    .first()
                    .is_some_and(|c| c.as_os_str().to_string_lossy().eq(prefix))
            {
                rel_comps = rel_comps[1..].to_vec();
            }
        }
        // Also strip if the zip already includes the target folder_name as its top-level dir.
        let folder_lower = folder_name.to_lowercase();
        if rel_comps.len() > 1
            && rel_comps.first().is_some_and(|c| {
                c.as_os_str()
                    .to_string_lossy()
                    .to_lowercase()
                    == folder_lower
            })
        {
            rel_comps = rel_comps[1..].to_vec();
        }
        let rel_path: PathBuf = rel_comps.iter().collect();

        if rel_path.as_os_str().is_empty() {
            processed = processed.saturating_add(1);
            on_progress(processed, total_entries, entry_name);
            continue;
        }

        let out_path = base_dir.join(rel_path);

        if entry.is_dir() {
            std::fs::create_dir_all(&out_path).map_err(|e| e.to_string())?;
            processed = processed.saturating_add(1);
            on_progress(processed, total_entries, entry_name);
            continue;
        }

        // Add-only: do not overwrite existing plugin files.
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
    mut on_progress: F,
) -> Result<(), String>
where
    F: FnMut(u64, u64, Option<String>),
{
    use std::collections::BTreeSet;

    let plugins_root = game_root.join("BepInEx").join("plugins");
    let patchers_root = game_root.join("BepInEx").join("patchers");

    std::fs::create_dir_all(&plugins_root).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&patchers_root).map_err(|e| e.to_string())?;

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
        let Ok(entry) = archive.by_index(i) else { continue };
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
        if lower.contains("/patchers/") || lower.contains("\\patchers\\") || lower.contains("/patcher/") {
            has_patchers_payload = true;
        }
    }
    let flatten_prefix: Option<String> = if !has_root_file && first_components.len() == 1 {
        first_components.iter().next().cloned()
    } else {
        None
    };

    // Reset per-mod plugin folder always (install/update semantics).
    let plugin_dest_dir = plugins_root.join(folder_name);
    let _ = std::fs::remove_dir_all(&plugin_dest_dir);
    std::fs::create_dir_all(&plugin_dest_dir).map_err(|e| e.to_string())?;

    // Reset per-mod patchers folder only if zip contains patchers payload.
    let patcher_dest_dir = patchers_root.join(folder_name);
    if has_patchers_payload {
        let _ = std::fs::remove_dir_all(&patcher_dest_dir);
        let _ = std::fs::create_dir_all(&patcher_dest_dir);
    }

    let folder_lower = folder_name.to_lowercase();

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
        let entry_name = Some(entry.name().to_string());

        let Some(safe_rel0) = entry.enclosed_name().map(|p| p.to_owned()) else {
            processed = processed.saturating_add(1);
            on_progress(processed, total_entries, Some("Skipped unsafe path".to_string()));
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
        let head = comps.get(start).map(|c| c.as_os_str().to_string_lossy().to_lowercase());
        let section = head.as_deref().unwrap_or("");

        let dest_base = match section {
            "plugins" | "plugin" => plugin_dest_dir.clone(),
            "patchers" | "patcher" => patcher_dest_dir.clone(),
            "config" => {
                // Explicitly ignore config payloads.
                processed = processed.saturating_add(1);
                on_progress(processed, total_entries, Some("Skipped config entry".to_string()));
                continue;
            }
            _ => plugin_dest_dir.clone(), // fallback
        };

        // Build relative path after stripping section prefix when present.
        let mut rel_comps: Vec<_> = comps[start..].to_vec();
        if matches!(section, "plugins" | "plugin" | "patchers" | "patcher" | "config") {
            rel_comps = rel_comps[1..].to_vec();
        }

        // If the payload includes the mod folder name again, strip it.
        if rel_comps.len() > 1
            && rel_comps.first().is_some_and(|c| {
                c.as_os_str()
                    .to_string_lossy()
                    .to_lowercase()
                    == folder_lower
            })
        {
            rel_comps = rel_comps[1..].to_vec();
        }

        let rel_path: PathBuf = rel_comps.iter().collect();
        if rel_path.as_os_str().is_empty() {
            processed = processed.saturating_add(1);
            on_progress(processed, total_entries, entry_name);
            continue;
        }

        let out_path = dest_base.join(rel_path);

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

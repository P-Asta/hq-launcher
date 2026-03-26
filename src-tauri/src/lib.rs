mod bepinex_cfg;
mod discord_presence;
mod downloader;
mod installer;
mod logger;
mod mod_config;
mod mods;
mod progress;
mod thunderstore;
mod variable;
mod zip_utils;

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::sync::Mutex;
use tauri::{Manager, State};

use crate::bepinex_cfg::read_manifest;
use crate::progress::{TaskErrorPayload, TaskProgressPayload};
use crate::{
    mod_config::ModsConfig,
    progress::{TaskFinishedPayload, TaskUpdatableProgressPayload},
};

fn preset_tags_for_name(preset: &str) -> Vec<String> {
    let p = preset.trim().to_lowercase();
    match p.as_str() {
        "brutal" | "bc" => vec!["Brutal".to_string()],
        "wesley" | "wesley's" | "wesleys" => vec!["Wesley".to_string()],
        "smhq" => vec!["SMHQ".to_string()],
        "wesley_smhq" | "wesleys_smhq" | "wesley-smhq" | "wesleys-smhq" => {
            vec!["Wesley".to_string(), "SMHQ".to_string()]
        }
        _ => vec![],
    }
}

fn preset_and_practice_for_run_mode(run_mode: &str) -> (String, bool) {
    let mode = run_mode.trim().to_lowercase();
    match mode.as_str() {
        "practice" => ("hq".to_string(), true),
        "brutal" => ("brutal".to_string(), false),
        "brutal_practice" => ("brutal".to_string(), true),
        "wesley" => ("wesley".to_string(), false),
        "wesley_practice" => ("wesley".to_string(), true),
        "wesley_smhq" => ("wesley_smhq".to_string(), false),
        "smhq" => ("smhq".to_string(), false),
        _ => ("hq".to_string(), false),
    }
}

fn merge_mod_entries_prefer_later(
    base: Vec<mod_config::ModEntry>,
    overlay: Vec<mod_config::ModEntry>,
) -> Vec<mod_config::ModEntry> {
    let overlay_names: std::collections::HashSet<String> = overlay
        .iter()
        .map(|m| m.name.to_lowercase())
        .collect();

    let mut merged: Vec<mod_config::ModEntry> = base
        .into_iter()
        .filter(|m| !overlay_names.contains(&m.name.to_lowercase()))
        .collect();
    merged.extend(overlay);
    merged
}

fn is_ui_hidden_mod(m: &mod_config::ModEntry) -> bool {
    m.tags.iter().any(|t| t.eq_ignore_ascii_case("ui_hidden"))
}

async fn effective_mods_config_for_run_mode(
    client: &reqwest::Client,
    version: u32,
    run_mode: &str,
    include_ui_hidden: bool,
    include_practice_mods: bool,
) -> Result<ModsConfig, String> {
    let (_remote_manifest_version, mods_cfg, _chain_config, _manifests) =
        ModsConfig::fetch_manifest(client).await?;
    let (preset, practice) = preset_and_practice_for_run_mode(run_mode);
    let tags = preset_tags_for_name(&preset);
    let want: Vec<String> = tags.iter().map(|t| t.to_lowercase()).collect();

    let mut base: Vec<mod_config::ModEntry> = vec![];
    let mut selected_tagged: Vec<mod_config::ModEntry> = vec![];

    for m in mods_cfg.mods {
        if m.tags.is_empty() {
            if m.is_compatible(version) {
                base.push(m);
            }
            continue;
        }

        if want.is_empty() {
            continue;
        }

        let has = m.tags.iter().any(|x| want.contains(&x.to_lowercase()));
        if !has {
            continue;
        }

        let mut mm = m;
        mm.enabled = true;
        if mm.is_compatible_for_tags(version, &tags) {
            selected_tagged.push(mm);
        }
    }

    let mut effective = base;
    if practice {
        let practice_mods: Vec<mod_config::ModEntry> = variable::get_practice_mod_list()
            .into_iter()
            .filter(|m| m.is_compatible(version))
            .collect();
        let practice_names: std::collections::HashSet<String> = practice_mods
            .iter()
            .map(|m| m.name.to_lowercase())
            .collect();
        effective.retain(|m| !practice_names.contains(&m.name.to_lowercase()));
        if include_practice_mods {
            effective.extend(practice_mods);
        }
    }
    if !selected_tagged.is_empty() {
        effective = merge_mod_entries_prefer_later(effective, selected_tagged);
    }

    if !include_ui_hidden {
        effective.retain(|m| !is_ui_hidden_mod(m));
    }

    Ok(ModsConfig { mods: effective })
}

async fn prepare_tagged_mods_for_version(
    app: &tauri::AppHandle,
    version: u32,
    tags: &[String],
    step_name: &str,
    cancel: Option<Arc<AtomicBool>>,
) -> Result<Vec<(String, String)>, String> {
    if tags.is_empty() {
        return Ok(vec![]);
    }
    let game_root = version_dir(app, version)?;
    if !game_root.exists() {
        return Err(format!(
            "version folder not found: {}",
            game_root.to_string_lossy()
        ));
    }

    // Tag-level minimum supported versions (hard rule, avoids accidentally offering presets on too-old game versions).
    let mut min_required: Option<u32> = None;
    for t in tags {
        let tl = t.to_lowercase();
        let req = if tl == "brutal" {
            Some(49)
        } else if tl == "wesley" || tl == "wesley's" {
            Some(69)
        } else {
            None
        };
        if let Some(r) = req {
            min_required = Some(min_required.map(|m| m.max(r)).unwrap_or(r));
        }
    }
    if let Some(min) = min_required {
        if version < min {
            return Err(format!(
                "{} preset requires game version >= v{} (current: v{})",
                tags.join(", "),
                min,
                version
            ));
        }
    }

    let client = reqwest::Client::new();
    let (_remote_manifest_version, mods_cfg, _chain_config, _manifests) =
        ModsConfig::fetch_manifest_with_cancel(&client, cancel.as_ref()).await?;

    let want: Vec<String> = tags.iter().map(|t| t.to_lowercase()).collect();
    let mut tagged: Vec<mod_config::ModEntry> = vec![];
    for m in mods_cfg.mods {
        // Match tags case-insensitively.
        let has = m.tags.iter().any(|x| want.contains(&x.to_lowercase()));
        if !has {
            continue;
        }
        // Override enabled=false for "optional" tagged mods.
        let mut mm = m;
        mm.enabled = true;
        tagged.push(mm);
    }

    // Only install compatible subset (same semantics as practice list).
    let tagged: Vec<mod_config::ModEntry> = tagged
        .into_iter()
        .filter(|m| m.is_compatible_for_tags(version, tags))
        .collect();
    if tagged.is_empty() {
        return Ok(vec![]);
    }

    // Return ids so the caller can force-enable these mods for this run (even if they overlap practice-disable rules).
    let tagged_ids: Vec<(String, String)> = tagged
        .iter()
        .map(|m| (m.dev.clone(), m.name.clone()))
        .collect();

    let missing_tagged_mods = filter_missing_mods_for_version(app, version, &tagged)?;
    if !missing_tagged_mods.is_empty() {
        const STEPS_TOTAL: u32 = 1;
        progress::emit_progress(
            app,
            TaskProgressPayload {
                version,
                steps_total: STEPS_TOTAL,
                step: 1,
                step_name: step_name.to_string(),
                step_progress: 0.0,
                overall_percent: 0.0,
                detail: Some(format!("Installing missing tagged mods: {}", tags.join(", "))),
                downloaded_bytes: None,
                total_bytes: None,
                extracted_files: Some(0),
                total_files: Some(missing_tagged_mods.len() as u64),
            },
        );

        let cfg = ModsConfig {
            mods: missing_tagged_mods,
        };
        mods::install_mods_with_progress(
            app,
            &game_root,
            version,
            &cfg,
            cancel,
            |done, total, progress_info| {
                let step_progress = if total == 0 {
                    1.0
                } else {
                    (done as f64 / total as f64).clamp(0.0, 1.0)
                };
                progress::emit_progress(
                    app,
                    TaskProgressPayload {
                        version,
                        steps_total: STEPS_TOTAL,
                        step: 1,
                        step_name: step_name.to_string(),
                        step_progress,
                        overall_percent: overall_from_step(1, step_progress, STEPS_TOTAL),
                        detail: progress_info.detail,
                        downloaded_bytes: progress_info.downloaded_bytes,
                        total_bytes: progress_info.total_bytes,
                        extracted_files: progress_info.extracted_files.or(Some(done)),
                        total_files: progress_info.total_files.or(Some(total)),
                    },
                );
            },
        )
        .await?;
    }

    Ok(tagged_ids)
}

fn force_enable_mods_for_version(
    app: &tauri::AppHandle,
    version: u32,
    mods_to_enable: &[(String, String)],
) -> Result<(), String> {
    if mods_to_enable.is_empty() {
        return Ok(());
    }

    // Remove from disablemod.json (global source of truth used by UI).
    let mut list = read_disablemod(app)?;
    for (dev, name) in mods_to_enable {
        let id = normalize_mod_id(dev, name);
        list.mods.retain(|m| m != &id);
    }
    // Keep deterministic ordering.
    list.mods
        .sort_by(|a, b| a.dev.cmp(&b.dev).then(a.name.cmp(&b.name)));
    list.mods.dedup();
    write_disablemod(app, &list)?;

    // Apply filesystem state for this version immediately: remove `.old` suffixes.
    let plugins = plugins_dir(app, version)?;
    let patchers = patchers_dir(app, version)?;
    for (dev, name) in mods_to_enable {
        if let Some(dir) = mod_dir_for(&plugins, dev, name) {
            let _ = set_mod_files_old_suffix(&dir, true);
        }
        if let Some(dir) = mod_dir_for(&patchers, dev, name) {
            let _ = set_mod_files_old_suffix(&dir, true);
        }
    }

    Ok(())
}

async fn disable_irrelevant_tagged_mods_for_run(
    app: &tauri::AppHandle,
    version: u32,
    active_tags: &[String],
    protected_ids: &[(String, String)],
) -> Result<(), String> {
    let client = reqwest::Client::new();
    let (_remote_manifest_version, mods_cfg, _chain_config, _manifests) =
        ModsConfig::fetch_manifest(&client).await?;

    let active_lower: Vec<String> = active_tags.iter().map(|t| t.to_lowercase()).collect();
    let protected: std::collections::HashSet<String> = protected_ids
        .iter()
        .map(|(d, n)| format!("{}::{}", d.to_lowercase(), n.to_lowercase()))
        .collect();
    let plugins = plugins_dir(app, version)?;
    let patchers = patchers_dir(app, version)?;

    for m in mods_cfg.mods {
        if m.tags.is_empty() {
            continue;
        }
        // Never disable mods we explicitly prepared (e.g. practice mods) even if their manifest entry is tagged.
        let id = format!("{}::{}", m.dev.to_lowercase(), m.name.to_lowercase());
        if protected.contains(&id) {
            continue;
        }
        let matches_active = m
            .tags
            .iter()
            .any(|t| active_lower.contains(&t.to_lowercase()));

        // If this mod has tags but doesn't match the current run's tags, disable it
        // for this run only (do NOT write to disablemod.json).
        if !matches_active {
            if let Some(dir) = mod_dir_for(&plugins, &m.dev, &m.name) {
                let _ = set_mod_files_old_suffix(&dir, false);
            }
            if let Some(dir) = mod_dir_for(&patchers, &m.dev, &m.name) {
                let _ = set_mod_files_old_suffix(&dir, false);
            }
        }
    }

    Ok(())
}

fn overall_from_step(step: u32, step_progress: f64, steps_total: u32) -> f64 {
    if steps_total == 0 {
        return 0.0;
    }
    let step0 = step.saturating_sub(1) as f64;
    let steps = steps_total as f64;
    (((step0 + step_progress.clamp(0.0, 1.0)) / steps) * 100.0).clamp(0.0, 100.0)
}

#[derive(Debug, Clone, Serialize)]
struct ManifestDto {
    version: u32,
    chain_config: Vec<Vec<String>>,
    mods: Vec<mod_config::ModEntry>,
    manifests: BTreeMap<u32, String>,
}

fn shared_config_dir(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    Ok(app
        .path()
        .app_data_dir()
        .map_err(|e| format!("failed to resolve app data dir: {e}"))?
        .join("config")
        .join("shared"))
}

fn is_safe_rel_path(rel: &std::path::Path) -> bool {
    use std::path::Component;
    rel.components().all(|c| match c {
        Component::Normal(_) => true,
        _ => false, // reject Prefix/RootDir/CurDir/ParentDir
    })
}

fn version_dir(app: &tauri::AppHandle, version: u32) -> Result<std::path::PathBuf, String> {
    Ok(app
        .path()
        .app_data_dir()
        .map_err(|e| format!("failed to resolve app data dir: {e}"))?
        .join("versions")
        .join(format!("v{version}")))
}

fn version_config_dir(app: &tauri::AppHandle, version: u32) -> Result<std::path::PathBuf, String> {
    Ok(version_dir(app, version)?.join("BepInEx").join("config"))
}

fn parse_font_assets_path_from_cfg(cfg_text: &str) -> Option<String> {
    let mut in_path_section = false;

    for raw_line in cfg_text.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
            continue;
        }

        if line.starts_with('[') && line.ends_with(']') {
            in_path_section = line[1..line.len() - 1].trim().eq_ignore_ascii_case("Path");
            continue;
        }

        if !in_path_section {
            continue;
        }

        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        if !key.trim().eq_ignore_ascii_case("FontAssetsPath") {
            continue;
        }

        let value = value.trim().trim_matches('"');
        if !value.is_empty() {
            return Some(value.to_string());
        }
    }

    None
}

fn fontpatcher_assets_dir_for_version(
    app: &tauri::AppHandle,
    version: u32,
) -> Result<std::path::PathBuf, String> {
    let cfg_dir = version_config_dir(app, version)?;
    let default_dir = cfg_dir.join("FontPatcher").join("default");
    let cfg_path = cfg_dir.join("lekakid.lcfontpatcher.cfg");

    if !cfg_path.exists() {
        return Ok(default_dir);
    }

    let cfg_text = std::fs::read_to_string(&cfg_path).map_err(|e| {
        format!(
            "failed to read fontpatcher config {}: {e}",
            cfg_path.to_string_lossy()
        )
    })?;

    let Some(raw_path) = parse_font_assets_path_from_cfg(&cfg_text) else {
        return Ok(default_dir);
    };

    if raw_path.eq_ignore_ascii_case("FontPatcher\\default")
        || raw_path.eq_ignore_ascii_case("FontPatcher/default")
    {
        return Ok(default_dir);
    }

    let rel_path = std::path::Path::new(&raw_path);
    if rel_path.is_absolute() {
        return Ok(rel_path.to_path_buf());
    }

    Ok(cfg_dir.join(rel_path))
}

fn sync_fontpatcher_with_assets_for_version(
    app: &tauri::AppHandle,
    version: u32,
) -> Result<(), String> {
    let assets_dir = fontpatcher_assets_dir_for_version(app, version)?;
    let assets_available = assets_dir.is_dir();
    let disabled = read_disablemod(app)?
        .mods
        .contains(&normalize_mod_id("LeKAKiD", "FontPatcher"));

    let enabled = assets_available && !disabled;
    let plugins = plugins_dir(app, version)?;
    if let Some(dir) = mod_dir_for(&plugins, "LeKAKiD", "FontPatcher") {
        let _ = set_mod_files_old_suffix(&dir, enabled);
    }

    let patchers = patchers_dir(app, version)?;
    if let Some(dir) = mod_dir_for(&patchers, "LeKAKiD", "FontPatcher") {
        let _ = set_mod_files_old_suffix(&dir, enabled);
    }

    Ok(())
}

fn ensure_wesley_moonscripts_cfg(
    app: &tauri::AppHandle,
    version: u32,
    lock_moons: bool,
) -> Result<(), String> {
    // Only meaningful for the Wesley preset (v69+).
    if version < 69 {
        return Ok(());
    }

    let cfg_dir = version_config_dir(app, version)?;
    std::fs::create_dir_all(&cfg_dir).map_err(|e| e.to_string())?;

    let cfg_path = cfg_dir.join("JacobG5.WesleyMoonScripts.cfg");
    if cfg_path.exists() {
        // Preserve everything else in the file and only update LockMoons.
        let bytes = std::fs::read(&cfg_path).map_err(|e| e.to_string())?;
        let text = String::from_utf8_lossy(&bytes);

        let mut changed = false;
        let mut out = String::with_capacity(text.len());
        let desired_value = if lock_moons { "true" } else { "false" };

        for seg in text.split_inclusive(['\n', '\r']) {
            // Preserve original newline sequence; only edit the assignment line.
            let (line, nl) = if seg.ends_with("\r\n") {
                (seg.trim_end_matches("\r\n"), "\r\n")
            } else if seg.ends_with('\n') {
                (seg.trim_end_matches('\n'), "\n")
            } else if seg.ends_with('\r') {
                (seg.trim_end_matches('\r'), "\r")
            } else {
                (seg, "")
            };

            let trimmed = line.trim_start();
            let indent_len = line.len().saturating_sub(trimmed.len());
            let indent = &line[..indent_len];

            if trimmed.starts_with("LockMoons") {
                if let Some(eq_idx) = trimmed.find('=') {
                    let (left, right_all) = trimmed.split_at(eq_idx);
                    let right_all = right_all.trim_start_matches('=');

                    // Keep inline comments if any.
                    let (right, comment) = match right_all.find('#') {
                        Some(i) => (&right_all[..i], &right_all[i..]),
                        None => (right_all, ""),
                    };

                    if left.trim() == "LockMoons" {
                        if !right.trim().eq_ignore_ascii_case(desired_value) {
                            out.push_str(indent);
                            out.push_str("LockMoons = ");
                            out.push_str(desired_value);
                            out.push_str(comment);
                            out.push_str(nl);
                            changed = true;
                            continue;
                        }
                    }
                }
            }

            out.push_str(line);
            out.push_str(nl);
        }

        if changed {
            std::fs::write(&cfg_path, out).map_err(|e| e.to_string())?;
        }
        return Ok(());
    }

    let content = format!(
        "## Settings file was created by plugin WesleyMoonScripts v1.1.6\n## Plugin GUID: JacobG5.WesleyMoonScripts\n\n[Core]\n\n## Locks moons that have progression integration set up to enable playing the campaign.\n# Setting type: Boolean\n# Default value: true\nLockMoons = {}\n\n",
        if lock_moons { "true" } else { "false" }
    );

    std::fs::write(&cfg_path, content).map_err(|e| e.to_string())?;
    Ok(())
}

pub(crate) fn ensure_reverb_trigger_fix_cfg(
    app: &tauri::AppHandle,
    version: u32,
) -> Result<(), String> {
    let plugins = plugins_dir(app, version)?;
    if mod_dir_for(&plugins, "JacobG5", "ReverbTriggerFix").is_none() {
        return Ok(());
    }

    let cfg_dir = version_config_dir(app, version)?;
    std::fs::create_dir_all(&cfg_dir).map_err(|e| e.to_string())?;

    let cfg_path = cfg_dir.join("JacobG5.ReverbTriggerFix.cfg");
    let baseline = "## Settings file was created by plugin ReverbTriggerFix v0.3.0\n## Plugin GUID: JacobG5.ReverbTriggerFix\n\n[Core]\n\n## Disables all reverb trigger modifications.\n## Requires a lobby restart to apply.\n## Game restart *not* required.\n# Setting type: Boolean\n# Default value: false\ndisableMod = false\n\n[Debug]\n\n## Logs more info to the console when enabled.\n## \n## *THIS WILL SPAM YOUR CONSOLE DEPENDING ON YOUR OTHER SETTINGS*\n# Setting type: Boolean\n# Default value: false\nextendedLogging = false\n\n[Experimental]\n\n## I'm not sure why reverb triggers run their calculations every frame when as far as I can tell they only need to run their changes when something enters their collider.\n## I'm leaving this as an experimental toggle because it seems to be very buggy atm.\n## \n## Feel free to try it if you wish. If you're experiencing problems then turn it back off.\n# Setting type: Boolean\n# Default value: false\ntriggerOnEnter = true\n";

    if !cfg_path.exists() {
        std::fs::write(&cfg_path, baseline).map_err(|e| e.to_string())?;
        return Ok(());
    }

    let bytes = std::fs::read(&cfg_path).map_err(|e| e.to_string())?;
    let text = String::from_utf8_lossy(&bytes);

    let mut changed = false;
    let mut saw_key = false;
    let mut out = String::with_capacity(text.len() + 128);

    for seg in text.split_inclusive(['\n', '\r']) {
        let (line, nl) = if seg.ends_with("\r\n") {
            (seg.trim_end_matches("\r\n"), "\r\n")
        } else if seg.ends_with('\n') {
            (seg.trim_end_matches('\n'), "\n")
        } else if seg.ends_with('\r') {
            (seg.trim_end_matches('\r'), "\r")
        } else {
            (seg, "")
        };

        let trimmed = line.trim_start();
        let indent_len = line.len().saturating_sub(trimmed.len());
        let indent = &line[..indent_len];

        if let Some(eq_idx) = trimmed.find('=') {
            let (left, right_all) = trimmed.split_at(eq_idx);
            if left.trim().eq_ignore_ascii_case("triggerOnEnter") {
                saw_key = true;
                let right_all = right_all.trim_start_matches('=');
                let (right, comment) = match right_all.find('#') {
                    Some(i) => (&right_all[..i], &right_all[i..]),
                    None => (right_all, ""),
                };

                if !right.trim().eq_ignore_ascii_case("true") {
                    out.push_str(indent);
                    out.push_str("triggerOnEnter = true");
                    out.push_str(comment);
                    out.push_str(nl);
                    changed = true;
                    continue;
                }
            }
        }

        out.push_str(line);
        out.push_str(nl);
    }

    if !saw_key {
        if !out.ends_with('\n') && !out.ends_with("\r\n") && !out.ends_with('\r') {
            out.push('\n');
        }
        out.push_str("\n[Experimental]\n\n");
        out.push_str("## I'm not sure why reverb triggers run their calculations every frame when as far as I can tell they only need to run their changes when something enters their collider.\n");
        out.push_str("## I'm leaving this as an experimental toggle because it seems to be very buggy atm.\n");
        out.push_str("## \n");
        out.push_str("## Feel free to try it if you wish. If you're experiencing problems then turn it back off.\n");
        out.push_str("# Setting type: Boolean\n");
        out.push_str("# Default value: false\n");
        out.push_str("triggerOnEnter = true\n");
        changed = true;
    }

    if changed {
        std::fs::write(&cfg_path, out).map_err(|e| e.to_string())?;
    }

    Ok(())
}

pub(crate) fn ensure_hqol_dont_store_item_cfg(
    app: &tauri::AppHandle,
    version: u32,
    wanted: &str,
) -> Result<(), String> {
    let plugins = plugins_dir(app, version)?;
    if hqol_mod_dir(&plugins).is_none() {
        return Ok(());
    }

    let cfg_dir = version_config_dir(app, version)?;
    std::fs::create_dir_all(&cfg_dir).map_err(|e| e.to_string())?;

    for file_name in ["OreoM.HQoL.72.cfg", "OreoM.HQoL.73.cfg"] {
        let cfg_path = cfg_dir.join(file_name);
        if !cfg_path.exists() {
            continue;
        }

        let bytes = std::fs::read(&cfg_path).map_err(|e| e.to_string())?;
        let text = String::from_utf8_lossy(&bytes);

        let mut changed = false;
        let mut in_general = false;
        let mut out = String::with_capacity(text.len() + wanted.len() + 4);

        for seg in text.split_inclusive(['\n', '\r']) {
            let (line, nl) = if seg.ends_with("\r\n") {
                (seg.trim_end_matches("\r\n"), "\r\n")
            } else if seg.ends_with('\n') {
                (seg.trim_end_matches('\n'), "\n")
            } else if seg.ends_with('\r') {
                (seg.trim_end_matches('\r'), "\r")
            } else {
                (seg, "")
            };

            let trimmed = line.trim_start();
            let trimmed_all = line.trim();

            if trimmed_all.starts_with('[') && trimmed_all.ends_with(']') {
                in_general = trimmed_all.eq_ignore_ascii_case("[General]");
            }

            if in_general {
                if let Some(eq_idx) = trimmed.find('=') {
                    let (left, right_all) = trimmed.split_at(eq_idx);
                    if left.trim() == "Dont store list" {
                        let indent_len = line.len().saturating_sub(trimmed.len());
                        let indent = &line[..indent_len];
                        let right_all = right_all.trim_start_matches('=');
                        let (right, comment) = match right_all.find('#') {
                            Some(i) => (&right_all[..i], &right_all[i..]),
                            None => (right_all, ""),
                        };

                        let mut items: Vec<String> = vec![];
                        let mut has_wanted = false;
                        for item in right.split(',') {
                            let item = item.trim();
                            if item.is_empty() {
                                continue;
                            }
                            if item.eq_ignore_ascii_case(wanted) {
                                if has_wanted {
                                    changed = true;
                                    continue;
                                }
                                has_wanted = true;
                            }
                            items.push(item.to_string());
                        }

                        if !has_wanted {
                            items.push(wanted.to_string());
                            changed = true;
                        }

                        out.push_str(indent);
                        out.push_str("Dont store list = ");
                        out.push_str(&items.join(", "));
                        out.push_str(comment);
                        out.push_str(nl);
                        continue;
                    }
                }
            }

            out.push_str(line);
            out.push_str(nl);
        }

        if changed {
            std::fs::write(&cfg_path, out).map_err(|e| e.to_string())?;
        }
    }

    Ok(())
}

fn ensure_weather_registry_cfg(app: &tauri::AppHandle, version: u32) -> Result<(), String> {
    // Only meaningful for the Wesley preset (v69+). Never overwrite entire configs.
    if version < 69 {
        return Ok(());
    }

    let cfg_dir = version_config_dir(app, version)?;
    std::fs::create_dir_all(&cfg_dir).map_err(|e| e.to_string())?;

    let cfg_path = cfg_dir.join("mrov.WeatherRegistry.cfg");
    let desired_first_day = "true";
    let desired_algo = "Hybrid";

    // If missing, create using the provided baseline.
    if !cfg_path.exists() {
        let content = r#"## Settings file was created by plugin WeatherRegistry v0.7.5
## Plugin GUID: mrov.WeatherRegistry

[|General]

## Enable colored weathers on map screen
# Setting type: Boolean
# Default value: true
Colored Weathers = true

## Display planet videos on map screen
# Setting type: Boolean
# Default value: true
Planet Videos = true

## Show weather multipliers on map screen
# Setting type: Boolean
# Default value: false
Show Weather Multipliers = false

## Use Registry's scrap multipliers. Disable if you prefer to use other mod's multiplier settings.
# Setting type: Boolean
# Default value: true
Scrap multipliers = true

[|Logging]

## Select which logs to show.
# Setting type: LoggingType
# Default value: Basic
# Acceptable values: Basic, Debug, Developer
Display Log Levels = Basic

[|WeatherSelection]

## Select the algorithm to use during weather selection.
# Setting type: WeatherAlgorithm
# Default value: Registry
# Acceptable values: Registry, Vanilla, Hybrid
Weather Selection Algorithm = Hybrid

## If enabled, the first day will always have clear weather, on all planets, regardless of the selected algorithm.
# Setting type: Boolean
# Default value: false
First Day Clear Weather = true

[Modded Weather: Earthquakes]

## The default weight of this weather
# Setting type: Int32
# Default value: 40
# Acceptable value range: From 0 to 10000
Default weight = 40

## Multiplier for the amount of scrap spawned
# Setting type: Single
# Default value: 1.1
# Acceptable value range: From 0 to 100
Scrap amount multiplier = 1.1

## Multiplier for the value of scrap spawned
# Setting type: Single
# Default value: 1
# Acceptable value range: From 0 to 100
Scrap value multiplier = 1

## Whether to make the filter a whitelist (false is blacklist, true is whitelist)
# Setting type: Boolean
# Default value: false
Filtering option = false

## Semicolon-separated list of level names to filter (use `Filtering Option` config to select filter type)
# Setting type: String
# Default value: Company;Galetry;Calist;Berunah
Level filter = Company;Galetry;Calist;Berunah

## Semicolon-separated list of level weights
# Setting type: String
# Default value: MoonName@50
Level weights = MoonName@50

## Semicolon-separated list of weather-to-weather weights - if previous day was Earthquakes, next day should have weights:
# Setting type: String
# Default value: WeatherName@50
WeatherToWeather weights = WeatherName@50

[Modded Weather: Forsaken]

## The default weight of this weather
# Setting type: Int32
# Default value: 30
# Acceptable value range: From 0 to 10000
Default weight = 30

## Multiplier for the amount of scrap spawned
# Setting type: Single
# Default value: 1.2
# Acceptable value range: From 0 to 100
Scrap amount multiplier = 1.2

## Multiplier for the value of scrap spawned
# Setting type: Single
# Default value: 1.2
# Acceptable value range: From 0 to 100
Scrap value multiplier = 1.2

## Whether to make the filter a whitelist (false is blacklist, true is whitelist)
# Setting type: Boolean
# Default value: false
Filtering option = false

## Semicolon-separated list of level names to filter (use `Filtering Option` config to select filter type)
# Setting type: String
# Default value: Company;Galetry;Berunah;Calist;Repress;Cosmocos
Level filter = Company;Galetry;Berunah;Calist;Repress;Cosmocos

## Semicolon-separated list of level weights
# Setting type: String
# Default value: MoonName@20
Level weights = MoonName@20

## Semicolon-separated list of weather-to-weather weights - if previous day was Forsaken, next day should have weights:
# Setting type: String
# Default value: WeatherName@20
WeatherToWeather weights = WeatherName@20

[Modded Weather: Hallowed]

## The default weight of this weather
# Setting type: Int32
# Default value: 8
# Acceptable value range: From 0 to 10000
Default weight = 8

## Multiplier for the amount of scrap spawned
# Setting type: Single
# Default value: 1
# Acceptable value range: From 0 to 100
Scrap amount multiplier = 1

## Multiplier for the value of scrap spawned
# Setting type: Single
# Default value: 1
# Acceptable value range: From 0 to 100
Scrap value multiplier = 1

## Whether to make the filter a whitelist (false is blacklist, true is whitelist)
# Setting type: Boolean
# Default value: false
Filtering option = false

## Semicolon-separated list of level names to filter (use `Filtering Option` config to select filter type)
# Setting type: String
# Default value: Company;Galetry;Calist;Berunah;Asteroid-13;Thalasso;Roart;Repress;Cosmocos
Level filter = Company;Galetry;Calist;Berunah;Asteroid-13;Thalasso;Roart;Repress;Cosmocos

## Semicolon-separated list of level weights
# Setting type: String
# Default value: MoonName@8
Level weights = MoonName@8

## Semicolon-separated list of weather-to-weather weights - if previous day was Hallowed, next day should have weights:
# Setting type: String
# Default value: WeatherName@8
WeatherToWeather weights = WeatherName@8

[Modded Weather: Hurricane]

## The default weight of this weather
# Setting type: Int32
# Default value: 80
# Acceptable value range: From 0 to 10000
Default weight = 80

## Multiplier for the amount of scrap spawned
# Setting type: Single
# Default value: 1
# Acceptable value range: From 0 to 100
Scrap amount multiplier = 1

## Multiplier for the value of scrap spawned
# Setting type: Single
# Default value: 1
# Acceptable value range: From 0 to 100
Scrap value multiplier = 1

## Whether to make the filter a whitelist (false is blacklist, true is whitelist)
# Setting type: Boolean
# Default value: false
Filtering option = false

## Semicolon-separated list of level names to filter (use `Filtering Option` config to select filter type)
# Setting type: String
# Default value: Company;Galetry;Calist;Berunah;Asteroid-13,Roart;Repress;Cosmocos
Level filter = Company;Galetry;Calist;Berunah;Asteroid-13,Roart;Repress;Cosmocos

## Semicolon-separated list of level weights
# Setting type: String
# Default value: WeatherName@20
Level weights = WeatherName@20

## Semicolon-separated list of weather-to-weather weights - if previous day was Hurricane, next day should have weights:
# Setting type: String
# Default value: WeatherName@20
WeatherToWeather weights = WeatherName@20

[Vanilla Weather: DustClouds]

## The default weight of this weather
# Setting type: Int32
# Default value: 100
# Acceptable value range: From 0 to 10000
Default weight = 100

## Multiplier for the amount of scrap spawned
# Setting type: Single
# Default value: 1
# Acceptable value range: From 0 to 100
Scrap amount multiplier = 1

## Multiplier for the value of scrap spawned
# Setting type: Single
# Default value: 1
# Acceptable value range: From 0 to 100
Scrap value multiplier = 1

## Whether to make the filter a whitelist (false is blacklist, true is whitelist)
# Setting type: Boolean
# Default value: false
Filtering option = false

## Semicolon-separated list of level names to filter (use `Filtering Option` config to select filter type)
# Setting type: String
# Default value: Company;
Level filter = Company;

## Semicolon-separated list of level weights
# Setting type: String
# Default value: MoonName@50;
Level weights = MoonName@50;

## Semicolon-separated list of weather-to-weather weights - if previous day was Dust Clouds, next day should have weights:
# Setting type: String
# Default value: WeatherName@50
WeatherToWeather weights = WeatherName@50

[Vanilla Weather: Eclipsed]

## The default weight of this weather
# Setting type: Int32
# Default value: 100
# Acceptable value range: From 0 to 10000
Default weight = 100

## Multiplier for the amount of scrap spawned
# Setting type: Single
# Default value: 1
# Acceptable value range: From 0 to 100
Scrap amount multiplier = 1

## Multiplier for the value of scrap spawned
# Setting type: Single
# Default value: 1
# Acceptable value range: From 0 to 100
Scrap value multiplier = 1

## Whether to make the filter a whitelist (false is blacklist, true is whitelist)
# Setting type: Boolean
# Default value: false
Filtering option = false

## Semicolon-separated list of level names to filter (use `Filtering Option` config to select filter type)
# Setting type: String
# Default value: Company;
Level filter = Company;

## Semicolon-separated list of level weights
# Setting type: String
# Default value: MoonName@50;
Level weights = MoonName@50;

## Semicolon-separated list of weather-to-weather weights - if previous day was Eclipsed, next day should have weights:
# Setting type: String
# Default value: None@300; Rainy@40; Stormy@16; Flooded@20; Foggy@60; Eclipsed@10;
WeatherToWeather weights = None@300; Rainy@40; Stormy@16; Flooded@20; Foggy@60; Eclipsed@10;

[Vanilla Weather: Flooded]

## The default weight of this weather
# Setting type: Int32
# Default value: 100
# Acceptable value range: From 0 to 10000
Default weight = 100

## Multiplier for the amount of scrap spawned
# Setting type: Single
# Default value: 1
# Acceptable value range: From 0 to 100
Scrap amount multiplier = 1

## Multiplier for the value of scrap spawned
# Setting type: Single
# Default value: 1
# Acceptable value range: From 0 to 100
Scrap value multiplier = 1

## Whether to make the filter a whitelist (false is blacklist, true is whitelist)
# Setting type: Boolean
# Default value: false
Filtering option = false

## Semicolon-separated list of level names to filter (use `Filtering Option` config to select filter type)
# Setting type: String
# Default value: Company;
Level filter = Company;

## Semicolon-separated list of level weights
# Setting type: String
# Default value: MoonName@50;
Level weights = MoonName@50;

## Semicolon-separated list of weather-to-weather weights - if previous day was Flooded, next day should have weights:
# Setting type: String
# Default value: None@160; Rainy@60; Stormy@50; Flooded@10; Foggy@60; Eclipsed@40;
WeatherToWeather weights = None@160; Rainy@60; Stormy@50; Flooded@10; Foggy@60; Eclipsed@40;

[Vanilla Weather: Foggy]

## The default weight of this weather
# Setting type: Int32
# Default value: 100
# Acceptable value range: From 0 to 10000
Default weight = 100

## Multiplier for the amount of scrap spawned
# Setting type: Single
# Default value: 1
# Acceptable value range: From 0 to 100
Scrap amount multiplier = 1

## Multiplier for the value of scrap spawned
# Setting type: Single
# Default value: 1
# Acceptable value range: From 0 to 100
Scrap value multiplier = 1

## Whether to make the filter a whitelist (false is blacklist, true is whitelist)
# Setting type: Boolean
# Default value: false
Filtering option = false

## Semicolon-separated list of level names to filter (use `Filtering Option` config to select filter type)
# Setting type: String
# Default value: Company;
Level filter = Company;

## Semicolon-separated list of level weights
# Setting type: String
# Default value: MoonName@50;
Level weights = MoonName@50;

## Semicolon-separated list of weather-to-weather weights - if previous day was Foggy, next day should have weights:
# Setting type: String
# Default value: None@200; Rainy@60; Stormy@50; Flooded@10; Foggy@30; Eclipsed@20;
WeatherToWeather weights = None@200; Rainy@60; Stormy@50; Flooded@10; Foggy@30; Eclipsed@20;

[Vanilla Weather: None]

## The default weight of this weather
# Setting type: Int32
# Default value: 100
# Acceptable value range: From 0 to 10000
Default weight = 100

## Multiplier for the amount of scrap spawned
# Setting type: Single
# Default value: 1
# Acceptable value range: From 0 to 100
Scrap amount multiplier = 1

## Multiplier for the value of scrap spawned
# Setting type: Single
# Default value: 1
# Acceptable value range: From 0 to 100
Scrap value multiplier = 1

## Whether to make the filter a whitelist (false is blacklist, true is whitelist)
# Setting type: Boolean
# Default value: false
Filtering option = false

## Semicolon-separated list of level names to filter (use `Filtering Option` config to select filter type)
# Setting type: String
# Default value: Company;
Level filter = Company;

## Semicolon-separated list of level weights
# Setting type: String
# Default value: MoonName@50;
Level weights = MoonName@50;

## Semicolon-separated list of weather-to-weather weights - if previous day was None, next day should have weights:
# Setting type: String
# Default value: None@160; Rainy@100; Stormy@70; Flooded@20; Foggy@40; Eclipsed@10;
WeatherToWeather weights = None@160; Rainy@100; Stormy@70; Flooded@20; Foggy@40; Eclipsed@10;

[Vanilla Weather: Rainy]

## The default weight of this weather
# Setting type: Int32
# Default value: 100
# Acceptable value range: From 0 to 10000
Default weight = 100

## Multiplier for the amount of scrap spawned
# Setting type: Single
# Default value: 1
# Acceptable value range: From 0 to 100
Scrap amount multiplier = 1

## Multiplier for the value of scrap spawned
# Setting type: Single
# Default value: 1
# Acceptable value range: From 0 to 100
Scrap value multiplier = 1

## Whether to make the filter a whitelist (false is blacklist, true is whitelist)
# Setting type: Boolean
# Default value: false
Filtering option = false

## Semicolon-separated list of level names to filter (use `Filtering Option` config to select filter type)
# Setting type: String
# Default value: Company;
Level filter = Company;

## Semicolon-separated list of level weights
# Setting type: String
# Default value: MoonName@50;
Level weights = MoonName@50;

## Semicolon-separated list of weather-to-weather weights - if previous day was Rainy, next day should have weights:
# Setting type: String
# Default value: None@100; Rainy@60; Stormy@40; Flooded@30; Foggy@50; Eclipsed@20;
WeatherToWeather weights = None@100; Rainy@60; Stormy@40; Flooded@30; Foggy@50; Eclipsed@20;

[Vanilla Weather: Stormy]

## The default weight of this weather
# Setting type: Int32
# Default value: 100
# Acceptable value range: From 0 to 10000
Default weight = 100

## Multiplier for the amount of scrap spawned
# Setting type: Single
# Default value: 1
# Acceptable value range: From 0 to 100
Scrap amount multiplier = 1

## Multiplier for the value of scrap spawned
# Setting type: Single
# Default value: 1
# Acceptable value range: From 0 to 100
Scrap value multiplier = 1

## Whether to make the filter a whitelist (false is blacklist, true is whitelist)
# Setting type: Boolean
# Default value: false
Filtering option = false

## Semicolon-separated list of level names to filter (use `Filtering Option` config to select filter type)
# Setting type: String
# Default value: Company;
Level filter = Company;

## Semicolon-separated list of level weights
# Setting type: String
# Default value: MoonName@50;
Level weights = MoonName@50;

## Semicolon-separated list of weather-to-weather weights - if previous day was Stormy, next day should have weights:
# Setting type: String
# Default value: None@160; Rainy@110; Stormy@10; Flooded@120; Foggy@20; Eclipsed@80;
WeatherToWeather weights = None@160; Rainy@110; Stormy@10; Flooded@120; Foggy@20; Eclipsed@80;




[|WeatherSelection]
First Day Clear Weather = true
Weather Selection Algorithm = Hybrid
"#;

        std::fs::write(&cfg_path, content).map_err(|e| e.to_string())?;
        return Ok(());
    }

    // If exists: enforce only these two keys (preserve everything else).
    let bytes = std::fs::read(&cfg_path).map_err(|e| e.to_string())?;
    let text = String::from_utf8_lossy(&bytes);

    let mut changed = false;
    let mut saw_first_day = false;
    let mut saw_algo = false;
    let mut out = String::with_capacity(text.len());

    for seg in text.split_inclusive(['\n', '\r']) {
        let (line, nl) = if seg.ends_with("\r\n") {
            (seg.trim_end_matches("\r\n"), "\r\n")
        } else if seg.ends_with('\n') {
            (seg.trim_end_matches('\n'), "\n")
        } else if seg.ends_with('\r') {
            (seg.trim_end_matches('\r'), "\r")
        } else {
            (seg, "")
        };

        let trimmed = line.trim_start();
        let indent_len = line.len().saturating_sub(trimmed.len());
        let indent = &line[..indent_len];

        let mut handled = false;
        for (key, desired, flag) in [
            (
                "First Day Clear Weather",
                desired_first_day,
                &mut saw_first_day,
            ),
            ("Weather Selection Algorithm", desired_algo, &mut saw_algo),
        ] {
            if trimmed.starts_with(key) {
                if let Some(eq_idx) = trimmed.find('=') {
                    let (left, right_all) = trimmed.split_at(eq_idx);
                    if left.trim() == key {
                        let right_all = right_all.trim_start_matches('=');
                        let (right, comment) = match right_all.find('#') {
                            Some(i) => (&right_all[..i], &right_all[i..]),
                            None => (right_all, ""),
                        };
                        *flag = true;
                        if right.trim() != desired {
                            out.push_str(indent);
                            out.push_str(key);
                            out.push_str(" = ");
                            out.push_str(desired);
                            out.push_str(comment);
                            out.push_str(nl);
                            changed = true;
                        } else {
                            out.push_str(line);
                            out.push_str(nl);
                        }
                        handled = true;
                        break;
                    }
                }
            }
        }
        if handled {
            continue;
        }

        out.push_str(line);
        out.push_str(nl);
    }

    if !saw_first_day || !saw_algo {
        // Append a minimal section at end if missing.
        if !out.ends_with('\n') && !out.ends_with("\r\n") && !out.ends_with('\r') {
            out.push('\n');
        }
        out.push_str("\n[|WeatherSelection]\n");
        if !saw_first_day {
            out.push_str("First Day Clear Weather = true\n");
        }
        if !saw_algo {
            out.push_str("Weather Selection Algorithm = Hybrid\n");
        }
        changed = true;
    }

    if changed {
        std::fs::write(&cfg_path, out).map_err(|e| e.to_string())?;
    }

    Ok(())
}

fn find_file_named(
    root: &std::path::Path,
    target_name: &str,
    max_depth: usize,
) -> Option<std::path::PathBuf> {
    let target_lower = target_name.to_lowercase();
    let mut stack: Vec<(std::path::PathBuf, usize)> = vec![(root.to_path_buf(), 0)];

    while let Some((dir, depth)) = stack.pop() {
        if depth > max_depth {
            continue;
        }
        let rd = std::fs::read_dir(&dir).ok()?;
        for e in rd.flatten() {
            let path = e.path();
            let ft = e.file_type().ok()?;
            if ft.is_dir() {
                stack.push((path, depth + 1));
                continue;
            }
            if !ft.is_file() {
                continue;
            }
            let name = path.file_name().and_then(|s| s.to_str())?;
            if name.to_lowercase() == target_lower {
                return Some(path);
            }
        }
    }
    None
}

fn plugins_dir(app: &tauri::AppHandle, version: u32) -> Result<std::path::PathBuf, String> {
    Ok(version_dir(app, version)?.join("BepInEx").join("plugins"))
}

fn patchers_dir(app: &tauri::AppHandle, version: u32) -> Result<std::path::PathBuf, String> {
    Ok(version_dir(app, version)?.join("BepInEx").join("patchers"))
}

fn mod_folder_name(dev: &str, name: &str) -> String {
    format!("{dev}-{name}")
}

fn mod_dir_for(plugins_dir: &std::path::Path, dev: &str, name: &str) -> Option<std::path::PathBuf> {
    // Fast paths (common on Windows; case-insensitive FS).
    let direct = plugins_dir.join(mod_folder_name(dev, name));
    if direct.exists() {
        return Some(direct);
    }

    let dev_l = dev.trim().to_lowercase();
    let name_l = name.trim().to_lowercase();
    let lowered = plugins_dir.join(mod_folder_name(&dev_l, &name_l));
    if lowered.exists() {
        return Some(lowered);
    }

    // Fallback: scan directories and match case-insensitively.
    let target = mod_folder_name(&dev_l, &name_l);
    let Ok(rd) = std::fs::read_dir(plugins_dir) else {
        return None;
    };
    for e in rd.flatten() {
        let path = e.path();
        if !path.is_dir() {
            continue;
        }
        let Some(folder) = path.file_name().and_then(|s| s.to_str()) else {
            continue;
        };
        if folder.to_lowercase() == target {
            return Some(path);
        }
    }
    None
}

fn find_mod_icon_path(mod_dir: &std::path::Path) -> Option<String> {
    for file_name in ["icon.png", "icon.png.old"] {
        let path = mod_dir.join(file_name);
        if path.is_file() {
            return Some(path.to_string_lossy().to_string());
        }
    }
    None
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct DisabledMod {
    dev: String,
    name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct InstalledModVersion {
    dev: String,
    name: String,
    version: String,
    icon_path: Option<String>,
    description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DisableModFile {
    version: u32,
    mods: Vec<DisabledMod>,
}

fn disablemod_path(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    Ok(app
        .path()
        .app_data_dir()
        .map_err(|e| format!("failed to resolve app data dir: {e}"))?
        .join("config")
        .join("disablemod.json"))
}

pub(crate) fn thunderstore_cache_path(
    app: &tauri::AppHandle,
) -> Result<std::path::PathBuf, String> {
    Ok(app
        .path()
        .app_data_dir()
        .map_err(|e| format!("failed to resolve app data dir: {e}"))?
        .join("cache")
        .join("thunderstore.json"))
}

fn read_disablemod(app: &tauri::AppHandle) -> Result<DisableModFile, String> {
    let path = disablemod_path(app)?;
    let default_mod = normalize_mod_id("SlushyRH", "FreeeeeeMoooooons");
    if !path.exists() {
        // v2 (migration): include default disabled mod entry.
        let f = DisableModFile {
            version: 2,
            mods: vec![default_mod],
        };
        // best-effort persist so frontend sees stable state
        let _ = write_disablemod(app, &f);
        return Ok(f);
    }
    let text = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let mut f = match serde_json::from_str::<DisableModFile>(&text) {
        Ok(v) => v,
        Err(e) => {
            // If the file is corrupted, recover with defaults rather than breaking the UI.
            log::warn!("Failed to parse disablemod.json, resetting: {e}");
            let f = DisableModFile {
                version: 2,
                mods: vec![default_mod],
            };
            let _ = write_disablemod(app, &f);
            return Ok(f);
        }
    };

    // Migration: v1 -> v2
    if f.version == 1 {
        f.version = 2;
        f.mods.push(default_mod);
        f.mods
            .sort_by(|a, b| a.dev.cmp(&b.dev).then(a.name.cmp(&b.name)));
        f.mods.dedup();
        let _ = write_disablemod(app, &f);
    }

    Ok(f)
}

fn write_disablemod(app: &tauri::AppHandle, f: &DisableModFile) -> Result<(), String> {
    let path = disablemod_path(app)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(f).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| e.to_string())
}

fn normalize_mod_id(dev: &str, name: &str) -> DisabledMod {
    DisabledMod {
        dev: dev.trim().to_lowercase(),
        name: name.trim().to_lowercase(),
    }
}

fn for_each_file_recursive(
    root: &std::path::Path,
    mut f: impl FnMut(&std::path::Path) -> Result<(), String>,
) -> Result<(), String> {
    let mut stack: Vec<std::path::PathBuf> = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for e in std::fs::read_dir(&dir).map_err(|e| e.to_string())? {
            let e = e.map_err(|e| e.to_string())?;
            let path = e.path();
            let ty = e.file_type().map_err(|e| e.to_string())?;
            if ty.is_dir() {
                stack.push(path);
            } else if ty.is_file() {
                f(&path)?;
            }
        }
    }
    Ok(())
}

fn set_mod_files_old_suffix(mod_dir: &std::path::Path, enabled: bool) -> Result<(), String> {
    if !mod_dir.exists() {
        return Ok(());
    }

    if enabled {
        // Remove .old suffix
        for_each_file_recursive(mod_dir, |path| {
            let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
            if !name.to_lowercase().ends_with(".old") {
                return Ok(());
            }
            let mut new_name = name.to_string();
            new_name.truncate(new_name.len().saturating_sub(4));
            let new_path = path.with_file_name(new_name);
            if new_path.exists() {
                // Don't overwrite; keep the .old file.
                return Ok(());
            }
            std::fs::rename(path, new_path).map_err(|e| e.to_string())
        })
    } else {
        // Add .old suffix
        for_each_file_recursive(mod_dir, |path| {
            let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
            if name.to_lowercase().ends_with(".old") {
                return Ok(());
            }
            let new_path = path.with_file_name(format!("{name}.old"));
            if new_path.exists() {
                return Ok(());
            }
            std::fs::rename(path, new_path).map_err(|e| e.to_string())
        })
    }
}

// (intentionally no "is_disabled"/"is_mod_enabled" helpers; frontend uses disablemod list as source of truth)

fn apply_disabled_mods_for_version(app: &tauri::AppHandle, version: u32) -> Result<(), String> {
    let list = read_disablemod(app)?;
    let plugins = plugins_dir(app, version)?;
    let patchers = patchers_dir(app, version)?;
    for m in list.mods {
        if let Some(dir) = mod_dir_for(&plugins, &m.dev, &m.name) {
            let _ = set_mod_files_old_suffix(&dir, false);
        }
        if let Some(dir) = mod_dir_for(&patchers, &m.dev, &m.name) {
            let _ = set_mod_files_old_suffix(&dir, false);
        }
    }
    let _ = sync_fontpatcher_with_assets_for_version(app, version);
    Ok(())
}

fn hqol_mod_dir(plugins_dir: &std::path::Path) -> Option<std::path::PathBuf> {
    mod_dir_for(plugins_dir, "HQHQTeam", "HQoL")
        .or_else(|| mod_dir_for(plugins_dir, "HQHQTeam", "HQOL"))
}

fn sync_hqol_with_disablemod_for_version(
    app: &tauri::AppHandle,
    version: u32,
) -> Result<(), String> {
    let list = read_disablemod(app)?;
    let id1 = normalize_mod_id("HQHQTeam", "HQoL");
    let id2 = normalize_mod_id("HQHQTeam", "HQOL");
    let disabled = list.mods.iter().any(|m| m == &id1 || m == &id2);

    let plugins = plugins_dir(app, version)?;
    if let Some(dir) = hqol_mod_dir(&plugins) {
        let _ = set_mod_files_old_suffix(&dir, !disabled);
    }
    Ok(())
}

fn ensure_practice_mods_disabled_for_version(
    app: &tauri::AppHandle,
    version: u32,
) -> Result<(), String> {
    let practice = variable::get_practice_mod_list();
    // Apply non-practice runtime state for this version immediately, without
    // overwriting the user's persisted practice toggle preferences.
    let plugins = plugins_dir(app, version)?;
    let patchers = patchers_dir(app, version)?;
    for m in practice {
        if let Some(dir) = mod_dir_for(&plugins, &m.dev, &m.name) {
            let _ = set_mod_files_old_suffix(&dir, false);
        }
        if let Some(dir) = mod_dir_for(&patchers, &m.dev, &m.name) {
            let _ = set_mod_files_old_suffix(&dir, false);
        }
    }

    Ok(())
}

fn filter_missing_mods_for_version(
    app: &tauri::AppHandle,
    version: u32,
    mods: &[mod_config::ModEntry],
) -> Result<Vec<mod_config::ModEntry>, String> {
    let plugins = plugins_dir(app, version)?;
    let patchers = patchers_dir(app, version)?;

    Ok(mods
        .iter()
        .filter(|m| {
            mod_dir_for(&plugins, &m.dev, &m.name).is_none()
                && mod_dir_for(&patchers, &m.dev, &m.name).is_none()
        })
        .cloned()
        .collect())
}

async fn prepare_practice_mods_for_version(
    app: &tauri::AppHandle,
    version: u32,
    cancel: Option<Arc<AtomicBool>>,
) -> Result<Vec<(String, String)>, String> {
    let game_root = version_dir(app, version)?;
    if !game_root.exists() {
        return Err(format!(
            "version folder not found: {}",
            game_root.to_string_lossy()
        ));
    }

    let practice_all = variable::get_practice_mod_list();
    let disabled_list = read_disablemod(app)?;
    let practice_enabled: Vec<mod_config::ModEntry> = practice_all
        .iter()
        .cloned()
        .filter(|m| {
            m.is_compatible(version)
                && !disabled_list
                    .mods
                    .contains(&normalize_mod_id(&m.dev, &m.name))
        })
        .collect();
    let practice_ids: Vec<(String, String)> = practice_enabled
        .iter()
        .map(|m| (m.dev.clone(), m.name.clone()))
        .collect();

    let missing_practice_mods = filter_missing_mods_for_version(app, version, &practice_enabled)?;
    if !missing_practice_mods.is_empty() {
        // Only show setup progress when new manifest mods actually need to be installed.
        const STEPS_TOTAL: u32 = 1;
        progress::emit_progress(
            app,
            TaskProgressPayload {
                version,
                steps_total: STEPS_TOTAL,
                step: 1,
                step_name: "Practice Mods".to_string(),
                step_progress: 0.0,
                overall_percent: 0.0,
                detail: Some("Installing missing practice mods...".to_string()),
                downloaded_bytes: None,
                total_bytes: None,
                extracted_files: Some(0),
                total_files: Some(missing_practice_mods.len() as u64),
            },
        );

        let cfg = ModsConfig {
            mods: missing_practice_mods,
        };

        let install_res: Result<(), String> = mods::install_mods_with_progress(
            app,
            &game_root,
            version,
            &cfg,
            cancel,
            |done, total, progress_info| {
                let step_progress = if total == 0 {
                    1.0
                } else {
                    (done as f64 / total as f64).clamp(0.0, 1.0)
                };
                progress::emit_progress(
                    app,
                    TaskProgressPayload {
                        version,
                        steps_total: STEPS_TOTAL,
                        step: 1,
                        step_name: "Practice Mods".to_string(),
                        step_progress,
                        overall_percent: overall_from_step(1, step_progress, STEPS_TOTAL),
                        detail: progress_info.detail,
                        downloaded_bytes: progress_info.downloaded_bytes,
                        total_bytes: progress_info.total_bytes,
                        extracted_files: progress_info.extracted_files.or(Some(done)),
                        total_files: progress_info.total_files.or(Some(total)),
                    },
                );
            },
        )
        .await;

        if let Err(e) = &install_res {
            progress::emit_error(
                app,
                TaskErrorPayload {
                    version,
                    message: e.clone(),
                },
            );
            return Err(e.clone());
        }
    }

    // Apply filesystem state for this version: disable all practice mods,
    // then re-enable only the compatible subset that the user has not disabled.
    let plugins = plugins_dir(app, version)?;
    let patchers = patchers_dir(app, version)?;
    for m in &practice_all {
        if let Some(dir) = mod_dir_for(&plugins, &m.dev, &m.name) {
            let _ = set_mod_files_old_suffix(&dir, false);
        }
        if let Some(dir) = mod_dir_for(&patchers, &m.dev, &m.name) {
            let _ = set_mod_files_old_suffix(&dir, false);
        }
    }
    for m in &practice_enabled {
        if let Some(dir) = mod_dir_for(&plugins, &m.dev, &m.name) {
            let _ = set_mod_files_old_suffix(&dir, true);
        }
        if let Some(dir) = mod_dir_for(&patchers, &m.dev, &m.name) {
            let _ = set_mod_files_old_suffix(&dir, true);
        }
    }

    // Special rule: when running Practice and Imperium is installed, force-disable HQoL (HQHQTeam).
    // Otherwise, HQoL should follow disablemod.json state.
    let imperium_installed = mod_dir_for(&plugins, "giosuel", "Imperium").is_some();
    if let Some(hqol_dir) = hqol_mod_dir(&plugins) {
        if imperium_installed {
            let _ = set_mod_files_old_suffix(&hqol_dir, false);
        } else {
            // Re-sync HQoL to user's config.
            let _ = sync_hqol_with_disablemod_for_version(app, version);
        }
    }

    progress::emit_finished(
        app,
        TaskFinishedPayload {
            version,
            path: game_root.to_string_lossy().to_string(),
        },
    );

    Ok(practice_ids)
}

#[derive(Default)]
struct GameState {
    child: Mutex<Option<std::process::Child>>,
}

#[derive(Default)]
struct DownloadState {
    active: Mutex<Option<ActiveDownload>>,
}

struct ActiveDownload {
    version: u32,
    cancel: Arc<AtomicBool>,
}

#[derive(Default)]
struct PrepareState {
    next_id: std::sync::atomic::AtomicU64,
    active: Mutex<Option<ActivePrepare>>,
}

struct ActivePrepare {
    id: u64,
    version: u32,
    cancel: Arc<AtomicBool>,
}

#[derive(Debug, Clone, Serialize)]
struct GameStatus {
    running: bool,
    pid: Option<u32>,
}

#[tauri::command]
async fn download(
    app: tauri::AppHandle,
    version: u32,
    state: State<'_, DownloadState>,
) -> Result<bool, String> {
    // Only allow one active download at a time (simplifies cancel + UI state).
    let cancel = Arc::new(AtomicBool::new(false));
    {
        let mut guard = state
            .active
            .lock()
            .map_err(|_| "download state lock poisoned".to_string())?;
        if let Some(active) = guard.as_ref() {
            if !active.cancel.load(Ordering::Relaxed) {
                return Err(format!(
                    "download already in progress (v{}). Please cancel it first.",
                    active.version
                ));
            }
        }
        *guard = Some(ActiveDownload {
            version,
            cancel: cancel.clone(),
        });
    }

    let res = installer::download_and_setup(app.clone(), version, cancel.clone()).await;

    // Clear active download state (best-effort).
    {
        let mut guard = state
            .active
            .lock()
            .map_err(|_| "download state lock poisoned".to_string())?;
        if guard.as_ref().is_some_and(|a| a.version == version) {
            *guard = None;
        }
    }
    res
}

#[tauri::command]
fn cancel_download(
    app: tauri::AppHandle,
    version: u32,
    state: State<'_, DownloadState>,
) -> Result<bool, String> {
    let mut did_signal = false;
    {
        let guard = state
            .active
            .lock()
            .map_err(|_| "download state lock poisoned".to_string())?;
        if let Some(active) = guard.as_ref() {
            if active.version == version {
                active.cancel.store(true, Ordering::Relaxed);
                did_signal = true;
            }
        }
    }

    // Best-effort cleanup: delete the partial version folder.
    // If files are still in use, the installer will retry cleanup on exit.
    if did_signal {
        if let Ok(dir) = version_dir(&app, version) {
            let _ = std::fs::remove_dir_all(&dir);
        }
    }

    Ok(did_signal)
}

#[tauri::command]
async fn sync_latest_install_from_manifest(app: tauri::AppHandle) -> Result<bool, String> {
    installer::sync_latest_install_from_manifest(app).await?;
    Ok(true)
}

#[tauri::command]
async fn open_version_folder(app: tauri::AppHandle) -> Result<bool, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("failed to resolve app data dir: {e}"))?
        .join("versions");
    std::fs::create_dir_all(&dir).map_err(|e| format!("failed to create versions dir: {e}"))?;
    let _ = opener::open(dir).map_err(|e| e.to_string())?;
    Ok(true)
}

fn collect_delete_targets(
    path: &std::path::Path,
    out: &mut Vec<std::path::PathBuf>,
) -> Result<(), String> {
    let rd = std::fs::read_dir(path).map_err(|e| e.to_string())?;
    for entry in rd {
        let entry = entry.map_err(|e| e.to_string())?;
        let child = entry.path();
        let ty = entry.file_type().map_err(|e| e.to_string())?;
        if ty.is_dir() && !ty.is_symlink() {
            collect_delete_targets(&child, out)?;
            out.push(child);
        } else {
            out.push(child);
        }
    }
    Ok(())
}

fn remove_delete_target(path: &std::path::Path) -> Result<(), String> {
    let ty = std::fs::symlink_metadata(path)
        .map_err(|e| e.to_string())?
        .file_type();

    if ty.is_dir() && !ty.is_symlink() {
        std::fs::remove_dir(path).map_err(|e| e.to_string())?;
        return Ok(());
    }

    if let Err(file_err) = std::fs::remove_file(path) {
        std::fs::remove_dir(path).map_err(|dir_err| format!("{file_err}; {dir_err}"))?;
    }
    Ok(())
}

#[tauri::command]
fn delete_installed_version(
    app: tauri::AppHandle,
    version: u32,
    game_state: State<'_, GameState>,
    download_state: State<'_, DownloadState>,
    prepare_state: State<'_, PrepareState>,
) -> Result<bool, String> {
    {
        let mut guard = game_state
            .child
            .lock()
            .map_err(|_| "game state lock poisoned".to_string())?;
        if let Some(child) = guard.as_mut() {
            if child.try_wait().map_err(|e| e.to_string())?.is_none() {
                return Err("Cannot delete a version while the game is running.".to_string());
            }
            *guard = None;
        }
    }

    {
        let guard = download_state
            .active
            .lock()
            .map_err(|_| "download state lock poisoned".to_string())?;
        if let Some(active) = guard.as_ref() {
            if active.version == version && !active.cancel.load(Ordering::Relaxed) {
                return Err("Cannot delete a version while it is downloading.".to_string());
            }
        }
    }

    {
        let guard = prepare_state
            .active
            .lock()
            .map_err(|_| "prepare state lock poisoned".to_string())?;
        if let Some(active) = guard.as_ref() {
            if active.version == version && !active.cancel.load(Ordering::Relaxed) {
                return Err("Cannot delete a version while it is being prepared.".to_string());
            }
        }
    }

    let dir = version_dir(&app, version)?;
    if !dir.exists() {
        return Ok(false);
    }

    let mut targets = Vec::new();
    collect_delete_targets(&dir, &mut targets)?;
    targets.push(dir.clone());

    let total = targets.len() as u64;
    progress::emit_progress(
        &app,
        TaskProgressPayload {
            version,
            steps_total: 1,
            step: 1,
            step_name: "Delete Version".to_string(),
            step_progress: 0.0,
            overall_percent: 0.0,
            detail: Some(format!("Preparing to delete v{version}...")),
            downloaded_bytes: None,
            total_bytes: None,
            extracted_files: Some(0),
            total_files: Some(total),
        },
    );

    for (idx, target) in targets.iter().enumerate() {
        remove_delete_target(target)?;
        let done = idx as u64 + 1;
        let step_progress = if total == 0 {
            1.0
        } else {
            done as f64 / total as f64
        };
        let detail = target
            .file_name()
            .and_then(|name| name.to_str())
            .map(|name| format!("Deleting {name}"))
            .unwrap_or_else(|| "Deleting files...".to_string());
        progress::emit_progress(
            &app,
            TaskProgressPayload {
                version,
                steps_total: 1,
                step: 1,
                step_name: "Delete Version".to_string(),
                step_progress,
                overall_percent: step_progress * 100.0,
                detail: Some(detail),
                downloaded_bytes: None,
                total_bytes: None,
                extracted_files: Some(done),
                total_files: Some(total),
            },
        );
    }

    Ok(true)
}

#[tauri::command]
async fn open_downloader_folder(app: tauri::AppHandle) -> Result<bool, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("failed to resolve app data dir: {e}"))?
        .join("downloader");
    std::fs::create_dir_all(&dir).map_err(|e| format!("failed to create downloader dir: {e}"))?;
    opener::open(dir).map_err(|e| e.to_string())?;
    Ok(true)
}

#[tauri::command]
async fn open_mod_folder(
    app: tauri::AppHandle,
    version: u32,
    dev: String,
    name: String,
) -> Result<bool, String> {
    let plugins = plugins_dir(&app, version)?;
    let patchers = patchers_dir(&app, version)?;

    let Some(dir) =
        mod_dir_for(&plugins, &dev, &name).or_else(|| mod_dir_for(&patchers, &dev, &name))
    else {
        return Err(format!(
            "mod folder not found for {dev}-{name} on v{version}"
        ));
    };

    opener::open(dir).map_err(|e| e.to_string())?;
    Ok(true)
}

#[tauri::command]
async fn check_mod_updates(
    app: tauri::AppHandle,
    version: u32,
    run_mode: Option<String>,
) -> Result<bool, String> {
    let client = reqwest::Client::new();

    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("failed to resolve app data dir: {e}"))?
        .join("versions");
    let extract_dir = dir.join(format!("v{version}"));
    let mods_cfg = effective_mods_config_for_run_mode(
        &client,
        version,
        run_mode.as_deref().unwrap_or("hq"),
        false,
        false,
    )
    .await?;

    let mut updatable_mods: Vec<String> = vec![];

    let res = mods::updatable_mods_with_progress(
        &app,
        &extract_dir,
        version,
        &mods_cfg,
        |checked, total, detail, mod_name| {
            if let Some(mod_name) = mod_name {
                if !updatable_mods.contains(&mod_name) {
                    updatable_mods.push(mod_name.clone());
                }
            }

            progress::emit_updatable_progress(
                &app,
                TaskUpdatableProgressPayload {
                    version,
                    total,
                    checked,
                    updatable_mods: updatable_mods.clone(),
                    detail,
                },
            );
        },
    )
    .await;

    if let Err(e) = res {
        progress::emit_updatable_error(
            &app,
            TaskErrorPayload {
                version,
                message: e.clone(),
            },
        );
        return Err(e);
    }

    progress::emit_updatable_finished(
        &app,
        TaskFinishedPayload {
            version,
            path: extract_dir.to_string_lossy().to_string(),
        },
    );
    Ok(true)
}

#[tauri::command]
async fn apply_mod_updates(
    app: tauri::AppHandle,
    version: u32,
    run_mode: Option<String>,
) -> Result<bool, String> {
    let res: Result<(), String> = async {
        let client = reqwest::Client::new();

        let dir = app
            .path()
            .app_data_dir()
            .map_err(|e| format!("failed to resolve app data dir: {e}"))?
            .join("versions");
        let game_root = dir.join(format!("v{version}"));
        if !game_root.exists() {
            return Err(format!(
                "version folder not found: {}",
                game_root.to_string_lossy()
            ));
        }

        let mods_cfg = effective_mods_config_for_run_mode(
            &client,
            version,
            run_mode.as_deref().unwrap_or("hq"),
            false,
            false,
        )
        .await?;

        const STEPS_TOTAL: u32 = 2;
        progress::emit_progress(
            &app,
            TaskProgressPayload {
                version,
                steps_total: STEPS_TOTAL,
                step: 1,
                step_name: "Check Updates".to_string(),
                step_progress: 0.0,
                overall_percent: overall_from_step(1, 0.0, STEPS_TOTAL),
                detail: Some("Checking updatable mods...".to_string()),
                downloaded_bytes: None,
                total_bytes: None,
                extracted_files: None,
                total_files: None,
            },
        );

        let mut updatable: Vec<String> = vec![];
        mods::updatable_mods_with_progress(
            &app,
            &game_root,
            version,
            &mods_cfg,
            |checked, total, detail, mod_name| {
                if let Some(m) = mod_name {
                    if !updatable.contains(&m) {
                        updatable.push(m);
                    }
                }
                let step_progress = if total == 0 {
                    1.0
                } else {
                    (checked as f64 / total as f64).clamp(0.0, 1.0)
                };
                progress::emit_progress(
                    &app,
                    TaskProgressPayload {
                        version,
                        steps_total: STEPS_TOTAL,
                        step: 1,
                        step_name: "Check Updates".to_string(),
                        step_progress,
                        overall_percent: overall_from_step(1, step_progress, STEPS_TOTAL),
                        detail,
                        downloaded_bytes: None,
                        total_bytes: None,
                        extracted_files: Some(checked),
                        total_files: Some(total),
                    },
                );
            },
        )
        .await?;

        if updatable.is_empty() {
            progress::emit_progress(
                &app,
                TaskProgressPayload {
                    version,
                    steps_total: STEPS_TOTAL,
                    step: 2,
                    step_name: "Update Mods".to_string(),
                    step_progress: 1.0,
                    overall_percent: 100.0,
                    detail: Some("No updates available".to_string()),
                    downloaded_bytes: None,
                    total_bytes: None,
                    extracted_files: None,
                    total_files: None,
                },
            );
            return Ok(());
        }

        progress::emit_progress(
            &app,
            TaskProgressPayload {
                version,
                steps_total: STEPS_TOTAL,
                step: 2,
                step_name: "Update Mods".to_string(),
                step_progress: 0.0,
                overall_percent: overall_from_step(2, 0.0, STEPS_TOTAL),
                detail: Some(format!("Updating {} mods...", updatable.len())),
                downloaded_bytes: None,
                total_bytes: None,
                extracted_files: Some(0),
                total_files: Some(updatable.len() as u64),
            },
        );

        mods::update_mods_with_progress(
            &app,
            &game_root,
            version,
            &mods_cfg,
            updatable.clone(),
            |done, total, progress_info| {
                let step_progress = if total == 0 {
                    1.0
                } else {
                    (done as f64 / total as f64).clamp(0.0, 1.0)
                };
                progress::emit_progress(
                    &app,
                    TaskProgressPayload {
                        version,
                        steps_total: STEPS_TOTAL,
                        step: 2,
                        step_name: "Update Mods".to_string(),
                        step_progress,
                        overall_percent: overall_from_step(2, step_progress, STEPS_TOTAL),
                        detail: progress_info.detail,
                        downloaded_bytes: progress_info.downloaded_bytes,
                        total_bytes: progress_info.total_bytes,
                        extracted_files: progress_info.extracted_files.or(Some(done)),
                        total_files: progress_info.total_files.or(Some(total)),
                    },
                );
            },
        )
        .await?;

        let _ = ensure_reverb_trigger_fix_cfg(&app, version);
        let _ = ensure_hqol_dont_store_item_cfg(&app, version, "DungeonKeyItem");

        Ok(())
    }
    .await;

    match res {
        Ok(()) => {
            progress::emit_finished(
                &app,
                TaskFinishedPayload {
                    version,
                    path: version_dir(&app, version)?.to_string_lossy().to_string(),
                },
            );
            Ok(true)
        }
        Err(e) => {
            progress::emit_error(
                &app,
                TaskErrorPayload {
                    version,
                    message: e.clone(),
                },
            );
            Err(e)
        }
    }
}

#[cfg(target_os = "linux")]
fn get_steam_client_path(launcher_root: &std::path::Path) -> std::path::PathBuf {
    if let Some(home_dir) = dirs::home_dir() {
        let steam_paths = [
            home_dir.join(".steam/steam"),
            home_dir.join(".local/share/Steam"),
            home_dir.join(".var/app/com.valvesoftware.Steam/.local/share/Steam"),
        ];
        for path in steam_paths.iter() {
            if path.exists() && path.join("steamapps").exists() {
                println!("Found real Steam installation at: {:?}", path);
                return path.clone();
            }
        }
    }

    println!("Steam not found. Mocking client path.");
    launcher_root.to_path_buf()
}

#[tauri::command]
fn launch_game(
    app: tauri::AppHandle,
    version: u32,
    state: State<'_, GameState>,
) -> Result<u32, String> {
    let dir = version_dir(&app, version)?;
    if !dir.exists() {
        return Err(format!(
            "version folder not found: {}",
            dir.to_string_lossy()
        ));
    }

    let _app_path = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("app path not found: {e}"))?;
    let exe_name = "Lethal Company.exe";
    let exe_path = dir.join(exe_name);
    let exe_path = if exe_path.exists() {
        exe_path
    } else {
        find_file_named(&dir, exe_name, 3)
            .ok_or_else(|| format!("{exe_name} not found under {}", dir.to_string_lossy()))?
    };

    let exe_dir = exe_path
        .parent()
        .ok_or_else(|| "invalid exe path".to_string())?;

    // If already running, return an error.
    {
        let mut guard = state
            .child
            .lock()
            .map_err(|_| "game state lock poisoned".to_string())?;
        if let Some(child) = guard.as_mut() {
            if child.try_wait().map_err(|e| e.to_string())?.is_none() {
                return Err("game is already running".to_string());
            }
        }
        *guard = None;
    }

    // Ensure disabled mods are applied for this version before launch.
    let _ = apply_disabled_mods_for_version(&app, version);
    // For HQoL specifically, also ensure `.old` matches disablemod.json on normal runs.
    let _ = sync_hqol_with_disablemod_for_version(&app, version);
    let _ = ensure_reverb_trigger_fix_cfg(&app, version);
    let _ = ensure_hqol_dont_store_item_cfg(&app, version, "DungeonKeyItem");

    #[cfg(target_os = "windows")]
    let mut command = std::process::Command::new(&exe_path);

    #[cfg(target_os = "macos")]
    let mut command = {
        let mut cmd = std::process::Command::new("open");
        cmd.arg("-a");
        cmd.arg(&exe_path);
        cmd
    };

    #[cfg(target_os = "linux")]
    let (proton_binary, compat_data_path) = {
        let proton_env_path = installer::proton_env_dir(&app)
            .map_err(|e| format!("proton_env path not found: {e}"))?;
        let proton_bin_path = installer::get_current_proton_dir_impl(&app)
            .map_err(|e| format!("proton path not found: {e}"))?
            .ok_or("found proton path but is None")?;
        let compat_pre_path = proton_env_path.join("wine_prefix");
        if !compat_pre_path.exists() {
            std::fs::create_dir(&compat_pre_path)
                .map_err(|e| format!("could not make prefix: {e}"))?;
        }
        (proton_bin_path.join("proton"), compat_pre_path)
    };

    #[cfg(target_os = "linux")]
    let mut command = {
        let steam_path = get_steam_client_path(&_app_path);
        let mut cmd = std::process::Command::new(&proton_binary);
        cmd.arg("run");
        cmd.arg(&exe_path);
        cmd.env("STEAM_COMPAT_DATA_PATH", &compat_data_path);
        cmd.env("STEAM_COMPAT_CLIENT_INSTALL_PATH", &steam_path);
        cmd.env("WINEDLLOVERRIDES", "winhttp=n,b");
        cmd.env_remove("PYTHONPATH");
        cmd.env_remove("PYTHONHOME");
        cmd
    };

    let child = command
        .current_dir(exe_dir)
        .spawn()
        .map_err(|e| format!("failed to launch: {e}"))?;

    let pid = child.id();
    let mut guard = state
        .child
        .lock()
        .map_err(|_| "game state lock poisoned".to_string())?;
    *guard = Some(child);
    Ok(pid)
}

#[tauri::command]
async fn launch_game_practice(
    app: tauri::AppHandle,
    version: u32,
    state: State<'_, GameState>,
) -> Result<u32, String> {
    let dir = version_dir(&app, version)?;
    if !dir.exists() {
        return Err(format!(
            "version folder not found: {}",
            dir.to_string_lossy()
        ));
    }

    let _app_path = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("app path not found: {e}"))?;
    let exe_name = "Lethal Company.exe";
    let exe_path = dir.join(exe_name);
    let exe_path = if exe_path.exists() {
        exe_path
    } else {
        find_file_named(&dir, exe_name, 3)
            .ok_or_else(|| format!("{exe_name} not found under {}", dir.to_string_lossy()))?
    };

    let exe_dir = exe_path
        .parent()
        .ok_or_else(|| "invalid exe path".to_string())?;

    // If already running, return an error.
    {
        let mut guard = state
            .child
            .lock()
            .map_err(|_| "game state lock poisoned".to_string())?;
        if let Some(child) = guard.as_mut() {
            if child.try_wait().map_err(|e| e.to_string())?.is_none() {
                return Err("game is already running".to_string());
            }
        }
        *guard = None;
    }

    // Practice run: install + enable practice mods (compatible with this game version).
    let _ = prepare_practice_mods_for_version(&app, version, None).await?;

    // Ensure disabled mods are applied for this version before launch.
    let _ = apply_disabled_mods_for_version(&app, version);

    #[cfg(target_os = "windows")]
    let mut command = std::process::Command::new(&exe_path);

    #[cfg(target_os = "macos")]
    let mut command = {
        let mut cmd = std::process::Command::new("open");
        cmd.arg("-a");
        cmd.arg(&exe_path);
        cmd
    };

    #[cfg(target_os = "linux")]
    let (proton_binary, compat_data_path) = {
        let proton_env_path = installer::proton_env_dir(&app)
            .map_err(|e| format!("proton_env path not found: {e}"))?;
        let proton_bin_path = installer::get_current_proton_dir_impl(&app)
            .map_err(|e| format!("proton path not found: {e}"))?
            .ok_or("found proton path but is None")?;
        let compat_pre_path = proton_env_path.join("wine_prefix");
        if !compat_pre_path.exists() {
            std::fs::create_dir(&compat_pre_path)
                .map_err(|e| format!("could not make prefix: {e}"))?;
        }
        (proton_bin_path.join("proton"), compat_pre_path)
    };

    #[cfg(target_os = "linux")]
    let mut command = {
        let steam_path = get_steam_client_path(&_app_path);
        let mut cmd = std::process::Command::new(&proton_binary);
        cmd.arg("run");
        cmd.arg(&exe_path);
        cmd.env("STEAM_COMPAT_DATA_PATH", &compat_data_path);
        cmd.env("STEAM_COMPAT_CLIENT_INSTALL_PATH", &steam_path);
        cmd.env("WINEDLLOVERRIDES", "winhttp=n,b");
        cmd.env_remove("PYTHONPATH");
        cmd.env_remove("PYTHONHOME");
        println!("{:?}", cmd);
        cmd
    };

    let child = command
        .current_dir(exe_dir)
        .spawn()
        .map_err(|e| format!("failed to launch: {e}"))?;

    let pid = child.id();
    let mut guard = state
        .child
        .lock()
        .map_err(|_| "game state lock poisoned".to_string())?;
    *guard = Some(child);
    Ok(pid)
}

#[tauri::command]
async fn launch_game_preset(
    app: tauri::AppHandle,
    version: u32,
    preset: String,
    practice: bool,
    state: State<'_, GameState>,
) -> Result<u32, String> {
    // Normalize preset and map to manifest tags.
    let tags = preset_tags_for_name(&preset);

    if practice {
        // Practice run: install + enable practice mods (compatible with this game version).
        let _ = prepare_practice_mods_for_version(&app, version, None).await?;
    }

    // Install preset-tagged mods additively (no overwrite).
    // For practice runs, install these AFTER practice mods so preset-specific pins can win.
    // (Practice list has its own pinning, e.g. for LethalNetworkAPI.)
    let preset_ids =
        prepare_tagged_mods_for_version(&app, version, &tags, "Preset Mods", None).await?;
    // Ensure preset mods are enabled for this run (even if they overlap practice-disable rules).
    // For practice runs we can apply immediately; for non-practice runs we must apply after
    // `ensure_practice_mods_disabled_for_version` which rewrites disablemod.json.
    if practice {
        let _ = force_enable_mods_for_version(&app, version, &preset_ids);
    }

    // Reuse normal launch path (includes "force-disable practice mods" for non-practice runs).
    // We inline the logic here to avoid refactoring large chunks right now.
    let dir = version_dir(&app, version)?;
    if !dir.exists() {
        return Err(format!(
            "version folder not found: {}",
            dir.to_string_lossy()
        ));
    }

    let exe_name = "Lethal Company.exe";
    let exe_path = dir.join(exe_name);
    let exe_path = if exe_path.exists() {
        exe_path
    } else {
        find_file_named(&dir, exe_name, 3)
            .ok_or_else(|| format!("{exe_name} not found under {}", dir.to_string_lossy()))?
    };
    let exe_dir = exe_path
        .parent()
        .ok_or_else(|| "invalid exe path".to_string())?;

    // If already running, return an error.
    {
        let mut guard = state
            .child
            .lock()
            .map_err(|_| "game state lock poisoned".to_string())?;
        if let Some(child) = guard.as_mut() {
            if child.try_wait().map_err(|e| e.to_string())?.is_none() {
                return Err("game is already running".to_string());
            }
        }
        *guard = None;
    }

    if !practice {
        // Non-practice run: force-disable practice mods.
        ensure_practice_mods_disabled_for_version(&app, version)?;
        // Re-enable preset mods for this run (Wesley includes LethalNetworkAPI which is otherwise forced off).
        let _ = force_enable_mods_for_version(&app, version, &preset_ids);
    }

    // Ensure disabled mods are applied for this version before launch.
    let _ = apply_disabled_mods_for_version(&app, version);
    // For HQoL specifically, also ensure `.old` matches disablemod.json on normal runs.
    let _ = sync_hqol_with_disablemod_for_version(&app, version);
    let _ = ensure_reverb_trigger_fix_cfg(&app, version);
    let _ = ensure_hqol_dont_store_item_cfg(&app, version, "DungeonKeyItem");
    if tags.iter().any(|t| t.eq_ignore_ascii_case("wesley")) {
        let lock_moons = !practice && !tags.iter().any(|t| t.eq_ignore_ascii_case("smhq"));
        let _ = ensure_wesley_moonscripts_cfg(&app, version, lock_moons);
    }

    // Runtime-only tag gating: disable tagged mods not relevant to this run.
    // (No disablemod.json writes; next run will re-evaluate.)
    let _ = disable_irrelevant_tagged_mods_for_run(&app, version, &tags, &[]).await;

    #[cfg(target_os = "windows")]
    let mut command = std::process::Command::new(&exe_path);

    #[cfg(target_os = "macos")]
    let mut command = {
        let mut cmd = std::process::Command::new("open");
        cmd.arg("-a");
        cmd.arg(&exe_path);
        cmd
    };

    #[cfg(target_os = "linux")]
    let (proton_binary, compat_data_path) = {
        let proton_env_path = installer::proton_env_dir(&app)
            .map_err(|e| format!("proton_env path not found: {e}"))?;
        let proton_bin_path = installer::get_current_proton_dir_impl(&app)
            .map_err(|e| format!("proton path not found: {e}"))?
            .ok_or("found proton path but is None")?;
        let compat_pre_path = proton_env_path.join("wine_prefix");
        if !compat_pre_path.exists() {
            std::fs::create_dir(&compat_pre_path)
                .map_err(|e| format!("could not make prefix: {e}"))?;
        }
        (proton_bin_path.join("proton"), compat_pre_path)
    };

    #[cfg(target_os = "linux")]
    let mut command = {
        let app_path = app
            .path()
            .app_data_dir()
            .map_err(|e| format!("app path not found: {e}"))?;
        let steam_path = get_steam_client_path(&app_path);
        let mut cmd = std::process::Command::new(&proton_binary);
        cmd.arg("run");
        cmd.arg(&exe_path);
        cmd.env("STEAM_COMPAT_DATA_PATH", &compat_data_path);
        cmd.env("STEAM_COMPAT_CLIENT_INSTALL_PATH", &steam_path);
        cmd.env("WINEDLLOVERRIDES", "winhttp=n,b");
        cmd.env_remove("PYTHONPATH");
        cmd.env_remove("PYTHONHOME");
        cmd
    };

    let child = command
        .current_dir(exe_dir)
        .spawn()
        .map_err(|e| format!("failed to launch: {e}"))?;

    let pid = child.id();
    let mut guard = state
        .child
        .lock()
        .map_err(|_| "game state lock poisoned".to_string())?;
    *guard = Some(child);
    Ok(pid)
}

async fn prepare_preset_for_version(
    app: &tauri::AppHandle,
    version: u32,
    preset: &str,
    practice: bool,
    cancel: Arc<AtomicBool>,
) -> Result<bool, String> {
    let tags = preset_tags_for_name(preset);

    if cancel.load(Ordering::Relaxed) {
        return Err("Cancelled".to_string());
    }

    // Wesley preset: ensure companion configs exist during prepare.
    if tags.iter().any(|t| t.eq_ignore_ascii_case("wesley")) {
        let _ = ensure_weather_registry_cfg(app, version);
        let lock_moons = !practice && !tags.iter().any(|t| t.eq_ignore_ascii_case("smhq"));
        let _ = ensure_wesley_moonscripts_cfg(app, version, lock_moons);
    }

    let practice_ids = if practice {
        prepare_practice_mods_for_version(app, version, Some(cancel.clone())).await?
    } else {
        // Selecting a non-practice run should disable practice mods now (not at launch).
        ensure_practice_mods_disabled_for_version(app, version)?;
        vec![]
    };

    if cancel.load(Ordering::Relaxed) {
        return Err("Cancelled".to_string());
    }

    let preset_ids =
        prepare_tagged_mods_for_version(app, version, &tags, "Preset Mods", Some(cancel.clone()))
            .await?;

    // Ensure preset mods are enabled (can override practice-disable overlap).
    let _ = force_enable_mods_for_version(app, version, &preset_ids);

    if cancel.load(Ordering::Relaxed) {
        return Err("Cancelled".to_string());
    }

    // Apply persisted disable list (user + practice) now.
    let _ = apply_disabled_mods_for_version(app, version);
    let _ = sync_hqol_with_disablemod_for_version(app, version);
    let _ = ensure_reverb_trigger_fix_cfg(app, version);
    let _ = ensure_hqol_dont_store_item_cfg(app, version, "DungeonKeyItem");

    // Runtime-only tag gating: disable tagged mods not relevant to this selected run.
    let _ = disable_irrelevant_tagged_mods_for_run(app, version, &tags, &practice_ids).await;

    Ok(true)
}

#[tauri::command]
async fn prepare_preset(
    app: tauri::AppHandle,
    version: u32,
    preset: String,
    practice: bool,
    state: State<'_, PrepareState>,
) -> Result<bool, String> {
    // Only allow one active prepare at a time; new prepares cancel previous ones.
    let cancel = Arc::new(AtomicBool::new(false));
    let id = state.next_id.fetch_add(1, Ordering::Relaxed) + 1;
    {
        let mut guard = state
            .active
            .lock()
            .map_err(|_| "prepare state lock poisoned".to_string())?;
        if let Some(active) = guard.as_ref() {
            active.cancel.store(true, Ordering::Relaxed);
        }
        *guard = Some(ActivePrepare {
            id,
            version,
            cancel: cancel.clone(),
        });
    }

    let res = prepare_preset_for_version(&app, version, &preset, practice, cancel.clone()).await;

    // Clear active prepare if it is still ours.
    {
        let mut guard = state
            .active
            .lock()
            .map_err(|_| "prepare state lock poisoned".to_string())?;
        if guard.as_ref().is_some_and(|a| a.id == id) {
            *guard = None;
        }
    }

    res
}

#[tauri::command]
fn cancel_prepare(version: u32, state: State<'_, PrepareState>) -> Result<bool, String> {
    let mut did = false;
    let guard = state
        .active
        .lock()
        .map_err(|_| "prepare state lock poisoned".to_string())?;
    if let Some(active) = guard.as_ref() {
        if active.version == version {
            active.cancel.store(true, Ordering::Relaxed);
            did = true;
        }
    }
    Ok(did)
}

#[tauri::command]
fn get_game_status(state: State<'_, GameState>) -> Result<GameStatus, String> {
    let mut guard = state
        .child
        .lock()
        .map_err(|_| "game state lock poisoned".to_string())?;
    if let Some(child) = guard.as_mut() {
        match child.try_wait().map_err(|e| e.to_string())? {
            None => Ok(GameStatus {
                running: true,
                pid: Some(child.id()),
            }),
            Some(_) => {
                *guard = None;
                Ok(GameStatus {
                    running: false,
                    pid: None,
                })
            }
        }
    } else {
        Ok(GameStatus {
            running: false,
            pid: None,
        })
    }
}

#[tauri::command]
fn stop_game(state: State<'_, GameState>) -> Result<bool, String> {
    let mut guard = state
        .child
        .lock()
        .map_err(|_| "game state lock poisoned".to_string())?;
    if let Some(mut child) = guard.take() {
        let _ = child.kill();
        let _ = child.wait();
        Ok(true)
    } else {
        Ok(false)
    }
}

#[tauri::command]
fn get_disabled_mods(app: tauri::AppHandle) -> Result<Vec<DisabledMod>, String> {
    Ok(read_disablemod(&app)?.mods)
}

#[tauri::command]
fn apply_disabled_mods(app: tauri::AppHandle, version: u32) -> Result<bool, String> {
    apply_disabled_mods_for_version(&app, version)?;
    Ok(true)
}

#[tauri::command]
fn set_mod_enabled(
    app: tauri::AppHandle,
    version: u32,
    dev: String,
    name: String,
    enabled: bool,
) -> Result<bool, String> {
    let mut list = read_disablemod(&app)?;

    // Use normalized ids in the file.
    let id = normalize_mod_id(&dev, &name);
    list.mods.retain(|m| m != &id);
    if !enabled {
        list.mods.push(id);
        list.mods
            .sort_by(|a, b| a.dev.cmp(&b.dev).then(a.name.cmp(&b.name)));
        list.mods.dedup();
    }
    write_disablemod(&app, &list)?;

    // Apply to current version immediately (still add-only / no overwrite).
    let plugins = plugins_dir(&app, version)?;
    if let Some(dir) = mod_dir_for(&plugins, &dev, &name) {
        let _ = set_mod_files_old_suffix(&dir, enabled);
    }
    let patchers = patchers_dir(&app, version)?;
    if let Some(dir) = mod_dir_for(&patchers, &dev, &name) {
        let _ = set_mod_files_old_suffix(&dir, enabled);
    }
    Ok(true)
}

#[tauri::command]
fn list_installed_mod_versions(
    app: tauri::AppHandle,
    version: u32,
) -> Result<Vec<InstalledModVersion>, String> {
    let plugins = plugins_dir(&app, version)?;
    if !plugins.exists() {
        return Ok(vec![]);
    }

    let mut out: Vec<InstalledModVersion> = vec![];
    let Ok(rd) = std::fs::read_dir(&plugins) else {
        return Ok(out);
    };

    for e in rd.flatten() {
        let path = e.path();
        if !path.is_dir() {
            continue;
        }

        // Plugin folder naming is deterministic: "{dev}-{name}"
        let folder = e.file_name().to_string_lossy().to_string();
        let Some((dev, name)) = folder.split_once('-') else {
            continue;
        };

        // When disabled, we suffix every file with `.old`, including manifest.json.
        let manifest_path = path.join("manifest.json");
        let manifest_old_path = path.join("manifest.json.old");

        let manifest = if manifest_path.exists() {
            read_manifest(&manifest_path)
        } else if manifest_old_path.exists() {
            read_manifest(&manifest_old_path)
        } else {
            continue;
        };

        match manifest {
            Ok(m) => {
                out.push(InstalledModVersion {
                    dev: dev.to_string(),
                    name: name.to_string(),
                    version: m.version_number,
                    icon_path: find_mod_icon_path(&path),
                    description: if m.description.trim().is_empty() {
                        None
                    } else {
                        Some(m.description)
                    },
                });
            }
            Err(err) => {
                log::warn!(
                    "Failed to read plugin manifest for {} (v{}): {}",
                    folder,
                    version,
                    err
                );
            }
        }
    }

    out.sort_by(|a, b| a.dev.cmp(&b.dev).then(a.name.cmp(&b.name)));
    Ok(out)
}

#[tauri::command]
async fn get_manifest() -> Result<ManifestDto, String> {
    let client = reqwest::Client::new();
    let (version, cfg, chain_config, manifests) =
        mod_config::ModsConfig::fetch_manifest(&client).await?;
    Ok(ManifestDto {
        version,
        chain_config,
        mods: cfg.mods,
        manifests,
    })
}

#[tauri::command]
fn get_practice_mod_list() -> Vec<mod_config::ModEntry> {
    variable::get_practice_mod_list()
}

#[tauri::command]
fn list_installed_versions(app: tauri::AppHandle) -> Result<Vec<u32>, String> {
    let base = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("failed to resolve app data dir: {e}"))?
        .join("versions");

    let mut out: Vec<u32> = vec![];
    let Ok(rd) = std::fs::read_dir(&base) else {
        return Ok(out);
    };

    for e in rd {
        let Ok(e) = e else { continue };
        let path = e.path();
        if !path.is_dir() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
            continue;
        };
        let Some(num) = name.strip_prefix('v') else {
            continue;
        };
        if let Ok(v) = num.parse::<u32>() {
            out.push(v);
        }
    }
    out.sort_unstable();
    Ok(out)
}

#[tauri::command]
fn list_config_files(app: tauri::AppHandle) -> Result<Vec<String>, String> {
    let base = shared_config_dir(&app)?;
    if !base.exists() {
        return Ok(vec![]);
    }

    let mut out: Vec<String> = vec![];
    let base_canon = std::fs::canonicalize(&base).map_err(|e| e.to_string())?;

    let mut stack: Vec<std::path::PathBuf> = vec![base.clone()];
    while let Some(dir) = stack.pop() {
        for e in std::fs::read_dir(&dir).map_err(|e| e.to_string())? {
            let e = e.map_err(|e| e.to_string())?;
            let path = e.path();
            let ty = e.file_type().map_err(|e| e.to_string())?;
            if ty.is_dir() {
                stack.push(path);
                continue;
            }
            if !ty.is_file() {
                continue;
            }
            let canon = std::fs::canonicalize(&path).map_err(|e| e.to_string())?;
            if !canon.starts_with(&base_canon) {
                continue;
            }
            let rel = canon
                .strip_prefix(&base_canon)
                .unwrap_or(&canon)
                .to_string_lossy()
                .replace('\\', "/");
            out.push(rel.trim_start_matches('/').to_string());
        }
    }

    out.sort();
    Ok(out)
}

fn list_config_files_for_version_impl(
    app: &tauri::AppHandle,
    version: u32,
) -> Result<Vec<String>, String> {
    let base = version_config_dir(app, version)?;
    if !base.exists() {
        return Ok(vec![]);
    }

    let mut out: Vec<String> = vec![];
    let base_canon = std::fs::canonicalize(&base).map_err(|e| e.to_string())?;

    let mut stack: Vec<std::path::PathBuf> = vec![base.clone()];
    while let Some(dir) = stack.pop() {
        for e in std::fs::read_dir(&dir).map_err(|e| e.to_string())? {
            let e = e.map_err(|e| e.to_string())?;
            let path = e.path();
            let ty = e.file_type().map_err(|e| e.to_string())?;
            if ty.is_dir() {
                stack.push(path);
                continue;
            }
            if !ty.is_file() {
                continue;
            }
            let canon = std::fs::canonicalize(&path).map_err(|e| e.to_string())?;
            if !canon.starts_with(&base_canon) {
                continue;
            }
            let rel = canon
                .strip_prefix(&base_canon)
                .unwrap_or(&canon)
                .to_string_lossy()
                .replace('\\', "/");
            out.push(rel.trim_start_matches('/').to_string());
        }
    }

    out.sort();
    Ok(out)
}

#[tauri::command]
fn get_config_link_state(app: tauri::AppHandle) -> Result<installer::ConfigLinkState, String> {
    installer::get_config_link_state(&app)
}

#[tauri::command]
fn get_config_link_state_for_version(
    app: tauri::AppHandle,
    version: u32,
) -> Result<installer::VersionConfigLinkState, String> {
    installer::get_config_link_state_for_version(&app, version)
}

#[tauri::command]
fn link_config(app: tauri::AppHandle) -> Result<installer::ConfigLinkState, String> {
    let _ = installer::link_config_for_all_versions(&app)?;
    installer::get_config_link_state(&app)
}

#[tauri::command]
fn link_config_for_version(
    app: tauri::AppHandle,
    version: u32,
) -> Result<installer::VersionConfigLinkState, String> {
    installer::link_config_for_version(&app, version)
}

#[tauri::command]
fn unlink_config(app: tauri::AppHandle) -> Result<installer::ConfigLinkState, String> {
    let _ = installer::unlink_config_for_all_versions(&app)?;
    installer::get_config_link_state(&app)
}

#[tauri::command]
fn unlink_config_for_version(
    app: tauri::AppHandle,
    version: u32,
) -> Result<installer::VersionConfigLinkState, String> {
    installer::unlink_config_for_version(&app, version)
}

#[tauri::command]
fn list_config_files_for_mod(
    app: tauri::AppHandle,
    dev: String,
    name: String,
) -> Result<Vec<String>, String> {
    let all = list_config_files(app)?;
    let d = dev.to_lowercase();
    let n = name.to_lowercase();
    Ok(all
        .into_iter()
        .filter(|p| {
            let lp = p.to_lowercase();
            lp.contains(&d) || lp.contains(&n)
        })
        .collect())
}

#[tauri::command]
fn list_config_files_for_mod_for_version(
    app: tauri::AppHandle,
    version: u32,
    dev: String,
    name: String,
) -> Result<Vec<String>, String> {
    let all = list_config_files_for_version_impl(&app, version)?;
    let d = dev.to_lowercase();
    let n = name.to_lowercase();
    Ok(all
        .into_iter()
        .filter(|p| {
            let lp = p.to_lowercase();
            lp.contains(&d) || lp.contains(&n)
        })
        .collect())
}

#[tauri::command]
fn read_config_file(app: tauri::AppHandle, rel_path: String) -> Result<String, String> {
    let base = shared_config_dir(&app)?;
    let rel = std::path::Path::new(&rel_path);
    if !is_safe_rel_path(rel) {
        return Err("invalid path".to_string());
    }
    let path = base.join(rel);
    std::fs::read_to_string(&path).map_err(|e| e.to_string())
}

#[tauri::command]
fn read_bepinex_cfg(
    app: tauri::AppHandle,
    rel_path: String,
) -> Result<bepinex_cfg::FileData, String> {
    let base = shared_config_dir(&app)?;
    let rel = std::path::Path::new(&rel_path);
    if !is_safe_rel_path(rel) {
        return Err("invalid path".to_string());
    }
    let path = base.join(rel);
    let text = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    bepinex_cfg::parse(&text)
}

#[tauri::command]
fn read_bepinex_cfg_for_version(
    app: tauri::AppHandle,
    version: u32,
    rel_path: String,
) -> Result<bepinex_cfg::FileData, String> {
    let base = version_config_dir(&app, version)?;
    let rel = std::path::Path::new(&rel_path);
    if !is_safe_rel_path(rel) {
        return Err("invalid path".to_string());
    }
    let path = base.join(rel);
    let text = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    bepinex_cfg::parse(&text)
}

#[derive(Debug, Clone, Deserialize)]
struct SetBepInExEntryArgs {
    rel_path: String,
    section: String,
    entry: String,
    value: bepinex_cfg::Value,
}

#[tauri::command]
fn set_bepinex_cfg_entry(app: tauri::AppHandle, args: SetBepInExEntryArgs) -> Result<bool, String> {
    let base = shared_config_dir(&app)?;
    let rel = std::path::Path::new(&args.rel_path);

    log::info!("set_bepinex_cfg_entry: {:?}", args);

    if !is_safe_rel_path(rel) {
        return Err("invalid path".to_string());
    }
    let path = base.join(rel);

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    // If the cfg doesn't exist yet, start from an empty file and create the
    // requested section/entry.
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(e.to_string()),
    };
    let mut file = bepinex_cfg::parse(&text)?;

    let section = match file.sections.iter_mut().find(|s| s.name == args.section) {
        Some(s) => s,
        None => {
            file.sections.push(bepinex_cfg::Section {
                name: args.section.clone(),
                entries: vec![],
            });
            file.sections
                .iter_mut()
                .find(|s| s.name == args.section)
                .ok_or("failed to create section".to_string())?
        }
    };

    match section.entries.iter_mut().find(|e| e.name == args.entry) {
        Some(e) => {
            e.value = args.value;
        }
        None => {
            section.entries.push(bepinex_cfg::Entry {
                name: args.entry,
                description: None,
                default: None,
                value: args.value,
            });
        }
    }

    let new_text = bepinex_cfg::write(&file)?;
    std::fs::write(&path, new_text).map_err(|e| e.to_string())?;
    Ok(true)
}

#[tauri::command]
fn set_bepinex_cfg_entry_for_version(
    app: tauri::AppHandle,
    version: u32,
    args: SetBepInExEntryArgs,
) -> Result<bool, String> {
    let base = version_config_dir(&app, version)?;
    let rel = std::path::Path::new(&args.rel_path);

    log::info!("set_bepinex_cfg_entry_for_version(v{version}): {:?}", args);

    if !is_safe_rel_path(rel) {
        return Err("invalid path".to_string());
    }
    let path = base.join(rel);

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    // If the cfg doesn't exist yet, start from an empty file and create the
    // requested section/entry.
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(e.to_string()),
    };
    let mut file = bepinex_cfg::parse(&text)?;

    let section = match file.sections.iter_mut().find(|s| s.name == args.section) {
        Some(s) => s,
        None => {
            file.sections.push(bepinex_cfg::Section {
                name: args.section.clone(),
                entries: vec![],
            });
            file.sections
                .iter_mut()
                .find(|s| s.name == args.section)
                .ok_or("failed to create section".to_string())?
        }
    };

    match section.entries.iter_mut().find(|e| e.name == args.entry) {
        Some(e) => {
            e.value = args.value;
        }
        None => {
            section.entries.push(bepinex_cfg::Entry {
                name: args.entry,
                description: None,
                default: None,
                value: args.value,
            });
        }
    }

    let new_text = bepinex_cfg::write(&file)?;
    std::fs::write(&path, new_text).map_err(|e| e.to_string())?;
    Ok(true)
}

#[derive(Debug, Clone, Deserialize)]
struct WriteConfigArgs {
    rel_path: String,
    contents: String,
}

#[tauri::command]
fn write_config_file(app: tauri::AppHandle, args: WriteConfigArgs) -> Result<bool, String> {
    let base = shared_config_dir(&app)?;
    let rel = std::path::Path::new(&args.rel_path);
    if !is_safe_rel_path(rel) {
        return Err("invalid path".to_string());
    }
    let path = base.join(rel);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(&path, args.contents).map_err(|e| e.to_string())?;
    Ok(true)
}

// =========================
// 🔹 AUTO-UPDATE COMMANDS
// =========================

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    name: String,
    published_at: String,
    body: Option<String>,
    assets: Vec<GitHubAsset>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GitHubAsset {
    name: String,
    browser_download_url: String,
    size: u64,
}

#[derive(Debug, Clone, Serialize)]
struct UpdateInfo {
    available: bool,
    current_version: String,
    version: Option<String>,
    date: Option<String>,
    body: Option<String>,
}

#[tauri::command]
async fn check_app_update(app: tauri::AppHandle) -> Result<UpdateInfo, String> {
    use semver::Version;

    let current_version_str = app.package_info().version.to_string();

    // GitHub Releases API에서 최신 릴리즈 가져오기
    let client = reqwest::Client::new();
    let github_release_url = "https://api.github.com/repos/p-asta/hq-launcher/releases/latest";

    let github_release: GitHubRelease = client
        .get(github_release_url)
        .header("User-Agent", "hq-launcher-updater")
        .send()
        .await
        .map_err(|e| format!("Failed to fetch GitHub release: {e}"))?
        .json()
        .await
        .map_err(|e| format!("Failed to parse GitHub release: {e}"))?;

    // 버전 비교 (tag_name에서 v 제거)
    let latest_version_str = github_release.tag_name.trim_start_matches('v').to_string();
    let current_version = Version::parse(&current_version_str)
        .map_err(|e| format!("Failed to parse current version: {e}"))?;
    let latest_version = Version::parse(&latest_version_str)
        .map_err(|e| format!("Failed to parse latest version: {e}"))?;

    let available = latest_version > current_version;

    Ok(UpdateInfo {
        available,
        current_version: current_version_str,
        version: if available {
            Some(latest_version_str.clone())
        } else {
            None
        },
        date: Some(github_release.published_at),
        body: github_release.body,
    })
}

#[derive(Debug, Clone, Serialize)]
struct UpdateProgress {
    downloaded: u64,
    total: u64,
    percent: f64,
}

#[tauri::command]
async fn download_app_update(app: tauri::AppHandle) -> Result<bool, String> {
    use tauri_plugin_updater::UpdaterExt;

    // Tauri updater 사용 (엔드포인트는 tauri.conf.json에서 설정, GitHub Releases latest.json)
    let updater = app
        .updater_builder()
        .build()
        .map_err(|e| format!("Failed to initialize updater: {e}"))?;

    let update = updater
        .check()
        .await
        .map_err(|e| format!("Failed to check for updates: {e}"))?
        .ok_or("No update available")?;

    // Download the update with progress tracking
    // on_chunk: FnMut(chunk_length: usize, content_length: Option<u64>)
    // on_download_finish: FnOnce()
    let mut downloaded = 0u64;
    update
        .download(
            |chunk_length, content_length| {
                downloaded += chunk_length as u64;
                if let Some(total) = content_length {
                    let percent = (downloaded as f64 / total as f64) * 100.0;
                    log::debug!(
                        "Update download progress: {:.2}% ({}/{} bytes)",
                        percent,
                        downloaded,
                        total
                    );
                } else {
                    log::debug!("Update download progress: {} bytes downloaded", downloaded);
                }
            },
            || {
                log::info!("Update download finished");
            },
        )
        .await
        .map_err(|e| format!("Failed to download update: {e}"))?;

    Ok(true)
}

#[tauri::command]
#[cfg(not(target_os = "macos"))]
async fn get_global_shortcut(_app: tauri::AppHandle, shortcut: String) -> Result<String, String> {
    let shortcut = shortcut.replace("CommandOrControl", "Ctrl");
    Ok(shortcut)
}

#[tauri::command]
#[cfg(target_os = "macos")]
async fn get_global_shortcut(_app: tauri::AppHandle, shortcut: String) -> Result<String, String> {
    let shortcut = shortcut.replace("CommandOrControl", "Cmd");
    Ok(shortcut)
}

#[tauri::command]
async fn install_app_update(app: tauri::AppHandle) -> Result<bool, String> {
    use tauri_plugin_updater::UpdaterExt;

    // Tauri updater 사용 (엔드포인트는 tauri.conf.json에서 설정, GitHub Releases latest.json)
    let updater = app
        .updater_builder()
        .build()
        .map_err(|e| format!("Failed to initialize updater: {e}"))?;

    let update = updater
        .check()
        .await
        .map_err(|e| format!("Failed to check for updates: {e}"))?
        .ok_or("No update available")?;

    // Download and install the update
    // on_chunk: FnMut(chunk_length: usize, content_length: Option<u64>)
    // on_download_finish: FnOnce()
    let mut downloaded = 0u64;
    update
        .download_and_install(
            |chunk_length, content_length| {
                downloaded += chunk_length as u64;
                if let Some(total) = content_length {
                    let percent = (downloaded as f64 / total as f64) * 100.0;
                    log::debug!(
                        "Update download progress: {:.2}% ({}/{} bytes)",
                        percent,
                        downloaded,
                        total
                    );
                } else {
                    log::debug!("Update download progress: {} bytes downloaded", downloaded);
                }
            },
            || {
                log::info!("Update download finished, installing...");
            },
        )
        .await
        .map_err(|e| format!("Failed to download and install update: {e}"))?;

    Ok(true)
}

#[tauri::command]
fn get_app_version(app: tauri::AppHandle) -> Result<String, String> {
    Ok(app.package_info().version.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(GameState::default())
        .manage(DownloadState::default())
        .manage(PrepareState::default())
        .manage(discord_presence::DiscordPresenceState::default())
        .manage(downloader::DepotLoginState::default())
        .setup(|app| {
            // File logging (AppDataDir/logs/hq-launcher.log)
            logger::init(&app.handle()).map_err(|e| tauri::Error::Setup(e.into()))?;

            // Startup housekeeping (best-effort, won't block UI):
            // - Purge mods that remote manifest marks as enabled=false (and their configs)
            // - Ensure default config is downloaded if shared config dir is empty
            // - Warm the Thunderstore package cache for later update checks
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) =
                    installer::purge_remote_disabled_mods_on_startup(app_handle.clone()).await
                {
                    log::warn!("Failed to purge remote-disabled mods on startup: {e}");
                }
                if let Err(e) = installer::ensure_default_config(app_handle.clone()).await {
                    log::warn!("Failed to ensure default config on startup: {e}");
                }
                if let Err(e) = installer::ensure_pack_specific_configs_on_startup(&app_handle) {
                    log::warn!("Failed to ensure pack-specific configs on startup: {e}");
                }
                match thunderstore_cache_path(&app_handle) {
                    Ok(cache_path) => {
                        let client = reqwest::Client::new();
                        if let Err(e) =
                            thunderstore::fetch_community_packages(&client, &cache_path).await
                        {
                            log::warn!("Failed to warm Thunderstore cache on startup: {e}");
                        }
                    }
                    Err(e) => {
                        log::warn!("Failed to resolve Thunderstore cache path on startup: {e}");
                    }
                }
                #[cfg(target_os = "linux")]
                {
                    if let Err(e) = installer::install_proton_ge_impl(&app_handle).await {
                        log::warn!("Failed to install Proton-GE on startup: {e}");
                    }
                }
            });

            Ok(())
        })
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            download,
            cancel_download,
            prepare_preset,
            cancel_prepare,
            sync_latest_install_from_manifest,
            check_mod_updates,
            apply_mod_updates,
            launch_game,
            launch_game_practice,
            launch_game_preset,
            get_game_status,
            stop_game,
            get_disabled_mods,
            apply_disabled_mods,
            set_mod_enabled,
            list_installed_mod_versions,
            get_manifest,
            get_practice_mod_list,
            list_installed_versions,
            list_config_files,
            get_config_link_state,
            link_config,
            unlink_config,
            get_config_link_state_for_version,
            link_config_for_version,
            unlink_config_for_version,
            list_config_files_for_mod_for_version,
            list_config_files_for_mod,
            read_config_file,
            read_bepinex_cfg,
            read_bepinex_cfg_for_version,
            set_bepinex_cfg_entry,
            set_bepinex_cfg_entry_for_version,
            write_config_file,
            downloader::depot_login,
            downloader::depot_login_start,
            downloader::depot_login_submit_code,
            downloader::depot_get_login_state,
            downloader::depot_logout,
            downloader::depot_download,
            downloader::depot_download_files,
            check_app_update,
            download_app_update,
            install_app_update,
            get_app_version,
            installer::install_proton_ge,
            installer::get_current_proton_dir,
            delete_installed_version,
            open_version_folder,
            open_downloader_folder,
            open_mod_folder,
            get_global_shortcut,
            discord_presence::set_discord_presence,
            discord_presence::clear_discord_presence
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

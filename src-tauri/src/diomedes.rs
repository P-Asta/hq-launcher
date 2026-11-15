//! Diomedes uses the newest catalog game and unpinned Thunderstore packages.
use crate::{mod_config::ModEntry, thunderstore::PackageListing};
use std::collections::{BTreeMap, HashMap, HashSet};

/// Retire only the preset's own payload; dependencies may be shared with HQ.
pub fn remove_retired_mod(game_root: &std::path::Path) -> Result<(), String> {
    let game_root = game_root.canonicalize().map_err(|error| error.to_string())?;
    for section in ["plugins", "patchers"] {
        let root = game_root.join("BepInEx").join(section);
        if !root.exists() { continue; }
        for entry in std::fs::read_dir(&root).map_err(|error| error.to_string())? {
            let entry = entry.map_err(|error| error.to_string())?;
            if !entry.file_name().to_string_lossy().eq_ignore_ascii_case("DiomedesLC-Diomedes") { continue; }
            let path = entry.path();
            let actual = path.canonicalize().map_err(|error| error.to_string())?;
            if !actual.starts_with(&game_root)
                || actual.parent() != Some(root.canonicalize().map_err(|error| error.to_string())?.as_path())
                || !actual.file_name().is_some_and(|name| name.to_string_lossy().eq_ignore_ascii_case("DiomedesLC-Diomedes"))
                || entry.file_type().map_err(|error| error.to_string())?.is_symlink() {
                return Err("Refusing to remove a redirected Diomedes mod directory".into());
            }
            std::fs::remove_dir_all(path).map_err(|error| error.to_string())?;
        }
    }
    Ok(())
}

pub fn is_excluded_mod(spec: &ModEntry) -> bool {
    let key = format!("{}::{}", spec.dev.to_lowercase(), spec.name.to_lowercase());
    matches!(key.as_str(), "hqhqteam::vlog" | "hqhqteam::hqol" | "pooble::lcbettersaves" | "timewaste::overworkedui" | "asta::lcf" | "asta::evlog" | "asta::replay" | "mikuoreo::lcstatstracker")
        || spec.switch_group.as_deref().is_some_and(|group| group.trim().eq_ignore_ascii_case("save-ui"))
}

pub fn excluded_mod_ids(specs: &[ModEntry]) -> Vec<(String, String)> {
    let mut ids: Vec<_> = [("HQHQTeam", "VLog"), ("HQHQTeam", "HQoL"), ("Pooble", "LCBetterSaves"), ("timewaste", "OverworkedUI"), ("asta", "LCF"), ("asta", "EVlog"), ("asta", "Replay"), ("mikuoreo", "LCStatsTracker")]
        .into_iter().map(|(dev, name)| (dev.to_string(), name.to_string())).collect();
    ids.extend(specs.iter().filter(|spec| is_excluded_mod(spec)).map(|spec| (spec.dev.clone(), spec.name.clone())));
    ids
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct InstallReceipt {
    pub config: crate::mod_config::ModsConfig,
    pub versions: HashMap<String, String>,
}

pub fn verify_installed_versions(
    expected: &HashMap<String, String>,
    actual: &HashMap<String, String>,
) -> Result<(), String> {
    if expected.is_empty() {
        return Err("The installation receipt contains no packages".into());
    }
    for (key, version) in expected {
        if actual.get(key) != Some(version) {
            return Err(format!("Package version mismatch for {key}: expected {version}, installed {}. Prepare the selected preset again.", actual.get(key).map(String::as_str).unwrap_or("missing")));
        }
    }
    Ok(())
}

pub fn latest_versions_for_mods(
    mods: &[ModEntry],
    packages: &[PackageListing],
) -> Result<HashMap<String, String>, String> {
    mods.iter()
        .map(|m| {
            let package = packages
                .iter()
                .find(|p| {
                    p.owner.eq_ignore_ascii_case(&m.dev) && p.name.eq_ignore_ascii_case(&m.name)
                })
                .ok_or_else(|| format!("Missing package: {}-{}", m.dev, m.name))?;
            let version = crate::mods::latest_pkg_version(&package.versions)
                .ok_or_else(|| format!("Missing version: {}-{}", m.dev, m.name))?;
            Ok((
                format!("{}::{}", m.dev.to_lowercase(), m.name.to_lowercase()),
                version.version_number.clone(),
            ))
        })
        .collect()
}

pub fn validate_version(version: u32, manifests: &BTreeMap<u32, String>) -> Result<(), String> {
    match manifests.keys().next_back() {
        Some(&latest) if version == latest => Ok(()),
        Some(latest) => Err(format!(
            "Diomedes requires the latest game version (v{latest})"
        )),
        None => Err("Diomedes requires a game version catalog".into()),
    }
}

pub fn resolve_mods(
    roots: &[ModEntry],
    packages: &[PackageListing],
) -> Result<Vec<ModEntry>, String> {
    let by_key: HashMap<_, _> = packages
        .iter()
        .map(|p| {
            (
                format!("{}::{}", p.owner.to_lowercase(), p.name.to_lowercase()),
                p,
            )
        })
        .collect();
    let mut pending: Vec<_> = roots
        .iter()
        .map(|m| (m.dev.clone(), m.name.clone()))
        .collect();
    let mut seen = HashSet::new();
    let mut resolved = vec![];
    while let Some((dev, name)) = pending.pop() {
        let key = format!("{}::{}", dev.to_lowercase(), name.to_lowercase());
        if !seen.insert(key.clone()) {
            continue;
        }
        let package = by_key
            .get(&key)
            .ok_or_else(|| format!("Package not found in Thunderstore: {dev}-{name}"))?;
        let latest = package
            .versions
            .iter()
            .filter_map(|v| {
                semver::Version::parse(&v.version_number)
                    .ok()
                    .map(|n| (n, v))
            })
            .max_by(|a, b| a.0.cmp(&b.0))
            .map(|(_, v)| v)
            .ok_or_else(|| format!("No valid version for {dev}-{name}"))?;
        for dependency in &latest.dependencies {
            let (package_id, minimum) = dependency
                .rsplit_once('-')
                .ok_or_else(|| format!("Invalid dependency: {dependency}"))?;
            let (owner, package_name) = package_id
                .split_once('-')
                .ok_or_else(|| format!("Invalid dependency: {dependency}"))?;
            let minimum = semver::Version::parse(minimum)
                .map_err(|_| format!("Invalid dependency: {dependency}"))?;
            let dependency_key =
                format!("{}::{}", owner.to_lowercase(), package_name.to_lowercase());
            let dependency_package = by_key
                .get(&dependency_key)
                .ok_or_else(|| format!("Missing dependency: {dependency}"))?;
            if !dependency_package
                .versions
                .iter()
                .any(|v| semver::Version::parse(&v.version_number).is_ok_and(|v| v >= minimum))
            {
                return Err(format!("No compatible dependency version: {dependency}"));
            }
            pending.push((owner.to_string(), package_name.to_string()));
        }
        resolved.push(serde_json::from_value(serde_json::json!({ "dev": package.owner, "name": package.name, "enabled": true, "tags": ["Diomedes"] })).map_err(|e| e.to_string())?);
    }
    resolved.sort_by(|a: &ModEntry, b: &ModEntry| a.dev.cmp(&b.dev).then(a.name.cmp(&b.name)));
    Ok(resolved)
}

pub fn shared_pins_to_restore(
    profile: &[ModEntry],
    normal: &[ModEntry],
    version: u32,
    installed: &HashMap<String, String>,
) -> Vec<ModEntry> {
    let shared: HashSet<_> = profile
        .iter()
        .map(|m| format!("{}::{}", m.dev.to_lowercase(), m.name.to_lowercase()))
        .collect();
    normal
        .iter()
        .filter(|m| {
            let key = format!("{}::{}", m.dev.to_lowercase(), m.name.to_lowercase());
            shared.contains(&key)
                && m.pinned_version_for(version)
                    .is_some_and(|pin| installed.get(&key).map(String::as_str) != Some(pin))
        })
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn retired_mod_cleanup_preserves_shared_packages_settings_and_saves() {
        let root = std::env::temp_dir().join(format!("hq-retire-diomedes-{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        for section in ["plugins", "patchers"] {
            for package in ["DiomedesLC-Diomedes", "Shared-Library"] {
                let directory = root.join("BepInEx").join(section).join(package);
                std::fs::create_dir_all(&directory).unwrap();
                std::fs::write(directory.join("mod.dll"), "payload").unwrap();
            }
        }
        std::fs::write(root.join("save.dat"), "save").unwrap();
        std::fs::create_dir_all(root.join("BepInEx/config")).unwrap();
        std::fs::write(root.join("BepInEx/config/diomedes.cfg"), "custom").unwrap();
        remove_retired_mod(&root).unwrap();
        remove_retired_mod(&root).unwrap();
        for section in ["plugins", "patchers"] {
            assert!(!root.join("BepInEx").join(section).join("DiomedesLC-Diomedes").exists());
            assert!(root.join("BepInEx").join(section).join("Shared-Library/mod.dll").exists());
        }
        assert_eq!(std::fs::read_to_string(root.join("save.dat")).unwrap(), "save");
        assert_eq!(std::fs::read_to_string(root.join("BepInEx/config/diomedes.cfg")).unwrap(), "custom");
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn selective_exclusions_keep_base_mods_and_disable_save_group_and_lcf() {
        for (dev, name) in [("HQHQTeam", "VLog"), ("HQHQTeam", "HQoL"), ("asta", "LCF"), ("Pooble", "LCBetterSaves"), ("timewaste", "OverworkedUI")] {
            assert!(is_excluded_mod(&entry(dev, name)), "{dev}-{name}");
        }
        let mut save = entry("New", "SaveUI");
        save.switch_group = Some("SAVE-UI".into());
        assert!(is_excluded_mod(&save));
        assert!(!is_excluded_mod(&entry("HQ", "QuotaTools")));
        assert!(!is_excluded_mod(&entry("LeKAKiD", "FontPatcher")));
        assert!(is_excluded_mod(&entry("asta", "Replay")));
        assert!(is_excluded_mod(&entry("mikuoreo", "LCStatsTracker")));
        assert!(crate::mod_has_run_mode_affinity(&entry("AsTa", "LcF")));
    }
    #[test]
    fn receipt_and_hq_restore_reject_missing_or_mismatched_shared_versions() {
        let expected = HashMap::from([("shared::library".into(), "2.0.0".into())]);
        assert!(verify_installed_versions(
            &expected,
            &HashMap::from([("shared::library".into(), "1.0.0".into())])
        )
        .is_err());
        assert!(verify_installed_versions(&expected, &HashMap::new()).is_err());
        assert!(verify_installed_versions(
            &expected,
            &HashMap::from([("shared::library".into(), "2.0.0".into())])
        )
        .is_ok());
    }
    fn entry(dev: &str, name: &str) -> ModEntry {
        serde_json::from_value(serde_json::json!({ "dev": dev, "name": name, "enabled": true }))
            .unwrap()
    }
    fn package(owner: &str, name: &str, versions: serde_json::Value) -> PackageListing {
        serde_json::from_value(serde_json::json!({ "owner": owner, "name": name, "full_name": format!("{owner}-{name}"), "versions": versions })).unwrap()
    }
    #[test]
    fn only_latest_catalog_game_is_allowed() {
        let manifests = BTreeMap::from([(49, "a".into()), (73, "b".into())]);
        assert!(validate_version(73, &manifests).is_ok());
        assert!(validate_version(49, &manifests).is_err());
        assert!(validate_version(74, &manifests).is_err());
        assert!(validate_version(73, &BTreeMap::new()).is_err());
    }
    #[test]
    fn hq_return_restores_only_shared_packages_whose_hq_pin_differs() {
        let mut shared = entry("Shared", "Library");
        shared.version_config.insert(49, "1.0.0".into());
        shared.version_config.insert(73, "1.5.0".into());
        let mut unrelated = entry("HQ", "Other");
        unrelated.version_config.insert(49, "1.0.0".into());
        let profile = vec![entry("Shared", "Library"), entry("DiomedesLC", "Diomedes")];
        let normal = vec![shared, unrelated];
        let installed = HashMap::from([("shared::library".into(), "2.0.0".into())]);
        let restore = shared_pins_to_restore(&profile, &normal, 73, &installed);
        assert_eq!(restore.len(), 1);
        assert_eq!(restore[0].pinned_version_for(73), Some("1.5.0"));
        assert_eq!(restore[0].pinned_version_for(49), Some("1.0.0"));
        assert!(shared_pins_to_restore(
            &profile,
            &normal,
            73,
            &HashMap::from([("shared::library".into(), "1.5.0".into())])
        )
        .is_empty());
    }
    #[test]
    fn preset_switch_enables_exact_selection_then_restores_hq_choices() {
        let root = std::env::temp_dir().join(format!(
            "hq-diomedes-switch-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        for folder in [
            "HQ-QuotaTools",
            "DiomedesLC-Diomedes",
            "Shared-Library",
            "LeKAKiD-FontPatcher",
            "HQHQTeam-VLog",
            "HQHQTeam-HQoL",
            "Pooble-LCBetterSaves",
            "asta-LCF",
            "HQ-DisabledByUser",
            "asta-Replay",
            "mikuoreo-LCStatsTracker",
        ] {
            std::fs::create_dir_all(root.join(folder)).unwrap();
            std::fs::write(root.join(folder).join("plugin.dll"), "payload").unwrap();
        }
        let saved_disabled = HashSet::from(["shared::library".into(), "hq::disabledbyuser".into()]);
        crate::sync_mod_dirs_with_effective_state(
            &root,
            &saved_disabled,
            &excluded_mod_ids(&[]).into_iter().map(|(dev, name)| format!("{}::{}", dev.to_lowercase(), name.to_lowercase())).collect(),
            &HashSet::from(["diomedeslc::diomedes".into()]),
        )
        .unwrap();
        assert!(root.join("HQ-QuotaTools/plugin.dll").exists());
        assert!(root.join("HQ-DisabledByUser/plugin.dll.old").exists());
        for folder in ["HQHQTeam-VLog", "HQHQTeam-HQoL", "Pooble-LCBetterSaves", "asta-LCF", "asta-Replay", "mikuoreo-LCStatsTracker"] {
            assert!(root.join(folder).join("plugin.dll.old").exists());
        }
        assert!(root.join("DiomedesLC-Diomedes/plugin.dll").exists());
        assert!(root.join("Shared-Library/plugin.dll.old").exists());
        crate::sync_fontpatcher_files(&[root.as_path()], true, false, false, false).unwrap();
        assert!(
            root.join("LeKAKiD-FontPatcher/plugin.dll").exists(),
            "Diomedes preserves the normal font patcher"
        );
        crate::sync_mod_dirs_with_effective_state(
            &root,
            &saved_disabled,
            &HashSet::from(["diomedeslc::diomedes".into()]),
            &HashSet::new(),
        )
        .unwrap();
        assert!(root.join("HQ-QuotaTools/plugin.dll").exists());
        assert!(root.join("DiomedesLC-Diomedes/plugin.dll.old").exists());
        assert!(root.join("Shared-Library/plugin.dll.old").exists());
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn resolves_latest_transitive_dependencies_without_pinning_or_duplicates() {
        let packages = vec![
            package(
                "Root",
                "Main",
                serde_json::json!([
                    { "version_number": "1.0.0", "download_url": "", "dependencies": ["Unused-Old-1.0.0"] },
                    { "version_number": "2.0.0", "download_url": "", "dependencies": ["Dep-Shared-1.0.0"] }
                ]),
            ),
            package(
                "Dep",
                "Shared",
                serde_json::json!([
                    { "version_number": "1.0.0", "download_url": "", "dependencies": [] },
                    { "version_number": "1.10.0", "download_url": "", "dependencies": ["Leaf-End-1.0.0"] }
                ]),
            ),
            package(
                "Leaf",
                "End",
                serde_json::json!([{ "version_number": "1.0.0", "download_url": "", "dependencies": ["Root-Main-1.0.0"] }]),
            ),
        ];
        let resolved =
            resolve_mods(&[entry("Root", "Main"), entry("dep", "shared")], &packages).unwrap();
        let names: HashSet<_> = resolved
            .iter()
            .map(|m| format!("{}-{}", m.dev, m.name))
            .collect();
        assert_eq!(
            names,
            HashSet::from(["Root-Main".into(), "Dep-Shared".into(), "Leaf-End".into()])
        );
        assert_eq!(resolved.len(), 3);
        let receipt = latest_versions_for_mods(&resolved, &packages).unwrap();
        assert_eq!(receipt.get("root::main").map(String::as_str), Some("2.0.0"));
        assert_eq!(
            receipt.get("dep::shared").map(String::as_str),
            Some("1.10.0")
        );
        assert_eq!(receipt.get("leaf::end").map(String::as_str), Some("1.0.0"));
        assert!(resolved.iter().all(|m| m.pinned_version_for(73).is_none()));
        assert!(resolve_mods(&[entry("Missing", "Package")], &packages).is_err());
    }
}

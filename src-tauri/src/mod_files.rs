//! Shared desktop/Android file toggle behavior.
fn collect_mod_entries_recursive(
    root: &std::path::Path,
) -> Result<Vec<std::path::PathBuf>, String> {
    let mut out: Vec<std::path::PathBuf> = vec![];
    let mut stack: Vec<std::path::PathBuf> = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for e in std::fs::read_dir(&dir).map_err(|e| e.to_string())? {
            let e = e.map_err(|e| e.to_string())?;
            let path = e.path();
            let ty = e.file_type().map_err(|e| e.to_string())?;
            out.push(path.clone());
            if ty.is_dir() {
                stack.push(path);
            }
        }
    }
    out.sort_by_key(|path| std::cmp::Reverse(path.components().count()));
    Ok(out)
}

fn is_launcher_visible_mod_asset_name(name: &str) -> bool {
    let base_name = if name.to_ascii_lowercase().ends_with(".old") {
        &name[..name.len().saturating_sub(4)]
    } else {
        name
    };
    std::path::Path::new(base_name)
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| {
            matches!(
                ext.to_ascii_lowercase().as_str(),
                "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "ico" | "svg" | "gpf"
            )
        })
}

pub(crate) fn set_mod_files_old_suffix(
    mod_dir: &std::path::Path,
    enabled: bool,
) -> Result<(), String> {
    if !mod_dir.exists() {
        return Ok(());
    }

    let entries = collect_mod_entries_recursive(mod_dir)?;

    if enabled {
        // Remove .old suffix
        for path in entries {
            let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
            if !name.to_lowercase().ends_with(".old") {
                continue;
            }
            let mut new_name = name.to_string();
            new_name.truncate(new_name.len().saturating_sub(4));
            let new_path = path.with_file_name(new_name);
            if new_path.exists() {
                // Don't overwrite; keep the .old file.
                continue;
            }
            std::fs::rename(&path, new_path).map_err(|e| e.to_string())?;
        }
    } else {
        // Add .old suffix
        for path in entries {
            let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
            if name.to_lowercase().ends_with(".old") {
                // Repair image assets renamed by older launcher versions.
                if path.is_file() && is_launcher_visible_mod_asset_name(name) {
                    let restored_name = &name[..name.len().saturating_sub(4)];
                    let restored_path = path.with_file_name(restored_name);
                    if !restored_path.exists() {
                        std::fs::rename(&path, restored_path).map_err(|e| e.to_string())?;
                    }
                }
                continue;
            }
            // Keep image assets readable so disabled mods can still show their
            // icons and previews in the launcher.
            if path.is_file() && is_launcher_visible_mod_asset_name(name) {
                continue;
            }
            let new_path = path.with_file_name(format!("{name}.old"));
            if new_path.exists() {
                continue;
            }
            std::fs::rename(&path, new_path).map_err(|e| e.to_string())?;
        }
    }

    Ok(())
}

pub(crate) fn reconcile_managed_mods(
    game: &std::path::Path,
    managed: &std::collections::BTreeSet<String>,
    enabled: &std::collections::BTreeMap<String, bool>,
) -> Result<(), String> {
    for (directory, key) in managed_mod_dirs(game, managed)? {
        active_payload(&directory)?; // Reject links before the rename operation.
        set_mod_files_old_suffix(&directory, enabled.get(&key) == Some(&true))?;
    }
    Ok(())
}

pub(crate) fn excluded_mods_are_disabled(
    game: &std::path::Path,
    managed: &std::collections::BTreeSet<String>,
    enabled: &std::collections::BTreeMap<String, bool>,
) -> Result<bool, String> {
    for (directory, key) in managed_mod_dirs(game, managed)? {
        if enabled.get(&key) != Some(&true) && active_payload(&directory)? {
            return Ok(false);
        }
    }
    Ok(true)
}

fn managed_mod_dirs(
    game: &std::path::Path,
    managed: &std::collections::BTreeSet<String>,
) -> Result<Vec<(std::path::PathBuf, String)>, String> {
    let mut result = Vec::new();
    for section in ["plugins", "patchers"] {
        let root = game.join("BepInEx").join(section);
        match std::fs::symlink_metadata(&root) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err("Symlink in managed mod files".into())
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error.to_string()),
        }
        for entry in std::fs::read_dir(root).map_err(|error| error.to_string())? {
            let entry = entry.map_err(|error| error.to_string())?;
            let folder = entry.file_name().to_string_lossy().to_string();
            let Some((dev, name)) = folder.split_once('-') else {
                continue;
            };
            let key = format!("{}::{}", dev.to_lowercase(), name.to_lowercase());
            if !managed.contains(&key) {
                continue;
            }
            let kind = entry.file_type().map_err(|error| error.to_string())?;
            if kind.is_symlink() {
                return Err("Symlink in managed mod files".into());
            }
            if kind.is_dir() {
                result.push((entry.path(), key));
            }
        }
    }
    Ok(result)
}

fn active_payload(directory: &std::path::Path) -> Result<bool, String> {
    let mut active = false;
    for path in collect_mod_entries_recursive(directory)? {
        let metadata = std::fs::symlink_metadata(&path).map_err(|error| error.to_string())?;
        if metadata.file_type().is_symlink() {
            return Err("Symlink in managed mod files".into());
        }
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or("Invalid mod filename")?;
        if metadata.is_file()
            && !name.to_ascii_lowercase().ends_with(".old")
            && !is_launcher_visible_mod_asset_name(name)
        {
            active = true;
        }
    }
    Ok(active)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{BTreeMap, BTreeSet};
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct Temp(PathBuf);
    impl Temp {
        fn new() -> Self {
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let root = std::env::temp_dir().join(format!(
                "hq-managed-mods-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir_all(&root).unwrap();
            Self(root)
        }
        fn plugin(&self, section: &str, folder: &str) -> PathBuf {
            let root = self.0.join("BepInEx").join(section).join(folder);
            std::fs::create_dir_all(&root).unwrap();
            std::fs::write(root.join("plugin.dll"), b"dll").unwrap();
            std::fs::write(root.join("manifest.json"), b"manifest").unwrap();
            std::fs::write(root.join("icon.png"), b"icon").unwrap();
            root
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn upgrading_game_disables_excluded_and_previously_removed_hq_mods_in_both_roots() {
        let temp = Temp::new();
        let legacy = temp.plugin("plugins", "Legacy-OnlyV73");
        let patcher = temp.plugin("patchers", "Legacy-OnlyV73");
        let removed = temp.plugin("plugins", "Previous-RemovedFromManifest");
        let current = temp.plugin("plugins", "Current-CompatibleV81");
        let unmanaged = temp.plugin("plugins", "User-Unmanaged");
        let managed = BTreeSet::from([
            "legacy::onlyv73".into(),
            "previous::removedfrommanifest".into(),
            "current::compatiblev81".into(),
        ]);
        let active = BTreeMap::from([("current::compatiblev81".into(), true)]);
        assert!(!excluded_mods_are_disabled(&temp.0, &managed, &active).unwrap());
        reconcile_managed_mods(&temp.0, &managed, &active).unwrap();
        for root in [legacy, patcher, removed] {
            assert!(root.join("plugin.dll.old").is_file());
            assert!(root.join("manifest.json.old").is_file());
            assert!(root.join("icon.png").is_file());
        }
        assert!(current.join("plugin.dll").is_file());
        assert!(unmanaged.join("plugin.dll").is_file());
        assert!(excluded_mods_are_disabled(&temp.0, &managed, &active).unwrap());
    }

    #[test]
    fn excluded_payload_cannot_look_disabled_just_because_its_manifest_is_old() {
        let temp = Temp::new();
        let excluded = temp.plugin("plugins", "Legacy-OnlyV73");
        set_mod_files_old_suffix(&excluded, false).unwrap();
        std::fs::create_dir_all(excluded.join("nested.old")).unwrap();
        std::fs::write(excluded.join("nested.old/leaked.dll"), b"dll").unwrap();
        let managed = BTreeSet::from(["legacy::onlyv73".into()]);
        assert!(!excluded_mods_are_disabled(&temp.0, &managed, &BTreeMap::new()).unwrap());
    }
    #[test]
    fn disabling_and_reenabling_nested_payload_preserves_visible_assets() {
        let root = std::env::temp_dir().join(format!("hq-mod-files-{}", std::process::id()));
        std::fs::create_dir_all(root.join("nested")).unwrap();
        std::fs::write(root.join("nested/plugin.dll"), b"plugin").unwrap();
        std::fs::write(root.join("manifest.json"), b"manifest").unwrap();
        std::fs::write(root.join("icon.png"), b"icon").unwrap();
        set_mod_files_old_suffix(&root, false).unwrap();
        assert!(root.join("nested.old/plugin.dll.old").is_file());
        assert!(root.join("manifest.json.old").is_file());
        assert!(root.join("icon.png").is_file());
        set_mod_files_old_suffix(&root, true).unwrap();
        assert_eq!(
            std::fs::read(root.join("nested/plugin.dll")).unwrap(),
            b"plugin"
        );
        assert!(root.join("manifest.json").is_file());
        std::fs::remove_dir_all(root).unwrap();
    }
}

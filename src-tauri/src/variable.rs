use std::collections::BTreeMap;

use crate::mod_config::ModEntry;

// Practice-run mods (Imperium, LethalDevMode, ...) live in manifest.json under `practice_mods`.

/// LCF - asta
/// Installed and enabled only for the LCF run mode; hardcoded here instead of the manifest.
pub fn get_lcf_mod_list() -> Vec<ModEntry> {
    vec![ModEntry {
        dev: "asta".to_string(),
        name: "LCF".to_string(),
        tags: vec![],
        enabled: true,
        switch_group: None,
        low_cap: None,
        high_cap: None,
        tag_constraints: BTreeMap::new(),
        version_config: BTreeMap::new(),
    }]
}

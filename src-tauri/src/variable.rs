use std::collections::BTreeMap;

use crate::mod_config::ModEntry;



/// LethalDevMode - megumin
/// Imperium - giosuel
/// OdinSerializer - Lordfirespeed
/// LethalNetworkAPI - xilophor
/// 56+
/// CruiserJumpPractice - aoirint
/// 
/// 
/// v70+: Imperium v1.1.1
/// v66 - v69: Imperium v0.2.8
/// v62 - v64: Imperium v0.2.7
/// v60: Imperium v0.2.2
/// v56: Imperium v0.2.1

pub fn get_practice_mod_list() -> Vec<ModEntry> {
    vec![
        ModEntry {
            dev: "giosuel".to_string(),
            name: "Imperium".to_string(),
            enabled: true,
            low_cap: Some(50),
            high_cap: None,
            version_config: BTreeMap::from(
                [
                    (50, "0.1.9".to_string()),
                    (56, "0.2.1".to_string()),
                    (60, "0.2.2".to_string()),
                    (62, "0.2.7".to_string()),
                    (66, "0.2.8".to_string()),
                    (70, "1.1.1".to_string()),
                ]
            ),
        },
        ModEntry {
            dev: "Lordfirespeed".to_string(),
            name: "OdinSerializer".to_string(),
            enabled: true,
            low_cap: Some(56),
            high_cap: None,
            version_config: BTreeMap::new(),
        },
        ModEntry {
            dev: "xilophor".to_string(),
            name: "LethalNetworkAPI".to_string(),
            enabled: true,
            low_cap: Some(56),
            high_cap: None,
            version_config: BTreeMap::from(
                [
                    (56, "2.2.0".to_string()),
                    (60, "3.2.0".to_string()),
                    (62, "3.2.1".to_string()),
                    (66, "3.3.1".to_string()),
                ]
            ),
        },
        ModEntry {
            dev: "megumin".to_string(),
            name: "LethalDevMode".to_string(),
            enabled: true,
            low_cap: Some(45),
            high_cap: None,
            version_config: BTreeMap::new(),
        },
        ModEntry {
            dev: "aoirint".to_string(),
            name: "CruiserJumpPractice".to_string(),
            enabled: true,
            low_cap: Some(56),
            high_cap: None,
            version_config: BTreeMap::new(),
        },
    ]
}
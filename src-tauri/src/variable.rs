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
            tags: vec![],
            enabled: true,
            low_cap: Some(50),
            high_cap: None,
            tag_constraints: BTreeMap::new(),
            version_config: BTreeMap::from([
                (50, "0.1.9".to_string()),
                (56, "0.2.1".to_string()),
                (60, "0.2.2".to_string()),
                (62, "0.2.7".to_string()),
                (66, "0.2.8".to_string()),
                (70, "1.1.1".to_string()),
                (80, "0.0.0".to_string()),
            ]),
        },
        ModEntry {
            dev: "Lordfirespeed".to_string(),
            name: "OdinSerializer".to_string(),
            tags: vec!["ui_hidden".to_string()],
            enabled: true,
            low_cap: Some(56),
            high_cap: None,
            tag_constraints: BTreeMap::new(),
            version_config: BTreeMap::new(),
        },
        ModEntry {
            dev: "xilophor".to_string(),
            name: "LethalNetworkAPI".to_string(),
            tags: vec!["ui_hidden".to_string()],
            enabled: true,
            low_cap: Some(56),
            high_cap: None,
            tag_constraints: BTreeMap::new(),
            version_config: BTreeMap::from([
                (56, "2.2.0".to_string()),
                (60, "3.2.0".to_string()),
                (62, "3.2.1".to_string()),
                (66, "3.3.1".to_string()),
                (69, "3.3.2".to_string()),
                (80, "0.0.0".to_string()),
            ]),
        },
        ModEntry {
            dev: "FlooflesDEV".to_string(),
            name: "LCSeedPicker".to_string(),
            tags: vec![],
            enabled: true,
            low_cap: Some(40),
            high_cap: Some(49),
            tag_constraints: BTreeMap::new(),
            version_config: BTreeMap::from([(40, "1.2.2".to_string())]),
        },
        ModEntry {
            dev: "kakeEdition".to_string(),
            name: "CoordinateForEasterEggs".to_string(),
            tags: vec![],
            enabled: true,
            low_cap: Some(50),
            high_cap: None,
            tag_constraints: BTreeMap::new(),
            version_config: BTreeMap::from([
                (50, "0.2.0".to_string())
            ]),
        },
        ModEntry {
            dev: "asta".to_string(),
            name: "CoordinateForEasterEggsFix".to_string(),
            tags: vec!["ui_hidden".to_string()],
            enabled: true,
            low_cap: Some(50),
            high_cap: Some(72),
            tag_constraints: BTreeMap::new(),
            version_config: BTreeMap::from([(50, "1.0.0".to_string())]),
        },
        ModEntry {
            dev: "Evaisa".to_string(),
            name: "LethalLib".to_string(),
            tags: vec!["ui_hidden".to_string()],
            enabled: true,
            low_cap: Some(50),
            high_cap: None,
            tag_constraints: BTreeMap::new(),
            version_config: BTreeMap::from([
                (50, "0.16.0".to_string()),
                (70, "1.1.0".to_string()),
                (73, "1.1.1".to_string()),
                (80, "0.0.0".to_string()),
            ]),
        },
        ModEntry {
            dev: "MonoDetour".to_string(),
            name: "MonoDetour_BepInEx_5".to_string(),
            tags: vec!["ui_hidden".to_string()],
            enabled: true,
            low_cap: Some(50),
            high_cap: None,
            tag_constraints: BTreeMap::new(),
            version_config: BTreeMap::from([
                (50, "0.6.3".to_string()),
                (80, "0.0.0".to_string())
            ]),
        },
        ModEntry {
            dev: "MonoDetour".to_string(),
            name: "MonoDetour".to_string(),
            tags: vec!["ui_hidden".to_string()],
            enabled: true,
            low_cap: Some(50),
            high_cap: None,
            tag_constraints: BTreeMap::new(),
            version_config: BTreeMap::from([
                (50, "0.6.3".to_string()),
                (80, "0.0.0".to_string())
            ]),
        },
        ModEntry {
            dev: "Evaisa".to_string(),
            name: "HookGenPatcher".to_string(),
            tags: vec!["ui_hidden".to_string()],
            enabled: true,
            low_cap: Some(50),
            high_cap: None,
            tag_constraints: BTreeMap::new(),
            version_config: BTreeMap::from([
                (50, "0.0.5".to_string()),
                (80, "0.0.0".to_string())
            ]),
        },
        ModEntry {
            dev: "megumin".to_string(),
            name: "LethalDevMode".to_string(),
            tags: vec![],
            enabled: true,
            low_cap: Some(45),
            high_cap: None,
            tag_constraints: BTreeMap::new(),
            version_config: BTreeMap::new(),
        },
        ModEntry {
            dev: "aoirint".to_string(),
            name: "CruiserJumpPractice".to_string(),
            tags: vec![],
            enabled: true,
            low_cap: Some(56),
            high_cap: None,
            tag_constraints: BTreeMap::new(),
            version_config: BTreeMap::new(),
        },
        ModEntry {
            dev: "Shinobi".to_string(),
            name: "DanceTools".to_string(),
            tags: vec![],
            enabled: true,
            low_cap: None,
            high_cap: Some(44),
            tag_constraints: BTreeMap::new(),
            version_config: BTreeMap::new(),
        },
        ModEntry {
            dev: "the_croods".to_string(),
            name: "FreeCammer".to_string(),
            tags: vec![],
            enabled: true,
            low_cap: None,
            high_cap: Some(49),
            tag_constraints: BTreeMap::new(),
            version_config: BTreeMap::new(),
        },
        ModEntry {
            dev: "Owen3H".to_string(),
            name: "IntroTweaks".to_string(),
            tags: vec![],
            enabled: true,
            low_cap: Some(50),
            high_cap: None,
            tag_constraints: BTreeMap::new(),
            version_config: BTreeMap::new(),
        },
        ModEntry {
            dev: "LethalCompanyModding".to_string(),
            name: "Yukieji_UnityExplorer".to_string(),
            tags: vec![],
            enabled: true,
            low_cap: None,
            high_cap: None,
            tag_constraints: BTreeMap::new(),
            version_config: BTreeMap::from([
                (40, "4.12.7".to_string()),
                (73, "4.13.1".to_string()),
            ]),
        },
    ]
}

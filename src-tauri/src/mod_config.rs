use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde::Deserializer;

/// New config format (requested):
/// - dev: thunderstore namespace/author
/// - name: thunderstore package name
/// - version_config: map of gameVersionLowerBound -> thunderstore version_number
/// - low_cap/high_cap: inclusive game version bounds for installation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModEntry {
    pub name: String,
    pub dev: String,

    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Inclusive installation range.
    #[serde(default)]
    pub low_cap: Option<u32>,
    #[serde(default)]
    pub high_cap: Option<u32>,

    /// Version pinning by game version thresholds.
    ///
    /// Example: { "56": "1.0.1", "73": "1.1.1" }
    /// Means:
    /// - game >= 56 uses 1.0.1
    /// - game >= 73 uses 1.1.1 (overrides)
    #[serde(default, deserialize_with="deserialize_version_config")]
    pub version_config: BTreeMap<u32, String>,
}

fn deserialize_version_config<'de, D>(deserializer: D) -> Result<BTreeMap<u32, String>, D::Error> where D: Deserializer<'de> {
    let string_map: BTreeMap<String, String> = BTreeMap::deserialize(deserializer)?;
    string_map
        .into_iter()
        .map(|(k, v)| {
            k.parse::<u32>()
                .map(|key| (key, v))
                .map_err(serde::de::Error::custom)
        })
        .collect()
}



#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModsConfig {
    pub mods: Vec<ModEntry>,
}

fn default_true() -> bool {
    true
}

// ---------- Public API ----------


#[derive(Debug, Clone, Deserialize)]
pub struct RemoteManifest {
    pub version: u32,
    pub mods: Vec<ModEntry>,
}

impl ModsConfig {
    #[allow(dead_code)]
    pub fn default_for_lethal_company() -> Self {
        Self {
            // low_cap 이상, high_cap 이하 버전에 설치
            mods: vec![
                ModEntry { dev: "HQHQTeam".into(), name: "VLog".into(), enabled: true, low_cap: None, high_cap: None, version_config: BTreeMap::new() },
                ModEntry { dev: "Chboo1".into(), name: "High_Quota_Fixes".into(), enabled: true, low_cap: None, high_cap: None, version_config: BTreeMap::new() },
                ModEntry { dev: "tinyhoot".into(), name: "ShipLoot".into(), enabled: true, low_cap: None, high_cap: None, version_config: BTreeMap::new() },
                ModEntry { dev: "Drakorle".into(), name: "MoreItems".into(), enabled: true, low_cap: None, high_cap: None, version_config: BTreeMap::new() },
                ModEntry { dev: "mattymatty".into(), name: "TooManyItems".into(), enabled: true, low_cap: None, high_cap: None, version_config: BTreeMap::new() },
                ModEntry { dev: "Zaggy1024".into(), name: "PathfindingLagFix".into(), enabled: true, low_cap: None, high_cap: None, version_config: BTreeMap::new() },
                ModEntry { dev: "LeKAKiD".into(), name: "FontPatcher".into(), enabled: true, low_cap: None, high_cap: None, version_config: BTreeMap::new() },
                ModEntry { dev: "ViVKo".into(), name: "NoSellLimit".into(), enabled: true, low_cap: None, high_cap: None, version_config: BTreeMap::new() },
                ModEntry { dev: "quackandcheese".into(), name: "ToggleMute".into(), enabled: true, low_cap: None, high_cap: None, version_config: BTreeMap::new() },
                ModEntry { dev: "Pooble".into(), name: "LCBetterSaves".into(), enabled: true, low_cap: None, high_cap: None, version_config: BTreeMap::new() },
                ModEntry { dev: "fumiko".into(), name: "CullFactory".into(), enabled: true, low_cap: None, high_cap: None, version_config: BTreeMap::new() },
                ModEntry { dev: "AdiBTW".into(), name: "Loadstone".into(), enabled: true, low_cap: None, high_cap: None, version_config: BTreeMap::new() },
                ModEntry { dev: "mrov".into(), name: "LightsOut".into(), enabled: true, low_cap: None, high_cap: None, version_config: BTreeMap::new() },
                ModEntry { dev: "Zehs".into(), name: "StreamOverlays".into(), enabled: true, low_cap: None, high_cap: None, version_config: BTreeMap::new() },
                ModEntry { dev: "MysticDEV".into(), name: "BetterCruiserSync".into(), enabled: true, low_cap: Some(56), high_cap: None, version_config: BTreeMap::new() },
                // Thunderstore: Hardy-LCMaxSoundsFix
                ModEntry { dev: "Hardy".into(), name: "LCMaxSoundsFix".into(), enabled: true, low_cap: None, high_cap: None, version_config: BTreeMap::new() },
                ModEntry { dev: "Scoops".into(), name: "LethalSpongeLegacy".into(), enabled: true, low_cap: None, high_cap: None, version_config: BTreeMap::new() },
            ],
        }
    }

    /// Load remote `manifest.json` (in-memory only; no file IO).
    ///
    /// New format:
    /// `{ "version": 1, "mods": [...] }`
    pub async fn fetch_manifest(client: &reqwest::Client) -> Result<(u32, Self), String> {
        let url = "https://f.asta.rs/hq-launcher/manifest.json";
        log::info!("Fetching manifest from {url}");

        let manifest = client
            .get(url)
            .send()
            .await
            .map_err(|e| e.to_string())?
            .error_for_status()
            .map_err(|e| e.to_string())?
            .json::<RemoteManifest>()
            .await
            .map_err(|e| e.to_string())?;

        let mut cfg = ModsConfig { mods: manifest.mods };
        let _ = normalize_aliases(&mut cfg);
        Ok((manifest.version, cfg))
    }
}

fn normalize_aliases(cfg: &mut ModsConfig) -> bool {
    let mut changed = false;
    for m in &mut cfg.mods {
        // Hardy-LCMaxSoundsFix (common typo: LCMaxSoundFix)
        if m.dev == "Hardy" && m.name == "LCMaxSoundFix" {
            m.name = "LCMaxSoundsFix".to_string();
            changed = true;
        }
    }
    changed
}

impl ModEntry {
    pub fn is_compatible(&self, game_version: u32) -> bool {
        if !self.enabled {
            return false;
        }
        if let Some(min) = self.low_cap {
            if game_version < min {
                return false;
            }
        }
        if let Some(max) = self.high_cap {
            if game_version > max {
                return false;
            }
        }
        true
    }

    pub fn pinned_version_for(&self, game_version: u32) -> Option<&str> {
        // Interpret `version_config` as "threshold pinning":
        // use the greatest key <= game_version.
        self.version_config
            .range(..=game_version)
            .next_back()
            .map(|(_, v)| v.as_str())
    }
}


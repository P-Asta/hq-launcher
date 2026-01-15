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
    pub chain_config: Vec<Vec<String>>,
    pub mods: Vec<ModEntry>,
}

impl ModsConfig {
    /// you can check json in https://f.asta.rs/hq-launcher/manifest.json
    /// output: (version, cfg, chain_config)
    pub async fn fetch_manifest(client: &reqwest::Client) -> Result<(u32, Self, Vec<Vec<String>>), String> {
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
        Ok((manifest.version, cfg, manifest.chain_config))
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


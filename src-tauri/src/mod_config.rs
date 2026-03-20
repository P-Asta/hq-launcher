use std::collections::BTreeMap;
use std::time::Duration;

use serde::Deserializer;
use serde::{Deserialize, Serialize};

/// New config format (requested):
/// - dev: thunderstore namespace/author
/// - name: thunderstore package name
/// - version_config: map of gameVersionLowerBound -> thunderstore version_number
/// - low_cap/high_cap: inclusive game version bounds for installation
/// - tag_constraints: optional per-tag low/high cap overrides
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TagConstraint {
    #[serde(default)]
    pub low_cap: Option<u32>,
    #[serde(default)]
    pub high_cap: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModEntry {
    pub name: String,
    pub dev: String,

    /// Optional tags for grouping mods by run presets (e.g. "Brutal", "Wesley").
    /// Missing field => empty tags.
    #[serde(default)]
    pub tags: Vec<String>,

    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Inclusive installation range.
    #[serde(default)]
    pub low_cap: Option<u32>,
    #[serde(default)]
    pub high_cap: Option<u32>,

    /// Optional per-tag compatibility overrides.
    /// If a tag rule omits a cap, the global low/high cap is used as fallback.
    #[serde(default)]
    pub tag_constraints: BTreeMap<String, TagConstraint>,

    /// Version pinning by game version thresholds.
    ///
    /// Example: { "56": "1.0.1", "73": "1.1.1" }
    /// Means:
    /// - game >= 56 uses 1.0.1
    /// - game >= 73 uses 1.1.1 (overrides)
    #[serde(default, deserialize_with = "deserialize_version_config")]
    pub version_config: BTreeMap<u32, String>,
}

fn deserialize_version_config<'de, D>(deserializer: D) -> Result<BTreeMap<u32, String>, D::Error>
where
    D: Deserializer<'de>,
{
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

fn deserialize_u32_string_map<'de, D>(deserializer: D) -> Result<BTreeMap<u32, String>, D::Error>
where
    D: Deserializer<'de>,
{
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
    #[serde(default, deserialize_with = "deserialize_u32_string_map")]
    pub manifests: BTreeMap<u32, String>,
    pub chain_config: Vec<Vec<String>>,
    pub mods: Vec<ModEntry>,
}

impl ModsConfig {
    /// you can check json in https://f.asta.rs/hq-launcher/manifest.json
    /// output: (manifest_version, cfg, chain_config, manifests)
    pub async fn fetch_manifest(
        client: &reqwest::Client,
    ) -> Result<(u32, Self, Vec<Vec<String>>, BTreeMap<u32, String>), String> {
        // Test mode: if a local `manifest.json` exists next to the repo/current folder,
        // prefer it over the remote manifest. This enables rapid iteration without publishing.
        fn try_read_local_manifest() -> Option<(std::path::PathBuf, RemoteManifest)> {
            let mut candidates: Vec<std::path::PathBuf> = vec![];

            if let Ok(cwd) = std::env::current_dir() {
                candidates.push(cwd.join("manifest.json"));
                candidates.push(cwd.join("..").join("manifest.json"));
            }

            if let Ok(exe) = std::env::current_exe() {
                // Walk up a few levels; in `tauri dev` this often lands under `target/`.
                let mut p = exe.parent().map(|p| p.to_path_buf());
                for _ in 0..8 {
                    let Some(dir) = p.take() else { break };
                    candidates.push(dir.join("manifest.json"));
                    p = dir.parent().map(|pp| pp.to_path_buf());
                }
            }

            for path in candidates {
                if !path.exists() {
                    continue;
                }
                let text = std::fs::read_to_string(&path).ok()?;
                let mf = serde_json::from_str::<RemoteManifest>(&text).ok()?;
                return Some((path, mf));
            }
            None
        }

        let manifest = if let Some((path, mf)) = try_read_local_manifest() {
            log::info!("Using local manifest: {}", path.to_string_lossy());
            mf
        } else {
            // Use stable remote manifest only.
            let url = "https://f.asta.rs/hq-launcher/manifest.json";
            log::info!("Fetching manifest from {url}");
            client
                .get(url)
                .timeout(Duration::from_secs(12))
                .send()
                .await
                .map_err(|e| e.to_string())?
                .error_for_status()
                .map_err(|e| e.to_string())?
                .json::<RemoteManifest>()
                .await
                .map_err(|e| e.to_string())?
        };

        let manifests = manifest.manifests.clone();
        let mut cfg = ModsConfig {
            mods: manifest.mods,
        };
        let _ = normalize_aliases(&mut cfg);
        Ok((manifest.version, cfg, manifest.chain_config, manifests))
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
    fn matches_caps(game_version: u32, low_cap: Option<u32>, high_cap: Option<u32>) -> bool {
        if let Some(min) = low_cap {
            if game_version < min {
                return false;
            }
        }
        if let Some(max) = high_cap {
            if game_version > max {
                return false;
            }
        }
        true
    }

    fn constraint_for_tag(&self, tag: &str) -> Option<&TagConstraint> {
        self.tag_constraints
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(tag))
            .map(|(_, value)| value)
    }

    pub fn is_compatible(&self, game_version: u32) -> bool {
        if !self.enabled {
            return false;
        }
        Self::matches_caps(game_version, self.low_cap, self.high_cap)
    }

    pub fn is_compatible_for_tags(&self, game_version: u32, active_tags: &[String]) -> bool {
        if !self.enabled {
            return false;
        }

        for active_tag in active_tags {
            if !self
                .tags
                .iter()
                .any(|tag| tag.eq_ignore_ascii_case(active_tag))
            {
                continue;
            }

            let constraint = self.constraint_for_tag(active_tag);
            let low_cap = constraint.and_then(|rule| rule.low_cap).or(self.low_cap);
            let high_cap = constraint.and_then(|rule| rule.high_cap).or(self.high_cap);

            if Self::matches_caps(game_version, low_cap, high_cap) {
                return true;
            }
        }

        false
    }

    pub fn pinned_version_for(&self, game_version: u32) -> Option<&str> {
        // Interpret `version_config` as "threshold pinning":
        // use the greatest key <= game_version.
        self.version_config
            .range(..=game_version)
            .next_back()
            .and_then(|(_, v)| {
                // Treat "0.0.0" as "no pin" => use latest version.
                if v.trim() == "0.0.0" {
                    None
                } else {
                    Some(v.as_str())
                }
            })
    }
}

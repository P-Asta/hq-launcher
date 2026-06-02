use std::collections::BTreeMap;
use std::future::Future;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering as AtomicOrdering;
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde::Deserializer;
use serde::{Deserialize, Serialize};

const REMOTE_MANIFEST_CACHE_TTL: Duration = Duration::from_secs(60);

type ManifestFetchResult = (
    u32,
    ModsConfig,
    Vec<Vec<String>>,
    BTreeMap<u32, String>,
    BTreeMap<String, TagConstraint>,
);

#[derive(Clone)]
struct CachedRemoteManifest {
    channel: crate::release_channel::ReleaseChannel,
    fetched_at: Instant,
    manifest: RemoteManifest,
}

#[derive(Default)]
struct RemoteManifestCache {
    cached: Option<CachedRemoteManifest>,
    fetching: Option<crate::release_channel::ReleaseChannel>,
}

static REMOTE_MANIFEST_CACHE: OnceLock<(Mutex<RemoteManifestCache>, Condvar)> = OnceLock::new();

fn remote_manifest_cache() -> &'static (Mutex<RemoteManifestCache>, Condvar) {
    REMOTE_MANIFEST_CACHE
        .get_or_init(|| (Mutex::new(RemoteManifestCache::default()), Condvar::new()))
}

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
    #[serde(default)]
    pub preset_tag_constraints: BTreeMap<String, TagConstraint>,
    pub chain_config: Vec<Vec<String>>,
    pub mods: Vec<ModEntry>,
}

impl ModsConfig {
    async fn await_with_cancel<T, F>(
        cancel: Option<&Arc<AtomicBool>>,
        future: F,
    ) -> Result<T, String>
    where
        F: Future<Output = Result<T, String>>,
    {
        tokio::pin!(future);
        let mut interval = tokio::time::interval(Duration::from_millis(100));
        loop {
            tokio::select! {
                result = &mut future => return result,
                _ = interval.tick() => {
                    if cancel.is_some_and(|c| c.load(AtomicOrdering::Relaxed)) {
                        return Err("Cancelled".to_string());
                    }
                }
            }
        }
    }

    /// you can check json in https://f.asta.rs/hq-launcher/manifest.json
    /// output: (manifest_version, cfg, chain_config, manifests, preset_tag_constraints)
    pub async fn fetch_manifest(client: &reqwest::Client) -> Result<ManifestFetchResult, String> {
        Self::fetch_manifest_with_cancel(client, None).await
    }

    pub async fn fetch_manifest_with_cancel(
        client: &reqwest::Client,
        cancel: Option<&Arc<AtomicBool>>,
    ) -> Result<ManifestFetchResult, String> {
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
            Self::fetch_remote_manifest_cached(client, cancel).await?
        };

        Ok(manifest.into_fetch_result())
    }

    async fn fetch_remote_manifest_cached(
        client: &reqwest::Client,
        cancel: Option<&Arc<AtomicBool>>,
    ) -> Result<RemoteManifest, String> {
        let channel = crate::release_channel::current();
        let now = Instant::now();
        let (cache_lock, cache_ready) = remote_manifest_cache();

        {
            let mut cache = cache_lock
                .lock()
                .map_err(|_| "remote manifest cache lock poisoned".to_string())?;
            loop {
                if let Some(cached) = cache.cached.as_ref() {
                    if cached.channel == channel
                        && now.duration_since(cached.fetched_at) < REMOTE_MANIFEST_CACHE_TTL
                    {
                        return Ok(cached.manifest.clone());
                    }
                }

                if cancel.is_none() && cache.fetching == Some(channel) {
                    cache = cache_ready
                        .wait(cache)
                        .map_err(|_| "remote manifest cache lock poisoned".to_string())?;
                    continue;
                }

                if cancel.is_none() {
                    cache.fetching = Some(channel);
                }
                break;
            }
        }

        let url = channel.manifest_url();
        log::info!("Fetching manifest from {url}");
        let fetch_result = Self::await_with_cancel(cancel, async {
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
                .map_err(|e| e.to_string())
        })
        .await;

        if cancel.is_none() {
            let mut cache = cache_lock
                .lock()
                .map_err(|_| "remote manifest cache lock poisoned".to_string())?;
            cache.fetching = None;
            if let Ok(manifest) = fetch_result.as_ref() {
                cache.cached = Some(CachedRemoteManifest {
                    channel,
                    fetched_at: Instant::now(),
                    manifest: manifest.clone(),
                });
            }
            cache_ready.notify_all();
        }

        fetch_result
    }
}

impl RemoteManifest {
    fn into_fetch_result(self) -> ManifestFetchResult {
        let manifests = self.manifests.clone();
        let preset_tag_constraints = self.preset_tag_constraints.clone();
        let mut cfg = ModsConfig { mods: self.mods };
        let _ = normalize_aliases(&mut cfg);
        (
            self.version,
            cfg,
            self.chain_config,
            manifests,
            preset_tag_constraints,
        )
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
    pub(crate) fn matches_caps(
        game_version: u32,
        low_cap: Option<u32>,
        high_cap: Option<u32>,
    ) -> bool {
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

    pub(crate) fn constraint_for_tag(&self, tag: &str) -> Option<&TagConstraint> {
        self.tag_constraints
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(tag))
            .map(|(_, value)| value)
    }

    pub fn applies_to_tag(&self, tag: &str) -> bool {
        self.tags
            .iter()
            .any(|entry| entry.eq_ignore_ascii_case(tag))
            || self.constraint_for_tag(tag).is_some()
    }

    pub fn is_compatible(&self, game_version: u32) -> bool {
        if !self.enabled {
            return false;
        }
        Self::matches_caps(game_version, self.low_cap, self.high_cap)
    }

    pub fn is_install_compatible(&self, game_version: u32) -> bool {
        Self::matches_caps(game_version, self.low_cap, self.high_cap)
    }

    pub fn is_compatible_for_tags(&self, game_version: u32, active_tags: &[String]) -> bool {
        if !self.enabled {
            return false;
        }

        for active_tag in active_tags {
            if !self.applies_to_tag(active_tag) {
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

    pub fn is_install_compatible_for_tags(
        &self,
        game_version: u32,
        active_tags: &[String],
    ) -> bool {
        for active_tag in active_tags {
            if !self.applies_to_tag(active_tag) {
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

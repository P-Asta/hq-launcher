use std::sync::{Condvar, Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::mod_config::ModEntry;

const EVENT_MANIFEST_CACHE_TTL: Duration = Duration::from_secs(60);

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EventLinks {
    #[serde(default)]
    pub discord: Option<String>,
    #[serde(default)]
    pub website: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventEntry {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub versions: Vec<u32>,
    #[serde(default)]
    pub testers: Vec<String>,
    #[serde(default = "default_preset")]
    pub preset: String,
    #[serde(default)]
    pub starts_at: Option<String>,
    #[serde(default)]
    pub ends_at: Option<String>,
    #[serde(default)]
    pub image: Option<String>,
    #[serde(default)]
    pub links: EventLinks,
    #[serde(default)]
    pub mods: Vec<ModEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EventManifest {
    #[serde(default)]
    pub version: u32,
    #[serde(default)]
    pub events: Vec<EventEntry>,
}

#[derive(Clone)]
struct CachedEventManifest {
    channel: crate::release_channel::ReleaseChannel,
    fetched_at: Instant,
    manifest: EventManifest,
}

#[derive(Default)]
struct EventManifestCache {
    cached: Option<CachedEventManifest>,
    fetching: Option<crate::release_channel::ReleaseChannel>,
}

static EVENT_MANIFEST_CACHE: OnceLock<(Mutex<EventManifestCache>, Condvar)> = OnceLock::new();

fn event_manifest_cache() -> &'static (Mutex<EventManifestCache>, Condvar) {
    EVENT_MANIFEST_CACHE.get_or_init(|| (Mutex::new(EventManifestCache::default()), Condvar::new()))
}

fn default_preset() -> String {
    "hq".to_string()
}

fn try_read_local_events() -> Option<(std::path::PathBuf, EventManifest)> {
    let mut candidates: Vec<std::path::PathBuf> = vec![];

    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join("events.json"));
        candidates.push(cwd.join("..").join("events.json"));
    }

    if let Ok(exe) = std::env::current_exe() {
        let mut p = exe.parent().map(|p| p.to_path_buf());
        for _ in 0..8 {
            let Some(dir) = p.take() else { break };
            candidates.push(dir.join("events.json"));
            p = dir.parent().map(|pp| pp.to_path_buf());
        }
    }

    for path in candidates {
        if !path.exists() {
            continue;
        }
        let text = std::fs::read_to_string(&path).ok()?;
        let mf = serde_json::from_str::<EventManifest>(&text).ok()?;
        return Some((path, mf));
    }

    None
}

pub async fn fetch_events(client: &reqwest::Client) -> Result<EventManifest, String> {
    if let Some((path, mf)) = try_read_local_events() {
        log::info!("Using local events manifest: {}", path.to_string_lossy());
        return Ok(mf);
    }

    let channel = crate::release_channel::current();
    let now = Instant::now();
    let (cache_lock, cache_ready) = event_manifest_cache();

    {
        let mut cache = cache_lock
            .lock()
            .map_err(|_| "event manifest cache lock poisoned".to_string())?;
        loop {
            if let Some(cached) = cache.cached.as_ref() {
                if cached.channel == channel
                    && now.duration_since(cached.fetched_at) < EVENT_MANIFEST_CACHE_TTL
                {
                    return Ok(cached.manifest.clone());
                }
            }

            if cache.fetching == Some(channel) {
                cache = cache_ready
                    .wait(cache)
                    .map_err(|_| "event manifest cache lock poisoned".to_string())?;
                continue;
            }

            cache.fetching = Some(channel);
            break;
        }
    }

    let url = channel.events_url();
    log::info!("Fetching events manifest from {url}");
    let fetch_result = async {
        let response = client
            .get(url)
            .timeout(Duration::from_secs(12))
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(EventManifest::default());
        }

        response
            .error_for_status()
            .map_err(|e| e.to_string())?
            .json::<EventManifest>()
            .await
            .map_err(|e| e.to_string())
    }
    .await;

    let mut cache = cache_lock
        .lock()
        .map_err(|_| "event manifest cache lock poisoned".to_string())?;
    cache.fetching = None;
    if let Ok(manifest) = fetch_result.as_ref() {
        cache.cached = Some(CachedEventManifest {
            channel,
            fetched_at: Instant::now(),
            manifest: manifest.clone(),
        });
    } else if let Some(cached) = cache.cached.as_ref() {
        if cached.channel == channel {
            log::warn!("Using stale events manifest cache after fetch failure");
            let manifest = cached.manifest.clone();
            cache_ready.notify_all();
            return Ok(manifest);
        }
    }
    cache_ready.notify_all();

    fetch_result
}

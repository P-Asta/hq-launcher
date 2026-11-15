use std::sync::OnceLock;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

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
    cached: Mutex<Option<CachedEventManifest>>,
}

static EVENT_MANIFEST_CACHE: OnceLock<EventManifestCache> = OnceLock::new();

impl EventManifestCache {
    async fn fetch(
        &self,
        client: &reqwest::Client,
        channel: crate::release_channel::ReleaseChannel,
        url: &str,
    ) -> Result<EventManifest, String> {
        // Startup event requests must yield while another request is fetching.
        let mut cached = self.cached.lock().await;
        if let Some(entry) = cached.as_ref() {
            if entry.channel == channel && entry.fetched_at.elapsed() < EVENT_MANIFEST_CACHE_TTL {
                return Ok(entry.manifest.clone());
            }
        }

        log::info!("Fetching events manifest from {url}");
        let result = async {
            let response = client
                .get(url)
                .timeout(Duration::from_secs(12))
                .send()
                .await
                .map_err(|error| error.to_string())?;
            if response.status() == reqwest::StatusCode::NOT_FOUND {
                return Ok(EventManifest::default());
            }
            response
                .error_for_status()
                .map_err(|error| error.to_string())?
                .json::<EventManifest>()
                .await
                .map_err(|error| error.to_string())
        }
        .await;
        let manifest = match result {
            Ok(manifest) => manifest,
            Err(error) => {
                let Some(entry) = cached.as_ref().filter(|entry| entry.channel == channel) else {
                    return Err(error);
                };
                log::warn!("Using stale events manifest cache after fetch failure: {error}");
                entry.manifest.clone()
            }
        };
        *cached = Some(CachedEventManifest {
            channel,
            fetched_at: Instant::now(),
            manifest: manifest.clone(),
        });
        Ok(manifest)
    }
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
    EVENT_MANIFEST_CACHE
        .get_or_init(EventManifestCache::default)
        .fetch(client, channel, channel.events_url())
        .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    #[test]
    fn concurrent_events_finish_without_blocking_runtime_timers() {
        let (done, result) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let url = format!("http://{}", listener.local_addr().unwrap());
            let server = std::thread::spawn(move || {
                for request in 0..3 {
                    let (mut stream, _) = listener.accept().unwrap();
                    stream
                        .set_read_timeout(Some(Duration::from_secs(2)))
                        .unwrap();
                    let mut headers = Vec::new();
                    let mut buffer = [0; 1024];
                    while !headers.windows(4).any(|bytes| bytes == b"\r\n\r\n") {
                        let count = stream.read(&mut buffer).unwrap();
                        if count == 0 {
                            break;
                        }
                        headers.extend_from_slice(&buffer[..count]);
                    }
                    std::thread::sleep(Duration::from_millis(60));
                    let body = r#"{"version":7,"events":[]}"#;
                    let status = if request == 0 {
                        "200 OK"
                    } else {
                        "503 Service Unavailable"
                    };
                    let response = format!(
                        "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    );
                    stream.write_all(response.as_bytes()).unwrap();
                }
            });
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            runtime.block_on(async {
                let cache = EventManifestCache::default();
                let client = reqwest::Client::builder().no_proxy().build().unwrap();
                let requests = (0..5).map(|_| {
                    cache.fetch(
                        &client,
                        crate::release_channel::ReleaseChannel::Stable,
                        &url,
                    )
                });
                let results = futures_util::future::join_all(requests).await;
                assert!(results
                    .iter()
                    .all(|result| result.as_ref().is_ok_and(|mf| mf.version == 7)));
                // Expired same-channel data remains usable if the server fails.
                cache.cached.lock().await.as_mut().unwrap().fetched_at -= EVENT_MANIFEST_CACHE_TTL;
                assert_eq!(
                    cache
                        .fetch(
                            &client,
                            crate::release_channel::ReleaseChannel::Stable,
                            &url
                        )
                        .await
                        .unwrap()
                        .version,
                    7
                );
                assert!(cache
                    .fetch(&client, crate::release_channel::ReleaseChannel::Beta, &url)
                    .await
                    .is_err());
            });
            server.join().unwrap();
            done.send(()).unwrap();
        });
        result
            .recv_timeout(Duration::from_secs(4))
            .expect("event requests blocked the runtime");
    }
}

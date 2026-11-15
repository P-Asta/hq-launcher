use super::*;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::atomic::AtomicUsize;

const MANIFEST: &str = r#"{"version":7,"chain_config":[["Example"]],"mods":[],"manifests":{"81":"depot-81"},"practice_mods":[{"name":"Practice","dev":"Test","version_config":{"81":"1.0.0"}}]}"#;

#[test]
fn retired_diomedes_is_not_returned_for_install_or_manifest_sync() {
    let mut manifest: RemoteManifest = serde_json::from_str(MANIFEST).unwrap();
    for (dev,name) in [("DiomedesLC","Diomedes"),("Shared","Library")] {
        manifest.mods.push(serde_json::from_value(serde_json::json!({"dev":dev,"name":name})).unwrap());
    }
    let (_, config, _, _, _) = manifest.into_fetch_result();
    assert_eq!(config.mods.len(), 1);
    assert_eq!(config.mods[0].name, "Library");
}

struct Server {
    url: String,
    hits: Arc<AtomicUsize>,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Server {
    fn new(status: &str, body: &str, delay: Duration) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/manifest.json", listener.local_addr().unwrap());
        listener.set_nonblocking(true).unwrap();
        let hits = Arc::new(AtomicUsize::new(0));
        let stop = Arc::new(AtomicBool::new(false));
        let (requests, stopping) = (hits.clone(), stop.clone());
        let response = format!("HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
        let thread = std::thread::spawn(move || {
            while !stopping.load(AtomicOrdering::Relaxed) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
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
                        requests.fetch_add(1, AtomicOrdering::Relaxed);
                        std::thread::sleep(delay);
                        let _ = stream.write_all(response.as_bytes());
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(2));
                    }
                    Err(error) => panic!("test HTTP server: {error}"),
                }
            }
        });
        Self {
            url,
            hits,
            stop,
            thread: Some(thread),
        }
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, AtomicOrdering::Relaxed);
        self.thread.take().unwrap().join().unwrap();
    }
}

struct CacheDir(PathBuf);

impl CacheDir {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "hq-manifest-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, AtomicOrdering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn path(&self, channel: crate::release_channel::ReleaseChannel) -> PathBuf {
        manifest_cache_path(&self.0, channel)
    }
}

impl Drop for CacheDir {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}

fn client() -> reqwest::Client {
    reqwest::Client::builder().no_proxy().build().unwrap()
}

#[test]
fn concurrent_manifest_requests_finish_on_one_runtime_thread() {
    // Keep the watchdog outside Tokio: a blocking wait can freeze its timers too.
    let (done, result) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            let server = Server::new("200 OK", MANIFEST, Duration::from_millis(80));
            let cache = RemoteManifestCache::default();
            let client = client();
            let requests = (0..8).map(|_| {
                cache.fetch(
                    &client,
                    crate::release_channel::ReleaseChannel::Stable,
                    &server.url,
                    None,
                    None,
                )
            });
            let results = futures_util::future::join_all(requests).await;
            assert!(results
                .iter()
                .all(|result| result.as_ref().is_ok_and(|mf| mf.version == 7)));
            assert_eq!(server.hits.load(AtomicOrdering::Relaxed), 1);
        });
        done.send(()).unwrap();
    });
    result
        .recv_timeout(Duration::from_secs(4))
        .expect("manifest requests blocked the runtime");
}

#[tokio::test]
async fn saves_good_manifest_and_recovers_after_restart_on_http_or_json_failure() {
    let directory = CacheDir::new();
    let path = directory.path(crate::release_channel::ReleaseChannel::Stable);
    let client = client();
    let good = Server::new("200 OK", MANIFEST, Duration::ZERO);
    RemoteManifestCache::default()
        .fetch(
            &client,
            crate::release_channel::ReleaseChannel::Stable,
            &good.url,
            Some(&path),
            None,
        )
        .await
        .unwrap();
    let saved = std::fs::read(&path).unwrap();

    for (status, body) in [
        ("503 Service Unavailable", "unavailable"),
        ("200 OK", "<html>bad gateway</html>"),
    ] {
        let failed = Server::new(status, body, Duration::ZERO);
        let restarted = RemoteManifestCache::default();
        let manifest = restarted
            .fetch(
                &client,
                crate::release_channel::ReleaseChannel::Stable,
                &failed.url,
                Some(&path),
                None,
            )
            .await
            .unwrap();
        assert_eq!(manifest.version, 7);
        assert_eq!(
            manifest.manifests.get(&81).map(String::as_str),
            Some("depot-81")
        );
        assert_eq!(
            manifest.practice_mods[0].pinned_version_for(81),
            Some("1.0.0")
        );
        assert_eq!(manifest.chain_config, vec![vec!["Example"]]);
        assert_eq!(
            std::fs::read(&path).unwrap(),
            saved,
            "failed responses must preserve last-good cache"
        );
        restarted
            .fetch(
                &client,
                crate::release_channel::ReleaseChannel::Stable,
                &failed.url,
                Some(&path),
                None,
            )
            .await
            .unwrap();
        assert_eq!(
            failed.hits.load(AtomicOrdering::Relaxed),
            1,
            "fallback is reused for the cache TTL"
        );
    }
}

#[tokio::test]
async fn stable_disk_cache_is_not_used_for_beta() {
    let directory = CacheDir::new();
    let stable = directory.path(crate::release_channel::ReleaseChannel::Stable);
    let beta = directory.path(crate::release_channel::ReleaseChannel::Beta);
    assert_ne!(stable, beta);
    std::fs::write(stable, MANIFEST).unwrap();
    let failed = Server::new("503 Service Unavailable", "offline", Duration::ZERO);
    let error = RemoteManifestCache::default()
        .fetch(
            &client(),
            crate::release_channel::ReleaseChannel::Beta,
            &failed.url,
            Some(&beta),
            None,
        )
        .await
        .unwrap_err();
    assert!(error.contains("503"));
}

#[tokio::test]
async fn corrupt_disk_cache_does_not_hide_download_failure() {
    let directory = CacheDir::new();
    let path = directory.path(crate::release_channel::ReleaseChannel::Stable);
    std::fs::write(&path, "{truncated").unwrap();
    let failed = Server::new("503 Service Unavailable", "offline", Duration::ZERO);
    let error = RemoteManifestCache::default()
        .fetch(
            &client(),
            crate::release_channel::ReleaseChannel::Stable,
            &failed.url,
            Some(&path),
            None,
        )
        .await
        .unwrap_err();
    assert!(error.contains("503"));
    assert_eq!(std::fs::read_to_string(path).unwrap(), "{truncated");
}

#[tokio::test]
async fn cache_write_failure_does_not_fail_a_successful_download() {
    let directory = CacheDir::new();
    let good = Server::new("200 OK", MANIFEST, Duration::ZERO);
    let not_a_directory = directory.0.join("file");
    std::fs::write(&not_a_directory, "file").unwrap();
    let path = not_a_directory.join("stable.json");
    let manifest = RemoteManifestCache::default()
        .fetch(
            &client(),
            crate::release_channel::ReleaseChannel::Stable,
            &good.url,
            Some(&path),
            None,
        )
        .await
        .unwrap();
    assert_eq!(manifest.version, 7);
}

#[tokio::test]
async fn cancellation_while_waiting_for_cache_lock_returns_cancelled() {
    let cache = RemoteManifestCache::default();
    let _guard = cache.cached.lock().await;
    let cancelled = Arc::new(AtomicBool::new(false));
    let client = client();
    let request = cache.fetch(
        &client,
        crate::release_channel::ReleaseChannel::Stable,
        "http://127.0.0.1:1",
        None,
        Some(&cancelled),
    );
    let cancel = async {
        tokio::time::sleep(Duration::from_millis(10)).await;
        cancelled.store(true, AtomicOrdering::Relaxed);
    };
    let (result, _) = tokio::time::timeout(Duration::from_secs(1), async {
        tokio::join!(request, cancel)
    })
    .await
    .unwrap();
    assert_eq!(result.unwrap_err(), "Cancelled");
}

#[tokio::test]
async fn already_cancelled_request_does_not_use_disk_fallback() {
    let directory = CacheDir::new();
    let path = directory.path(crate::release_channel::ReleaseChannel::Stable);
    std::fs::write(&path, MANIFEST).unwrap();
    let cancelled = Arc::new(AtomicBool::new(true));
    let error = RemoteManifestCache::default()
        .fetch(
            &client(),
            crate::release_channel::ReleaseChannel::Stable,
            "http://127.0.0.1:1",
            Some(&path),
            Some(&cancelled),
        )
        .await
        .unwrap_err();
    assert_eq!(error, "Cancelled");
}

#[tokio::test]
async fn successful_refresh_replaces_existing_disk_cache() {
    let directory = CacheDir::new();
    let path = directory.path(crate::release_channel::ReleaseChannel::Stable);
    std::fs::write(&path, MANIFEST.replace("\"version\":7", "\"version\":6")).unwrap();
    let good = Server::new("200 OK", MANIFEST, Duration::ZERO);
    RemoteManifestCache::default()
        .fetch(
            &client(),
            crate::release_channel::ReleaseChannel::Stable,
            &good.url,
            Some(&path),
            None,
        )
        .await
        .unwrap();
    let saved: RemoteManifest = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(saved.version, 7);
    assert!(!path.with_extension("json.tmp").exists());
}

#[tokio::test]
async fn expired_memory_cache_survives_network_failure_but_not_channel_change() {
    let cache = RemoteManifestCache::default();
    *cache.cached.lock().await = Some(CachedRemoteManifest {
        channel: crate::release_channel::ReleaseChannel::Stable,
        fetched_at: Instant::now() - REMOTE_MANIFEST_CACHE_TTL - Duration::from_secs(1),
        manifest: serde_json::from_str(MANIFEST).unwrap(),
    });
    let client = client();
    let offline = Server::new("503 Service Unavailable", "offline", Duration::ZERO);
    assert_eq!(
        cache
            .fetch(
                &client,
                crate::release_channel::ReleaseChannel::Stable,
                &offline.url,
                None,
                None
            )
            .await
            .unwrap()
            .version,
        7
    );
    assert!(cache
        .fetch(
            &client,
            crate::release_channel::ReleaseChannel::Beta,
            &offline.url,
            None,
            None
        )
        .await
        .is_err());
    assert_eq!(offline.hits.load(AtomicOrdering::Relaxed), 2);
}

#[tokio::test]
async fn aborted_fetch_releases_cache_for_following_requests() {
    let cache = Arc::new(RemoteManifestCache::default());
    let slow = Server::new("200 OK", MANIFEST, Duration::from_millis(80));
    let owner_cache = cache.clone();
    let url = slow.url.clone();
    let owner = tokio::spawn(async move {
        owner_cache
            .fetch(
                &client(),
                crate::release_channel::ReleaseChannel::Stable,
                &url,
                None,
                None,
            )
            .await
    });
    tokio::time::timeout(Duration::from_secs(1), async {
        while slow.hits.load(AtomicOrdering::Relaxed) == 0 {
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
    })
    .await
    .unwrap();
    owner.abort();
    assert!(owner.await.unwrap_err().is_cancelled());
    let good = Server::new("200 OK", MANIFEST, Duration::ZERO);
    let client = client();
    let manifest = tokio::time::timeout(
        Duration::from_secs(1),
        cache.fetch(
            &client,
            crate::release_channel::ReleaseChannel::Stable,
            &good.url,
            None,
            None,
        ),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(manifest.version, 7);
}

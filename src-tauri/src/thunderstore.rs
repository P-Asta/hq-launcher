use serde::{Deserialize, Serialize};
use std::{
    path::Path,
    sync::atomic::AtomicBool,
    sync::atomic::Ordering as AtomicOrdering,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

/// Minimal Thunderstore package model used for install resolution.
///
/// Endpoint: `https://thunderstore.io/c/{community}/api/v1/package/`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageListing {
    pub name: String,
    pub owner: String,
    #[serde(rename = "full_name")]
    #[allow(dead_code)]
    pub full_name: String,
    pub versions: Vec<PackageVersion>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageVersion {
    pub version_number: String,
    pub download_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThunderstoreCache {
    pub time: u64,
    pub packages: Vec<PackageListing>,
}

const CACHE_TTL_SECS: u64 = 60 * 60;

fn now_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

fn write_cache(cache_path: &Path, packages: &[PackageListing], now: u64) {
    let cache = ThunderstoreCache {
        packages: packages.to_vec(),
        time: now,
    };

    if let Some(parent) = cache_path.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            log::warn!(
                target: "fetch_packages",
                "Failed to create cache directory {}: {e}",
                parent.to_string_lossy()
            );
        }
    }
    match serde_json::to_string(&cache) {
        Ok(json) => {
            if let Err(e) = std::fs::write(cache_path, json) {
                log::warn!(
                    target: "fetch_packages",
                    "Failed to write cache file {}: {e}",
                    cache_path.to_string_lossy()
                );
            }
        }
        Err(e) => {
            log::warn!(target: "fetch_packages", "Failed to serialize cache: {e}");
        }
    }
}

pub fn read_cached_packages(
    cache_path: &Path,
) -> Result<Option<(Vec<PackageListing>, bool)>, String> {
    if !cache_path.exists() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(cache_path).map_err(|e| e.to_string())?;
    match serde_json::from_str::<ThunderstoreCache>(&content) {
        Ok(cache) => {
            let stale = now_unix_secs().saturating_sub(cache.time) >= CACHE_TTL_SECS;
            Ok(Some((cache.packages, stale)))
        }
        Err(e) => {
            log::warn!(
                target: "fetch_packages",
                "Failed to parse cache file {}: {e} (will refetch)",
                cache_path.to_string_lossy()
            );
            let _ = std::fs::remove_file(cache_path);
            Ok(None)
        }
    }
}

pub async fn refresh_community_packages_with_cancel(
    client: &reqwest::Client,
    cache_path: &Path,
    cancel: Option<&Arc<AtomicBool>>,
) -> Result<Vec<PackageListing>, String> {
    let now = now_unix_secs();
    let url = "https://thunderstore.io/c/lethal-company/api/v1/package/".to_string();
    log::info!(target: "fetch_packages", "Thunderstore GET {url}");
    let request = async {
        client
            .get(&url)
            .send()
            .await
            .map_err(|e| e.to_string())?
            .error_for_status()
            .map_err(|e| e.to_string())?
            .json::<Vec<PackageListing>>()
            .await
            .map_err(|e| e.to_string())
    };
    tokio::pin!(request);
    let mut interval = tokio::time::interval(std::time::Duration::from_millis(100));
    let packages: Vec<PackageListing> = loop {
        tokio::select! {
            result = &mut request => break result?,
            _ = interval.tick() => {
                if cancel.is_some_and(|c| c.load(AtomicOrdering::Relaxed)) {
                    return Err("Cancelled".to_string());
                }
            }
        }
    };

    write_cache(cache_path, &packages, now);
    Ok(packages)
}

/// Fetch all packages for a lethal company.
///
/// Note: Thunderstore's per-package endpoint may not be available (404),
/// but the list endpoint returns full version/download_url data.
pub async fn fetch_community_packages(
    client: &reqwest::Client,
    cache_path: &Path,
) -> Result<Vec<PackageListing>, String> {
    fetch_community_packages_with_cancel(client, cache_path, None).await
}

pub async fn fetch_community_packages_with_cancel(
    client: &reqwest::Client,
    cache_path: &Path,
    cancel: Option<&Arc<AtomicBool>>,
) -> Result<Vec<PackageListing>, String> {
    log::info!(target: "fetch_packages", "Cache path: {cache_path:?}");
    if let Some((packages, stale)) = read_cached_packages(cache_path)? {
        if !stale {
            log::info!(target: "fetch_packages", "Using cached packages");
            return Ok(packages);
        }
        log::info!(target: "fetch_packages", "Cache expired, fetching new packages");
    }

    refresh_community_packages_with_cancel(client, cache_path, cancel).await
}

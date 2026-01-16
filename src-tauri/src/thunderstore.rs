use std::{path::Path, sync::LazyLock, time::{SystemTime, Instant, UNIX_EPOCH}};

use futures_util::lock::Mutex;
use serde::{Deserialize, Serialize};


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

/// Fetch all packages for a lethal company.
///
/// Note: Thunderstore's per-package endpoint may not be available (404),
/// but the list endpoint returns full version/download_url data.
pub async fn fetch_community_packages(
    client: &reqwest::Client,
) -> Result<Vec<PackageListing>, String> {
    let cache_path = Path::new("lc-launcher-cache.json");
    log::info!(target: "fetch_packages", "Cache path: {cache_path:?}");
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    if cache_path.exists() {
        let content = std::fs::read_to_string(cache_path).map_err(|e| e.to_string())?;
        let cache: ThunderstoreCache = serde_json::from_str(&content).map_err(|e| e.to_string())?;
        if now - cache.time < 60*60 {
            log::info!(target: "fetch_packages", "Using cached packages");
            return Ok(cache.packages);
        }
        log::info!(target: "fetch_packages", "Cache expired, fetching new packages");
    }

    let url = format!("https://thunderstore.io/c/lethal-company/api/v1/package/");
    log::info!(target: "fetch_packages", "Thunderstore GET {url}");
    let packages = client
        .get(url)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?
        .json::<Vec<PackageListing>>()
        .await
        .map_err(|e| e.to_string());


    let cache = ThunderstoreCache {
        packages: packages.clone().unwrap(),
        time: now,
    };
    std::fs::write(cache_path, serde_json::to_string(&cache).unwrap()).unwrap();
    packages
}


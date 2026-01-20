use std::{
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};
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
    cache_path: &Path,
) -> Result<Vec<PackageListing>, String> {
    log::info!(target: "fetch_packages", "Cache path: {cache_path:?}");
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    if cache_path.exists() {
        let content = std::fs::read_to_string(cache_path).map_err(|e| e.to_string())?;
        let cache: ThunderstoreCache = serde_json::from_str(&content).map_err(|e| e.to_string())?;
        if now - cache.time < 60 * 60 {
            log::info!(target: "fetch_packages", "Using cached packages");
            return Ok(cache.packages);
        }
        log::info!(target: "fetch_packages", "Cache expired, fetching new packages");
    }

    let url = "https://thunderstore.io/c/lethal-company/api/v1/package/".to_string();
    log::info!(target: "fetch_packages", "Thunderstore GET {url}");
    let packages: Vec<PackageListing> = client
        .get(url)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?
        .json::<Vec<PackageListing>>()
        .await
        .map_err(|e| e.to_string())?;

    let cache = ThunderstoreCache {
        packages: packages.clone(),
        time: now,
    };

    // Best-effort persist; failure shouldn't crash installs/updates.
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

    Ok(packages)
}

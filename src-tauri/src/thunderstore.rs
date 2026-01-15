use serde::Deserialize;

/// Minimal Thunderstore package model used for install resolution.
///
/// Endpoint: `https://thunderstore.io/c/{community}/api/v1/package/`
#[derive(Debug, Clone, Deserialize)]
pub struct PackageListing {
    pub name: String,
    pub owner: String,
    #[serde(rename = "full_name")]
    #[allow(dead_code)]
    pub full_name: String,
    pub versions: Vec<PackageVersion>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PackageVersion {
    pub version_number: String,
    pub download_url: String,
}

/// Fetch all packages for a community.
///
/// Note: Thunderstore's per-package endpoint may not be available (404),
/// but the list endpoint returns full version/download_url data.
pub async fn fetch_community_packages(
    client: &reqwest::Client,
    community: &str,
) -> Result<Vec<PackageListing>, String> {
    let url = format!("https://thunderstore.io/c/{community}/api/v1/package/");
    log::info!("Thunderstore GET {url}");
    client
        .get(url)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?
        .json::<Vec<PackageListing>>()
        .await
        .map_err(|e| e.to_string())
}


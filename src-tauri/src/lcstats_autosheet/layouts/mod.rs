pub mod autosheetmodel;
pub mod makusheet;
pub mod wafrody;

use serde_json::Value;

use crate::google_oauth::LcStatsSettings;

pub const AUTOSHEETMODEL_LAYOUT: &str = "AutoSheetModel";
pub const MAKUSHEET_LAYOUT: &str = "MakuSheet 1.0";
pub const WAFRODY_LAYOUT: &str = "WafrodyAutoSheet";

pub fn is_supported_layout(layout: &str) -> bool {
    layout.eq_ignore_ascii_case(AUTOSHEETMODEL_LAYOUT)
        || layout.eq_ignore_ascii_case(MAKUSHEET_LAYOUT)
        || layout.eq_ignore_ascii_case(WAFRODY_LAYOUT)
}

pub async fn write_stats(
    app: tauri::AppHandle,
    client: &reqwest::Client,
    settings: &LcStatsSettings,
    stats: &Value,
) -> Result<(), String> {
    let token = crate::google_oauth::access_token(app).await?;
    if settings.layout.eq_ignore_ascii_case(WAFRODY_LAYOUT) {
        wafrody::write(client, &token, settings, stats).await
    } else if settings.layout.eq_ignore_ascii_case(MAKUSHEET_LAYOUT) {
        makusheet::write(client, &token, settings, stats).await
    } else {
        autosheetmodel::write(client, &token, settings, stats).await
    }
}

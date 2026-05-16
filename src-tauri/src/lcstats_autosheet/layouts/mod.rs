pub mod autosheetmodel;
pub mod breadsheet;
pub mod makusheet;
pub mod moddedsheet;
pub mod wafrody;

use serde_json::Value;

use crate::google_oauth::LcStatsSettings;

pub const AUTOSHEETMODEL_LAYOUT: &str = "AutoSheetModel";
pub const BREADSHEET_LAYOUT: &str = "BreadSheet";
pub const MAKUSHEET_LAYOUT: &str = "MakuSheet 1.0";
pub const MODDEDSHEET_LAYOUT: &str = "ModdedSheet";
pub const WAFRODY_LAYOUT: &str = "WafrodyAutoSheet";

pub fn is_supported_layout(layout: &str) -> bool {
    layout.eq_ignore_ascii_case(AUTOSHEETMODEL_LAYOUT)
        || layout.eq_ignore_ascii_case(BREADSHEET_LAYOUT)
        || layout.eq_ignore_ascii_case(MAKUSHEET_LAYOUT)
        || layout.eq_ignore_ascii_case(MODDEDSHEET_LAYOUT)
        || layout.eq_ignore_ascii_case(WAFRODY_LAYOUT)
}

pub async fn write_stats(
    app: tauri::AppHandle,
    client: &reqwest::Client,
    settings: &LcStatsSettings,
    stats: &Value,
) -> Result<(), String> {
    crate::google_oauth::assert_spreadsheet_can_edit(
        app.clone(),
        client,
        settings.spreadsheet_id.trim(),
    )
    .await?;
    let token = crate::google_oauth::access_token(app).await?;
    if settings.layout.eq_ignore_ascii_case(WAFRODY_LAYOUT) {
        wafrody::write(client, &token, settings, stats).await
    } else if settings.layout.eq_ignore_ascii_case(MODDEDSHEET_LAYOUT) {
        moddedsheet::write(client, &token, settings, stats).await
    } else if settings.layout.eq_ignore_ascii_case(MAKUSHEET_LAYOUT) {
        makusheet::write(client, &token, settings, stats).await
    } else if settings.layout.eq_ignore_ascii_case(BREADSHEET_LAYOUT) {
        breadsheet::write(client, &token, settings, stats).await
    } else {
        autosheetmodel::write(client, &token, settings, stats).await
    }
}

pub mod autosheetmodel;
pub mod breadsheet;
pub mod charlyautosheet;
pub mod customlayout;
pub mod evilsheet;
pub mod makusheet;
pub mod moddedsheet;
pub mod serenadesheet;
pub mod wafrody;

use serde_json::Value;

use crate::google_oauth::LcStatsSettings;

pub const AUTOSHEETMODEL_LAYOUT: &str = "AutoSheetModel";
pub const BREADSHEET_LAYOUT: &str = "BreadSheet";
pub const CHARLY_AUTOSHEET_LAYOUT: &str = "CharlyAutoSheet";
pub const CUSTOM_LAYOUT: &str = "Custom Layout";
pub const EVILSHEET_LAYOUT: &str = "Evilsheet";
pub const MAKUSHEET_LAYOUT: &str = "MakuSheet 1.0";
pub const MODDEDSHEET_LAYOUT: &str = "ModdedSheet";
pub const SERENADE_LAYOUT: &str = "SerenadeSheet";
pub const WAFRODY_LAYOUT: &str = "WafrodyAutoSheet";

pub fn is_supported_layout(layout: &str) -> bool {
    layout.eq_ignore_ascii_case(AUTOSHEETMODEL_LAYOUT)
        || layout.eq_ignore_ascii_case(BREADSHEET_LAYOUT)
        || layout.eq_ignore_ascii_case(CHARLY_AUTOSHEET_LAYOUT)
        || layout.eq_ignore_ascii_case(CUSTOM_LAYOUT)
        || layout.eq_ignore_ascii_case(EVILSHEET_LAYOUT)
        || layout.eq_ignore_ascii_case(MAKUSHEET_LAYOUT)
        || layout.eq_ignore_ascii_case(MODDEDSHEET_LAYOUT)
        || layout.eq_ignore_ascii_case(SERENADE_LAYOUT)
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
    } else if settings
        .layout
        .eq_ignore_ascii_case(CHARLY_AUTOSHEET_LAYOUT)
    {
        charlyautosheet::write(client, &token, settings, stats).await
    } else if settings.layout.eq_ignore_ascii_case(CUSTOM_LAYOUT) {
        customlayout::write(client, &token, settings, stats).await
    } else if settings.layout.eq_ignore_ascii_case(EVILSHEET_LAYOUT) {
        evilsheet::write(client, &token, settings, stats).await
    } else if settings.layout.eq_ignore_ascii_case(MODDEDSHEET_LAYOUT) {
        moddedsheet::write(client, &token, settings, stats).await
    } else if settings.layout.eq_ignore_ascii_case(SERENADE_LAYOUT) {
        serenadesheet::write(client, &token, settings, stats).await
    } else if settings.layout.eq_ignore_ascii_case(MAKUSHEET_LAYOUT) {
        makusheet::write(client, &token, settings, stats).await
    } else if settings.layout.eq_ignore_ascii_case(BREADSHEET_LAYOUT) {
        breadsheet::write(client, &token, settings, stats).await
    } else {
        autosheetmodel::write(client, &token, settings, stats).await
    }
}

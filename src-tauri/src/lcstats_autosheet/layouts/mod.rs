pub mod autosheetmodel;
pub mod makusheet;
pub mod wafrody;

use serde_json::Value;

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
    layout: &str,
    stats: &Value,
) -> Result<(), String> {
    if layout.eq_ignore_ascii_case(WAFRODY_LAYOUT) {
        wafrody::write(app, client, stats).await
    } else if layout.eq_ignore_ascii_case(MAKUSHEET_LAYOUT) {
        makusheet::write(app, client, stats).await
    } else {
        autosheetmodel::write(app, client, stats).await
    }
}

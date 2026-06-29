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
use crate::lcstats_autosheet::sheets::SheetInfo;

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
    let token = crate::google_oauth::access_token(app.clone()).await?;
    let settings = resolve_active_sheet(app.clone(), client, &token, settings).await?;
    match write_stats_for_layout(client, &token, &settings, stats).await {
        Ok(()) => Ok(()),
        Err(error) => {
            let recovered =
                resolve_active_sheet_for_retry(app, client, &token, settings.clone()).await;
            match recovered {
                Ok(recovered_settings)
                    if recovered_settings.active_sheet_name != settings.active_sheet_name
                        || recovered_settings.active_sheet_id != settings.active_sheet_id =>
                {
                    log::warn!(
                        "LCStatsTracker AutoSheet write failed on saved sheet {}; retrying with resolved sheet {} ({}): {error}",
                        settings.active_sheet_name,
                        recovered_settings.active_sheet_name,
                        recovered_settings.active_sheet_id
                    );
                    write_stats_for_layout(client, &token, &recovered_settings, stats).await
                }
                Ok(_) => Err(error),
                Err(recovery_error) => {
                    log::warn!(
                        "LCStatsTracker AutoSheet could not recover saved sheet selection after write failure: {recovery_error}"
                    );
                    Err(format!("{error}; {recovery_error}"))
                }
            }
        }
    }
}

async fn write_stats_for_layout(
    client: &reqwest::Client,
    token: &str,
    settings: &LcStatsSettings,
    stats: &Value,
) -> Result<(), String> {
    if settings.layout.eq_ignore_ascii_case(WAFRODY_LAYOUT) {
        wafrody::write(client, token, settings, stats).await
    } else if settings
        .layout
        .eq_ignore_ascii_case(CHARLY_AUTOSHEET_LAYOUT)
    {
        charlyautosheet::write(client, token, settings, stats).await
    } else if settings.layout.eq_ignore_ascii_case(CUSTOM_LAYOUT) {
        customlayout::write(client, token, settings, stats).await
    } else if settings.layout.eq_ignore_ascii_case(EVILSHEET_LAYOUT) {
        evilsheet::write(client, token, settings, stats).await
    } else if settings.layout.eq_ignore_ascii_case(MODDEDSHEET_LAYOUT) {
        moddedsheet::write(client, token, settings, stats).await
    } else if settings.layout.eq_ignore_ascii_case(SERENADE_LAYOUT) {
        serenadesheet::write(client, token, settings, stats).await
    } else if settings.layout.eq_ignore_ascii_case(MAKUSHEET_LAYOUT) {
        makusheet::write(client, token, settings, stats).await
    } else if settings.layout.eq_ignore_ascii_case(BREADSHEET_LAYOUT) {
        breadsheet::write(client, token, settings, stats).await
    } else {
        autosheetmodel::write(client, token, settings, stats).await
    }
}

async fn resolve_active_sheet(
    app: tauri::AppHandle,
    client: &reqwest::Client,
    token: &str,
    settings: &LcStatsSettings,
) -> Result<LcStatsSettings, String> {
    let spreadsheet_id = settings.spreadsheet_id.trim();
    if spreadsheet_id.is_empty() || settings.active_sheet_name.trim().is_empty() {
        return Ok(settings.clone());
    }

    let sheet_id = settings.active_sheet_id.trim().parse::<i64>().ok();
    let sheets =
        crate::lcstats_autosheet::sheets::get_sheet_infos(client, token, spreadsheet_id).await?;
    let Some(resolved) =
        resolve_sheet_from_id_or_name(&sheets, sheet_id, &settings.active_sheet_name)
    else {
        return Ok(settings.clone());
    };

    Ok(persist_resolved_sheet(app, settings.clone(), resolved))
}

async fn resolve_active_sheet_for_retry(
    app: tauri::AppHandle,
    client: &reqwest::Client,
    token: &str,
    settings: LcStatsSettings,
) -> Result<LcStatsSettings, String> {
    let spreadsheet_id = settings.spreadsheet_id.trim();
    if spreadsheet_id.is_empty() || settings.active_sheet_name.trim().is_empty() {
        return Ok(settings);
    }

    let sheets =
        crate::lcstats_autosheet::sheets::get_sheet_infos(client, token, spreadsheet_id).await?;
    let resolved = resolve_sheet_from_name(&sheets, &settings.active_sheet_name)
        .or_else(|| single_sheet_fallback(&sheets))
        .ok_or_else(|| {
            let available = sheets
                .iter()
                .map(|sheet| sheet.title.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            if available.is_empty() {
                format!(
                    "saved sheet '{}' was not found and the spreadsheet has no sheets",
                    settings.active_sheet_name
                )
            } else {
                format!(
                    "saved sheet '{}' was not found; available sheets: {available}",
                    settings.active_sheet_name
                )
            }
        })?;

    Ok(persist_resolved_sheet(app, settings, resolved))
}

fn persist_resolved_sheet(
    app: tauri::AppHandle,
    settings: LcStatsSettings,
    resolved: SheetInfo,
) -> LcStatsSettings {
    let resolved_id = resolved.id.to_string();
    if resolved.title == settings.active_sheet_name && resolved_id == settings.active_sheet_id {
        return settings;
    }

    let mut next = settings.clone();
    next.active_sheet_name = resolved.title;
    next.active_sheet_id = resolved_id;
    log::info!(
        "LCStatsTracker AutoSheet resolved active sheet to {} ({})",
        next.active_sheet_name,
        next.active_sheet_id
    );
    if let Err(e) = crate::google_oauth::set_settings(app, next.clone()) {
        log::warn!("Failed to persist resolved LCStatsTracker sheet selection: {e}");
    }
    next
}

fn resolve_sheet_from_id_or_name(
    sheets: &[SheetInfo],
    sheet_id: Option<i64>,
    preferred_name: &str,
) -> Option<SheetInfo> {
    if let Some(sheet_id) = sheet_id {
        if let Some(sheet) = sheets.iter().find(|sheet| sheet.id == sheet_id) {
            return Some(sheet.clone());
        }
    }

    resolve_sheet_from_name(sheets, preferred_name)
}

fn resolve_sheet_from_name(sheets: &[SheetInfo], preferred_name: &str) -> Option<SheetInfo> {
    let preferred = normalize_sheet_match_name(preferred_name);
    if preferred.is_empty() {
        return None;
    }

    sheets
        .iter()
        .find(|sheet| normalize_sheet_match_name(&sheet.title) == preferred)
        .or_else(|| {
            sheets.iter().find(|sheet| {
                let title = normalize_sheet_match_name(&sheet.title);
                title.contains(&preferred) || preferred.contains(&title)
            })
        })
        .cloned()
}

fn single_sheet_fallback(sheets: &[SheetInfo]) -> Option<SheetInfo> {
    if sheets.len() == 1 {
        sheets.first().cloned()
    } else {
        None
    }
}

fn normalize_sheet_match_name(value: &str) -> String {
    value
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

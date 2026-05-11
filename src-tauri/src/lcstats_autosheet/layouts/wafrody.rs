use serde_json::{json, Value};

use crate::lcstats_autosheet::layouts::WAFRODY_LAYOUT;
use crate::lcstats_autosheet::sheets::{
    batch_update_spreadsheet, batch_write_cells_user_entered, first_empty_row_from, get_sheet_id,
    read_range, write_cells,
};
use crate::lcstats_autosheet::stats::{object_at, string_at, strip_moon_number, value_at};

const TARGET_SHEET_CELL: &str = "A1";
const CHECK_COLUMN: &str = "X";
const START_ROW: usize = 4;
const PLAYER_NAME_ROW: usize = 3;
const PLAYER_COLUMNS: [&str; 4] = ["AD", "AE", "AF", "AG"];

const MOON_COLUMN: &str = "G";
const WEATHER_COLUMN: &str = "H";
const INTERIOR_COLUMN: &str = "I";
const ITEM_COUNT_COLUMN: &str = "K";
const COLLECTED_TOTAL_COLUMN: &str = "X";
const BOTTOM_LINE_COLUMN: &str = "Y";
const VALUE_SOLD_COLUMN: &str = "AJ";
const NEW_QUOTA_COLUMN: &str = "C";
const EXTRA_NUMBER_COLUMN: &str = "L";
const SEED_COLUMN: &str = "BE";
const SID_COLUMN: &str = "J";
const INFESTATION_COLUMN: &str = "R";

pub async fn write(
    app: tauri::AppHandle,
    client: &reqwest::Client,
    stats: &Value,
) -> Result<(), String> {
    let settings = crate::google_oauth::get_settings(app.clone())?;
    if !settings.layout.eq_ignore_ascii_case(WAFRODY_LAYOUT) {
        return Ok(());
    }
    let spreadsheet_id = settings.spreadsheet_id.trim();
    let source_sheet = settings.active_sheet_name.trim();
    if spreadsheet_id.is_empty() || source_sheet.is_empty() {
        return Err("spreadsheet or sheet is not set".to_string());
    }

    let token = crate::google_oauth::access_token(app).await?;
    let target_sheet = resolve_target_sheet(client, &token, spreadsheet_id, source_sheet).await?;
    let target_row = first_empty_row_from(
        client,
        &token,
        spreadsheet_id,
        &target_sheet,
        CHECK_COLUMN,
        START_ROW,
    )
    .await?;

    if string_at(stats, &["MoonInfo", "Name"]).trim() == "71 Gordion" {
        handle_gordion(
            client,
            &token,
            spreadsheet_id,
            &target_sheet,
            target_row,
            stats,
        )
        .await?;
        return Ok(());
    }

    let normalized = NormalizedStats::from_stats(stats);
    batch_write_cells_user_entered(
        client,
        &token,
        spreadsheet_id,
        &target_sheet,
        build_value_updates(&normalized, target_row),
    )
    .await?;
    write_rich_cells(
        client,
        &token,
        spreadsheet_id,
        &target_sheet,
        target_row,
        &normalized,
    )
    .await
}

async fn read_target_sheet(
    client: &reqwest::Client,
    token: &str,
    spreadsheet_id: &str,
    source_sheet: &str,
) -> Result<Option<String>, String> {
    let range = format!(
        "{}!{TARGET_SHEET_CELL}",
        crate::lcstats_autosheet::sheets::quote_sheet_name(source_sheet)
    );
    let data = read_range(client, token, spreadsheet_id, &range).await?;
    Ok(data
        .get("values")
        .and_then(Value::as_array)
        .and_then(|rows| rows.first())
        .and_then(Value::as_array)
        .and_then(|cells| cells.first())
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .filter(|value| !value.eq_ignore_ascii_case("R"))
        .map(ToOwned::to_owned))
}

async fn resolve_target_sheet(
    client: &reqwest::Client,
    token: &str,
    spreadsheet_id: &str,
    source_sheet: &str,
) -> Result<String, String> {
    let Some(candidate) = read_target_sheet(client, token, spreadsheet_id, source_sheet).await?
    else {
        return Ok(source_sheet.to_string());
    };
    match get_sheet_id(client, token, spreadsheet_id, &candidate).await {
        Ok(_) => Ok(candidate),
        Err(e) => {
            log::warn!(
                "Wafrody target sheet cell {TARGET_SHEET_CELL} contained '{candidate}', but it is not a valid sheet name ({e}); using '{source_sheet}'"
            );
            Ok(source_sheet.to_string())
        }
    }
}

#[derive(Debug, Clone)]
struct NormalizedPlayer {
    name: String,
    status: String,
    note: String,
}

#[derive(Debug, Clone)]
struct NormalizedStats {
    moon_name: String,
    weather: String,
    interior: String,
    item_count: i64,
    collected_total: i64,
    bottom_line: i64,
    value_sold: i64,
    new_quota: i64,
    extra_number: usize,
    seed: String,
    sid_type: String,
    infestation_type: String,
    players: Vec<NormalizedPlayer>,
}

impl NormalizedStats {
    fn from_stats(stats: &Value) -> Self {
        let bee_count = value_at(stats, &["BeeInfo", "Values"])
            .and_then(Value::as_array)
            .map(Vec::len)
            .unwrap_or(0);
        let egg_count = value_at(stats, &["BirdInfo", "EggValues"])
            .and_then(Value::as_array)
            .map(Vec::len)
            .unwrap_or(0);
        Self {
            moon_name: strip_apostrophe(&string_at(stats, &["MoonInfo", "Name"])),
            weather: wafrody_weather(&string_at(stats, &["MoonInfo", "Weather"])),
            interior: strip_apostrophe(&string_at(stats, &["DungeonInfo", "Interior"])),
            item_count: intish_at(stats, &["DungeonInfo", "ItemCount"]),
            collected_total: intish_at(stats, &["CollectedTotal"]),
            bottom_line: intish_at(stats, &["BottomLine"]),
            value_sold: intish_at(stats, &["ValueSold"]),
            new_quota: intish_at(stats, &["NewQuota"]),
            extra_number: bee_count + egg_count,
            seed: strip_apostrophe(&string_at(stats, &["Seed"])),
            sid_type: strip_apostrophe(&string_at(stats, &["SIDType"])),
            infestation_type: strip_apostrophe(&string_at(stats, &["InfestationType"])),
            players: normalize_players(stats),
        }
    }
}

fn normalize_players(stats: &Value) -> Vec<NormalizedPlayer> {
    object_at(stats, &["Players"])
        .into_iter()
        .map(|(steam_id, player)| {
            let name = strip_apostrophe(
                player
                    .get("Name")
                    .and_then(Value::as_str)
                    .unwrap_or(steam_id.as_str()),
            );
            let alive = player
                .get("Alive")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let disconnected = player
                .get("Disconnected")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let time_of_death = strip_apostrophe(
                player
                    .get("TimeOfDeath")
                    .and_then(Value::as_str)
                    .unwrap_or_default(),
            )
            .trim()
            .to_string();
            let cause_of_death = strip_apostrophe(
                player
                    .get("CauseOfDeath")
                    .and_then(Value::as_str)
                    .unwrap_or_default(),
            )
            .trim()
            .to_string();

            let status = if disconnected {
                "DC"
            } else if cause_of_death == "Abandonment" {
                "M"
            } else if alive {
                "S"
            } else {
                "X"
            }
            .to_string();

            let mut note_parts = vec![name.clone()];
            if !time_of_death.is_empty() {
                note_parts.push(format!("Time of Death: {time_of_death}"));
            }
            if !cause_of_death.is_empty() {
                note_parts.push(format!("Cause of Death: {cause_of_death}"));
            }

            NormalizedPlayer {
                name: name.clone(),
                status,
                note: note_parts.join("\n"),
            }
        })
        .collect()
}

fn build_value_updates(stats: &NormalizedStats, row: usize) -> Vec<(String, usize, Value)> {
    let mut values = vec![
        (
            MOON_COLUMN.to_string(),
            row,
            json!(strip_moon_number(&stats.moon_name)),
        ),
        (WEATHER_COLUMN.to_string(), row, json!(stats.weather)),
        (INTERIOR_COLUMN.to_string(), row, json!(stats.interior)),
        (ITEM_COUNT_COLUMN.to_string(), row, json!(stats.item_count)),
        (
            COLLECTED_TOTAL_COLUMN.to_string(),
            row,
            json!(stats.collected_total),
        ),
        (
            BOTTOM_LINE_COLUMN.to_string(),
            row,
            json!(stats.bottom_line),
        ),
        (
            EXTRA_NUMBER_COLUMN.to_string(),
            row,
            json!(stats.extra_number),
        ),
        (SEED_COLUMN.to_string(), row, json!(stats.seed)),
    ];

    if stats.value_sold != 0 {
        values.push((VALUE_SOLD_COLUMN.to_string(), row, json!(stats.value_sold)));
    }
    if stats.new_quota != 0 {
        values.push((NEW_QUOTA_COLUMN.to_string(), row, json!(stats.new_quota)));
    }
    for (index, player) in stats.players.iter().take(PLAYER_COLUMNS.len()).enumerate() {
        values.push((
            PLAYER_COLUMNS[index].to_string(),
            PLAYER_NAME_ROW,
            json!(player.name),
        ));
    }

    values
}

async fn write_rich_cells(
    client: &reqwest::Client,
    token: &str,
    spreadsheet_id: &str,
    sheet_name: &str,
    row: usize,
    stats: &NormalizedStats,
) -> Result<(), String> {
    let sheet_id = get_sheet_id(client, token, spreadsheet_id, sheet_name).await?;
    let mut requests = vec![
        checkbox_with_note_request(sheet_id, SID_COLUMN, row, &stats.sid_type),
        checkbox_with_note_request(sheet_id, INFESTATION_COLUMN, row, &stats.infestation_type),
    ];

    for (index, player) in stats.players.iter().take(PLAYER_COLUMNS.len()).enumerate() {
        requests.push(value_with_note_request(
            sheet_id,
            PLAYER_COLUMNS[index],
            row,
            json!(player.status),
            &player.note,
        ));
    }

    batch_update_spreadsheet(client, token, spreadsheet_id, requests).await
}

async fn handle_gordion(
    client: &reqwest::Client,
    token: &str,
    spreadsheet_id: &str,
    sheet_name: &str,
    target_row: usize,
    stats: &Value,
) -> Result<(), String> {
    let value_sold = intish_at(stats, &["ValueSold"]);
    let new_quota = intish_at(stats, &["NewQuota"]);
    if value_sold == 0 || new_quota == 0 {
        return Ok(());
    }
    if value_sold != 0 {
        write_cells(
            client,
            token,
            spreadsheet_id,
            sheet_name,
            &format!("{VALUE_SOLD_COLUMN}{}", target_row.saturating_sub(3)),
            vec![vec![json!(value_sold)]],
        )
        .await?;
    }
    if new_quota != 0 {
        write_cells(
            client,
            token,
            spreadsheet_id,
            sheet_name,
            &format!("{NEW_QUOTA_COLUMN}{target_row}"),
            vec![vec![json!(new_quota)]],
        )
        .await?;
    }
    Ok(())
}

fn checkbox_with_note_request(sheet_id: i64, column: &str, row: usize, note: &str) -> Value {
    let checked = !note.trim().is_empty();
    value_with_note_request(
        sheet_id,
        column,
        row,
        json!(checked),
        if checked { note } else { "" },
    )
}

fn value_with_note_request(
    sheet_id: i64,
    column: &str,
    row: usize,
    value: Value,
    note: &str,
) -> Value {
    let column_index = column_to_index(column);
    json!({
        "updateCells": {
            "range": {
                "sheetId": sheet_id,
                "startRowIndex": row.saturating_sub(1),
                "endRowIndex": row,
                "startColumnIndex": column_index,
                "endColumnIndex": column_index + 1
            },
            "rows": [{
                "values": [{
                    "userEnteredValue": google_user_value(value),
                    "note": note
                }]
            }],
            "fields": "userEnteredValue,note"
        }
    })
}

fn google_user_value(value: Value) -> Value {
    if let Some(value) = value.as_bool() {
        json!({ "boolValue": value })
    } else if let Some(value) = value.as_i64() {
        json!({ "numberValue": value })
    } else if let Some(value) = value.as_f64() {
        json!({ "numberValue": value })
    } else {
        json!({ "stringValue": value.as_str().unwrap_or_default() })
    }
}

fn strip_apostrophe(value: &str) -> String {
    value.trim_start_matches('\'').to_string()
}

fn wafrody_weather(value: &str) -> String {
    let weather = strip_apostrophe(value);
    if weather.eq_ignore_ascii_case("Mild") {
        "Clear".to_string()
    } else {
        weather
    }
}

fn intish_at(stats: &Value, path: &[&str]) -> i64 {
    value_at(stats, path).map(value_as_i64).unwrap_or(0)
}

fn value_as_i64(value: &Value) -> i64 {
    value
        .as_i64()
        .or_else(|| {
            value
                .as_str()
                .and_then(|text| strip_apostrophe(text).trim().parse::<i64>().ok())
        })
        .unwrap_or(0)
}

fn column_to_index(column: &str) -> usize {
    column.chars().fold(0, |index, ch| {
        index * 26 + (ch.to_ascii_uppercase() as usize - 'A' as usize + 1)
    }) - 1
}

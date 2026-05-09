use serde_json::{json, Value};
use std::collections::HashMap;

use crate::lcstats_autosheet::layouts::MAKUSHEET_LAYOUT;
use crate::lcstats_autosheet::sheets::{
    batch_write_cells_user_entered, first_empty_row_from, quote_sheet_name, read_range,
};
use crate::lcstats_autosheet::stats::{
    array_at, int_at, object_at, string_at, strip_moon_number,
};

const CHECK_COLUMN: &str = "F";
const START_ROW: usize = 3;
const PLAYER_COLUMN_PAIRS: [(&str, &str); 4] = [("V", "W"), ("X", "Y"), ("Z", "AA"), ("AB", "AC")];
const PLAYER_ID_ROW: usize = 199;
const PLAYER_NAME_ROW: usize = 2;

pub async fn write(
    app: tauri::AppHandle,
    client: &reqwest::Client,
    stats: &Value,
) -> Result<(), String> {
    let settings = crate::google_oauth::get_settings(app.clone())?;
    if !settings.layout.eq_ignore_ascii_case(MAKUSHEET_LAYOUT) {
        return Ok(());
    }
    let spreadsheet_id = settings.spreadsheet_id.trim();
    let sheet_name = settings.active_sheet_name.trim();
    if spreadsheet_id.is_empty() || sheet_name.is_empty() {
        return Err("spreadsheet or sheet is not set".to_string());
    }

    let token = crate::google_oauth::access_token(app).await?;
    let row =
        first_empty_row_from(client, &token, spreadsheet_id, sheet_name, CHECK_COLUMN, START_ROW)
            .await?;
    let player_columns =
        setup_or_match_player_columns(client, &token, spreadsheet_id, sheet_name, stats).await?;
    let values = build_values(stats, &player_columns, row);
    batch_write_cells_user_entered(client, &token, spreadsheet_id, sheet_name, values).await
}

async fn setup_or_match_player_columns(
    client: &reqwest::Client,
    token: &str,
    spreadsheet_id: &str,
    sheet_name: &str,
    stats: &Value,
) -> Result<HashMap<String, String>, String> {
    let id_range = format!(
        "{}!{}{}:{}{}",
        quote_sheet_name(sheet_name),
        PLAYER_COLUMN_PAIRS[0].0,
        PLAYER_ID_ROW,
        PLAYER_COLUMN_PAIRS[PLAYER_COLUMN_PAIRS.len() - 1].0,
        PLAYER_ID_ROW
    );
    let name_range = format!(
        "{}!{}{}:{}{}",
        quote_sheet_name(sheet_name),
        PLAYER_COLUMN_PAIRS[0].0,
        PLAYER_NAME_ROW,
        PLAYER_COLUMN_PAIRS[PLAYER_COLUMN_PAIRS.len() - 1].1,
        PLAYER_NAME_ROW
    );
    let data = read_range(client, token, spreadsheet_id, &id_range).await?;
    let existing_row = data
        .get("values")
        .and_then(Value::as_array)
        .and_then(|rows| rows.first())
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let name_data = read_range(client, token, spreadsheet_id, &name_range).await?;
    let existing_name_row = name_data
        .get("values")
        .and_then(Value::as_array)
        .and_then(|rows| rows.first())
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut existing_slots = HashMap::new();
    for (index, (column, _)) in PLAYER_COLUMN_PAIRS.iter().enumerate() {
        let steam_id = existing_row
            .get(index * 2)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .trim();
        if !steam_id.is_empty() {
            existing_slots.insert(steam_id.to_string(), (*column).to_string());
        }
    }

    let players = object_at(stats, &["Players"]);
    let mut player_columns = HashMap::new();
    if existing_slots.is_empty() {
        let mut updates = vec![];
        for (index, (steam_id, player)) in players.iter().take(PLAYER_COLUMN_PAIRS.len()).enumerate() {
            let (column, next_column) = PLAYER_COLUMN_PAIRS[index];
            let column = column.to_string();
            player_columns.insert(steam_id.clone(), column.clone());
            updates.push((column.clone(), PLAYER_ID_ROW, json!(steam_id)));
            updates.push((
                column.clone(),
                PLAYER_NAME_ROW,
                json!(player.get("Name").and_then(Value::as_str).unwrap_or_default()),
            ));
            updates.push((next_column.to_string(), PLAYER_NAME_ROW, json!("")));
        }
        batch_write_cells_user_entered(client, token, spreadsheet_id, sheet_name, updates).await?;
    } else {
        let mut updates = vec![];
        for steam_id in players.keys() {
            if let Some(column) = existing_slots.get(steam_id) {
                player_columns.insert(steam_id.clone(), column.clone());
                if let Some((index, _)) = PLAYER_COLUMN_PAIRS
                    .iter()
                    .enumerate()
                    .find(|(_, (first, _))| *first == column)
                {
                    let current_name = existing_name_row
                        .get(index * 2)
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .trim();
                    if current_name.is_empty() {
                        let name = players
                            .get(steam_id)
                            .and_then(|player| player.get("Name"))
                            .and_then(Value::as_str)
                            .unwrap_or_default();
                        updates.push((column.clone(), PLAYER_NAME_ROW, json!(name)));
                    }
                }
            }
        }
        batch_write_cells_user_entered(client, token, spreadsheet_id, sheet_name, updates).await?;
    }
    Ok(player_columns)
}

fn build_values(
    stats: &Value,
    player_columns: &HashMap<String, String>,
    row: usize,
) -> Vec<(String, usize, Value)> {
    let collected = int_at(stats, &["CollectedTotal"]);
    let available = int_at(stats, &["BottomLineTrue"]);
    let lost_scrap = lost_scrap(stats);
    let mut values = vec![
        ("F".to_string(), row, json!(maku_moon_name(stats))),
        ("G".to_string(), row, json!(maku_weather(stats))),
        ("H".to_string(), row, json!(string_at(stats, &["DungeonInfo", "Interior"]).to_uppercase())),
        ("I".to_string(), row, json!(int_at(stats, &["DungeonInfo", "ItemCount"]))),
        ("J".to_string(), row, json!(array_at(stats, &["BeeInfo", "Values"]).len())),
        ("K".to_string(), row, json!(collected)),
        ("L".to_string(), row, json!(available)),
    ];
    if let Some(quota_amount) = quota_amount_value(stats) {
        values.push(("B".to_string(), row, quota_amount));
    }
    if let Some(average) = average_value(collected, available) {
        values.push(("M".to_string(), row, average));
    }
    if lost_scrap != 0 {
        values.push(("U".to_string(), row, json!(lost_scrap)));
    }

    for (steam_id, player) in object_at(stats, &["Players"]) {
        if let Some(column) = player_columns.get(&steam_id) {
            values.push((column.clone(), row, json!(death_status(&player))));
        }
    }

    values
}

fn quota_amount_value(stats: &Value) -> Option<Value> {
    let new_quota = int_at(stats, &["NewQuota"]);
    if new_quota == 0 {
        None
    } else {
        Some(json!(new_quota))
    }
}

fn average_value(numerator: i64, denominator: i64) -> Option<Value> {
    if denominator <= 0 {
        None
    } else {
        Some(json!(numerator as f64 / denominator as f64))
    }
}

fn maku_moon_name(stats: &Value) -> String {
    strip_moon_number(&string_at(stats, &["MoonInfo", "Name"]))
        .chars()
        .filter(|ch| ch.is_alphabetic())
        .flat_map(char::to_uppercase)
        .collect()
}

fn maku_weather(stats: &Value) -> String {
    let weather = string_at(stats, &["MoonInfo", "Weather"]);
    if weather.eq_ignore_ascii_case("Mild") {
        "CLEAR".to_string()
    } else {
        weather.to_uppercase()
    }
}

fn lost_scrap(stats: &Value) -> i64 {
    array_at(stats, &["MissedItems"])
        .iter()
        .filter(|item| {
            item.get("CollectedOnPreviousDay")
                .and_then(Value::as_bool)
                .unwrap_or(false)
        })
        .filter_map(|item| item.get("Value").and_then(Value::as_i64))
        .sum()
}

fn death_status(player: &Value) -> String {
    if player.get("Disconnected").and_then(Value::as_bool) == Some(true)
        || player.get("CauseOfDeath").and_then(Value::as_str) == Some("Abandoned")
    {
        return "M".to_string();
    }
    if player.get("Alive").and_then(Value::as_bool) == Some(true) {
        return "S".to_string();
    }
    "X".to_string()
}

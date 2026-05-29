use serde_json::{json, Value};
use std::collections::HashMap;

use crate::google_oauth::LcStatsSettings;
use crate::lcstats_autosheet::layouts::EVILSHEET_LAYOUT;
use crate::lcstats_autosheet::sheets::{
    batch_read_ranges, batch_update_spreadsheet, batch_write_cells_user_entered,
    first_empty_row_from, get_sheet_id, number_value, quote_sheet_name, read_number, read_range,
};
use crate::lcstats_autosheet::stats::{
    array_at, array_at_any, initial_available_value, object_at, players_at, string_at,
    strip_moon_number, value_at,
};

const TARGET_SHEET_CELL: &str = "A1";
const START_ROW: usize = 4;
const CHECK_COLUMN: &str = "L";
const PLAYER_NAME_COLUMNS: [&str; 4] = ["AB", "AC", "AD", "AE"];
const PLAYER_STATE_COLUMNS: [&str; 4] = ["U", "V", "W", "X"];
const PLAYER_NAME_ROW: usize = 56;

const QUOTA_COLUMN: &str = "C";
const MOON_COLUMN: &str = "G";
const WEATHER_COLUMN: &str = "H";
const LAYOUT_COLUMN: &str = "I";
const ITEM_COUNT_COLUMN: &str = "J";
const BEE_COUNT_COLUMN: &str = "K";
const COLLECTED_COLUMN: &str = "L";
const AVAILABLE_COLUMN: &str = "M";
const SOLD_COLUMN: &str = "R";
const LOST_SCRAP_COLUMN: &str = "S";

pub async fn write(
    client: &reqwest::Client,
    token: &str,
    settings: &LcStatsSettings,
    stats: &Value,
) -> Result<(), String> {
    if !settings.layout.eq_ignore_ascii_case(EVILSHEET_LAYOUT) {
        return Ok(());
    }
    let spreadsheet_id = settings.spreadsheet_id.trim();
    let source_sheet = settings.active_sheet_name.trim();
    if spreadsheet_id.is_empty() || source_sheet.is_empty() {
        return Err("spreadsheet or sheet is not set".to_string());
    }

    let target_sheet = resolve_target_sheet(client, token, spreadsheet_id, source_sheet).await?;
    let row = first_empty_row_from(
        client,
        token,
        spreadsheet_id,
        &target_sheet.name,
        CHECK_COLUMN,
        START_ROW,
    )
    .await?;

    if is_gordion_stats(stats) {
        return handle_gordion(
            client,
            token,
            spreadsheet_id,
            &target_sheet.name,
            row,
            stats,
        )
        .await;
    }

    let normalized = NormalizedStats::from_stats(stats);
    let player_columns =
        setup_or_match_player_columns(client, token, spreadsheet_id, &target_sheet.name, stats)
            .await?;
    batch_write_cells_user_entered(
        client,
        token,
        spreadsheet_id,
        &target_sheet.name,
        build_value_updates(&normalized, &player_columns, row),
    )
    .await?;
    write_death_notes(
        client,
        token,
        spreadsheet_id,
        &target_sheet.name,
        target_sheet.id,
        stats,
        &normalized,
        &player_columns,
        row,
    )
    .await
}

async fn read_target_sheet(
    client: &reqwest::Client,
    token: &str,
    spreadsheet_id: &str,
    source_sheet: &str,
) -> Result<Option<String>, String> {
    let range = format!("{}!{TARGET_SHEET_CELL}", quote_sheet_name(source_sheet));
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

#[derive(Debug, Clone)]
struct TargetSheet {
    name: String,
    id: Option<i64>,
}

async fn resolve_target_sheet(
    client: &reqwest::Client,
    token: &str,
    spreadsheet_id: &str,
    source_sheet: &str,
) -> Result<TargetSheet, String> {
    let Some(candidate) = read_target_sheet(client, token, spreadsheet_id, source_sheet).await?
    else {
        return Ok(TargetSheet {
            name: source_sheet.to_string(),
            id: None,
        });
    };
    match get_sheet_id(client, token, spreadsheet_id, &candidate).await {
        Ok(sheet_id) => Ok(TargetSheet {
            name: candidate,
            id: Some(sheet_id),
        }),
        Err(e) => {
            log::warn!(
                "Evilsheet target sheet cell {TARGET_SHEET_CELL} contained '{candidate}', but it is not a valid sheet name ({e}); using '{source_sheet}'"
            );
            Ok(TargetSheet {
                name: source_sheet.to_string(),
                id: None,
            })
        }
    }
}

async fn setup_or_match_player_columns(
    client: &reqwest::Client,
    token: &str,
    spreadsheet_id: &str,
    sheet_name: &str,
    stats: &Value,
) -> Result<HashMap<String, String>, String> {
    let name_range = format!(
        "{}!{}{}:{}{}",
        quote_sheet_name(sheet_name),
        PLAYER_NAME_COLUMNS[0],
        PLAYER_NAME_ROW,
        PLAYER_NAME_COLUMNS[PLAYER_NAME_COLUMNS.len() - 1],
        PLAYER_NAME_ROW
    );
    let ranges = batch_read_ranges(client, token, spreadsheet_id, &[&name_range]).await?;
    let existing_row = ranges
        .first()
        .and_then(|data| data.get("values"))
        .and_then(Value::as_array)
        .and_then(|rows| rows.first())
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let mut existing_slots = HashMap::new();
    for index in 0..PLAYER_NAME_COLUMNS.len() {
        let player_name = existing_row
            .get(index)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .trim();
        if !player_name.is_empty() {
            existing_slots.insert(
                normalize_player_name_key(player_name),
                PLAYER_STATE_COLUMNS[index].to_string(),
            );
        }
    }

    let players = players_at(stats);
    let mut player_columns = HashMap::new();
    if existing_slots.is_empty() {
        let mut updates = vec![];
        for (index, (steam_id, player)) in
            players.iter().take(PLAYER_STATE_COLUMNS.len()).enumerate()
        {
            let name = player
                .get("Name")
                .and_then(Value::as_str)
                .unwrap_or_default();
            player_columns.insert(steam_id.clone(), PLAYER_STATE_COLUMNS[index].to_string());
            updates.push((
                PLAYER_NAME_COLUMNS[index].to_string(),
                PLAYER_NAME_ROW,
                json!(strip_apostrophe(name)),
            ));
        }
        batch_write_cells_user_entered(client, token, spreadsheet_id, sheet_name, updates).await?;
    } else {
        for (steam_id, player) in players {
            let player_name = player
                .get("Name")
                .and_then(Value::as_str)
                .unwrap_or_default();
            if let Some(column) = existing_slots.get(&normalize_player_name_key(player_name)) {
                player_columns.insert(steam_id, column.clone());
            }
        }
    }

    Ok(player_columns)
}

#[derive(Debug, Clone)]
struct NormalizedPlayer {
    status: String,
}

#[derive(Debug, Clone)]
struct NormalizedStats {
    moon_name: String,
    weather: String,
    interior: String,
    item_count: i64,
    bee_count: usize,
    collected_total: i64,
    available_total: i64,
    value_sold: i64,
    lost_scrap: i64,
    new_quota: i64,
    players: HashMap<String, NormalizedPlayer>,
}

impl NormalizedStats {
    fn from_stats(stats: &Value) -> Self {
        Self {
            moon_name: strip_moon_number(&strip_apostrophe(&string_at(
                stats,
                &["MoonInfo", "Name"],
            ))),
            weather: evilsheet_weather(&string_at(stats, &["MoonInfo", "Weather"])),
            interior: strip_apostrophe(&string_at(stats, &["DungeonInfo", "Interior"])),
            item_count: intish_at(stats, &["DungeonInfo", "ItemCount"]),
            bee_count: array_at_any(
                stats,
                &[&["BeeInfo", "Available"][..], &["BeeInfo", "Values"][..]],
            )
            .len(),
            collected_total: intish_at(stats, &["CollectedTotal"]),
            available_total: initial_available_value(stats),
            value_sold: intish_at(stats, &["ValueSold"]),
            lost_scrap: lost_scrap(stats),
            new_quota: intish_at(stats, &["NewQuota"]),
            players: normalize_players(stats),
        }
    }
}

fn build_value_updates(
    stats: &NormalizedStats,
    player_columns: &HashMap<String, String>,
    row: usize,
) -> Vec<(String, usize, Value)> {
    let mut values = vec![
        (MOON_COLUMN.to_string(), row, json!(stats.moon_name)),
        (WEATHER_COLUMN.to_string(), row, json!(stats.weather)),
        (LAYOUT_COLUMN.to_string(), row, json!(stats.interior)),
        (ITEM_COUNT_COLUMN.to_string(), row, json!(stats.item_count)),
        (BEE_COUNT_COLUMN.to_string(), row, json!(stats.bee_count)),
        (
            COLLECTED_COLUMN.to_string(),
            row,
            json!(stats.collected_total),
        ),
        (
            AVAILABLE_COLUMN.to_string(),
            row,
            json!(stats.available_total),
        ),
    ];

    if stats.value_sold != 0 {
        values.push((SOLD_COLUMN.to_string(), row, json!(stats.value_sold)));
    }
    if stats.lost_scrap != 0 {
        values.push((LOST_SCRAP_COLUMN.to_string(), row, json!(stats.lost_scrap)));
    }
    if stats.new_quota != 0 {
        values.push((QUOTA_COLUMN.to_string(), row, json!(stats.new_quota)));
    }
    for (steam_id, player) in &stats.players {
        if let Some(column) = player_columns.get(steam_id) {
            values.push((column.clone(), row, json!(player.status)));
        }
    }

    values
}

fn normalize_players(stats: &Value) -> HashMap<String, NormalizedPlayer> {
    let takeoff_time = string_at(stats, &["TakeOffTime"]);
    players_at(stats)
        .into_iter()
        .map(|(steam_id, player)| {
            let alive = player
                .get("Alive")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let disconnected = player
                .get("Disconnected")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let cause_of_death = strip_apostrophe(
                player
                    .get("CauseOfDeath")
                    .and_then(Value::as_str)
                    .unwrap_or_default(),
            )
            .trim()
            .to_string();
            let time_of_death = strip_apostrophe(
                player
                    .get("TimeOfDeath")
                    .and_then(Value::as_str)
                    .unwrap_or_default(),
            )
            .trim()
            .to_string();
            let has_death_details = !cause_of_death.is_empty() || !time_of_death.is_empty();
            let status = if alive {
                "A"
            } else if disconnected {
                "DC"
            } else if cause_of_death.eq_ignore_ascii_case("Abandoned")
                || cause_of_death.eq_ignore_ascii_case("Abandonment")
            {
                "M"
            } else if has_death_details || died_before_ship_leave_cutoff(&player, &takeoff_time) {
                "X"
            } else {
                "S"
            }
            .to_string();

            (steam_id, NormalizedPlayer { status })
        })
        .collect()
}

async fn write_death_notes(
    client: &reqwest::Client,
    token: &str,
    spreadsheet_id: &str,
    sheet_name: &str,
    sheet_id: Option<i64>,
    raw_stats: &Value,
    stats: &NormalizedStats,
    player_columns: &HashMap<String, String>,
    row: usize,
) -> Result<(), String> {
    let mut requests = vec![];
    let sheet_id = match sheet_id {
        Some(sheet_id) => sheet_id,
        None => get_sheet_id(client, token, spreadsheet_id, sheet_name).await?,
    };
    for (steam_id, player) in object_at(raw_stats, &["Players"]) {
        let Some(column) = player_columns.get(&steam_id) else {
            continue;
        };
        let Some(note) = player_death_note(&player) else {
            continue;
        };
        let status = stats
            .players
            .get(&steam_id)
            .map(|player| player.status.as_str())
            .unwrap_or_default();
        requests.push(value_with_note_request(
            sheet_id,
            column,
            row,
            json!(status),
            &note,
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
    let target_line = run_block_start_row(target_row);
    let mut updates = vec![];

    if value_sold != 0 {
        let current_value = read_number(
            client,
            token,
            spreadsheet_id,
            sheet_name,
            &format!("{SOLD_COLUMN}{target_line}"),
        )
        .await?;
        updates.push((
            SOLD_COLUMN.to_string(),
            target_line,
            number_value(current_value + value_sold as f64),
        ));
    }
    if new_quota != 0 {
        updates.push((QUOTA_COLUMN.to_string(), target_line + 3, json!(new_quota)));
    }
    batch_write_cells_user_entered(client, token, spreadsheet_id, sheet_name, updates).await
}

fn value_with_note_request(
    sheet_id: i64,
    column: &str,
    row: usize,
    value: Value,
    note: &str,
) -> Value {
    let column_index = column_to_index(column);
    let mut cell = json!({ "userEnteredValue": google_user_value(value) });
    if !note.is_empty() {
        cell["note"] = json!(note);
    }
    json!({
        "updateCells": {
            "range": {
                "sheetId": sheet_id,
                "startRowIndex": row.saturating_sub(1),
                "endRowIndex": row,
                "startColumnIndex": column_index,
                "endColumnIndex": column_index + 1
            },
            "rows": [{ "values": [cell] }],
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

fn player_death_note(player: &Value) -> Option<String> {
    if player.get("Alive").and_then(Value::as_bool) == Some(true)
        || player.get("Disconnected").and_then(Value::as_bool) == Some(true)
    {
        return None;
    }
    let cause = strip_apostrophe(
        player
            .get("CauseOfDeath")
            .and_then(Value::as_str)
            .unwrap_or_default(),
    )
    .trim()
    .to_string();
    let death_time = strip_apostrophe(
        player
            .get("TimeOfDeath")
            .and_then(Value::as_str)
            .unwrap_or_default(),
    )
    .trim()
    .to_string();
    if cause.is_empty() && death_time.is_empty() {
        return None;
    }
    Some(format!("Cause: {cause}\nTime: {death_time}"))
}

fn lost_scrap(stats: &Value) -> i64 {
    array_at(stats, &["MissedItems"])
        .iter()
        .filter(|item| item.get("CollectedOnPreviousDay").and_then(Value::as_bool) == Some(true))
        .map(|item| item.get("Value").map(value_as_i64).unwrap_or(0))
        .sum()
}

fn is_gordion_stats(stats: &Value) -> bool {
    let moon = strip_moon_number(&strip_apostrophe(&string_at(stats, &["MoonInfo", "Name"])));
    let normalized = moon
        .trim()
        .chars()
        .filter(|ch| ch.is_ascii_alphabetic())
        .collect::<String>()
        .to_ascii_uppercase();
    normalized == "GORDION" || normalized == "GORION" || normalized == "GALETRY"
}

fn evilsheet_weather(value: &str) -> String {
    let weather = strip_apostrophe(value);
    if weather.eq_ignore_ascii_case("Mild") {
        "Clear".to_string()
    } else {
        weather
    }
}

fn died_before_ship_leave_cutoff(player: &Value, takeoff_time: &str) -> bool {
    let death_minutes = player
        .get("TimeOfDeath")
        .and_then(Value::as_str)
        .and_then(parse_time_to_minutes);
    let limit_minutes = parse_time_to_minutes(takeoff_time)
        .map(|minutes| minutes - 120)
        .or_else(|| parse_time_to_minutes("10:00 PM"));
    matches!((death_minutes, limit_minutes), (Some(death), Some(limit)) if death <= limit)
}

fn parse_time_to_minutes(value: &str) -> Option<i64> {
    let normalized = value.trim().to_ascii_uppercase();
    let mut parts = normalized.split_whitespace();
    let time = parts.next()?;
    let period = parts.next()?;
    if parts.next().is_some() {
        return None;
    }
    let mut time_parts = time.split(':');
    let mut hour = time_parts.next()?.parse::<i64>().ok()?;
    let minute = time_parts.next()?.parse::<i64>().ok()?;
    if time_parts.next().is_some() || !(0..60).contains(&minute) {
        return None;
    }
    if period == "PM" && hour != 12 {
        hour += 12;
    } else if period == "AM" && hour == 12 {
        hour = 0;
    } else if period != "AM" && period != "PM" {
        return None;
    }
    Some(hour * 60 + minute)
}

fn run_block_start_row(current_row: usize) -> usize {
    let line_to_place = current_row as isize - 1;
    let offset = (line_to_place - START_ROW as isize).div_euclid(3) * 3;
    (START_ROW as isize + offset).max(1) as usize
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

fn strip_apostrophe(value: &str) -> String {
    value.trim_start_matches('\'').to_string()
}

fn normalize_player_name_key(value: &str) -> String {
    strip_apostrophe(value).trim().to_ascii_lowercase()
}

fn column_to_index(column: &str) -> usize {
    column.chars().fold(0, |index, ch| {
        index * 26 + (ch.to_ascii_uppercase() as usize - 'A' as usize + 1)
    }) - 1
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn maps_evilsheet_columns() {
        let stats = json!({
            "NewQuota": "'900",
            "ValueSold": "'130",
            "MoonInfo": { "Name": "'68 Artifice", "Weather": "'Mild" },
            "DungeonInfo": { "Interior": "'Mineshaft", "ItemCount": "'34" },
            "BeeInfo": { "Available": [64, 132] },
            "CollectedTotal": "'926",
            "InitialAvailableValue": "'2133",
            "MissedItems": [
                { "Value": 30, "CollectedOnPreviousDay": true }
            ],
            "TakeOffTime": "11:00 PM",
            "Players": {
                "1": { "Name": "One", "Alive": true, "Disconnected": false },
                "2": { "Name": "Two", "Alive": false, "Disconnected": false, "CauseOfDeath": "'Forest Giant", "TimeOfDeath": "'8:00 PM" }
            }
        });
        let normalized = NormalizedStats::from_stats(&stats);
        let player_columns = HashMap::from([
            ("1".to_string(), "U".to_string()),
            ("2".to_string(), "V".to_string()),
        ]);

        let updates = build_value_updates(&normalized, &player_columns, 7);

        assert_eq!(cell_value(&updates, QUOTA_COLUMN), Some(&json!(900)));
        assert_eq!(cell_value(&updates, MOON_COLUMN), Some(&json!("Artifice")));
        assert_eq!(cell_value(&updates, WEATHER_COLUMN), Some(&json!("Clear")));
        assert_eq!(
            cell_value(&updates, LAYOUT_COLUMN),
            Some(&json!("Mineshaft"))
        );
        assert_eq!(cell_value(&updates, ITEM_COUNT_COLUMN), Some(&json!(34)));
        assert_eq!(cell_value(&updates, BEE_COUNT_COLUMN), Some(&json!(2)));
        assert_eq!(cell_value(&updates, COLLECTED_COLUMN), Some(&json!(926)));
        assert_eq!(cell_value(&updates, AVAILABLE_COLUMN), Some(&json!(2133)));
        assert_eq!(cell_value(&updates, SOLD_COLUMN), Some(&json!(130)));
        assert_eq!(cell_value(&updates, LOST_SCRAP_COLUMN), Some(&json!(30)));
        assert_eq!(cell_value(&updates, "U"), Some(&json!("A")));
        assert_eq!(cell_value(&updates, "V"), Some(&json!("X")));
    }

    #[test]
    fn gordion_block_start_matches_wafrody_style() {
        assert_eq!(run_block_start_row(4), 1);
        assert_eq!(run_block_start_row(5), 4);
        assert_eq!(run_block_start_row(7), 4);
        assert_eq!(run_block_start_row(8), 7);
    }

    #[test]
    fn galetry_stats_use_gordion_economy_path() {
        let stats = json!({
            "MoonInfo": { "Name": "'Galetry" }
        });

        assert!(is_gordion_stats(&stats));
    }

    fn cell_value<'a>(values: &'a [(String, usize, Value)], column: &str) -> Option<&'a Value> {
        values
            .iter()
            .find(|(value_column, _, _)| value_column == column)
            .map(|(_, _, value)| value)
    }
}

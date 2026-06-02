use serde_json::{json, Value};
use std::collections::HashMap;

use crate::google_oauth::LcStatsSettings;
use crate::lcstats_autosheet::layouts::MAKUSHEET_LAYOUT;
use crate::lcstats_autosheet::sheets::{
    batch_read_ranges, batch_update_spreadsheet, batch_write_cells_user_entered, first_empty_row,
    first_empty_row_from, get_sheet_id, number_value, quote_sheet_name, read_number,
};
use crate::lcstats_autosheet::stats::{
    lcstats, strip_apostrophe, strip_moon_number, LcStats, PlayerStats,
};

const CHECK_COLUMN: &str = "K";
const START_ROW: usize = 3;
const PLAYER_COLUMNS: [&str; 4] = ["V", "W", "X", "Y"];
const PLAYER_ID_ROW: usize = 199;
const PLAYER_NAME_ROW: usize = 2;

const QUOTA_AMOUNT_COLUMN: &str = "B";
const MOON_COLUMN: &str = "F";
const WEATHER_COLUMN: &str = "G";
const LAYOUT_COLUMN: &str = "H";
const ITEM_COUNT_COLUMN: &str = "I";
const HIVE_COUNT_COLUMN: &str = "J";
const COLLECTED_COLUMN: &str = "K";
const AVAILABLE_COLUMN: &str = "L";
const SOLD_COLUMN: &str = "R";
const LOST_SCRAP_COLUMN: &str = "U";

pub async fn write(
    client: &reqwest::Client,
    token: &str,
    settings: &LcStatsSettings,
    stats: &Value,
) -> Result<(), String> {
    if !settings.layout.eq_ignore_ascii_case(MAKUSHEET_LAYOUT) {
        return Ok(());
    }
    let spreadsheet_id = settings.spreadsheet_id.trim();
    let sheet_name = settings.active_sheet_name.trim();
    if spreadsheet_id.is_empty() || sheet_name.is_empty() {
        return Err("spreadsheet or sheet is not set".to_string());
    }

    let lc_stats = lcstats(stats);
    if lc_stats.is_quota_event() {
        return handle_quota_event(client, token, spreadsheet_id, sheet_name, &lc_stats).await;
    }
    if !lc_stats.has_dungeon_info() {
        return handle_sell_event(client, token, spreadsheet_id, sheet_name, &lc_stats).await;
    }

    let row = first_empty_row_from(
        client,
        token,
        spreadsheet_id,
        sheet_name,
        CHECK_COLUMN,
        START_ROW,
    )
    .await?;
    if lc_stats.is_gordion_moon() {
        return handle_sell_event(client, token, spreadsheet_id, sheet_name, &lc_stats).await;
    }
    let player_columns =
        setup_or_match_player_columns(client, token, spreadsheet_id, sheet_name, &lc_stats).await?;
    let values = build_values(&lc_stats, &player_columns, row);
    batch_write_cells_user_entered(client, token, spreadsheet_id, sheet_name, values).await?;
    write_death_notes(
        client,
        token,
        spreadsheet_id,
        sheet_name,
        &lc_stats,
        &player_columns,
        row,
    )
    .await
}

async fn handle_quota_event(
    client: &reqwest::Client,
    token: &str,
    spreadsheet_id: &str,
    sheet_name: &str,
    payload: &LcStats,
) -> Result<(), String> {
    let current_quota_row = first_empty_row(
        client,
        token,
        spreadsheet_id,
        sheet_name,
        QUOTA_AMOUNT_COLUMN,
    )
    .await?;
    let sold_row = current_quota_row.saturating_sub(1).max(START_ROW);
    let quota_row = current_quota_row + 2;
    let mut updates = vec![];

    let value_sold = payload.value_sold();
    if value_sold != 0 {
        let current_value = read_number(
            client,
            token,
            spreadsheet_id,
            sheet_name,
            &format!("{SOLD_COLUMN}{sold_row}"),
        )
        .await?;
        updates.push((
            SOLD_COLUMN.to_string(),
            sold_row,
            number_value(current_value + value_sold as f64),
        ));
    }

    if let Some(quota_amount) = quota_amount_value(payload) {
        updates.push((QUOTA_AMOUNT_COLUMN.to_string(), quota_row, quota_amount));
    }

    batch_write_cells_user_entered(client, token, spreadsheet_id, sheet_name, updates).await
}

async fn handle_sell_event(
    client: &reqwest::Client,
    token: &str,
    spreadsheet_id: &str,
    sheet_name: &str,
    payload: &LcStats,
) -> Result<(), String> {
    let value_sold = payload.value_sold();
    if value_sold == 0 {
        return Ok(());
    }

    let current_sell_row =
        first_empty_row(client, token, spreadsheet_id, sheet_name, SOLD_COLUMN).await?;
    let sold_row = if current_sell_row == 1 {
        2
    } else {
        current_sell_row + 2
    };
    let current_value = if current_sell_row == 1 {
        0.0
    } else {
        read_number(
            client,
            token,
            spreadsheet_id,
            sheet_name,
            &format!("{SOLD_COLUMN}{sold_row}"),
        )
        .await?
    };

    batch_write_cells_user_entered(
        client,
        token,
        spreadsheet_id,
        sheet_name,
        vec![(
            SOLD_COLUMN.to_string(),
            sold_row,
            number_value(current_value + value_sold as f64),
        )],
    )
    .await
}

async fn setup_or_match_player_columns(
    client: &reqwest::Client,
    token: &str,
    spreadsheet_id: &str,
    sheet_name: &str,
    lc_stats: &LcStats,
) -> Result<HashMap<String, String>, String> {
    let id_range = format!(
        "{}!{}{}:{}{}",
        quote_sheet_name(sheet_name),
        PLAYER_COLUMNS[0],
        PLAYER_ID_ROW,
        PLAYER_COLUMNS[PLAYER_COLUMNS.len() - 1],
        PLAYER_ID_ROW
    );
    let name_range = format!(
        "{}!{}{}:{}{}",
        quote_sheet_name(sheet_name),
        PLAYER_COLUMNS[0],
        PLAYER_NAME_ROW,
        PLAYER_COLUMNS[PLAYER_COLUMNS.len() - 1],
        PLAYER_NAME_ROW
    );
    let ranges =
        batch_read_ranges(client, token, spreadsheet_id, &[&id_range, &name_range]).await?;
    let data = ranges.first().cloned().unwrap_or_default();
    let existing_row = data
        .get("values")
        .and_then(Value::as_array)
        .and_then(|rows| rows.first())
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let name_data = ranges.get(1).cloned().unwrap_or_default();
    let existing_name_row = name_data
        .get("values")
        .and_then(Value::as_array)
        .and_then(|rows| rows.first())
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut existing_slots = HashMap::new();
    for (index, column) in PLAYER_COLUMNS.iter().enumerate() {
        let steam_id = existing_row
            .get(index)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .trim();
        if !steam_id.is_empty() {
            existing_slots.insert(steam_id.to_string(), column.to_string());
        }
    }

    let players = lc_stats.players_sorted();
    let mut player_columns = HashMap::new();
    if existing_slots.is_empty() {
        let mut updates = vec![];
        for (index, player) in players.iter().take(PLAYER_COLUMNS.len()).enumerate() {
            let column = PLAYER_COLUMNS[index];
            let column = column.to_string();
            player_columns.insert(player.steam_id.clone(), column.clone());
            updates.push((column.clone(), PLAYER_ID_ROW, json!(player.steam_id)));
            updates.push((
                column.clone(),
                PLAYER_NAME_ROW,
                json!(uppercase_text(&player.stats.name)),
            ));
        }
        batch_write_cells_user_entered(client, token, spreadsheet_id, sheet_name, updates).await?;
    } else {
        let mut updates = vec![];
        for player in &players {
            if let Some(column) = existing_slots.get(&player.steam_id) {
                player_columns.insert(player.steam_id.clone(), column.clone());
                if let Some((index, _)) = PLAYER_COLUMNS
                    .iter()
                    .enumerate()
                    .find(|(_, first)| **first == column.as_str())
                {
                    let current_name = existing_name_row
                        .get(index)
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .trim();
                    if current_name.is_empty() {
                        updates.push((
                            column.clone(),
                            PLAYER_NAME_ROW,
                            json!(uppercase_text(&player.stats.name)),
                        ));
                    }
                }
            }
        }
        batch_write_cells_user_entered(client, token, spreadsheet_id, sheet_name, updates).await?;
    }
    Ok(player_columns)
}

fn build_values(
    lc_stats: &LcStats,
    player_columns: &HashMap<String, String>,
    row: usize,
) -> Vec<(String, usize, Value)> {
    let collected = lc_stats.collected_total();
    let available = lc_stats.total_available_value();
    let lost_scrap = lc_stats.lost_scrap_value();
    let mut values = vec![
        (
            MOON_COLUMN.to_string(),
            row,
            json!(maku_moon_name(lc_stats)),
        ),
        (
            WEATHER_COLUMN.to_string(),
            row,
            json!(maku_weather(lc_stats)),
        ),
        (
            LAYOUT_COLUMN.to_string(),
            row,
            json!(uppercase_text(&strip_apostrophe(
                &lc_stats.dungeon_interior()
            ))),
        ),
        (
            ITEM_COUNT_COLUMN.to_string(),
            row,
            json!(lc_stats.dungeon_item_count()),
        ),
        (
            HIVE_COUNT_COLUMN.to_string(),
            row,
            json!(lc_stats.bee_available_count()),
        ),
        (COLLECTED_COLUMN.to_string(), row, json!(collected)),
        (AVAILABLE_COLUMN.to_string(), row, json!(available)),
    ];
    if let Some(quota_amount) = quota_amount_value(lc_stats) {
        values.push((QUOTA_AMOUNT_COLUMN.to_string(), row, quota_amount));
    }
    if let Some(_average) = average_value(collected, available) {
        // values.push(("M".to_string(), row, average));
    }
    if lost_scrap != 0 {
        values.push((LOST_SCRAP_COLUMN.to_string(), row, json!(lost_scrap)));
    }

    let takeoff_time = lc_stats.take_off_time().to_string();
    for player in lc_stats.players_sorted() {
        if let Some(column) = player_columns.get(&player.steam_id) {
            values.push((
                column.clone(),
                row,
                json!(death_status(&player.stats, &takeoff_time)),
            ));
        }
    }

    values
}

fn quota_amount_value(payload: &LcStats) -> Option<Value> {
    let new_quota = payload.new_quota();
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

fn maku_moon_name(payload: &LcStats) -> String {
    uppercase_text(&strip_moon_number(&strip_apostrophe(&payload.moon_name())))
}

fn maku_weather(payload: &LcStats) -> String {
    let weather = strip_apostrophe(&payload.moon_weather());
    if weather.eq_ignore_ascii_case("Mild") {
        "CLEAR".to_string()
    } else {
        uppercase_text(&weather)
    }
}

fn death_status(player: &PlayerStats, takeoff_time: &str) -> String {
    if player.alive {
        if player.disconnected {
            return "D".to_string();
        }
        return "A".to_string();
    }
    if strip_apostrophe(&player.cause_of_death).eq_ignore_ascii_case("Abandoned") {
        return "M".to_string();
    }
    if convert_time_to_number(&player.time_of_death) + 120 < convert_time_to_number(takeoff_time) {
        return "X".to_string();
    }
    "S".to_string()
}

fn convert_time_to_number(time: &str) -> i64 {
    let normalized = strip_apostrophe(time).to_ascii_uppercase();
    let numbers = normalized
        .split(|ch: char| !ch.is_ascii_digit())
        .filter(|part| !part.is_empty())
        .filter_map(|part| part.parse::<i64>().ok())
        .collect::<Vec<_>>();
    if numbers.len() < 2 {
        return 0;
    }
    let day_mod = if normalized.contains("AM") {
        Some("AM")
    } else if normalized.contains("PM") {
        Some("PM")
    } else {
        None
    };
    let Some(day_mod) = day_mod else {
        return 0;
    };
    60 * (numbers[0] % 12) + numbers[1] + if day_mod == "AM" { 0 } else { 720 }
}

fn uppercase_text(value: &str) -> String {
    value.to_uppercase()
}

async fn write_death_notes(
    client: &reqwest::Client,
    token: &str,
    spreadsheet_id: &str,
    sheet_name: &str,
    lc_stats: &LcStats,
    player_columns: &HashMap<String, String>,
    row: usize,
) -> Result<(), String> {
    let takeoff_time = lc_stats.take_off_time().to_string();
    let death_cells = lc_stats
        .players_sorted()
        .into_iter()
        .filter_map(|player| {
            if death_status(&player.stats, &takeoff_time) != "X" {
                return None;
            }
            Some((
                player_columns.get(&player.steam_id)?.clone(),
                death_note(&player.stats),
            ))
        })
        .collect::<Vec<_>>();
    if death_cells.is_empty() {
        return Ok(());
    }

    let sheet_id = get_sheet_id(client, token, spreadsheet_id, sheet_name).await?;
    let requests = death_cells
        .into_iter()
        .map(|(column, note)| value_with_note_request(sheet_id, &column, row, json!("X"), &note))
        .collect::<Vec<_>>();
    batch_update_spreadsheet(client, token, spreadsheet_id, requests).await
}

fn death_note(player: &PlayerStats) -> String {
    let time_of_death = player.time_of_death.trim();
    let cause_of_death = player.cause_of_death.trim();

    let mut parts = vec![];
    if !time_of_death.is_empty() {
        parts.push(format!(
            "TIME: {}",
            uppercase_text(&strip_apostrophe(time_of_death))
        ));
    }
    if !cause_of_death.is_empty() {
        parts.push(format!(
            "CAUSE: {}",
            uppercase_text(&strip_apostrophe(cause_of_death))
        ));
    }
    parts.join("\n")
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
            "rows": [{
                "values": [cell]
            }],
            "fields": "userEnteredValue,note"
        }
    })
}

fn google_user_value(value: Value) -> Value {
    if let Some(value) = value.as_i64() {
        json!({ "numberValue": value })
    } else if let Some(value) = value.as_f64() {
        json!({ "numberValue": value })
    } else {
        json!({ "stringValue": value.as_str().unwrap_or_default() })
    }
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
    fn j_column_uses_new_available_beehive_array() {
        let stats = json!({
            "MoonInfo": { "Name": "68 Artifice", "Weather": "Mild" },
            "DungeonInfo": { "Interior": "Mineshaft", "ItemCount": 34 },
            "BeeInfo": { "Available": [64, 88, 64] },
            "CollectedTotal": 926,
            "TotalAvailableValue": 2133
        });

        let lc_stats = lcstats(&stats);
        let values = build_values(&lc_stats, &HashMap::new(), 7);

        assert_eq!(cell_value(&values, "J"), Some(&json!(3)));
    }

    #[test]
    fn makusheet_uses_v1_columns_and_uppercase_wafrody_values() {
        let stats = json!({
            "NewQuota": "'900",
            "ValueSold": "'130",
            "MoonInfo": { "Name": "'68 Artifice", "Weather": "'Mild" },
            "DungeonInfo": { "Interior": "'Mineshaft", "ItemCount": "'34" },
            "BeeInfo": { "Available": [64, 88, 64] },
            "CollectedTotal": "'926",
            "TotalAvailableValue": "'2133",
            "MissedItems": [
                { "Value": "'40", "CollectedOnPreviousDay": true }
            ],
            "TakeOffTime": "11:00 PM",
            "Players": {
                "1": { "Alive": true, "Disconnected": false },
                "2": { "Alive": true, "Disconnected": true },
                "3": { "Alive": false, "Disconnected": false, "CauseOfDeath": "'Forest Giant", "TimeOfDeath": "'8:00 PM" },
                "4": { "Alive": false, "Disconnected": false }
            }
        });
        let player_columns = HashMap::from([
            ("1".to_string(), "V".to_string()),
            ("2".to_string(), "W".to_string()),
            ("3".to_string(), "X".to_string()),
            ("4".to_string(), "Y".to_string()),
        ]);

        let lc_stats = lcstats(&stats);
        let values = build_values(&lc_stats, &player_columns, 7);

        assert_eq!(cell_value(&values, "B"), Some(&json!(900)));
        assert_eq!(cell_value(&values, "F"), Some(&json!("ARTIFICE")));
        assert_eq!(cell_value(&values, "G"), Some(&json!("CLEAR")));
        assert_eq!(cell_value(&values, "H"), Some(&json!("MINESHAFT")));
        assert_eq!(cell_value(&values, "I"), Some(&json!(34)));
        assert_eq!(cell_value(&values, "K"), Some(&json!(926)));
        assert_eq!(cell_value(&values, "L"), Some(&json!(2133)));
        assert_eq!(cell_value(&values, "R"), None);
        assert_eq!(cell_value(&values, "U"), Some(&json!(40)));
        assert_eq!(cell_value(&values, "V"), Some(&json!("A")));
        assert_eq!(cell_value(&values, "W"), Some(&json!("D")));
        assert_eq!(cell_value(&values, "X"), Some(&json!("X")));
        assert_eq!(cell_value(&values, "Y"), Some(&json!("X")));
    }

    #[test]
    fn gordion_stats_are_economy_only() {
        let stats = json!({
            "NewQuota": 900,
            "ValueSold": 130,
            "MoonInfo": { "Name": "'71 Gorion", "Weather": "Mild" },
            "DungeonInfo": { "Interior": "Mineshaft", "ItemCount": 34 },
            "CollectedTotal": 926,
            "TotalAvailableValue": 2133
        });

        assert!(lcstats(&stats).is_gordion_moon());
    }

    #[test]
    fn galetry_stats_are_economy_only() {
        let stats = json!({
            "MoonInfo": { "Name": "'Galetry" }
        });

        assert!(lcstats(&stats).is_gordion_moon());
    }

    #[test]
    fn dead_players_get_x_with_time_and_cause_note() {
        let player = json!({
            "Name": "Aureo",
            "Alive": false,
            "Disconnected": false,
            "TimeOfDeath": "'8:00 PM",
            "CauseOfDeath": "'Forest Giant"
        });

        let player_stats: PlayerStats = serde_json::from_value(player).unwrap();
        assert_eq!(death_status(&player_stats, "11:00 PM"), "X");
        assert_eq!(
            death_note(&player_stats),
            "TIME: 8:00 PM\nCAUSE: FOREST GIANT"
        );
    }

    fn cell_value<'a>(values: &'a [(String, usize, Value)], column: &str) -> Option<&'a Value> {
        values
            .iter()
            .find(|(value_column, _, _)| value_column == column)
            .map(|(_, _, value)| value)
    }
}

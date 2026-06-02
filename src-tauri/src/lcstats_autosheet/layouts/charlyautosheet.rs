use serde_json::{json, Value};

use crate::google_oauth::LcStatsSettings;
use crate::lcstats_autosheet::layouts::CHARLY_AUTOSHEET_LAYOUT;
use crate::lcstats_autosheet::sheets::{
    batch_update_spreadsheet, batch_write_cells_user_entered, first_empty_row_from, get_sheet_id,
};
use crate::lcstats_autosheet::stats::{
    is_gordion_moon_name, lcstats, strip_apostrophe, strip_moon_number, LcStats,
};

const START_ROW: usize = 3;
const CHECK_COLUMN: &str = "F";
const PLAYER_COLUMNS: [&str; 4] = ["AC", "AD", "AE", "AF"];

const QUOTA_AMOUNT_COLUMN: &str = "B";
const MOON_COLUMN: &str = "F";
const WEATHER_COLUMN: &str = "G";
const LAYOUT_COLUMN: &str = "H";
const ITEM_COUNT_COLUMN: &str = "I";
const BEEHIVE_AMOUNT_COLUMN: &str = "J";
const BEEHIVE_VALUE_COLUMN: &str = "K";
const EGG_VALUE_COLUMN: &str = "L";
const NUTCRACKER_COLUMN: &str = "M";
const BUTLER_COLUMN: &str = "N";
const COLLECTED_COLUMN: &str = "O";
const AVAILABLE_COLUMN: &str = "P";
const MISSING_COLUMN: &str = "Q";
const SOLD_COLUMN: &str = "X";
const SID_COLUMN: &str = "Y";
const INFESTATION_COLUMN: &str = "Z";
const LOST_SCRAP_COLUMN: &str = "AB";
const FOG_COLUMN: &str = "AG";
const METEOR_COLUMN: &str = "AH";
const GIFTS_COLUMN: &str = "AI";

pub async fn write(
    client: &reqwest::Client,
    token: &str,
    settings: &LcStatsSettings,
    stats: &Value,
) -> Result<(), String> {
    if !settings
        .layout
        .eq_ignore_ascii_case(CHARLY_AUTOSHEET_LAYOUT)
    {
        return Ok(());
    }
    let spreadsheet_id = settings.spreadsheet_id.trim();
    let sheet_name = settings.active_sheet_name.trim();
    if spreadsheet_id.is_empty() || sheet_name.is_empty() {
        return Err("spreadsheet or sheet is not set".to_string());
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
    let payload = lcstats(stats);
    if is_economy_moon(&payload) {
        return handle_economy_event(client, token, spreadsheet_id, sheet_name, row, &payload).await;
    }

    let normalized = NormalizedStats::from_stats(&payload);
    batch_write_cells_user_entered(
        client,
        token,
        spreadsheet_id,
        sheet_name,
        build_value_updates(&normalized, row),
    )
    .await?;
    write_note_cells(client, token, spreadsheet_id, sheet_name, row, &normalized).await
}

async fn handle_economy_event(
    client: &reqwest::Client,
    token: &str,
    spreadsheet_id: &str,
    sheet_name: &str,
    row: usize,
    payload: &LcStats,
) -> Result<(), String> {
    let mut updates = vec![];
    let value_sold = payload.value_sold();
    let new_quota = payload.new_quota();

    if value_sold != 0 {
        updates.push((
            SOLD_COLUMN.to_string(),
            row.saturating_sub(3).max(START_ROW),
            json!(value_sold),
        ));
    }
    if new_quota != 0 {
        updates.push((QUOTA_AMOUNT_COLUMN.to_string(), row, json!(new_quota)));
    }

    batch_write_cells_user_entered(client, token, spreadsheet_id, sheet_name, updates).await
}

#[derive(Debug, Clone)]
struct NormalizedPlayer {
    status: String,
    note: Option<String>,
}

#[derive(Debug, Clone)]
struct NoteCell {
    column: &'static str,
    value: Value,
    note: Option<String>,
}

#[derive(Debug, Clone)]
struct NormalizedStats {
    new_quota: i64,
    moon_name: String,
    weather: String,
    interior: String,
    item_count: i64,
    beehive_amount: String,
    beehive_value: String,
    egg_value: String,
    nutcracker_count: usize,
    butler_count: usize,
    collected_total: i64,
    available_total: i64,
    missing: NoteCell,
    value_sold: i64,
    sid: NoteCell,
    infestation: NoteCell,
    lost_scrap: i64,
    players: Vec<NormalizedPlayer>,
    fog: bool,
    meteor: NoteCell,
    gifts: NoteCell,
}

impl NormalizedStats {
    fn from_stats(payload: &LcStats) -> Self {
        let sid_type = non_false_text(payload.sid_type());
        let infestation_type = non_false_text(payload.infestation_type());
        let meteor_time = non_false_text(payload.meteor_shower_time());
        Self {
            new_quota: payload.new_quota(),
            moon_name: strip_moon_number(&strip_apostrophe(&payload.moon_name())),
            weather: charly_weather(&payload.moon_weather()),
            interior: normalize_interior_name(&strip_apostrophe(&payload.dungeon_interior())),
            item_count: payload.dungeon_item_count(),
            beehive_amount: beehive_amount(payload),
            beehive_value: beehive_value(payload),
            egg_value: egg_value(payload),
            nutcracker_count: payload.indoor_enemy_count("Nutcracker"),
            butler_count: payload.indoor_enemy_count("Butler"),
            collected_total: payload.collected_total(),
            available_total: payload.initial_available_value(),
            missing: missing_items_cell(payload),
            value_sold: payload.value_sold(),
            sid: NoteCell {
                column: SID_COLUMN,
                value: json!(sid_type.is_some()),
                note: sid_type,
            },
            infestation: NoteCell {
                column: INFESTATION_COLUMN,
                value: json!(infestation_type.is_some()),
                note: infestation_type,
            },
            lost_scrap: lost_scrap(payload),
            players: normalize_players(payload),
            fog: payload.indoor_fog(),
            meteor: NoteCell {
                column: METEOR_COLUMN,
                value: json!(meteor_time.is_some()),
                note: meteor_time,
            },
            gifts: gifts_cell(payload),
        }
    }
}

fn build_value_updates(stats: &NormalizedStats, row: usize) -> Vec<(String, usize, Value)> {
    let mut updates = vec![
        (MOON_COLUMN.to_string(), row, json!(stats.moon_name)),
        (WEATHER_COLUMN.to_string(), row, json!(stats.weather)),
        (LAYOUT_COLUMN.to_string(), row, json!(stats.interior)),
        (ITEM_COUNT_COLUMN.to_string(), row, json!(stats.item_count)),
        (
            BEEHIVE_AMOUNT_COLUMN.to_string(),
            row,
            blank_or_x(&stats.beehive_amount),
        ),
        (
            BEEHIVE_VALUE_COLUMN.to_string(),
            row,
            blank_or_x(&stats.beehive_value),
        ),
        (
            EGG_VALUE_COLUMN.to_string(),
            row,
            blank_or_x(&stats.egg_value),
        ),
        (
            NUTCRACKER_COLUMN.to_string(),
            row,
            json!(stats.nutcracker_count),
        ),
        (BUTLER_COLUMN.to_string(), row, json!(stats.butler_count)),
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
        (FOG_COLUMN.to_string(), row, json!(stats.fog)),
    ];

    if stats.new_quota != 0 {
        updates.push((QUOTA_AMOUNT_COLUMN.to_string(), row, json!(stats.new_quota)));
    }
    if stats.value_sold != 0 {
        updates.push((SOLD_COLUMN.to_string(), row, json!(stats.value_sold)));
    }
    if stats.lost_scrap != 0 {
        updates.push((LOST_SCRAP_COLUMN.to_string(), row, json!(stats.lost_scrap)));
    }
    for (index, player) in stats.players.iter().take(PLAYER_COLUMNS.len()).enumerate() {
        if player.note.is_none() {
            updates.push((PLAYER_COLUMNS[index].to_string(), row, json!(player.status)));
        }
    }

    updates
}

async fn write_note_cells(
    client: &reqwest::Client,
    token: &str,
    spreadsheet_id: &str,
    sheet_name: &str,
    row: usize,
    stats: &NormalizedStats,
) -> Result<(), String> {
    let sheet_id = get_sheet_id(client, token, spreadsheet_id, sheet_name).await?;
    let mut requests = vec![
        value_with_note_request(sheet_id, &stats.missing, row),
        value_with_note_request(sheet_id, &stats.sid, row),
        value_with_note_request(sheet_id, &stats.infestation, row),
        value_with_note_request(sheet_id, &stats.meteor, row),
        value_with_note_request(sheet_id, &stats.gifts, row),
    ];

    for (index, player) in stats.players.iter().take(PLAYER_COLUMNS.len()).enumerate() {
        if let Some(note) = &player.note {
            requests.push(value_with_note_request(
                sheet_id,
                &NoteCell {
                    column: PLAYER_COLUMNS[index],
                    value: json!(player.status),
                    note: Some(note.clone()),
                },
                row,
            ));
        }
    }

    batch_update_spreadsheet(client, token, spreadsheet_id, requests).await
}

fn value_with_note_request(sheet_id: i64, cell: &NoteCell, row: usize) -> Value {
    let column_index = column_to_index(cell.column);
    let mut value = json!({ "userEnteredValue": google_user_value(cell.value.clone()) });
    if let Some(note) = cell.note.as_ref().filter(|note| !note.trim().is_empty()) {
        value["note"] = json!(note);
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
            "rows": [{ "values": [value] }],
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

fn normalize_players(stats: &LcStats) -> Vec<NormalizedPlayer> {
    stats
        .players_sorted()
        .into_iter()
        .map(|player| {
            let alive = player.stats.alive;
            let disconnected = player.stats.disconnected;
            let cause = strip_apostrophe(&player.stats.cause_of_death)
            .trim()
            .to_string();
            let death_time = strip_apostrophe(&player.stats.time_of_death)
            .trim()
            .to_string();

            let status = if disconnected {
                "DC"
            } else if cause.eq_ignore_ascii_case("abandonment")
                || cause.eq_ignore_ascii_case("abandoned")
            {
                "M"
            } else if alive {
                "S"
            } else {
                "X"
            }
            .to_string();

            let note = if status == "M" {
                None
            } else {
                let mut parts = vec![];
                if !death_time.is_empty() {
                    parts.push(format!("Time of Death: {death_time}"));
                }
                if !cause.is_empty() {
                    parts.push(format!("Cause of Death: {cause}"));
                }
                (!parts.is_empty()).then(|| parts.join("\n"))
            };

            NormalizedPlayer { status, note }
        })
        .collect()
}

fn beehive_amount(stats: &LcStats) -> String {
    let values = beehive_values(stats);
    if values.is_empty() {
        return String::new();
    }
    let small = values.iter().filter(|&&value| value < 100).count();
    let large = values.iter().filter(|&&value| value >= 100).count();
    format!("{small}|{large}")
}

fn beehive_value(stats: &LcStats) -> String {
    let values = beehive_values(stats);
    if values.is_empty() {
        return String::new();
    }
    let small = values
        .iter()
        .find(|&&value| value < 100)
        .copied()
        .unwrap_or(0);
    let large = values
        .iter()
        .find(|&&value| value >= 100)
        .copied()
        .unwrap_or(0);
    format!("{small}|{large}")
}

fn beehive_values(stats: &LcStats) -> Vec<i64> {
    stats.bee_available_values()
}

fn egg_value(stats: &LcStats) -> String {
    let mut values = stats.egg_available_values();
    values.sort_unstable();
    values
        .into_iter()
        .map(|value| value.to_string())
        .collect::<Vec<_>>()
        .join("|")
}

fn gifts_cell(stats: &LcStats) -> NoteCell {
    let gifts = stats.gift_boxes();
    if gifts.is_empty() {
        return NoteCell {
            column: GIFTS_COLUMN,
            value: json!("X"),
            note: None,
        };
    }

    let collected = gifts
        .iter()
        .filter(|gift| gift.collected)
        .collect::<Vec<_>>();
    let total_net = collected
        .iter()
        .map(|gift| gift.new_scrap_value - gift.gift_scrap_value)
        .sum::<i64>();
    let sign = if total_net >= 0 { "+" } else { "" };
    let cell_value = if collected.is_empty() {
        "X".to_string()
    } else {
        format!("{}|{sign}{total_net}", collected.len())
    };
    let note = gifts
        .iter()
        .enumerate()
        .map(|(index, gift)| {
            format!(
                "Box {}: NewScrapValue={}, GiftScrapValue={}, Collected={}",
                index + 1,
                gift.new_scrap_value,
                gift.gift_scrap_value,
                gift.collected
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    NoteCell {
        column: GIFTS_COLUMN,
        value: json!(cell_value),
        note: Some(note),
    }
}

fn missing_items_cell(stats: &LcStats) -> NoteCell {
    let missing = stats.active_missed_items()
        .collect::<Vec<_>>();
    if missing.is_empty() {
        return NoteCell {
            column: MISSING_COLUMN,
            value: json!("X"),
            note: None,
        };
    }
    let note = missing
        .iter()
        .map(|item| {
            format!(
                "{}: {}",
                if item.item_type.is_empty() {
                    "Unknown"
                } else {
                    &item.item_type
                },
                item.value
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    NoteCell {
        column: MISSING_COLUMN,
        value: json!(missing.len().to_string()),
        note: Some(note),
    }
}

fn lost_scrap(stats: &LcStats) -> i64 {
    stats.lost_missed_items().map(|item| item.value).sum()
}

fn is_economy_moon(payload: &LcStats) -> bool {
    is_gordion_moon_name(&payload.moon_name())
}

fn charly_weather(value: &str) -> String {
    let weather = strip_apostrophe(value);
    if weather.eq_ignore_ascii_case("Mild") {
        "Clear".to_string()
    } else {
        weather
    }
}

fn normalize_interior_name(value: &str) -> String {
    let without_flow = value.replace("Flow", "").replace("flow", "");
    let mut out = String::new();
    let mut previous_lowercase = false;
    for ch in without_flow.chars().filter(|ch| !ch.is_ascii_digit()) {
        if ch.is_ascii_uppercase() && previous_lowercase {
            out.push(' ');
        }
        previous_lowercase = ch.is_ascii_lowercase();
        out.push(ch);
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn non_false_text(value: &str) -> Option<String> {
    let value = strip_apostrophe(value).trim().to_string();
    if value.is_empty()
        || value.eq_ignore_ascii_case("false")
        || value.eq_ignore_ascii_case("none")
        || value == "0"
    {
        None
    } else {
        Some(value)
    }
}

fn blank_or_x(value: &str) -> Value {
    if value.trim().is_empty() {
        json!("X")
    } else {
        json!(value)
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
    fn maps_charly_columns() {
        let stats = json!({
            "NewQuota": "'900",
            "ValueSold": "'130",
            "MoonInfo": { "Name": "'68 Artifice", "Weather": "'Mild" },
            "DungeonInfo": { "Interior": "'MineshaftFlow", "ItemCount": "'34" },
            "BeeInfo": { "Available": [64, 132] },
            "EggInfo": { "Available": [18, 12] },
            "IndoorSpawns": [
                { "Enemy": "Nutcracker" },
                { "Enemy": "Butler" },
                { "Enemy": "Butler" }
            ],
            "CollectedTotal": "'926",
            "InitialAvailableValue": "'2133",
            "SIDType": "'Mineshaft",
            "InfestationType": "'Spiders",
            "IndoorFog": true,
            "MeteorShowerTime": "'8:30 PM",
            "GiftBoxes": [
                { "GiftValue": 80, "ScrapValue": 20, "Collected": true }
            ],
            "MissedItems": [
                { "ItemType": "V-type engine", "Value": "'40", "CollectedOnPreviousDay": false },
                { "ItemType": "Stop sign", "Value": 30, "CollectedOnPreviousDay": true }
            ],
            "Players": {
                "1": { "Alive": true, "Disconnected": false },
                "2": { "Alive": false, "Disconnected": true },
                "3": { "Alive": false, "Disconnected": false, "CauseOfDeath": "'Forest Giant", "TimeOfDeath": "'8:00 PM" }
            }
        });

        let payload = lcstats(&stats);
        let normalized = NormalizedStats::from_stats(&payload);
        let updates = build_value_updates(&normalized, 7);

        assert_eq!(cell_value(&updates, QUOTA_AMOUNT_COLUMN), Some(&json!(900)));
        assert_eq!(cell_value(&updates, MOON_COLUMN), Some(&json!("Artifice")));
        assert_eq!(cell_value(&updates, WEATHER_COLUMN), Some(&json!("Clear")));
        assert_eq!(
            cell_value(&updates, LAYOUT_COLUMN),
            Some(&json!("Mineshaft"))
        );
        assert_eq!(cell_value(&updates, ITEM_COUNT_COLUMN), Some(&json!(34)));
        assert_eq!(
            cell_value(&updates, BEEHIVE_AMOUNT_COLUMN),
            Some(&json!("1|1"))
        );
        assert_eq!(
            cell_value(&updates, BEEHIVE_VALUE_COLUMN),
            Some(&json!("64|132"))
        );
        assert_eq!(
            cell_value(&updates, EGG_VALUE_COLUMN),
            Some(&json!("12|18"))
        );
        assert_eq!(cell_value(&updates, NUTCRACKER_COLUMN), Some(&json!(1)));
        assert_eq!(cell_value(&updates, BUTLER_COLUMN), Some(&json!(2)));
        assert_eq!(cell_value(&updates, COLLECTED_COLUMN), Some(&json!(926)));
        assert_eq!(cell_value(&updates, AVAILABLE_COLUMN), Some(&json!(2133)));
        assert_eq!(cell_value(&updates, SOLD_COLUMN), Some(&json!(130)));
        assert_eq!(cell_value(&updates, LOST_SCRAP_COLUMN), Some(&json!(30)));
        assert_eq!(cell_value(&updates, FOG_COLUMN), Some(&json!(true)));
        assert_eq!(normalized.missing.value, json!("1"));
        assert_eq!(normalized.sid.value, json!(true));
        assert_eq!(normalized.infestation.value, json!(true));
        assert_eq!(normalized.meteor.value, json!(true));
        assert_eq!(normalized.gifts.value, json!("1|+60"));
        assert_eq!(normalized.players[0].status, "S");
        assert_eq!(normalized.players[1].status, "DC");
        assert_eq!(normalized.players[2].status, "X");
    }

    #[test]
    fn empty_optional_items_write_x() {
        let stats = json!({});
        let payload = lcstats(&stats);
        let normalized = NormalizedStats::from_stats(&payload);
        let updates = build_value_updates(&normalized, 7);

        assert_eq!(
            cell_value(&updates, BEEHIVE_AMOUNT_COLUMN),
            Some(&json!("X"))
        );
        assert_eq!(
            cell_value(&updates, BEEHIVE_VALUE_COLUMN),
            Some(&json!("X"))
        );
        assert_eq!(cell_value(&updates, EGG_VALUE_COLUMN), Some(&json!("X")));
        assert_eq!(normalized.missing.value, json!("X"));
        assert_eq!(normalized.gifts.value, json!("X"));
    }

    #[test]
    fn gift_boxes_opened_feed_gift_value() {
        let stats = json!({
            "GiftBoxesOpened": [
                { "NewScrapValue": 39, "GiftScrapValue": 12, "Collected": false },
                { "NewScrapValue": 162, "GiftScrapValue": 26, "Collected": true }
            ]
        });
        let payload = lcstats(&stats);
        let normalized = NormalizedStats::from_stats(&payload);

        assert_eq!(normalized.gifts.value, json!("1|+136"));
        assert!(normalized
            .gifts
            .note
            .as_deref()
            .unwrap_or_default()
            .contains("NewScrapValue=162"));
    }

    #[test]
    fn economy_moons_are_detected() {
        assert!(is_economy_moon(&lcstats(&json!({
            "MoonInfo": { "Name": "'71 Gordion" }
        }))));
        assert!(is_economy_moon(&lcstats(&json!({
            "MoonInfo": { "Name": "'Galetry" }
        }))));
    }

    fn cell_value<'a>(values: &'a [(String, usize, Value)], column: &str) -> Option<&'a Value> {
        values
            .iter()
            .find(|(value_column, _, _)| value_column == column)
            .map(|(_, _, value)| value)
    }
}

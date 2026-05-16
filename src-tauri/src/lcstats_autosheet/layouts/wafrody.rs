use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap};

use crate::google_oauth::LcStatsSettings;
use crate::lcstats_autosheet::layouts::WAFRODY_LAYOUT;
use crate::lcstats_autosheet::sheets::{
    batch_read_ranges, batch_update_spreadsheet, batch_write_cells_user_entered,
    first_empty_row_from, get_sheet_id, number_value, quote_sheet_name, read_number, read_range,
};
use crate::lcstats_autosheet::stats::{
    array_at, array_at_any, bool_at, object_at, string_at, strip_moon_number, value_at,
};

const TARGET_SHEET_CELL: &str = "A1";
const CHECK_COLUMN: &str = "X";
const START_ROW: usize = 4;
const PLAYER_ID_ROW: usize = 199;
const PLAYER_NAME_ROW: usize = 3;
const PLAYER_COLUMNS: [&str; 4] = ["AD", "AE", "AF", "AG"];

const MOON_COLUMN: &str = "G";
const WEATHER_COLUMN: &str = "H";
const INTERIOR_COLUMN: &str = "I";
const ITEM_COUNT_COLUMN: &str = "K";
const BEEHIVE_VALUE_COUNT_COLUMN: &str = "L";
const CHEAP_BEEHIVE_COLUMN: &str = "M";
const EXPENSIVE_BEEHIVE_COLUMN: &str = "N";
const APPARATUS_SPAWNED_COLUMN: &str = "O";
const EGG_VALUE_COLUMN: &str = "P";
const INDOOR_FOG_COLUMN: &str = "Q";
const METEOR_SHOWER_TIME_COLUMN: &str = "S";
const SHOTGUNS_COLLECTED_COLUMN: &str = "T";
const NUTCRACKER_COUNT_COLUMN: &str = "U";
const KNIVES_COLLECTED_COLUMN: &str = "V";
const BUTLER_COUNT_COLUMN: &str = "W";
const COLLECTED_TOTAL_COLUMN: &str = "X";
const BOTTOM_LINE_COLUMN: &str = "Y";
const REAL_LINE_COLUMN: &str = "Z";
const MISSED_ITEMS_NOTE_COLUMN: &str = "AA";
const VALUE_SOLD_COLUMN: &str = "AJ";
const LOST_SCRAP_TOTAL_COLUMN: &str = "AR";
const LOST_SCRAP_TOTAL_ROW: usize = 31;
const NEW_QUOTA_COLUMN: &str = "C";
const SEED_COLUMN: &str = "BE";
const SID_COLUMN: &str = "J";
const INFESTATION_COLUMN: &str = "R";

pub async fn write(
    client: &reqwest::Client,
    token: &str,
    settings: &LcStatsSettings,
    stats: &Value,
) -> Result<(), String> {
    if !settings.layout.eq_ignore_ascii_case(WAFRODY_LAYOUT) {
        return Ok(());
    }
    let spreadsheet_id = settings.spreadsheet_id.trim();
    let source_sheet = settings.active_sheet_name.trim();
    if spreadsheet_id.is_empty() || source_sheet.is_empty() {
        return Err("spreadsheet or sheet is not set".to_string());
    }

    let target_sheet = resolve_target_sheet(client, token, spreadsheet_id, source_sheet).await?;
    let target_row = first_empty_row_from(
        client,
        token,
        spreadsheet_id,
        &target_sheet.name,
        CHECK_COLUMN,
        START_ROW,
    )
    .await?;

    if is_gordion_stats(stats) {
        handle_gordion(
            client,
            token,
            spreadsheet_id,
            &target_sheet.name,
            target_row,
            stats,
        )
        .await?;
        return Ok(());
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
        build_value_updates(&normalized, &player_columns, target_row),
    )
    .await?;
    write_rich_cells(
        client,
        token,
        spreadsheet_id,
        &target_sheet.name,
        target_sheet.id,
        target_row,
        stats,
        &normalized,
        &player_columns,
    )
    .await?;
    add_lost_scraps_to_total(client, token, spreadsheet_id, &target_sheet.name, stats).await
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
                "Wafrody target sheet cell {TARGET_SHEET_CELL} contained '{candidate}', but it is not a valid sheet name ({e}); using '{source_sheet}'"
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
    let id_range = format!(
        "{}!{}{}:{}{}",
        quote_sheet_name(sheet_name),
        PLAYER_COLUMNS[0],
        PLAYER_ID_ROW,
        PLAYER_COLUMNS[PLAYER_COLUMNS.len() - 1],
        PLAYER_ID_ROW
    );
    let ranges = batch_read_ranges(client, token, spreadsheet_id, &[&id_range]).await?;
    let existing_row = ranges
        .first()
        .and_then(|data| data.get("values"))
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
            existing_slots.insert(steam_id.to_string(), (*column).to_string());
        }
    }

    let players = object_at(stats, &["Players"]);
    let mut player_columns = HashMap::new();
    if existing_slots.is_empty() {
        let mut updates = vec![];
        for (index, (steam_id, player)) in players.iter().take(PLAYER_COLUMNS.len()).enumerate() {
            let column = PLAYER_COLUMNS[index].to_string();
            player_columns.insert(steam_id.clone(), column.clone());
            updates.push((column.clone(), PLAYER_ID_ROW, json!(steam_id)));
            updates.push((
                column,
                PLAYER_NAME_ROW,
                json!(player
                    .get("Name")
                    .and_then(Value::as_str)
                    .unwrap_or_default()),
            ));
        }
        batch_write_cells_user_entered(client, token, spreadsheet_id, sheet_name, updates).await?;
    } else {
        for steam_id in players.keys() {
            if let Some(column) = existing_slots.get(steam_id) {
                player_columns.insert(steam_id.clone(), column.clone());
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
    beehive_value_count: String,
    cheap_beehive_value: Option<i64>,
    expensive_beehive_value: Option<i64>,
    egg_value: Option<i64>,
    egg_values: Vec<i64>,
    has_meteor_shower: bool,
    meteor_shower_time: Option<String>,
    shotguns_collected: i64,
    nutcracker_count: usize,
    knives_collected: i64,
    butler_count: usize,
    collected_total: i64,
    bottom_line: i64,
    real_line: i64,
    value_sold: i64,
    new_quota: i64,
    seed: String,
    has_sid: bool,
    sid_type: Option<String>,
    apparatus_spawned: bool,
    indoor_fog: bool,
    infestation_type: String,
    players: HashMap<String, NormalizedPlayer>,
}

impl NormalizedStats {
    fn from_stats(stats: &Value) -> Self {
        let item_count = intish_at(stats, &["DungeonInfo", "ItemCount"]);
        let beehives = beehive_price_summary(stats);
        let egg_values = intish_array_any(
            stats,
            &[
                &["EggInfo", "Available"][..],
                &["BirdInfo", "EggValues"][..],
            ],
        );
        let sid_type = non_false_text(&string_at(stats, &["SIDType"]));
        let meteor_shower_time = non_false_text(&string_at(stats, &["MeteorShowerTime"]));
        Self {
            moon_name: strip_apostrophe(&string_at(stats, &["MoonInfo", "Name"])),
            weather: wafrody_weather(&string_at(stats, &["MoonInfo", "Weather"])),
            interior: strip_apostrophe(&string_at(stats, &["DungeonInfo", "Interior"])),
            item_count,
            beehive_value_count: beehives.count_by_value,
            cheap_beehive_value: beehives.cheap_value,
            expensive_beehive_value: beehives.expensive_value,
            egg_value: (!egg_values.is_empty()).then(|| egg_values.iter().sum()),
            egg_values,
            has_meteor_shower: meteor_shower_time.is_some(),
            meteor_shower_time,
            shotguns_collected: collected_count_or_legacy_int(
                stats,
                &["ShotgunInfo", "Collected"],
                &["ShotgunsCollected"],
            ),
            nutcracker_count: indoor_enemy_count(stats, "Nutcracker"),
            knives_collected: collected_count_or_legacy_int(
                stats,
                &["KnifeInfo", "Collected"],
                &["KnivesCollected"],
            ),
            butler_count: indoor_enemy_count(stats, "Butler"),
            collected_total: intish_at(stats, &["CollectedTotal"]),
            bottom_line: intish_at(stats, &["BottomLine"]),
            real_line: intish_at(stats, &["BottomLineTrue"]),
            value_sold: intish_at(stats, &["ValueSold"]),
            new_quota: intish_at(stats, &["NewQuota"]),
            seed: strip_apostrophe(&string_at(stats, &["Seed"])),
            has_sid: sid_type.is_some(),
            sid_type,
            apparatus_spawned: bool_at(stats, &["AppSpawned"]),
            indoor_fog: bool_at(stats, &["IndoorFog"]),
            infestation_type: strip_apostrophe(&string_at(stats, &["InfestationType"])),
            players: normalize_players(stats),
        }
    }
}

fn normalize_players(stats: &Value) -> HashMap<String, NormalizedPlayer> {
    let takeoff_time = string_at(stats, &["TakeOffTime"]);
    object_at(stats, &["Players"])
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
            } else if cause_of_death == "Abandoned" {
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

fn build_value_updates(
    stats: &NormalizedStats,
    player_columns: &HashMap<String, String>,
    row: usize,
) -> Vec<(String, usize, Value)> {
    let mut values = vec![
        (
            MOON_COLUMN.to_string(),
            row,
            json!(strip_moon_number(&stats.moon_name)),
        ),
        (WEATHER_COLUMN.to_string(), row, json!(stats.weather)),
        (INTERIOR_COLUMN.to_string(), row, json!(stats.interior)),
        (SID_COLUMN.to_string(), row, json!(stats.has_sid)),
        (ITEM_COUNT_COLUMN.to_string(), row, json!(stats.item_count)),
        (
            BEEHIVE_VALUE_COUNT_COLUMN.to_string(),
            row,
            json!(stats.beehive_value_count),
        ),
        (
            APPARATUS_SPAWNED_COLUMN.to_string(),
            row,
            json!(stats.apparatus_spawned),
        ),
        (
            EGG_VALUE_COLUMN.to_string(),
            row,
            optional_i64_or_blank(stats.egg_value),
        ),
        (INDOOR_FOG_COLUMN.to_string(), row, json!(stats.indoor_fog)),
        (
            INFESTATION_COLUMN.to_string(),
            row,
            json!(stats.infestation_type),
        ),
        (
            METEOR_SHOWER_TIME_COLUMN.to_string(),
            row,
            json!(stats.has_meteor_shower),
        ),
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
        (REAL_LINE_COLUMN.to_string(), row, json!(stats.real_line)),
        (SEED_COLUMN.to_string(), row, json!(stats.seed)),
    ];

    if stats.nutcracker_count != 0 {
        values.push((
            SHOTGUNS_COLLECTED_COLUMN.to_string(),
            row,
            json!(stats.shotguns_collected),
        ));
        values.push((
            NUTCRACKER_COUNT_COLUMN.to_string(),
            row,
            json!(stats.nutcracker_count),
        ));
    }
    if stats.butler_count != 0 {
        values.push((
            KNIVES_COLLECTED_COLUMN.to_string(),
            row,
            json!(stats.knives_collected),
        ));
        values.push((
            BUTLER_COUNT_COLUMN.to_string(),
            row,
            json!(stats.butler_count),
        ));
    }
    if let Some(value) = stats.cheap_beehive_value {
        values.push((CHEAP_BEEHIVE_COLUMN.to_string(), row, json!(value)));
    }
    if let Some(value) = stats.expensive_beehive_value {
        values.push((EXPENSIVE_BEEHIVE_COLUMN.to_string(), row, json!(value)));
    }
    if stats.value_sold != 0 {
        values.push((VALUE_SOLD_COLUMN.to_string(), row, json!(stats.value_sold)));
    }
    if stats.new_quota != 0 {
        values.push((NEW_QUOTA_COLUMN.to_string(), row, json!(stats.new_quota)));
    }
    for (steam_id, player) in &stats.players {
        if let Some(column) = player_columns.get(steam_id) {
            values.push((column.clone(), row, json!(player.status)));
        }
    }

    values
}

async fn write_rich_cells(
    client: &reqwest::Client,
    token: &str,
    spreadsheet_id: &str,
    sheet_name: &str,
    sheet_id: Option<i64>,
    row: usize,
    raw_stats: &Value,
    stats: &NormalizedStats,
    player_columns: &HashMap<String, String>,
) -> Result<(), String> {
    let sheet_id = match sheet_id {
        Some(sheet_id) => sheet_id,
        None => get_sheet_id(client, token, spreadsheet_id, sheet_name).await?,
    };
    let mut requests = vec![];
    if let Some(sid_type) = &stats.sid_type {
        requests.push(value_with_note_request(
            sheet_id,
            SID_COLUMN,
            row,
            json!(stats.has_sid),
            sid_type,
        ));
    }
    if let Some(meteor_shower_time) = &stats.meteor_shower_time {
        requests.push(value_with_note_request(
            sheet_id,
            METEOR_SHOWER_TIME_COLUMN,
            row,
            json!(stats.has_meteor_shower),
            meteor_shower_time,
        ));
    }

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

    if !stats.egg_values.is_empty() {
        requests.push(note_request(
            sheet_id,
            EGG_VALUE_COLUMN,
            row,
            &format!("{:?}", stats.egg_values),
        ));
    }

    if let Some(note) = enemy_spawn_times_note(raw_stats, "Nutcracker") {
        requests.push(note_request(sheet_id, NUTCRACKER_COUNT_COLUMN, row, &note));
    }
    if let Some(note) = enemy_spawn_times_note(raw_stats, "Butler") {
        requests.push(note_request(sheet_id, BUTLER_COUNT_COLUMN, row, &note));
    }
    if let Some(note) = missed_items_note(raw_stats) {
        requests.push(note_request(sheet_id, MISSED_ITEMS_NOTE_COLUMN, row, &note));
    }

    if requests.is_empty() {
        return Ok(());
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
            &format!("{VALUE_SOLD_COLUMN}{target_line}"),
        )
        .await?;
        updates.push((
            VALUE_SOLD_COLUMN.to_string(),
            target_line,
            number_value(current_value + value_sold as f64),
        ));
    }
    if new_quota != 0 {
        updates.push((
            NEW_QUOTA_COLUMN.to_string(),
            target_line + 3,
            json!(new_quota),
        ));
    }
    if updates.is_empty() {
        return Ok(());
    }
    batch_write_cells_user_entered(client, token, spreadsheet_id, sheet_name, updates).await
}

async fn add_lost_scraps_to_total(
    client: &reqwest::Client,
    token: &str,
    spreadsheet_id: &str,
    sheet_name: &str,
    stats: &Value,
) -> Result<(), String> {
    let lost_total = lost_scrap_total(stats);
    if lost_total == 0 {
        return Ok(());
    }
    let current_value = read_number(
        client,
        token,
        spreadsheet_id,
        sheet_name,
        &format!("{LOST_SCRAP_TOTAL_COLUMN}{LOST_SCRAP_TOTAL_ROW}"),
    )
    .await?;
    batch_write_cells_user_entered(
        client,
        token,
        spreadsheet_id,
        sheet_name,
        vec![(
            LOST_SCRAP_TOTAL_COLUMN.to_string(),
            LOST_SCRAP_TOTAL_ROW,
            number_value(current_value + lost_total as f64),
        )],
    )
    .await
}

fn value_with_note_request(
    sheet_id: i64,
    column: &str,
    row: usize,
    value: Value,
    note: &str,
) -> Value {
    row_values_with_notes_request(sheet_id, column, row, vec![(value, note.to_string())])
}

fn note_request(sheet_id: i64, column: &str, row: usize, note: &str) -> Value {
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
                    "note": note
                }]
            }],
            "fields": "note"
        }
    })
}

fn row_values_with_notes_request(
    sheet_id: i64,
    column: &str,
    row: usize,
    cells: Vec<(Value, String)>,
) -> Value {
    let column_index = column_to_index(column);
    let has_notes = cells.iter().any(|(_, note)| !note.is_empty());
    let values = cells
        .into_iter()
        .map(|(value, note)| {
            let mut cell = json!({ "userEnteredValue": google_user_value(value) });
            if !note.is_empty() {
                cell["note"] = json!(note);
            }
            cell
        })
        .collect::<Vec<_>>();
    let fields = if has_notes {
        "userEnteredValue,note"
    } else {
        "userEnteredValue"
    };
    json!({
        "updateCells": {
            "range": {
                "sheetId": sheet_id,
                "startRowIndex": row.saturating_sub(1),
                "endRowIndex": row,
                "startColumnIndex": column_index,
                "endColumnIndex": column_index + values.len()
            },
            "rows": [{
                "values": values
            }],
            "fields": fields
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

fn is_gordion_stats(stats: &Value) -> bool {
    let moon = strip_moon_number(&strip_apostrophe(&string_at(stats, &["MoonInfo", "Name"])));
    let normalized = moon
        .trim()
        .chars()
        .filter(|ch| ch.is_ascii_alphabetic())
        .collect::<String>()
        .to_ascii_uppercase();
    normalized == "GORDION" || normalized == "GORION"
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

fn intish_array_any(stats: &Value, paths: &[&[&str]]) -> Vec<i64> {
    array_at_any(stats, paths)
        .iter()
        .map(value_as_i64)
        .collect()
}

fn collected_count_or_legacy_int(
    stats: &Value,
    collected_path: &[&str],
    legacy_path: &[&str],
) -> i64 {
    if let Some(collected) = value_at(stats, collected_path).and_then(Value::as_array) {
        collected.len() as i64
    } else {
        intish_at(stats, legacy_path)
    }
}

fn optional_i64_or_blank(value: Option<i64>) -> Value {
    value.map_or_else(|| json!(""), |value| json!(value))
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

fn enemy_spawn_times_note(stats: &Value, enemy: &str) -> Option<String> {
    let times = array_at(stats, &["IndoorSpawns"])
        .iter()
        .filter(|spawn| spawn.get("Enemy").and_then(Value::as_str) == Some(enemy))
        .filter_map(|spawn| spawn.get("SpawnTime").and_then(Value::as_str))
        .filter(|time| !time.trim().is_empty())
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    (!times.is_empty()).then(|| format!("Spawn times:\n{}", times.join("\n")))
}

fn missed_items_note(stats: &Value) -> Option<String> {
    let mut missed_by_type: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for item in array_at(stats, &["MissedItems"]) {
        let item_type = item
            .get("ItemType")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .trim();
        if item_type.is_empty() {
            continue;
        }
        let mut value_text = item
            .get("Value")
            .map(value_as_i64)
            .map(|value| value.to_string())
            .unwrap_or_default();
        if item.get("CollectedOnPreviousDay").and_then(Value::as_bool) == Some(true) {
            value_text.push_str("(lost)");
        }
        missed_by_type
            .entry(item_type.to_string())
            .or_default()
            .push(value_text);
    }
    if missed_by_type.is_empty() {
        return None;
    }
    Some(
        missed_by_type
            .into_iter()
            .map(|(item_type, values)| format!("{item_type} : {}", values.join(",")))
            .collect::<Vec<_>>()
            .join("\n"),
    )
}

fn lost_scrap_total(stats: &Value) -> i64 {
    array_at(stats, &["MissedItems"])
        .iter()
        .filter(|item| item.get("CollectedOnPreviousDay").and_then(Value::as_bool) == Some(true))
        .map(|item| item.get("Value").map(value_as_i64).unwrap_or(0))
        .sum()
}

fn run_block_start_row(current_row: usize) -> usize {
    let line_to_place = current_row as isize - 1;
    let offset = (line_to_place - START_ROW as isize).div_euclid(3) * 3;
    (START_ROW as isize + offset).max(1) as usize
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BeehivePriceSummary {
    count_by_value: String,
    cheap_value: Option<i64>,
    expensive_value: Option<i64>,
}

fn beehive_price_summary(stats: &Value) -> BeehivePriceSummary {
    let mut values = array_at_any(
        stats,
        &[&["BeeInfo", "Available"][..], &["BeeInfo", "Values"][..]],
    )
    .iter()
    .filter_map(value_as_i64_option)
    .collect::<Vec<_>>();
    values.sort_unstable();

    let Some(&cheap_value) = values.first() else {
        return BeehivePriceSummary {
            count_by_value: String::new(),
            cheap_value: None,
            expensive_value: None,
        };
    };
    let expensive_value = *values.last().unwrap_or(&cheap_value);
    if cheap_value == expensive_value {
        return BeehivePriceSummary {
            count_by_value: values.len().to_string(),
            cheap_value: Some(cheap_value),
            expensive_value: None,
        };
    }

    let cheap_count = values.iter().filter(|&&value| value == cheap_value).count();
    let expensive_count = values
        .iter()
        .filter(|&&value| value == expensive_value)
        .count();
    BeehivePriceSummary {
        count_by_value: format!("{cheap_count}|{expensive_count}"),
        cheap_value: Some(cheap_value),
        expensive_value: Some(expensive_value),
    }
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

fn indoor_enemy_count(stats: &Value, enemy: &str) -> usize {
    array_at(stats, &["IndoorSpawns"])
        .iter()
        .filter(|spawn| {
            spawn
                .get("Enemy")
                .and_then(Value::as_str)
                .map(|value| value.eq_ignore_ascii_case(enemy))
                .unwrap_or(false)
        })
        .count()
}

fn value_as_i64(value: &Value) -> i64 {
    value_as_i64_option(value).unwrap_or(0)
}

fn value_as_i64_option(value: &Value) -> Option<i64> {
    value.as_i64().or_else(|| {
        value
            .as_str()
            .and_then(|text| strip_apostrophe(text).trim().parse::<i64>().ok())
    })
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
    fn k_line_uses_spawned_item_count() {
        let stats = json!({
            "DungeonInfo": { "ItemCount": 12 },
            "MissedItems": [
                { "ItemType": "Apparatus" },
                { "ItemType": "Bee hive" }
            ],
            "BeeInfo": { "Values": [64, 88, 64] }
        });

        let normalized = NormalizedStats::from_stats(&stats);
        let updates = build_value_updates(&normalized, &HashMap::new(), 7);

        assert_eq!(normalized.item_count, 12);
        assert_eq!(cell_value(&updates, ITEM_COUNT_COLUMN), Some(&json!(12)));
    }

    #[test]
    fn j_o_q_lines_use_boolean_stats() {
        let stats = json!({
            "SIDType": "Mineshaft",
            "AppSpawned": false,
            "IndoorFog": true
        });

        let normalized = NormalizedStats::from_stats(&stats);
        let updates = build_value_updates(&normalized, &HashMap::new(), 7);

        assert_eq!(cell_value(&updates, SID_COLUMN), Some(&json!(true)));
        assert_eq!(
            cell_value(&updates, APPARATUS_SPAWNED_COLUMN),
            Some(&json!(false))
        );
        assert_eq!(cell_value(&updates, INDOOR_FOG_COLUMN), Some(&json!(true)));
    }

    #[test]
    fn empty_sid_type_writes_false() {
        let stats = json!({
            "SIDType": "",
            "AppSpawned": false,
            "IndoorFog": false
        });

        let normalized = NormalizedStats::from_stats(&stats);
        let updates = build_value_updates(&normalized, &HashMap::new(), 7);

        assert_eq!(cell_value(&updates, SID_COLUMN), Some(&json!(false)));
    }

    #[test]
    fn l_line_counts_low_and_high_price_beehives() {
        let stats = json!({
            "BeeInfo": { "Values": [88, 64, 64, 88, 64] }
        });

        let summary = beehive_price_summary(&stats);

        assert_eq!(
            summary,
            BeehivePriceSummary {
                count_by_value: "3|2".to_string(),
                cheap_value: Some(64),
                expensive_value: Some(88),
            }
        );
    }

    #[test]
    fn l_line_uses_one_count_when_beehive_prices_match() {
        let stats = json!({
            "BeeInfo": { "Values": [80, 80] }
        });

        let summary = beehive_price_summary(&stats);

        assert_eq!(
            summary,
            BeehivePriceSummary {
                count_by_value: "2".to_string(),
                cheap_value: Some(80),
                expensive_value: None,
            }
        );
    }

    #[test]
    fn new_stat_arrays_feed_wafrody_values() {
        let stats = json!({
            "BeeInfo": { "Available": [64, 88, 64], "Collected": [64] },
            "EggInfo": { "Available": [12, 18], "Collected": [12] },
            "ShotgunInfo": { "Available": [60], "Collected": [60] },
            "KnifeInfo": { "Available": [35, 35], "Collected": [35, 35] }
        });

        let normalized = NormalizedStats::from_stats(&stats);
        let summary = beehive_price_summary(&stats);

        assert_eq!(summary.count_by_value, "2|1");
        assert_eq!(summary.cheap_value, Some(64));
        assert_eq!(summary.expensive_value, Some(88));
        assert_eq!(normalized.egg_value, Some(30));
        assert_eq!(normalized.shotguns_collected, 1);
        assert_eq!(normalized.knives_collected, 2);
    }

    #[test]
    fn player_with_death_cause_is_x_even_after_cutoff() {
        let stats = json!({
            "TakeOffTime": "11:57 PM",
            "Players": {
                "765": {
                    "Name": "AureoHatsune",
                    "Alive": false,
                    "Disconnected": false,
                    "TimeOfDeath": "10:12 PM",
                    "CauseOfDeath": "Forest Giant"
                }
            }
        });

        let players = normalize_players(&stats);

        assert_eq!(
            players.get("765").map(|player| player.status.as_str()),
            Some("X")
        );
    }

    #[test]
    fn gordion_block_start_matches_script_rows() {
        assert_eq!(run_block_start_row(4), 1);
        assert_eq!(run_block_start_row(5), 4);
        assert_eq!(run_block_start_row(7), 4);
        assert_eq!(run_block_start_row(8), 7);
    }

    fn cell_value<'a>(updates: &'a [(String, usize, Value)], column: &str) -> Option<&'a Value> {
        updates
            .iter()
            .find(|(update_column, _, _)| update_column == column)
            .map(|(_, _, value)| value)
    }
}

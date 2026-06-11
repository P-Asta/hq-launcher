use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap};

use crate::google_oauth::LcStatsSettings;
use crate::lcstats_autosheet::layouts::WAFRODY_LAYOUT;
use crate::lcstats_autosheet::sheets::{
    batch_read_ranges, batch_update_spreadsheet, batch_write_cells_user_entered,
    first_empty_row_from, get_sheet_id, number_value, quote_sheet_name, read_number, read_range,
};
use crate::lcstats_autosheet::stats::{
    array_at, array_at_any, lcstats, object_at, parse_lcstats_time_to_minutes, players_at,
    strip_apostrophe, strip_moon_number, value_at, value_at_any, LcStats,
};

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
const MISSED_ITEMS_NOTE_COLUMN: &str = "AA";
const VALUE_SOLD_COLUMN: &str = "AJ";
const GIFT_BONUS_COLUMN: &str = "AK";
const LOST_SCRAP_TOTAL_COLUMN: &str = "AS";
const LOST_SCRAP_TOTAL_ROW: usize = 31;
const HAZARD_TOTAL_COLUMN: &str = "AS";
const TURRET_TOTAL_ROW: usize = 16;
const LANDMINE_TOTAL_ROW: usize = 17;
const SPIKETRAP_TOTAL_ROW: usize = 18;
const NEW_QUOTA_COLUMN: &str = "C";
const SEED_COLUMN: &str = "BF";
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

    let sheet_id = get_sheet_id(client, token, spreadsheet_id, source_sheet)
        .await
        .ok();
    let target_row = first_empty_row_from(
        client,
        token,
        spreadsheet_id,
        source_sheet,
        CHECK_COLUMN,
        START_ROW,
    )
    .await?;

    let payload = lcstats(stats);
    if payload.is_gordion_moon() {
        handle_gordion(
            client,
            token,
            spreadsheet_id,
            source_sheet,
            target_row,
            stats,
            &payload,
        )
        .await?;
        return Ok(());
    }

    let normalized = NormalizedStats::from_stats(stats, &payload);
    let player_columns =
        setup_or_match_player_columns(client, token, spreadsheet_id, source_sheet, stats).await?;
    batch_write_cells_user_entered(
        client,
        token,
        spreadsheet_id,
        source_sheet,
        build_value_updates(&normalized, &player_columns, target_row),
    )
    .await?;
    write_rich_cells(
        client,
        token,
        spreadsheet_id,
        source_sheet,
        sheet_id,
        target_row,
        stats,
        &payload,
        &normalized,
        &player_columns,
    )
    .await?;
    add_lost_scraps_to_total(client, token, spreadsheet_id, source_sheet, stats).await?;
    update_hazard_totals(
        client,
        token,
        spreadsheet_id,
        source_sheet,
        target_row,
        stats,
        &payload,
    )
    .await?;
    add_old_giftbox_extra_to_previous_day(
        client,
        token,
        spreadsheet_id,
        source_sheet,
        target_row,
        stats,
    )
    .await
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

    let players = players_at(stats);
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
        for (steam_id, _) in players {
            if let Some(column) = existing_slots.get(&steam_id) {
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
    meteor_shower_time: Option<String>,
    has_available_shotguns: bool,
    shotguns_collected: i64,
    nutcracker_count: usize,
    has_available_knives: bool,
    knives_collected: i64,
    butler_count: usize,
    collected_total: i64,
    bottom_line: i64,
    value_sold: i64,
    gift_bonus: i64,
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
    fn from_stats(stats: &Value, payload: &LcStats) -> Self {
        let item_count = payload.dungeon_item_count();
        let beehives = beehive_price_summary(stats);
        let egg_values = intish_array_any(
            stats,
            &[
                &["EggInfo", "Available"][..],
                &["BirdInfo", "EggValues"][..],
            ],
        );
        let sid_type = non_false_text(payload.sid_type());
        let meteor_shower_time = non_false_text(payload.meteor_shower_time());
        let shotgun_available = intish_array_any(stats, &[&["ShotgunInfo", "Available"][..]]);
        let knife_available = intish_array_any(stats, &[&["KnifeInfo", "Available"][..]]);
        let bottom_line = payload.initial_available_value() + shotgun_available.iter().sum::<i64>();
        Self {
            moon_name: strip_apostrophe(&payload.moon_name()),
            weather: wafrody_weather(&payload.moon_weather()),
            interior: strip_apostrophe(&payload.dungeon_interior()),
            item_count,
            beehive_value_count: beehives.count_by_value,
            cheap_beehive_value: beehives.cheap_value,
            expensive_beehive_value: beehives.expensive_value,
            egg_value: (!egg_values.is_empty()).then(|| egg_values.iter().sum()),
            egg_values,
            meteor_shower_time,
            has_available_shotguns: !shotgun_available.is_empty(),
            shotguns_collected: collected_count_or_legacy_int(
                stats,
                &["ShotgunInfo", "Collected"],
                &["ShotgunsCollected"],
            ),
            nutcracker_count: indoor_enemy_count(stats, "Nutcracker"),
            has_available_knives: !knife_available.is_empty(),
            knives_collected: collected_count_or_legacy_int(
                stats,
                &["KnifeInfo", "Collected"],
                &["KnivesCollected"],
            ),
            butler_count: indoor_enemy_count(stats, "Butler"),
            collected_total: payload.collected_total(),
            bottom_line,
            value_sold: payload.value_sold(),
            gift_bonus: gift_bonus_total(stats),
            new_quota: payload.new_quota(),
            seed: strip_apostrophe(&payload.seed_text()),
            has_sid: sid_type.is_some(),
            sid_type,
            apparatus_spawned: payload.app_spawned(),
            indoor_fog: payload.indoor_fog(),
            infestation_type: strip_apostrophe(payload.infestation_type()),
            players: normalize_players(stats, payload),
        }
    }
}

fn normalize_players(stats: &Value, payload: &LcStats) -> HashMap<String, NormalizedPlayer> {
    let takeoff_time = payload.take_off_time().to_string();
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
            stats
                .meteor_shower_time
                .as_ref()
                .map_or_else(|| json!(""), |value| json!(value)),
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
        (SEED_COLUMN.to_string(), row, json!(stats.seed)),
    ];

    if stats.has_available_shotguns {
        values.push((
            SHOTGUNS_COLLECTED_COLUMN.to_string(),
            row,
            json!(stats.shotguns_collected),
        ));
    }
    if stats.nutcracker_count != 0 {
        values.push((
            NUTCRACKER_COUNT_COLUMN.to_string(),
            row,
            json!(stats.nutcracker_count),
        ));
    }
    if stats.has_available_knives {
        values.push((
            KNIVES_COLLECTED_COLUMN.to_string(),
            row,
            json!(stats.knives_collected),
        ));
    }
    if stats.butler_count != 0 {
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
    if stats.gift_bonus != 0 {
        values.push((GIFT_BONUS_COLUMN.to_string(), row, json!(stats.gift_bonus)));
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
    payload: &LcStats,
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
    for (steam_id, player) in object_at(raw_stats, &["Players"]) {
        let Some(column) = player_columns.get(&steam_id) else {
            continue;
        };
        let Some(note) = player_death_note(&player, raw_stats) else {
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

    if is_version_45_or_49(payload) {
        if let Some(note) = collected_values_note(raw_stats, &["ShotgunInfo", "Collected"]) {
            requests.push(note_request(
                sheet_id,
                SHOTGUNS_COLLECTED_COLUMN,
                row,
                &note,
            ));
        }
        if let Some(note) = nut_spawn_available_note(raw_stats) {
            requests.push(note_request(sheet_id, NUTCRACKER_COUNT_COLUMN, row, &note));
        }
    } else if let Some(note) = enemy_spawn_times_note(raw_stats, "Nutcracker") {
        requests.push(note_request(sheet_id, NUTCRACKER_COUNT_COLUMN, row, &note));
    }
    if let Some(note) = enemy_spawn_times_note(raw_stats, "Butler") {
        requests.push(note_request(sheet_id, BUTLER_COUNT_COLUMN, row, &note));
    }
    if let Some(note) = missed_items_note(raw_stats, payload) {
        requests.push(note_request(sheet_id, MISSED_ITEMS_NOTE_COLUMN, row, &note));
    }
    if let Some(note) = hazard_note(raw_stats, payload) {
        requests.push(note_request(sheet_id, INTERIOR_COLUMN, row, &note));
    }
    if let Some(note) = gift_bonus_note(raw_stats) {
        requests.push(note_request(sheet_id, GIFT_BONUS_COLUMN, row, &note));
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
    payload: &LcStats,
) -> Result<(), String> {
    let value_sold = payload.value_sold();
    let new_quota = payload.new_quota();
    let gift_bonus = gift_bonus_total(stats);
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
    if gift_bonus != 0 && target_row > 1 {
        let gift_row = target_row - 1;
        let current_value = read_number(
            client,
            token,
            spreadsheet_id,
            sheet_name,
            &format!("{GIFT_BONUS_COLUMN}{gift_row}"),
        )
        .await?;
        updates.push((
            GIFT_BONUS_COLUMN.to_string(),
            gift_row,
            number_value(current_value + gift_bonus as f64),
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

async fn update_hazard_totals(
    client: &reqwest::Client,
    token: &str,
    spreadsheet_id: &str,
    sheet_name: &str,
    target_row: usize,
    stats: &Value,
    payload: &LcStats,
) -> Result<(), String> {
    if target_row <= 6
        || value_at(stats, &["HazardInfo"])
            .map(Value::is_null)
            .unwrap_or(true)
    {
        return Ok(());
    }

    let turret_count = payload.turret_count();
    let landmine_count = payload.landmine_count();
    let spiketrap_count = payload.spiketrap_count();

    let current_turret_total = read_total_from_cell(
        client,
        token,
        spreadsheet_id,
        sheet_name,
        &format!("{HAZARD_TOTAL_COLUMN}{TURRET_TOTAL_ROW}"),
    )
    .await?;
    let current_landmine_total = read_total_from_cell(
        client,
        token,
        spreadsheet_id,
        sheet_name,
        &format!("{HAZARD_TOTAL_COLUMN}{LANDMINE_TOTAL_ROW}"),
    )
    .await?;
    let current_spiketrap_total = read_total_from_cell(
        client,
        token,
        spreadsheet_id,
        sheet_name,
        &format!("{HAZARD_TOTAL_COLUMN}{SPIKETRAP_TOTAL_ROW}"),
    )
    .await?;

    let counted_days = (target_row - 6) as f64;
    let new_turret_total = current_turret_total + turret_count;
    let new_landmine_total = current_landmine_total + landmine_count;
    let new_spiketrap_total = current_spiketrap_total + spiketrap_count;

    batch_write_cells_user_entered(
        client,
        token,
        spreadsheet_id,
        sheet_name,
        vec![
            (
                HAZARD_TOTAL_COLUMN.to_string(),
                TURRET_TOTAL_ROW,
                json!(format!(
                    "{}/{:.2}",
                    new_turret_total,
                    new_turret_total as f64 / counted_days
                )),
            ),
            (
                HAZARD_TOTAL_COLUMN.to_string(),
                LANDMINE_TOTAL_ROW,
                json!(format!(
                    "{}/{:.2}",
                    new_landmine_total,
                    new_landmine_total as f64 / counted_days
                )),
            ),
            (
                HAZARD_TOTAL_COLUMN.to_string(),
                SPIKETRAP_TOTAL_ROW,
                json!(format!(
                    "{}/{:.2}",
                    new_spiketrap_total,
                    new_spiketrap_total as f64 / counted_days
                )),
            ),
        ],
    )
    .await
}

async fn read_total_from_cell(
    client: &reqwest::Client,
    token: &str,
    spreadsheet_id: &str,
    sheet_name: &str,
    cell: &str,
) -> Result<i64, String> {
    let range = format!("{}!{cell}", quote_sheet_name(sheet_name));
    let data = read_range(client, token, spreadsheet_id, &range).await?;
    Ok(data
        .get("values")
        .and_then(Value::as_array)
        .and_then(|rows| rows.first())
        .and_then(Value::as_array)
        .and_then(|cells| cells.first())
        .map(parse_total_average_cell)
        .unwrap_or(0))
}

fn parse_total_average_cell(value: &Value) -> i64 {
    let text = value
        .as_str()
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| value.to_string());
    let total = text.split('/').next().unwrap_or_default().replace(',', ".");
    total
        .trim()
        .parse::<f64>()
        .map(|value| value as i64)
        .unwrap_or(0)
}

async fn add_old_giftbox_extra_to_previous_day(
    client: &reqwest::Client,
    token: &str,
    spreadsheet_id: &str,
    sheet_name: &str,
    target_row: usize,
    stats: &Value,
) -> Result<(), String> {
    let extra = intish_at(stats, &["ExtraFromOldGiftbox"]);
    if extra == 0 || target_row <= START_ROW {
        return Ok(());
    }
    let previous_day_row = target_row - 1;
    let current_value = read_number(
        client,
        token,
        spreadsheet_id,
        sheet_name,
        &format!("{GIFT_BONUS_COLUMN}{previous_day_row}"),
    )
    .await?;
    batch_write_cells_user_entered(
        client,
        token,
        spreadsheet_id,
        sheet_name,
        vec![(
            GIFT_BONUS_COLUMN.to_string(),
            previous_day_row,
            number_value(current_value + extra as f64),
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
        "repeatCell": {
            "range": {
                "sheetId": sheet_id,
                "startRowIndex": row.saturating_sub(1),
                "endRowIndex": row,
                "startColumnIndex": column_index,
                "endColumnIndex": column_index + 1
            },
            "cell": {
                "note": note
            },
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
    parse_lcstats_time_to_minutes(value)
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

fn player_death_note(player: &Value, stats: &Value) -> Option<String> {
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
    let mut death_time = strip_apostrophe(
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
    if cause == "Abandoned" {
        death_time = "11:57 PM".to_string();
    }

    let mut lines = vec![format!("Cause: {cause}"), format!("Time: {death_time}")];
    let indoor_before_death = spawns_before_death(stats, &["IndoorSpawns"], &death_time);
    if !indoor_before_death.is_empty() {
        lines.push(String::new());
        lines.push("Inside spawns before death:".to_string());
        lines.extend(indoor_before_death);
    }
    let night_before_death = spawns_before_death(stats, &["NightTimeSpawns"], &death_time);
    if !night_before_death.is_empty() {
        lines.push(String::new());
        lines.push("Night outside spawns before death:".to_string());
        lines.extend(night_before_death);
    }
    Some(lines.join("\n"))
}

fn spawns_before_death(stats: &Value, path: &[&str], death_time: &str) -> Vec<String> {
    let Some(death_minutes) = parse_time_to_minutes(death_time) else {
        return vec![];
    };
    array_at(stats, path)
        .iter()
        .filter(|spawn| {
            spawn
                .get("SpawnTime")
                .and_then(Value::as_str)
                .and_then(parse_time_to_minutes)
                .map(|spawn_minutes| spawn_minutes <= death_minutes)
                .unwrap_or(false)
        })
        .map(format_spawn_note)
        .collect()
}

fn format_spawn_note(spawn: &Value) -> String {
    let enemy = spawn
        .get("Enemy")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let spawn_time = spawn
        .get("SpawnTime")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let mut note = format!("{enemy} - {spawn_time}");
    if let Some(death_time) = spawn
        .get("TimeOfDeath")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
    {
        note.push_str(&format!(" / died {death_time}"));
    }
    note
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

fn is_version_45_or_49(payload: &LcStats) -> bool {
    matches!(stats_version_number(payload), Some(45 | 49))
}

fn stats_version_number(payload: &LcStats) -> Option<i64> {
    let version = payload.version_text();
    let version = version
        .trim()
        .trim_start_matches('V')
        .trim_start_matches('v');
    version.parse::<i64>().ok()
}

fn collected_values_note(stats: &Value, path: &[&str]) -> Option<String> {
    let values = intish_array_any(stats, &[path]);
    (!values.is_empty()).then(|| {
        format!(
            "Collected:\n{}",
            values
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        )
    })
}

fn nut_spawn_available_note(stats: &Value) -> Option<String> {
    let available_shotguns = intish_array_any(stats, &[&["ShotgunInfo", "Available"][..]]);
    let lines = array_at(stats, &["IndoorSpawns"])
        .iter()
        .filter(|spawn| spawn.get("Enemy").and_then(Value::as_str) == Some("Nutcracker"))
        .enumerate()
        .map(|(index, spawn)| {
            let spawn_time = spawn
                .get("SpawnTime")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let mut line = spawn_time.to_string();
            if let Some(value) = available_shotguns.get(index) {
                line.push_str(&format!(" : {value}"));
            }
            if let Some(death_time) = spawn
                .get("TimeOfDeath")
                .and_then(Value::as_str)
                .filter(|value| !value.trim().is_empty())
            {
                line.push_str(&format!(" / died {death_time}"));
            }
            line
        })
        .filter(|line| !line.trim().is_empty())
        .collect::<Vec<_>>();
    (!lines.is_empty()).then(|| lines.join("\n"))
}

fn missed_items_note(stats: &Value, payload: &LcStats) -> Option<String> {
    let mut inside_by_type: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut outside_by_type: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for item in array_at(stats, &["MissedItems"]) {
        let item_type = item
            .get("ItemType")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .trim();
        if item_type.is_empty() {
            continue;
        }
        let y_position = item
            .get("DespawnPosition")
            .and_then(Value::as_array)
            .and_then(|position| position.get(1))
            .map(value_as_f64)
            .unwrap_or(0.0);
        let mut value_text = if item_type == "Gift box" {
            format!(
                "{}=>{}",
                item.get("Value")
                    .map(value_as_i64)
                    .map(|value| value.to_string())
                    .unwrap_or_default(),
                intish_item_at(item, "ScrapInsideGiftValue")
            )
        } else {
            item.get("Value")
                .map(value_as_i64)
                .map(|value| value.to_string())
                .unwrap_or_default()
        };
        if item.get("CollectedOnPreviousDay").and_then(Value::as_bool) == Some(true) {
            value_text.push_str("(lost)");
        }
        if missed_item_is_voided(payload, y_position) {
            value_text.push_str("(voided)");
        }
        let target = if y_position <= -100.0 {
            &mut inside_by_type
        } else {
            &mut outside_by_type
        };
        target
            .entry(item_type.to_string())
            .or_default()
            .push(value_text);
    }
    if inside_by_type.is_empty() && outside_by_type.is_empty() {
        return None;
    }
    let mut sections = vec![];
    if !inside_by_type.is_empty() {
        sections.push(missed_item_section("Inside :", inside_by_type));
    }
    if !outside_by_type.is_empty() {
        sections.push(missed_item_section("Outside :", outside_by_type));
    }
    Some(sections.join("\n\n"))
}

fn missed_item_is_voided(payload: &LcStats, y_position: f64) -> bool {
    let interior = strip_apostrophe(&payload.dungeon_interior())
        .trim()
        .to_ascii_lowercase();
    match interior.as_str() {
        "facility" => y_position_matches(y_position, &[-241.7, -235.4]),
        "mansion" if matches!(stats_version_number(payload), Some(40 | 45 | 49)) => {
            y_position_matches(
                y_position,
                &[-239.1, -239.0, -238.9, -229.1, -229.0, -228.9],
            )
        }
        _ => false,
    }
}

fn y_position_matches(y_position: f64, targets: &[f64]) -> bool {
    targets
        .iter()
        .any(|target| (y_position - target).abs() < 0.0001)
}

fn missed_item_section(label: &str, items: BTreeMap<String, Vec<String>>) -> String {
    let mut lines = vec![label.to_string()];
    lines.extend(
        items
            .into_iter()
            .map(|(item_type, values)| format!("{item_type} : {}", values.join(","))),
    );
    lines.join("\n")
}

fn hazard_note(stats: &Value, payload: &LcStats) -> Option<String> {
    let hazard_info = value_at(stats, &["HazardInfo"])?;
    if hazard_info.is_null() {
        return None;
    }
    Some(format!(
        "Turrets: {}\nLandmines: {}\nSpiketraps: {}",
        payload.turret_count(),
        payload.landmine_count(),
        payload.spiketrap_count()
    ))
}

fn lost_scrap_total(stats: &Value) -> i64 {
    array_at(stats, &["MissedItems"])
        .iter()
        .filter(|item| item.get("CollectedOnPreviousDay").and_then(Value::as_bool) == Some(true))
        .map(|item| item.get("Value").map(value_as_i64).unwrap_or(0))
        .sum()
}

fn gift_bonus_total(stats: &Value) -> i64 {
    let opened_bonus: i64 = array_at_any(stats, &[&["GiftBoxesOpened"][..], &["GiftBoxes"][..]])
        .iter()
        .map(|gift| gift_new_scrap_value(gift) - gift_original_value(gift))
        .sum();
    let missed_bonus: i64 = array_at(stats, &["MissedItems"])
        .iter()
        .filter(|item| item.get("ItemType").and_then(Value::as_str) == Some("Gift box"))
        .map(|item| {
            intish_item_at(item, "ScrapInsideGiftValue")
                - item.get("Value").map(value_as_i64).unwrap_or(0)
        })
        .sum();
    opened_bonus + missed_bonus + intish_at(stats, &["ExtraFromOldGift"])
}

fn gift_bonus_note(stats: &Value) -> Option<String> {
    let mut lines = vec![];
    lines.extend(
        array_at_any(stats, &[&["GiftBoxesOpened"][..], &["GiftBoxes"][..]])
            .iter()
            .map(|gift| {
                format!(
                    "{}=>{}",
                    gift_note_original_value(gift),
                    gift_new_scrap_value(gift)
                )
            }),
    );
    lines.extend(array_at(stats, &["MissedItems"]).iter().filter_map(|item| {
        (item.get("ItemType").and_then(Value::as_str) == Some("Gift box")).then(|| {
            format!(
                "{}=>{}",
                item.get("Value")
                    .map(value_as_i64)
                    .map(|value| value.to_string())
                    .unwrap_or_default(),
                intish_item_at(item, "ScrapInsideGiftValue")
            )
        })
    }));
    (!lines.is_empty()).then(|| lines.join("\n"))
}

fn gift_new_scrap_value(gift: &Value) -> i64 {
    value_at_any(gift, &[&["NewScrapValue"][..], &["GiftValue"][..]])
        .map(value_as_i64)
        .unwrap_or(0)
}

fn gift_note_original_value(gift: &Value) -> i64 {
    value_at_any(
        gift,
        &[&["GiftScrapValue"][..], &["Value"][..], &["ScrapValue"][..]],
    )
    .map(value_as_i64)
    .unwrap_or(0)
}

fn gift_original_value(gift: &Value) -> i64 {
    value_at_any(
        gift,
        &[&["Value"][..], &["GiftScrapValue"][..], &["ScrapValue"][..]],
    )
    .map(value_as_i64)
    .unwrap_or(0)
}

fn intish_item_at(item: &Value, key: &str) -> i64 {
    item.get(key).map(value_as_i64).unwrap_or(0)
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

fn value_as_f64(value: &Value) -> f64 {
    value
        .as_f64()
        .or_else(|| {
            value.as_str().and_then(|text| {
                strip_apostrophe(text)
                    .trim()
                    .replace(',', ".")
                    .parse::<f64>()
                    .ok()
            })
        })
        .unwrap_or(0.0)
}

fn value_as_i64_option(value: &Value) -> Option<i64> {
    value
        .as_i64()
        .or_else(|| value.as_f64().map(|value| value as i64))
        .or_else(|| {
            value.as_str().and_then(|text| {
                strip_apostrophe(text)
                    .trim()
                    .replace(',', ".")
                    .parse::<f64>()
                    .ok()
                    .map(|value| value as i64)
            })
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

        let normalized = normalized_stats(&stats);
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

        let normalized = normalized_stats(&stats);
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

        let normalized = normalized_stats(&stats);
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

        let normalized = normalized_stats(&stats);
        let summary = beehive_price_summary(&stats);

        assert_eq!(summary.count_by_value, "2|1");
        assert_eq!(summary.cheap_value, Some(64));
        assert_eq!(summary.expensive_value, Some(88));
        assert_eq!(normalized.egg_value, Some(30));
        assert_eq!(normalized.shotguns_collected, 1);
        assert_eq!(normalized.knives_collected, 2);
    }

    #[test]
    fn gift_boxes_write_bonus_to_ak_from_new_scrap_minus_value() {
        let stats = json!({
            "GiftBoxesOpened": [
                { "NewScrapValue": 162, "Value": 115, "Collected": true },
                { "NewScrapValue": 39, "Value": 14, "Collected": false }
            ]
        });

        let normalized = normalized_stats(&stats);
        let updates = build_value_updates(&normalized, &HashMap::new(), 7);

        assert_eq!(normalized.gift_bonus, 72);
        assert_eq!(cell_value(&updates, GIFT_BONUS_COLUMN), Some(&json!(72)));
    }

    #[test]
    fn gift_boxes_are_not_added_to_lost_scrap_total() {
        let stats = json!({
            "MissedItems": [
                { "Value": 30, "CollectedOnPreviousDay": true }
            ],
            "GiftBoxesOpened": [
                { "NewScrapValue": 162, "Value": 26 }
            ]
        });

        assert_eq!(lost_scrap_total(&stats), 30);
    }

    #[test]
    fn lost_scrap_total_cell_matches_reference_script() {
        assert_eq!(LOST_SCRAP_TOTAL_COLUMN, "AS");
        assert_eq!(LOST_SCRAP_TOTAL_ROW, 31);
    }

    #[test]
    fn bottom_line_includes_available_shotgun_values() {
        let stats = json!({
            "InitialAvailableValue": 500,
            "ShotgunInfo": { "Available": [60, 70], "Collected": [] }
        });

        let normalized = normalized_stats(&stats);
        let updates = build_value_updates(&normalized, &HashMap::new(), 7);

        assert_eq!(cell_value(&updates, BOTTOM_LINE_COLUMN), Some(&json!(630)));
        assert_eq!(
            cell_value(&updates, SHOTGUNS_COLLECTED_COLUMN),
            Some(&json!(0))
        );
    }

    #[test]
    fn missed_items_note_splits_inside_and_outside() {
        let stats = json!({
            "MissedItems": [
                { "ItemType": "Cash register", "Value": 80, "DespawnPosition": [0, -120, 0] },
                { "ItemType": "Gift box", "Value": 20, "ScrapInsideGiftValue": 45, "DespawnPosition": [0, 2, 0], "CollectedOnPreviousDay": true }
            ]
        });

        let payload = lcstats(&stats);
        let note = missed_items_note(&stats, &payload).unwrap();

        assert!(note.contains("Inside :\nCash register : 80"));
        assert!(note.contains("Outside :\nGift box : 20=>45(lost)"));
    }

    #[test]
    fn missed_items_note_marks_voided_scrap() {
        let stats = json!({
            "Version": "45",
            "DungeonInfo": { "Interior": "Mansion" },
            "MissedItems": [
                { "ItemType": "Gold bar", "Value": 210, "DespawnPosition": [0, -229.0, 0] }
            ]
        });

        let payload = lcstats(&stats);
        let note = missed_items_note(&stats, &payload).unwrap();

        assert!(note.contains("Gold bar : 210(voided)"));
    }

    #[test]
    fn gift_bonus_note_lists_opened_and_missed_gift_boxes() {
        let stats = json!({
            "GiftBoxesOpened": [
                { "GiftScrapValue": 20, "NewScrapValue": 111 }
            ],
            "MissedItems": [
                { "ItemType": "Gift box", "Value": 15, "ScrapInsideGiftValue": 60 }
            ]
        });

        assert_eq!(gift_bonus_note(&stats).as_deref(), Some("20=>111\n15=>60"));
    }

    #[test]
    fn player_death_note_includes_spawns_before_death() {
        let stats = json!({
            "Players": {},
            "IndoorSpawns": [
                { "Enemy": "Bracken", "SpawnTime": "9:00 PM" },
                { "Enemy": "Coil-head", "SpawnTime": "11:00 PM" }
            ],
            "NightTimeSpawns": [
                { "Enemy": "Earth Leviathan", "SpawnTime": "9:30 PM", "TimeOfDeath": "10:00 PM" }
            ]
        });
        let player = json!({
            "Alive": false,
            "Disconnected": false,
            "CauseOfDeath": "Forest Giant",
            "TimeOfDeath": "10:00 PM"
        });

        let note = player_death_note(&player, &stats).unwrap();

        assert!(note.contains("Inside spawns before death:\nBracken - 9:00 PM"));
        assert!(!note.contains("Coil-head - 11:00 PM"));
        assert!(note.contains(
            "Night outside spawns before death:\nEarth Leviathan - 9:30 PM / died 10:00 PM"
        ));
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

        let payload = lcstats(&stats);
        let players = normalize_players(&stats, &payload);

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

    #[test]
    fn galetry_stats_use_gordion_economy_path() {
        let stats = json!({
            "MoonInfo": { "Name": "'Galetry" }
        });

        assert!(lcstats(&stats).is_gordion_moon());
    }

    fn cell_value<'a>(updates: &'a [(String, usize, Value)], column: &str) -> Option<&'a Value> {
        updates
            .iter()
            .find(|(update_column, _, _)| update_column == column)
            .map(|(_, _, value)| value)
    }

    fn normalized_stats(stats: &Value) -> NormalizedStats {
        let payload = lcstats(stats);
        NormalizedStats::from_stats(stats, &payload)
    }
}

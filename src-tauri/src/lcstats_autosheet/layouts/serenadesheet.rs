use serde_json::{json, Value};

use crate::google_oauth::LcStatsSettings;
use crate::lcstats_autosheet::layouts::SERENADE_LAYOUT;
use crate::lcstats_autosheet::sheets::{
    batch_update_spreadsheet, batch_write_cells_user_entered, first_empty_row_from, get_sheet_id,
    number_value, read_number,
};
use crate::lcstats_autosheet::stats::{lcstats, strip_apostrophe, strip_moon_number, LcStats};

const START_ROW: usize = 4;
const CHECK_COLUMN: &str = "AI";
const PLAYER_STATE_COLUMNS: [&str; 4] = ["BA", "BB", "BC", "BD"];
const PLAYER_NAME_COLUMN: &str = "BJ";
const PLAYER_NAME_START_ROW: usize = 42;

const QUOTA_COLUMN: &str = "C";
const MOON_COLUMN: &str = "N";
const WEATHER_COLUMN: &str = "O";
const LAYOUT_COLUMN: &str = "P";
const ITEM_COUNT_COLUMN: &str = "Q";
const APPARATUS_COLUMN: &str = "R";
const MISSING_COLUMN: &str = "S";
const SID_COLUMN: &str = "T";
const HIVE_COLLECTED_COUNT_COLUMN: &str = "U";
const HIVE_SPAWNED_COUNT_COLUMN: &str = "V";
const HIVE_COLLECTED_VALUE_COLUMN: &str = "W";
const HIVE_SPAWNED_VALUE_COLUMN: &str = "X";
const EGG_COLLECTED_COUNT_COLUMN: &str = "Y";
const EGG_SPAWNED_COUNT_COLUMN: &str = "Z";
const EGG_COLLECTED_VALUE_COLUMN: &str = "AA";
const EGG_SPAWNED_VALUE_COLUMN: &str = "AB";
const SHOTGUN_COLLECTED_COUNT_COLUMN: &str = "AC";
const SHOTGUN_SPAWNED_COUNT_COLUMN: &str = "AD";
const SHOTGUN_COLLECTED_VALUE_COLUMN: &str = "AE";
const SHOTGUN_SPAWNED_VALUE_COLUMN: &str = "AF";
const KNIFE_COLLECTED_VALUE_COLUMN: &str = "AG";
const BUTLER_COUNT_COLUMN: &str = "AH";
const TOPLINE_COLUMN: &str = "AI";
const BOTTOMLINE_COLUMN: &str = "AJ";
const GIFT_NET_COLUMN: &str = "AT";
const GIFT_OPENED_VALUE_COLUMN: &str = "AU";
const SOLD_COLUMN: &str = "AW";

pub async fn write(
    client: &reqwest::Client,
    token: &str,
    settings: &LcStatsSettings,
    stats: &Value,
) -> Result<(), String> {
    if !settings.layout.eq_ignore_ascii_case(SERENADE_LAYOUT) {
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
    let lc_stats = lcstats(stats);
    if lc_stats.is_gordion_moon() {
        return handle_gordion(client, token, spreadsheet_id, sheet_name, row, &lc_stats).await;
    }
    let normalized = NormalizedStats::from_stats(&lc_stats);
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

async fn handle_gordion(
    client: &reqwest::Client,
    token: &str,
    spreadsheet_id: &str,
    sheet_name: &str,
    target_row: usize,
    lc_stats: &LcStats,
) -> Result<(), String> {
    let value_sold = lc_stats.value_sold();
    let new_quota = lc_stats.new_quota();
    let sold_row = gordion_sold_row(target_row);
    let current_sold = if value_sold == 0 {
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
        build_gordion_updates(value_sold, new_quota, target_row, sold_row, current_sold),
    )
    .await
}

fn build_gordion_updates(
    value_sold: i64,
    new_quota: i64,
    quota_row: usize,
    sold_row: usize,
    current_sold: f64,
) -> Vec<(String, usize, Value)> {
    let mut updates = vec![];
    if value_sold != 0 {
        updates.push((
            SOLD_COLUMN.to_string(),
            sold_row,
            number_value(current_sold + value_sold as f64),
        ));
    }
    if new_quota != 0 {
        updates.push((QUOTA_COLUMN.to_string(), quota_row, json!(new_quota)));
    }
    updates
}

fn gordion_sold_row(target_row: usize) -> usize {
    target_row.saturating_sub(1).max(START_ROW)
}

#[derive(Debug, Clone)]
struct NormalizedPlayer {
    name: String,
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
    apparatus: String,
    missing: NoteCell,
    sid: NoteCell,
    hive_collected_count: usize,
    hive_spawned_count: usize,
    hive_collected_value: i64,
    hive_spawned_value: i64,
    egg_collected_count: usize,
    egg_spawned_count: usize,
    egg_collected_value: i64,
    egg_spawned_value: i64,
    shotgun_collected_count: usize,
    shotgun_spawned_count: usize,
    shotgun_collected_value: Option<i64>,
    shotgun_spawned_value: Option<i64>,
    knife_collected_value: i64,
    butler_count: usize,
    topline: i64,
    bottomline: i64,
    gift_net: NoteCell,
    gift_opened_value: NoteCell,
    value_sold: i64,
    players: Vec<NormalizedPlayer>,
}

impl NormalizedStats {
    fn from_stats(payload: &LcStats) -> Self {
        let hive_collected = payload.bee_collected_values();
        let hive_spawned = payload.bee_available_values();
        let egg_collected = payload.egg_collected_values();
        let egg_spawned = payload.egg_available_values();
        let shotgun_collected = payload.shotgun_collected_values();
        let shotgun_spawned = payload.shotgun_available_values();
        let gifts = gifts_cells(payload);
        Self {
            new_quota: payload.new_quota(),
            moon_name: serenade_moon(&payload.moon_name()),
            weather: serenade_weather(&payload.moon_weather()),
            interior: normalize_interior_name(&strip_apostrophe(&payload.dungeon_interior())),
            item_count: payload.dungeon_item_count(),
            apparatus: apparatus_state(payload),
            missing: missing_items_cell(payload),
            sid: sid_cell(payload),
            hive_collected_count: hive_collected.len(),
            hive_spawned_count: hive_spawned.len(),
            hive_collected_value: hive_collected.iter().sum(),
            hive_spawned_value: hive_spawned.iter().sum(),
            egg_collected_count: egg_collected.len(),
            egg_spawned_count: egg_spawned.len(),
            egg_collected_value: egg_collected.iter().sum(),
            egg_spawned_value: egg_spawned.iter().sum(),
            shotgun_collected_count: shotgun_collected.len(),
            shotgun_spawned_count: payload.indoor_enemy_count("Nutcracker"),
            shotgun_collected_value: legacy_shotgun_value_enabled(payload)
                .then(|| shotgun_collected.iter().sum()),
            shotgun_spawned_value: legacy_shotgun_value_enabled(payload)
                .then(|| shotgun_spawned.iter().sum()),
            knife_collected_value: payload.knife_collected_values().iter().sum(),
            butler_count: payload.indoor_enemy_count("Butler"),
            topline: payload.collected_total(),
            bottomline: payload.initial_available_value(),
            gift_net: gifts.0,
            gift_opened_value: gifts.1,
            value_sold: payload.value_sold(),
            players: normalize_players(payload),
        }
    }
}

fn build_value_updates(stats: &NormalizedStats, row: usize) -> Vec<(String, usize, Value)> {
    let mut updates = vec![];
    if stats.new_quota != 0 {
        push_standard(&mut updates, QUOTA_COLUMN, row, json!(stats.new_quota));
    }
    push_standard(&mut updates, MOON_COLUMN, row, json!(stats.moon_name));
    push_standard(&mut updates, WEATHER_COLUMN, row, json!(stats.weather));
    push_standard(&mut updates, LAYOUT_COLUMN, row, json!(stats.interior));
    push_standard(
        &mut updates,
        ITEM_COUNT_COLUMN,
        row,
        json!(stats.item_count),
    );
    push_standard(&mut updates, APPARATUS_COLUMN, row, json!(stats.apparatus));
    push_standard(
        &mut updates,
        HIVE_COLLECTED_COUNT_COLUMN,
        row,
        json!(stats.hive_collected_count),
    );
    push_standard(
        &mut updates,
        HIVE_SPAWNED_COUNT_COLUMN,
        row,
        json!(stats.hive_spawned_count),
    );
    push_standard(
        &mut updates,
        HIVE_COLLECTED_VALUE_COLUMN,
        row,
        json!(stats.hive_collected_value),
    );
    push_standard(
        &mut updates,
        HIVE_SPAWNED_VALUE_COLUMN,
        row,
        json!(stats.hive_spawned_value),
    );
    push_standard(
        &mut updates,
        EGG_COLLECTED_COUNT_COLUMN,
        row,
        json!(stats.egg_collected_count),
    );
    push_standard(
        &mut updates,
        EGG_SPAWNED_COUNT_COLUMN,
        row,
        json!(stats.egg_spawned_count),
    );
    push_standard(
        &mut updates,
        EGG_COLLECTED_VALUE_COLUMN,
        row,
        json!(stats.egg_collected_value),
    );
    push_standard(
        &mut updates,
        EGG_SPAWNED_VALUE_COLUMN,
        row,
        json!(stats.egg_spawned_value),
    );
    push_standard(
        &mut updates,
        SHOTGUN_COLLECTED_COUNT_COLUMN,
        row,
        json!(stats.shotgun_collected_count),
    );
    push_standard(
        &mut updates,
        SHOTGUN_SPAWNED_COUNT_COLUMN,
        row,
        json!(stats.shotgun_spawned_count),
    );
    if let Some(value) = stats.shotgun_collected_value {
        push_standard(
            &mut updates,
            SHOTGUN_COLLECTED_VALUE_COLUMN,
            row,
            json!(value),
        );
    }
    if let Some(value) = stats.shotgun_spawned_value {
        push_standard(
            &mut updates,
            SHOTGUN_SPAWNED_VALUE_COLUMN,
            row,
            json!(value),
        );
    }
    push_standard(
        &mut updates,
        KNIFE_COLLECTED_VALUE_COLUMN,
        row,
        json!(stats.knife_collected_value),
    );
    push_standard(
        &mut updates,
        BUTLER_COUNT_COLUMN,
        row,
        json!(stats.butler_count),
    );
    updates.push((TOPLINE_COLUMN.to_string(), row, json!(stats.topline)));
    updates.push((BOTTOMLINE_COLUMN.to_string(), row, json!(stats.bottomline)));
    updates.push((
        GIFT_NET_COLUMN.to_string(),
        row,
        stats.gift_net.value.clone(),
    ));
    updates.push((
        GIFT_OPENED_VALUE_COLUMN.to_string(),
        row,
        stats.gift_opened_value.value.clone(),
    ));
    updates.push((SOLD_COLUMN.to_string(), row, json!(stats.value_sold)));

    for (index, player) in stats
        .players
        .iter()
        .take(PLAYER_STATE_COLUMNS.len())
        .enumerate()
    {
        if player.note.is_none() {
            updates.push((
                PLAYER_STATE_COLUMNS[index].to_string(),
                row,
                json!(player.status),
            ));
        }
        if !player.name.trim().is_empty() {
            updates.push((
                PLAYER_NAME_COLUMN.to_string(),
                PLAYER_NAME_START_ROW + index,
                json!(player.name),
            ));
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
        value_with_note_request(sheet_id, &stats.gift_net, row),
        value_with_note_request(sheet_id, &stats.gift_opened_value, row),
    ];

    for (index, player) in stats
        .players
        .iter()
        .take(PLAYER_STATE_COLUMNS.len())
        .enumerate()
    {
        if let Some(note) = &player.note {
            requests.push(value_with_note_request(
                sheet_id,
                &NoteCell {
                    column: PLAYER_STATE_COLUMNS[index],
                    value: json!(player.status),
                    note: Some(note.clone()),
                },
                row,
            ));
        }
    }

    batch_update_spreadsheet(client, token, spreadsheet_id, requests).await
}

fn push_standard(
    updates: &mut Vec<(String, usize, Value)>,
    column: &str,
    row: usize,
    value: Value,
) {
    updates.push((column.to_string(), row, dash_zero(value)));
}

fn dash_zero(value: Value) -> Value {
    if value.as_i64() == Some(0)
        || value.as_u64() == Some(0)
        || value.as_f64() == Some(0.0)
        || value.as_str().map(str::trim).unwrap_or_default().is_empty()
    {
        json!("-")
    } else {
        value
    }
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
    if let Some(value) = value.as_i64() {
        json!({ "numberValue": value })
    } else if let Some(value) = value.as_u64() {
        json!({ "numberValue": value })
    } else if let Some(value) = value.as_f64() {
        json!({ "numberValue": value })
    } else if let Some(value) = value.as_bool() {
        json!({ "boolValue": value })
    } else {
        json!({ "stringValue": value.as_str().unwrap_or_default() })
    }
}

fn normalize_players(stats: &LcStats) -> Vec<NormalizedPlayer> {
    stats
        .players_sorted()
        .into_iter()
        .map(|player| {
            let name = strip_apostrophe(&player.stats.name);
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
                "A"
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

            NormalizedPlayer { name, status, note }
        })
        .collect()
}

fn missing_items_cell(stats: &LcStats) -> NoteCell {
    let missing = stats.active_missed_items().collect::<Vec<_>>();
    if missing.is_empty() {
        return NoteCell {
            column: MISSING_COLUMN,
            value: json!("-"),
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
        value: json!(missing.len()),
        note: Some(note),
    }
}

fn sid_cell(payload: &LcStats) -> NoteCell {
    let sid_type = non_false_text(payload.sid_type());
    NoteCell {
        column: SID_COLUMN,
        value: json!(sid_type.clone().unwrap_or_else(|| "-".to_string())),
        note: sid_type,
    }
}

fn gifts_cells(stats: &LcStats) -> (NoteCell, NoteCell) {
    let gifts = stats.gift_boxes();
    if gifts.is_empty() {
        return (
            NoteCell {
                column: GIFT_NET_COLUMN,
                value: json!(0),
                note: None,
            },
            NoteCell {
                column: GIFT_OPENED_VALUE_COLUMN,
                value: json!(0),
                note: None,
            },
        );
    }

    let opened = gifts
        .iter()
        .filter(|gift| gift.collected)
        .collect::<Vec<_>>();
    let net = opened
        .iter()
        .map(|gift| gift.new_scrap_value - gift.gift_scrap_value)
        .sum::<i64>();
    let gift_value = opened.iter().map(|gift| gift.gift_scrap_value).sum::<i64>();
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

    (
        NoteCell {
            column: GIFT_NET_COLUMN,
            value: json!(net),
            note: Some(note.clone()),
        },
        NoteCell {
            column: GIFT_OPENED_VALUE_COLUMN,
            value: json!(gift_value),
            note: Some(note),
        },
    )
}

fn apparatus_state(payload: &LcStats) -> String {
    if !payload.app_spawned() {
        return "-".to_string();
    }
    let missed_apparatus = payload
        .active_missed_items()
        .any(|item| item.item_type.eq_ignore_ascii_case("Apparatus"));
    if missed_apparatus {
        "E".to_string()
    } else {
        "C".to_string()
    }
}

fn serenade_moon(value: &str) -> String {
    let raw = strip_apostrophe(value);
    let moon = strip_moon_number(&raw);
    let number = raw
        .trim_start()
        .chars()
        .take_while(|ch| ch.is_ascii_digit())
        .collect::<String>();
    let name = normalize_moon_name(&moon);
    let Some(short) = moon_short_name(&name) else {
        return "?".to_string();
    };
    let candidate = if number.is_empty() {
        short.to_string()
    } else {
        format!("{number}-{short}")
    };
    if matches!(
        candidate.as_str(),
        "8-Titan"
            | "7-Dine"
            | "68-Artifice"
            | "85-Rend"
            | "5-Embrion"
            | "20-Adam"
            | "21-Offence"
            | "220-Ass"
            | "61-March"
            | "56-Vow"
            | "41-Exp"
    ) {
        candidate
    } else {
        "?".to_string()
    }
}

fn moon_short_name(name: &str) -> Option<&'static str> {
    match name {
        "titan" => Some("Titan"),
        "dine" => Some("Dine"),
        "artifice" => Some("Artifice"),
        "rend" => Some("Rend"),
        "embrion" => Some("Embrion"),
        "adamance" | "adam" => Some("Adam"),
        "offence" => Some("Offence"),
        "assurance" | "ass" => Some("Ass"),
        "march" => Some("March"),
        "vow" => Some("Vow"),
        "experimentation" | "exp" => Some("Exp"),
        _ => None,
    }
}

fn normalize_moon_name(value: &str) -> String {
    value
        .trim()
        .trim_start_matches('-')
        .trim()
        .chars()
        .filter(|ch| ch.is_ascii_alphabetic())
        .collect::<String>()
        .to_ascii_lowercase()
}

fn serenade_weather(value: &str) -> String {
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

fn legacy_shotgun_value_enabled(stats: &LcStats) -> bool {
    let version = stats.version_text();
    let digits = version
        .chars()
        .filter(|ch| ch.is_ascii_digit())
        .collect::<String>();
    digits == "45" || digits == "49"
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

fn column_to_index(column: &str) -> usize {
    column.chars().fold(0, |index, ch| {
        index * 26 + (ch.to_ascii_uppercase() as usize - 'A' as usize + 1)
    }) - 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serenade_moon_uses_expected_short_names() {
        assert_eq!(serenade_moon("'20 Adamance"), "20-Adam");
        assert_eq!(serenade_moon("'220-Assurance"), "220-Ass");
        assert_eq!(serenade_moon("'41 Experimentation"), "41-Exp");
        assert_eq!(serenade_moon("'71 Unknown"), "?");
    }

    #[test]
    fn zero_values_become_dash_for_standard_cells() {
        let stats = json!({
            "MoonInfo": { "Name": "'68 Artifice", "Weather": "'Mild" },
            "DungeonInfo": { "Interior": "'MineshaftFlow", "ItemCount": 0 },
            "AppSpawned": false
        });
        let payload = lcstats(&stats);
        let normalized = NormalizedStats::from_stats(&payload);
        let updates = build_value_updates(&normalized, 7);

        assert_eq!(cell_value(&updates, QUOTA_COLUMN), None);
        assert_eq!(cell_value(&updates, ITEM_COUNT_COLUMN), Some(&json!("-")));
        assert_eq!(cell_value(&updates, APPARATUS_COLUMN), Some(&json!("-")));
        assert_eq!(cell_value(&updates, TOPLINE_COLUMN), Some(&json!(0)));
    }

    #[test]
    fn player_names_follow_player_id_order() {
        let stats = json!({
            "Players": {
                "steam-c": { "Name": "C", "PlayerID": 2, "Alive": true },
                "steam-a": { "Name": "A", "PlayerID": 0, "Alive": true }
            }
        });
        let payload = lcstats(&stats);
        let normalized = NormalizedStats::from_stats(&payload);
        let updates = build_value_updates(&normalized, 7);

        assert_eq!(
            cell_value_at(&updates, PLAYER_NAME_COLUMN, 42),
            Some(&json!("A"))
        );
        assert_eq!(
            cell_value_at(&updates, PLAYER_NAME_COLUMN, 43),
            Some(&json!("C"))
        );
    }

    #[test]
    fn gordion_sold_uses_previous_day_row_and_quota_uses_next_row() {
        let stats = json!({
            "NewQuota": "'900",
            "ValueSold": "'130",
            "MoonInfo": { "Name": "'71 Gordion" },
            "DungeonInfo": { "Interior": "'Mineshaft", "ItemCount": 34 },
            "CollectedTotal": 926
        });
        let target_row = 8;
        let sold_row = gordion_sold_row(target_row);
        let payload = lcstats(&stats);
        let updates = build_gordion_updates(
            payload.value_sold(),
            payload.new_quota(),
            target_row,
            sold_row,
            25.0,
        );

        assert!(lcstats(&stats).is_gordion_moon());
        assert_eq!(sold_row, 7);
        assert_eq!(cell_value_at(&updates, SOLD_COLUMN, 7), Some(&json!(155)));
        assert_eq!(cell_value_at(&updates, QUOTA_COLUMN, 8), Some(&json!(900)));
        assert_eq!(cell_value_at(&updates, MOON_COLUMN, 8), None);
        assert_eq!(cell_value_at(&updates, TOPLINE_COLUMN, 8), None);
    }

    #[test]
    fn galetry_stats_use_gordion_economy_path() {
        let stats = json!({
            "MoonInfo": { "Name": "'Galetry" }
        });

        assert!(lcstats(&stats).is_gordion_moon());
    }

    fn cell_value<'a>(values: &'a [(String, usize, Value)], column: &str) -> Option<&'a Value> {
        values
            .iter()
            .find(|(value_column, _, _)| value_column == column)
            .map(|(_, _, value)| value)
    }

    fn cell_value_at<'a>(
        values: &'a [(String, usize, Value)],
        column: &str,
        row: usize,
    ) -> Option<&'a Value> {
        values
            .iter()
            .find(|(value_column, value_row, _)| value_column == column && *value_row == row)
            .map(|(_, _, value)| value)
    }
}

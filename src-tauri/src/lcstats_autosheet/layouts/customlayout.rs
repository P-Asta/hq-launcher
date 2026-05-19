use serde_json::{json, Value};

use crate::google_oauth::{CustomLcStatsLayoutSettings, LcStatsSettings};
use crate::lcstats_autosheet::layouts::CUSTOM_LAYOUT;
use crate::lcstats_autosheet::sheets::{
    batch_update_spreadsheet, batch_write_cells_user_entered, first_empty_row_from, get_sheet_id,
};
use crate::lcstats_autosheet::stats::{
    array_at, array_at_any, bool_at, intish_value, object_at, string_at, strip_moon_number,
    value_at,
};

pub async fn write(
    client: &reqwest::Client,
    token: &str,
    settings: &LcStatsSettings,
    stats: &Value,
) -> Result<(), String> {
    if !settings.layout.eq_ignore_ascii_case(CUSTOM_LAYOUT) {
        return Ok(());
    }
    let spreadsheet_id = settings.spreadsheet_id.trim();
    let sheet_name = settings.active_sheet_name.trim();
    if spreadsheet_id.is_empty() || sheet_name.is_empty() {
        return Err("spreadsheet or sheet is not set".to_string());
    }

    let layout = ResolvedCustomLayout::from_settings(&settings.custom_layout);
    let row = first_empty_row_from(
        client,
        token,
        spreadsheet_id,
        sheet_name,
        &layout.check_column,
        layout.start_row,
    )
    .await?;
    if is_economy_moon(stats) {
        return handle_economy_event(
            client,
            token,
            spreadsheet_id,
            sheet_name,
            &layout,
            row,
            stats,
        )
        .await;
    }

    let normalized = NormalizedStats::from_stats(stats, &layout);
    batch_write_cells_user_entered(
        client,
        token,
        spreadsheet_id,
        sheet_name,
        build_value_updates(&normalized, &layout, row),
    )
    .await?;
    write_note_cells(
        client,
        token,
        spreadsheet_id,
        sheet_name,
        row,
        &normalized,
        &layout,
    )
    .await
}

#[derive(Debug, Clone)]
struct ResolvedCustomLayout {
    start_row: usize,
    check_column: String,
    text_case: String,
    quota_column: Option<String>,
    seed_column: Option<String>,
    moon_column: Option<String>,
    weather_column: Option<String>,
    layout_column: Option<String>,
    item_count_column: Option<String>,
    apparatus_column: Option<String>,
    bee_amount_column: Option<String>,
    split_hive_count: bool,
    bee_value_column: Option<String>,
    cheap_hive_column: Option<String>,
    expensive_hive_column: Option<String>,
    write_zero_for_missing_hives: bool,
    egg_column: Option<String>,
    egg_notes_enabled: bool,
    collected_egg_column: Option<String>,
    collected_egg_notes_enabled: bool,
    nut_column: Option<String>,
    nut_collect_column: Option<String>,
    butler_column: Option<String>,
    butler_collect_column: Option<String>,
    collected_column: Option<String>,
    available_column: Option<String>,
    real_available_column: Option<String>,
    collected_no_extra_column: Option<String>,
    missing_column: Option<String>,
    sold_column: Option<String>,
    sid_column: Option<String>,
    sid_write_false: bool,
    infestation_column: Option<String>,
    infestation_write_false: bool,
    lost_scrap_column: Option<String>,
    takeoff_time_column: Option<String>,
    turret_column: Option<String>,
    landmine_column: Option<String>,
    spiketrap_column: Option<String>,
    death_columns: Vec<String>,
    player_name_columns: Vec<String>,
    player_name_row: usize,
    alive_state: String,
    dead_state: String,
    missing_state: String,
    disconnected_state: String,
    death_notes_enabled: bool,
    fog_column: Option<String>,
    fog_write_false: bool,
    meteor_column: Option<String>,
    meteor_write_false: bool,
    gifts_column: Option<String>,
}

impl ResolvedCustomLayout {
    fn from_settings(settings: &CustomLcStatsLayoutSettings) -> Self {
        let moon_column = normalize_optional_column(&settings.moon_column);
        let collected_column = normalize_optional_column(&settings.collected_column);
        let available_column = normalize_optional_column(&settings.available_column);
        let quota_column = normalize_optional_column(&settings.quota_column);
        Self {
            start_row: settings.start_row.max(1),
            check_column: normalize_optional_column(&settings.check_column)
                .or_else(|| moon_column.clone())
                .or_else(|| collected_column.clone())
                .or_else(|| available_column.clone())
                .or_else(|| quota_column.clone())
                .unwrap_or_else(|| "A".to_string()),
            text_case: normalize_text_case(&settings.text_case),
            quota_column,
            seed_column: normalize_optional_column(&settings.seed_column),
            moon_column,
            weather_column: normalize_optional_column(&settings.weather_column),
            layout_column: normalize_optional_column(&settings.layout_column),
            item_count_column: normalize_optional_column(&settings.item_count_column),
            apparatus_column: normalize_optional_column(&settings.apparatus_column),
            bee_amount_column: normalize_optional_column(&settings.bee_amount_column),
            split_hive_count: settings.split_hive_count,
            bee_value_column: normalize_optional_column(&settings.bee_value_column),
            cheap_hive_column: normalize_optional_column(&settings.cheap_hive_column),
            expensive_hive_column: normalize_optional_column(&settings.expensive_hive_column),
            write_zero_for_missing_hives: settings.write_zero_for_missing_hives,
            egg_column: normalize_optional_column(&settings.egg_column),
            egg_notes_enabled: settings.egg_notes_enabled,
            collected_egg_column: normalize_optional_column(&settings.collected_egg_column),
            collected_egg_notes_enabled: settings.collected_egg_notes_enabled,
            nut_column: normalize_optional_column(&settings.nut_column),
            nut_collect_column: normalize_optional_column(&settings.nut_collect_column),
            butler_column: normalize_optional_column(&settings.butler_column),
            butler_collect_column: normalize_optional_column(&settings.butler_collect_column),
            collected_column,
            available_column,
            real_available_column: normalize_optional_column(&settings.real_available_column),
            collected_no_extra_column: normalize_optional_column(
                &settings.collected_no_extra_column,
            ),
            missing_column: normalize_optional_column(&settings.missing_column),
            sold_column: normalize_optional_column(&settings.sold_column),
            sid_column: normalize_optional_column(&settings.sid_column),
            sid_write_false: settings.sid_write_false,
            infestation_column: normalize_optional_column(&settings.infestation_column),
            infestation_write_false: settings.infestation_write_false,
            lost_scrap_column: normalize_optional_column(&settings.lost_scrap_column),
            takeoff_time_column: normalize_optional_column(&settings.takeoff_time_column),
            turret_column: normalize_optional_column(&settings.turret_column),
            landmine_column: normalize_optional_column(&settings.landmine_column),
            spiketrap_column: normalize_optional_column(&settings.spiketrap_column),
            death_columns: settings
                .death_columns
                .split(',')
                .filter_map(normalize_optional_column)
                .collect(),
            player_name_columns: settings
                .player_name_columns
                .split(',')
                .filter_map(normalize_optional_column)
                .collect(),
            player_name_row: settings.player_name_row.max(1),
            alive_state: settings.alive_state.clone(),
            dead_state: settings.dead_state.clone(),
            missing_state: settings.missing_state.clone(),
            disconnected_state: settings.disconnected_state.clone(),
            death_notes_enabled: settings.death_notes_enabled,
            fog_column: normalize_optional_column(&settings.fog_column),
            fog_write_false: settings.fog_write_false,
            meteor_column: normalize_optional_column(&settings.meteor_column),
            meteor_write_false: settings.meteor_write_false,
            gifts_column: normalize_optional_column(&settings.gifts_column),
        }
    }
}

async fn handle_economy_event(
    client: &reqwest::Client,
    token: &str,
    spreadsheet_id: &str,
    sheet_name: &str,
    layout: &ResolvedCustomLayout,
    row: usize,
    stats: &Value,
) -> Result<(), String> {
    let mut updates = vec![];
    let value_sold = intish_at(stats, &["ValueSold"]);
    let new_quota = intish_at(stats, &["NewQuota"]);

    if value_sold != 0 {
        if let Some(column) = &layout.sold_column {
            updates.push((
                column.clone(),
                row.saturating_sub(3).max(layout.start_row),
                json!(value_sold),
            ));
        }
    }
    if new_quota != 0 {
        if let Some(column) = &layout.quota_column {
            updates.push((column.clone(), row, json!(new_quota)));
        }
    }

    batch_write_cells_user_entered(client, token, spreadsheet_id, sheet_name, updates).await
}

#[derive(Debug, Clone)]
struct NormalizedPlayer {
    name: String,
    status: String,
    note: Option<String>,
}

#[derive(Debug, Clone)]
struct NoteCell {
    column: String,
    value: Value,
    note: Option<String>,
}

#[derive(Debug, Clone)]
struct NormalizedStats {
    new_quota: i64,
    seed: String,
    moon_name: String,
    weather: String,
    interior: String,
    item_count: i64,
    apparatus_spawned: bool,
    beehive_amount: String,
    beehive_value: String,
    cheap_hive_value: Option<i64>,
    expensive_hive_value: Option<i64>,
    egg_available: NoteCell,
    collected_egg: NoteCell,
    nutcracker_count: usize,
    nutcracker_collected: i64,
    butler_count: usize,
    butler_collected: i64,
    collected_total: i64,
    available_total: i64,
    real_available_total: i64,
    collected_no_extra: i64,
    missing: NoteCell,
    value_sold: i64,
    sid: NoteCell,
    infestation: NoteCell,
    lost_scrap: i64,
    takeoff_time: String,
    turret_count: i64,
    landmine_count: i64,
    spiketrap_count: i64,
    players: Vec<NormalizedPlayer>,
    fog: bool,
    meteor: NoteCell,
    gifts: NoteCell,
}

impl NormalizedStats {
    fn from_stats(stats: &Value, layout: &ResolvedCustomLayout) -> Self {
        let sid_type = non_false_text(&string_at(stats, &["SIDType"]));
        let infestation_type = non_false_text(&string_at(stats, &["InfestationType"]));
        let meteor_time = non_false_text(&string_at(stats, &["MeteorShowerTime"]));
        Self {
            new_quota: intish_at(stats, &["NewQuota"]),
            seed: strip_apostrophe(&string_at(stats, &["Seed"])),
            moon_name: strip_moon_number(&strip_apostrophe(&string_at(
                stats,
                &["MoonInfo", "Name"],
            ))),
            weather: custom_weather(&string_at(stats, &["MoonInfo", "Weather"])),
            interior: normalize_interior_name(&strip_apostrophe(&string_at(
                stats,
                &["DungeonInfo", "Interior"],
            ))),
            item_count: intish_at(stats, &["DungeonInfo", "ItemCount"]),
            apparatus_spawned: bool_at(stats, &["AppSpawned"]),
            beehive_amount: beehive_amount(stats, layout.split_hive_count),
            beehive_value: beehive_value(stats),
            cheap_hive_value: cheap_hive_value(stats),
            expensive_hive_value: expensive_hive_value(stats),
            egg_available: egg_available_cell(stats),
            collected_egg: collected_egg_cell(stats),
            nutcracker_count: enemy_count(stats, "Nutcracker"),
            nutcracker_collected: collected_count_or_legacy_int(
                stats,
                &["ShotgunInfo", "Collected"],
                &["ShotgunsCollected"],
            ),
            butler_count: enemy_count(stats, "Butler"),
            butler_collected: collected_count_or_legacy_int(
                stats,
                &["KnifeInfo", "Collected"],
                &["KnivesCollected"],
            ),
            collected_total: intish_at(stats, &["CollectedTotal"]),
            available_total: intish_at(stats, &["BottomLine"]),
            real_available_total: intish_at(stats, &["BottomLineTrue"]),
            collected_no_extra: intish_at(stats, &["CollectedNoExtra"]),
            missing: missing_items_cell(stats),
            value_sold: intish_at(stats, &["ValueSold"]),
            sid: NoteCell {
                column: String::new(),
                value: json!(sid_type.is_some()),
                note: sid_type,
            },
            infestation: NoteCell {
                column: String::new(),
                value: json!(infestation_type.is_some()),
                note: infestation_type,
            },
            lost_scrap: lost_scrap(stats),
            takeoff_time: strip_apostrophe(&string_at(stats, &["TakeOffTime"])),
            turret_count: intish_at(stats, &["HazardInfo", "TurretCount"]),
            landmine_count: intish_at(stats, &["HazardInfo", "LandmineCount"]),
            spiketrap_count: intish_at(stats, &["HazardInfo", "SpiketrapCount"]),
            players: normalize_players(stats, layout),
            fog: bool_at(stats, &["IndoorFog"]),
            meteor: NoteCell {
                column: String::new(),
                value: json!(meteor_time.is_some()),
                note: meteor_time,
            },
            gifts: gifts_cell(stats),
        }
    }
}

fn build_value_updates(
    stats: &NormalizedStats,
    layout: &ResolvedCustomLayout,
    row: usize,
) -> Vec<(String, usize, Value)> {
    let mut updates = vec![];
    push_value(
        &mut updates,
        &layout.seed_column,
        row,
        blank_or_x(&stats.seed),
    );
    push_value(
        &mut updates,
        &layout.moon_column,
        row,
        json!(apply_text_case(&stats.moon_name, &layout.text_case)),
    );
    push_value(
        &mut updates,
        &layout.weather_column,
        row,
        json!(apply_text_case(&stats.weather, &layout.text_case)),
    );
    push_value(
        &mut updates,
        &layout.layout_column,
        row,
        json!(apply_text_case(&stats.interior, &layout.text_case)),
    );
    push_value(
        &mut updates,
        &layout.item_count_column,
        row,
        json!(stats.item_count),
    );
    push_value(
        &mut updates,
        &layout.apparatus_column,
        row,
        json!(stats.apparatus_spawned),
    );
    push_hive_text_value(
        &mut updates,
        &layout.bee_amount_column,
        row,
        &stats.beehive_amount,
        layout.write_zero_for_missing_hives,
    );
    push_hive_text_value(
        &mut updates,
        &layout.bee_value_column,
        row,
        &stats.beehive_value,
        layout.write_zero_for_missing_hives,
    );
    push_hive_number_value(
        &mut updates,
        &layout.cheap_hive_column,
        row,
        stats.cheap_hive_value,
        layout.write_zero_for_missing_hives,
    );
    push_hive_number_value(
        &mut updates,
        &layout.expensive_hive_column,
        row,
        stats.expensive_hive_value,
        layout.write_zero_for_missing_hives,
    );
    if layout.egg_column.is_some() && !layout.egg_notes_enabled {
        push_value(
            &mut updates,
            &layout.egg_column,
            row,
            stats.egg_available.value.clone(),
        );
    }
    if layout.collected_egg_column.is_some() && !layout.collected_egg_notes_enabled {
        push_value(
            &mut updates,
            &layout.collected_egg_column,
            row,
            stats.collected_egg.value.clone(),
        );
    }
    push_value(
        &mut updates,
        &layout.nut_column,
        row,
        json!(stats.nutcracker_count),
    );
    push_value(
        &mut updates,
        &layout.nut_collect_column,
        row,
        json!(stats.nutcracker_collected),
    );
    push_value(
        &mut updates,
        &layout.butler_column,
        row,
        json!(stats.butler_count),
    );
    push_value(
        &mut updates,
        &layout.butler_collect_column,
        row,
        json!(stats.butler_collected),
    );
    push_value(
        &mut updates,
        &layout.collected_column,
        row,
        json!(stats.collected_total),
    );
    push_value(
        &mut updates,
        &layout.available_column,
        row,
        json!(stats.available_total),
    );
    push_value(
        &mut updates,
        &layout.real_available_column,
        row,
        json!(stats.real_available_total),
    );
    push_value(
        &mut updates,
        &layout.collected_no_extra_column,
        row,
        json!(stats.collected_no_extra),
    );
    if stats.fog || layout.fog_write_false {
        push_value(&mut updates, &layout.fog_column, row, json!(stats.fog));
    }
    push_value(
        &mut updates,
        &layout.takeoff_time_column,
        row,
        json!(apply_text_case(&stats.takeoff_time, &layout.text_case)),
    );
    push_value(
        &mut updates,
        &layout.turret_column,
        row,
        json!(stats.turret_count),
    );
    push_value(
        &mut updates,
        &layout.landmine_column,
        row,
        json!(stats.landmine_count),
    );
    push_value(
        &mut updates,
        &layout.spiketrap_column,
        row,
        json!(stats.spiketrap_count),
    );

    if stats.new_quota != 0 {
        push_value(
            &mut updates,
            &layout.quota_column,
            row,
            json!(stats.new_quota),
        );
    }
    if stats.value_sold != 0 {
        push_value(
            &mut updates,
            &layout.sold_column,
            row,
            json!(stats.value_sold),
        );
    }
    if stats.lost_scrap != 0 {
        push_value(
            &mut updates,
            &layout.lost_scrap_column,
            row,
            json!(stats.lost_scrap),
        );
    }
    for (index, player) in stats
        .players
        .iter()
        .take(layout.death_columns.len())
        .enumerate()
    {
        if player.note.is_none() {
            updates.push((
                layout.death_columns[index].clone(),
                row,
                json!(player.status),
            ));
        }
    }
    for (index, player) in stats
        .players
        .iter()
        .take(layout.player_name_columns.len())
        .enumerate()
    {
        if !player.name.trim().is_empty() {
            updates.push((
                layout.player_name_columns[index].clone(),
                layout.player_name_row,
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
    layout: &ResolvedCustomLayout,
) -> Result<(), String> {
    let sheet_id = get_sheet_id(client, token, spreadsheet_id, sheet_name).await?;
    let mut requests = vec![];
    push_note_request(
        &mut requests,
        sheet_id,
        &layout.missing_column,
        row,
        &stats.missing,
    );
    if layout.egg_notes_enabled {
        push_note_request(
            &mut requests,
            sheet_id,
            &layout.egg_column,
            row,
            &stats.egg_available,
        );
    }
    if layout.collected_egg_notes_enabled {
        push_note_request(
            &mut requests,
            sheet_id,
            &layout.collected_egg_column,
            row,
            &stats.collected_egg,
        );
    }
    if stats.sid.value.as_bool().unwrap_or(false) || layout.sid_write_false {
        push_note_request(&mut requests, sheet_id, &layout.sid_column, row, &stats.sid);
    }
    if stats.infestation.value.as_bool().unwrap_or(false) || layout.infestation_write_false {
        push_note_request(
            &mut requests,
            sheet_id,
            &layout.infestation_column,
            row,
            &stats.infestation,
        );
    }
    if stats.meteor.value.as_bool().unwrap_or(false) || layout.meteor_write_false {
        push_note_request(
            &mut requests,
            sheet_id,
            &layout.meteor_column,
            row,
            &stats.meteor,
        );
    }
    push_note_request(
        &mut requests,
        sheet_id,
        &layout.gifts_column,
        row,
        &stats.gifts,
    );

    for (index, player) in stats
        .players
        .iter()
        .take(layout.death_columns.len())
        .enumerate()
    {
        if let Some(note) = &player.note {
            requests.push(value_with_note_request(
                sheet_id,
                &NoteCell {
                    column: layout.death_columns[index].clone(),
                    value: json!(player.status),
                    note: Some(note.clone()),
                },
                row,
            ));
        }
    }

    batch_update_spreadsheet(client, token, spreadsheet_id, requests).await
}

fn push_value(
    updates: &mut Vec<(String, usize, Value)>,
    column: &Option<String>,
    row: usize,
    value: Value,
) {
    if let Some(column) = column {
        updates.push((column.clone(), row, value));
    }
}

fn push_hive_text_value(
    updates: &mut Vec<(String, usize, Value)>,
    column: &Option<String>,
    row: usize,
    value: &str,
    write_zero_for_missing_hives: bool,
) {
    if value.trim().is_empty() {
        if write_zero_for_missing_hives {
            push_value(updates, column, row, json!(0));
        }
    } else {
        push_value(updates, column, row, json!(value));
    }
}

fn push_hive_number_value(
    updates: &mut Vec<(String, usize, Value)>,
    column: &Option<String>,
    row: usize,
    value: Option<i64>,
    write_zero_for_missing_hives: bool,
) {
    if let Some(value) = value {
        push_value(updates, column, row, json!(value));
    } else if write_zero_for_missing_hives {
        push_value(updates, column, row, json!(0));
    }
}

fn push_note_request(
    requests: &mut Vec<Value>,
    sheet_id: i64,
    column: &Option<String>,
    row: usize,
    source: &NoteCell,
) {
    if let Some(column) = column {
        let mut cell = source.clone();
        cell.column = column.clone();
        requests.push(value_with_note_request(sheet_id, &cell, row));
    }
}

fn value_with_note_request(sheet_id: i64, cell: &NoteCell, row: usize) -> Value {
    let column_index = column_to_index(&cell.column);
    let mut value = json!({ "userEnteredValue": google_user_value(cell.value.clone()) });
    let fields = if let Some(note) = cell.note.as_ref().filter(|note| !note.trim().is_empty()) {
        value["note"] = json!(note);
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
                "endColumnIndex": column_index + 1
            },
            "rows": [{ "values": [value] }],
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

fn normalize_players(stats: &Value, layout: &ResolvedCustomLayout) -> Vec<NormalizedPlayer> {
    object_at(stats, &["Players"])
        .into_values()
        .map(|player| {
            let name = strip_apostrophe(
                player
                    .get("Name")
                    .and_then(Value::as_str)
                    .unwrap_or_default(),
            );
            let alive = player
                .get("Alive")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let disconnected = player
                .get("Disconnected")
                .and_then(Value::as_bool)
                .unwrap_or(false);
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

            let status = if disconnected {
                layout.disconnected_state.as_str()
            } else if cause.eq_ignore_ascii_case("abandonment")
                || cause.eq_ignore_ascii_case("abandoned")
            {
                layout.missing_state.as_str()
            } else if alive {
                layout.alive_state.as_str()
            } else {
                layout.dead_state.as_str()
            }
            .to_string();

            let note = if !layout.death_notes_enabled || status == layout.missing_state {
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

fn beehive_amount(stats: &Value, split_hive_count: bool) -> String {
    let values = beehive_values(stats);
    if values.is_empty() {
        return String::new();
    }
    if !split_hive_count {
        return values.len().to_string();
    }
    let small = values.iter().filter(|&&value| value < 100).count();
    let large = values.iter().filter(|&&value| value >= 100).count();
    format!("{small}/{large}")
}

fn beehive_value(stats: &Value) -> String {
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

fn cheap_hive_value(stats: &Value) -> Option<i64> {
    beehive_values(stats)
        .into_iter()
        .filter(|value| *value < 100)
        .min()
}

fn expensive_hive_value(stats: &Value) -> Option<i64> {
    beehive_values(stats)
        .into_iter()
        .filter(|value| *value >= 100)
        .max()
}

fn beehive_values(stats: &Value) -> Vec<i64> {
    array_at_any(
        stats,
        &[&["BeeInfo", "Available"][..], &["BeeInfo", "Values"][..]],
    )
    .iter()
    .map(intish_value)
    .collect()
}

fn egg_available_cell(stats: &Value) -> NoteCell {
    let mut values = array_at_any(
        stats,
        &[
            &["EggInfo", "Available"][..],
            &["BirdInfo", "EggValues"][..],
        ],
    )
    .iter()
    .map(intish_value)
    .collect::<Vec<_>>();
    values.sort_unstable();
    let cell_value = values
        .iter()
        .map(|value| value.to_string())
        .collect::<Vec<_>>()
        .join("|");
    let note = (!values.is_empty()).then(|| egg_values_note("Available eggs", &values));
    NoteCell {
        column: String::new(),
        value: blank_or_x(&cell_value),
        note,
    }
}

fn collected_egg_cell(stats: &Value) -> NoteCell {
    let mut values = array_at_any(
        stats,
        &[
            &["EggInfo", "Collected"][..],
            &["BirdInfo", "CollectedEggValues"][..],
        ],
    )
    .iter()
    .map(intish_value)
    .collect::<Vec<_>>();
    values.sort_unstable();
    let total = values.iter().sum::<i64>();
    let value = if values.is_empty() {
        json!("X")
    } else {
        json!(total)
    };
    let note = (!values.is_empty()).then(|| egg_values_note("Collected eggs", &values));
    NoteCell {
        column: String::new(),
        value,
        note,
    }
}

fn egg_values_note(label: &str, values: &[i64]) -> String {
    format!(
        "{label}: {}",
        values
            .iter()
            .map(|value| value.to_string())
            .collect::<Vec<_>>()
            .join(", ")
    )
}

fn gifts_cell(stats: &Value) -> NoteCell {
    let gifts = array_at(stats, &["GiftBoxes"]);
    if gifts.is_empty() {
        return NoteCell {
            column: String::new(),
            value: json!("X"),
            note: None,
        };
    }

    let collected = gifts
        .iter()
        .filter(|gift| {
            gift.get("Collected")
                .and_then(Value::as_bool)
                .unwrap_or(false)
        })
        .collect::<Vec<_>>();
    let total_net = collected
        .iter()
        .map(|gift| {
            gift.get("GiftValue").map(intish_value).unwrap_or(0)
                - gift.get("ScrapValue").map(intish_value).unwrap_or(0)
        })
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
                "Box {}: GiftValue={}, ScrapValue={}, Collected={}",
                index + 1,
                gift.get("GiftValue").map(intish_value).unwrap_or(0),
                gift.get("ScrapValue").map(intish_value).unwrap_or(0),
                gift.get("Collected")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    NoteCell {
        column: String::new(),
        value: json!(cell_value),
        note: Some(note),
    }
}

fn missing_items_cell(stats: &Value) -> NoteCell {
    let missing = array_at(stats, &["MissedItems"])
        .iter()
        .filter(|item| {
            !item
                .get("CollectedOnPreviousDay")
                .and_then(Value::as_bool)
                .unwrap_or(false)
        })
        .collect::<Vec<_>>();
    if missing.is_empty() {
        return NoteCell {
            column: String::new(),
            value: json!("X"),
            note: None,
        };
    }
    let note = missing
        .iter()
        .map(|item| {
            format!(
                "{}: {}",
                item.get("ItemType")
                    .and_then(Value::as_str)
                    .unwrap_or("Unknown"),
                item.get("Value").map(intish_value).unwrap_or(0)
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    NoteCell {
        column: String::new(),
        value: json!(missing.len().to_string()),
        note: Some(note),
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
        .map(|item| item.get("Value").map(intish_value).unwrap_or(0))
        .sum()
}

fn enemy_count(stats: &Value, enemy: &str) -> usize {
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

fn is_economy_moon(stats: &Value) -> bool {
    let moon = strip_moon_number(&strip_apostrophe(&string_at(stats, &["MoonInfo", "Name"])));
    let normalized = moon
        .trim()
        .chars()
        .filter(|ch| ch.is_ascii_alphabetic())
        .collect::<String>()
        .to_ascii_uppercase();
    normalized == "GORDION" || normalized == "GORION" || normalized == "GALETRY"
}

fn custom_weather(value: &str) -> String {
    let weather = strip_apostrophe(value);
    if weather.eq_ignore_ascii_case("Mild") {
        "Clear".to_string()
    } else {
        weather
    }
}

fn normalize_text_case(value: &str) -> String {
    match value.trim().to_ascii_lowercase().as_str() {
        "uppercase" | "upper" => "UPPERCASE",
        "lowercase" | "lower" => "lowercase",
        "title case" | "titlecase" | "title" => "Title Case",
        "camelcase" | "camel case" | "camel" => "camelCase",
        "pascalcase" | "pascal case" | "pascal" => "PascalCase",
        _ => "Original",
    }
    .to_string()
}

fn apply_text_case(value: &str, text_case: &str) -> String {
    match normalize_text_case(text_case).as_str() {
        "UPPERCASE" => value.to_uppercase(),
        "lowercase" => value.to_lowercase(),
        "Title Case" => words_for_case(value)
            .into_iter()
            .map(|word| capitalize_word(&word.to_lowercase()))
            .collect::<Vec<_>>()
            .join(" "),
        "camelCase" => {
            let mut words = words_for_case(value).into_iter();
            let Some(first) = words.next() else {
                return String::new();
            };
            let mut out = first.to_lowercase();
            for word in words {
                out.push_str(&capitalize_word(&word.to_lowercase()));
            }
            out
        }
        "PascalCase" => words_for_case(value)
            .into_iter()
            .map(|word| capitalize_word(&word.to_lowercase()))
            .collect::<Vec<_>>()
            .join(""),
        _ => value.to_string(),
    }
}

fn words_for_case(value: &str) -> Vec<String> {
    let mut normalized = String::new();
    let mut previous_lowercase = false;
    for ch in value.chars() {
        if ch.is_ascii_uppercase() && previous_lowercase {
            normalized.push(' ');
        }
        if ch.is_alphanumeric() {
            normalized.push(ch);
            previous_lowercase = ch.is_ascii_lowercase();
        } else {
            normalized.push(' ');
            previous_lowercase = false;
        }
    }
    normalized
        .split_whitespace()
        .map(ToOwned::to_owned)
        .collect()
}

fn capitalize_word(value: &str) -> String {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return String::new();
    };
    format!("{}{}", first.to_uppercase(), chars.as_str())
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

fn intish_at(stats: &Value, path: &[&str]) -> i64 {
    value_at(stats, path).map(intish_value).unwrap_or(0)
}

fn strip_apostrophe(value: &str) -> String {
    value.trim_start_matches('\'').to_string()
}

fn normalize_optional_column(value: &str) -> Option<String> {
    let column = value
        .trim()
        .chars()
        .filter(|ch| ch.is_ascii_alphabetic())
        .collect::<String>()
        .to_ascii_uppercase();
    (!column.is_empty()).then_some(column)
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
    fn maps_custom_columns_from_settings() {
        let stats = json!({
            "NewQuota": "'900",
            "ValueSold": "'130",
            "MoonInfo": { "Name": "'68 Artifice", "Weather": "'Mild" },
            "DungeonInfo": { "Interior": "'MineshaftFlow", "ItemCount": "'34" },
            "BeeInfo": { "Available": [64, 132] },
            "EggInfo": { "Available": [18, 12] },
            "IndoorSpawns": [{ "Enemy": "Nutcracker" }],
            "CollectedTotal": "'926",
            "BottomLine": "'2133",
            "IndoorFog": true,
            "Players": {
                "1": { "Name": "'Aureo", "Alive": true, "Disconnected": false }
            }
        });
        let layout = ResolvedCustomLayout::from_settings(&CustomLcStatsLayoutSettings {
            start_row: 4,
            check_column: "G".to_string(),
            text_case: "UPPERCASE".to_string(),
            quota_column: "C".to_string(),
            seed_column: "AI".to_string(),
            moon_column: "G".to_string(),
            weather_column: "H".to_string(),
            layout_column: "I".to_string(),
            item_count_column: "J".to_string(),
            apparatus_column: "AJ".to_string(),
            bee_amount_column: "K".to_string(),
            split_hive_count: true,
            bee_value_column: "".to_string(),
            cheap_hive_column: "AD".to_string(),
            expensive_hive_column: "AE".to_string(),
            write_zero_for_missing_hives: false,
            egg_column: "L".to_string(),
            egg_notes_enabled: false,
            collected_egg_column: "AF".to_string(),
            collected_egg_notes_enabled: false,
            nut_column: "M".to_string(),
            nut_collect_column: "AG".to_string(),
            butler_column: "N".to_string(),
            butler_collect_column: "AH".to_string(),
            collected_column: "O".to_string(),
            available_column: "P".to_string(),
            real_available_column: "AK".to_string(),
            collected_no_extra_column: "AL".to_string(),
            missing_column: "Q".to_string(),
            sold_column: "R".to_string(),
            sid_column: "S".to_string(),
            sid_write_false: true,
            infestation_column: "T".to_string(),
            infestation_write_false: true,
            lost_scrap_column: "AA".to_string(),
            takeoff_time_column: "AM".to_string(),
            turret_column: "AN".to_string(),
            landmine_column: "AO".to_string(),
            spiketrap_column: "AP".to_string(),
            death_columns: "U,V,W,X".to_string(),
            player_name_columns: "AB,AC,AD,AE".to_string(),
            player_name_row: 56,
            alive_state: "A".to_string(),
            dead_state: "D".to_string(),
            missing_state: "M".to_string(),
            disconnected_state: "DC".to_string(),
            death_notes_enabled: false,
            fog_column: "Y".to_string(),
            fog_write_false: true,
            meteor_column: "Z".to_string(),
            meteor_write_false: true,
            gifts_column: "AB".to_string(),
        });
        let normalized = NormalizedStats::from_stats(&stats, &layout);

        let updates = build_value_updates(&normalized, &layout, 7);

        assert_eq!(cell_value(&updates, "C"), Some(&json!(900)));
        assert_eq!(cell_value(&updates, "G"), Some(&json!("ARTIFICE")));
        assert_eq!(cell_value(&updates, "H"), Some(&json!("CLEAR")));
        assert_eq!(cell_value(&updates, "I"), Some(&json!("MINESHAFT")));
        assert_eq!(cell_value(&updates, "J"), Some(&json!(34)));
        assert_eq!(cell_value(&updates, "K"), Some(&json!("1/1")));
        assert_eq!(cell_value(&updates, "AD"), Some(&json!(64)));
        assert_eq!(cell_value(&updates, "AE"), Some(&json!(132)));
        assert_eq!(cell_value(&updates, "L"), Some(&json!("12|18")));
        assert_eq!(cell_value(&updates, "M"), Some(&json!(1)));
        assert_eq!(cell_value(&updates, "AG"), Some(&json!(0)));
        assert_eq!(cell_value(&updates, "AH"), Some(&json!(0)));
        assert_eq!(cell_value(&updates, "O"), Some(&json!(926)));
        assert_eq!(cell_value(&updates, "P"), Some(&json!(2133)));
        assert_eq!(cell_value(&updates, "R"), Some(&json!(130)));
        assert_eq!(cell_value(&updates, "U"), Some(&json!("A")));
        assert_eq!(cell_value_at(&updates, "AB", 56), Some(&json!("Aureo")));
        assert_eq!(cell_value(&updates, "Y"), Some(&json!(true)));
        assert_eq!(cell_value(&updates, "K").is_some(), true);
        assert_eq!(cell_value(&updates, "AA"), None);
    }

    #[test]
    fn custom_weather_replaces_mild_before_case_transform() {
        assert_eq!(
            apply_text_case(&custom_weather("'Mild"), "Original"),
            "Clear"
        );
        assert_eq!(
            apply_text_case(&custom_weather("'Mild"), "camelCase"),
            "clear"
        );
    }

    #[test]
    fn missing_hives_are_blank_unless_zero_is_enabled() {
        let stats = json!({});
        let settings = CustomLcStatsLayoutSettings {
            bee_amount_column: "J".to_string(),
            split_hive_count: false,
            bee_value_column: "K".to_string(),
            cheap_hive_column: "BA".to_string(),
            expensive_hive_column: "BB".to_string(),
            write_zero_for_missing_hives: false,
            ..Default::default()
        };
        let layout = ResolvedCustomLayout::from_settings(&settings);
        let normalized = NormalizedStats::from_stats(&stats, &layout);

        let updates = build_value_updates(&normalized, &layout, 7);

        assert_eq!(cell_value(&updates, "J"), None);
        assert_eq!(cell_value(&updates, "K"), None);
        assert_eq!(cell_value(&updates, "BA"), None);
        assert_eq!(cell_value(&updates, "BB"), None);

        let layout = ResolvedCustomLayout::from_settings(&CustomLcStatsLayoutSettings {
            write_zero_for_missing_hives: true,
            ..settings
        });
        let normalized = NormalizedStats::from_stats(&stats, &layout);
        let updates = build_value_updates(&normalized, &layout, 7);

        assert_eq!(cell_value(&updates, "J"), Some(&json!(0)));
        assert_eq!(cell_value(&updates, "K"), Some(&json!(0)));
        assert_eq!(cell_value(&updates, "BA"), Some(&json!(0)));
        assert_eq!(cell_value(&updates, "BB"), Some(&json!(0)));
    }

    #[test]
    fn hive_count_uses_total_by_default_and_split_when_enabled() {
        let stats = json!({
            "BeeInfo": { "Available": [60, 72, 108, 132, 144] }
        });
        let settings = CustomLcStatsLayoutSettings {
            bee_amount_column: "J".to_string(),
            split_hive_count: false,
            ..Default::default()
        };
        let layout = ResolvedCustomLayout::from_settings(&settings);
        let normalized = NormalizedStats::from_stats(&stats, &layout);
        let updates = build_value_updates(&normalized, &layout, 7);

        assert_eq!(cell_value(&updates, "J"), Some(&json!("5")));

        let layout = ResolvedCustomLayout::from_settings(&CustomLcStatsLayoutSettings {
            split_hive_count: true,
            ..settings
        });
        let normalized = NormalizedStats::from_stats(&stats, &layout);
        let updates = build_value_updates(&normalized, &layout, 7);

        assert_eq!(cell_value(&updates, "J"), Some(&json!("2/3")));
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

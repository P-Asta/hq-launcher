use serde_json::{json, Value};
use std::collections::BTreeSet;

use crate::google_oauth::{CustomLcStatsLayoutSettings, LcStatsSettings};
use crate::lcstats_autosheet::layouts::CUSTOM_LAYOUT;
use crate::lcstats_autosheet::sheets::{
    batch_update_spreadsheet, batch_write_cells_user_entered, first_empty_row_from, get_sheet_id,
};
use crate::lcstats_autosheet::stats::{
    array_at, array_at_any, intish_value, is_gordion_moon_name, lcstats,
    parse_lcstats_time_to_minutes, players_at, strip_apostrophe, strip_moon_number, value_at,
    value_at_any, LcStats,
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
    let payload = lcstats(stats);
    if is_economy_moon(&payload) {
        return handle_economy_event(
            client,
            token,
            spreadsheet_id,
            sheet_name,
            &layout,
            row,
            &payload,
        )
        .await;
    }

    let normalized = NormalizedStats::from_stats(stats, &payload, &layout);
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
    time_format: String,
    quota_column: Option<String>,
    seed_column: Option<String>,
    moon_column: Option<String>,
    weather_column: Option<String>,
    layout_column: Option<String>,
    item_count_column: Option<String>,
    apparatus_column: Option<String>,
    bee_amount_column: Option<String>,
    split_hive_count: bool,
    beehive_collected_column: Option<String>,
    beehive_collected_value_column: Option<String>,
    beehive_collected_notes_enabled: bool,
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
    nut_notes_enabled: bool,
    butler_column: Option<String>,
    butler_collect_column: Option<String>,
    butler_notes_enabled: bool,
    collected_column: Option<String>,
    available_column: Option<String>,
    real_available_column: Option<String>,
    collected_no_extra_column: Option<String>,
    missing_column: Option<String>,
    filter_collected_gift_scrap_from_missing: bool,
    outside_items_column: Option<String>,
    sold_column: Option<String>,
    sid_column: Option<String>,
    sid_item_column: Option<String>,
    sid_notes_enabled: bool,
    sid_write_false: bool,
    infestation_column: Option<String>,
    infestation_write_false: bool,
    lost_scrap_column: Option<String>,
    takeoff_time_column: Option<String>,
    turret_column: Option<String>,
    landmine_column: Option<String>,
    spiketrap_column: Option<String>,
    app_less_column: Option<String>,
    death_columns: Vec<String>,
    player_name_columns: Vec<String>,
    player_name_row: usize,
    alive_state: String,
    dead_state: String,
    missing_state: String,
    disconnected_state: String,
    late_dead_state: String,
    death_notes_enabled: bool,
    player_names_as_notes: bool,
    death_enemy_notes_enabled: bool,
    enemy_write_false: bool,
    enemy_write_zero: bool,
    enemy_columns: Vec<EnemyColumnConfig>,
    fog_column: Option<String>,
    fog_write_false: bool,
    meteor_column: Option<String>,
    meteor_write_false: bool,
    gifts_column: Option<String>,
    gift_boxes_net_only: bool,
}

impl ResolvedCustomLayout {
    fn from_settings(settings: &CustomLcStatsLayoutSettings) -> Self {
        let moon_column = normalize_optional_column(&settings.moon_column);
        let collected_column = normalize_optional_column(&settings.collected_column);
        let available_column = normalize_optional_column(&settings.available_column);
        let quota_column = normalize_optional_column(&settings.quota_column);
        let check_column = normalize_optional_column(&settings.check_column)
            .or_else(|| moon_column.clone())
            .or_else(|| collected_column.clone())
            .or_else(|| available_column.clone())
            .or_else(|| quota_column.clone())
            .unwrap_or_else(|| "A".to_string());
        Self {
            start_row: settings.start_row.max(1),
            check_column,
            text_case: normalize_text_case(&settings.text_case),
            time_format: normalize_time_format(&settings.time_format),
            quota_column,
            seed_column: normalize_optional_column(&settings.seed_column),
            moon_column,
            weather_column: normalize_optional_column(&settings.weather_column),
            layout_column: normalize_optional_column(&settings.layout_column),
            item_count_column: normalize_optional_column(&settings.item_count_column),
            apparatus_column: normalize_optional_column(&settings.apparatus_column),
            bee_amount_column: normalize_optional_column(&settings.bee_amount_column),
            split_hive_count: settings.split_hive_count,
            beehive_collected_column: normalize_optional_column(&settings.beehive_collected_column),
            beehive_collected_value_column: normalize_optional_column(
                &settings.beehive_collected_value_column,
            ),
            beehive_collected_notes_enabled: settings.beehive_collected_notes_enabled,
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
            nut_notes_enabled: settings.nut_notes_enabled,
            butler_column: normalize_optional_column(&settings.butler_column),
            butler_collect_column: normalize_optional_column(&settings.butler_collect_column),
            butler_notes_enabled: settings.butler_notes_enabled,
            collected_column,
            available_column,
            real_available_column: normalize_optional_column(&settings.real_available_column),
            collected_no_extra_column: normalize_optional_column(
                &settings.collected_no_extra_column,
            ),
            missing_column: normalize_optional_column(&settings.missing_column),
            filter_collected_gift_scrap_from_missing: settings
                .filter_collected_gift_scrap_from_missing,
            outside_items_column: normalize_optional_column(&settings.outside_items_column),
            sold_column: normalize_optional_column(&settings.sold_column),
            sid_column: normalize_optional_column(&settings.sid_column),
            sid_item_column: normalize_optional_column(&settings.sid_item_column),
            sid_notes_enabled: settings.sid_notes_enabled,
            sid_write_false: settings.sid_write_false,
            infestation_column: normalize_optional_column(&settings.infestation_column),
            infestation_write_false: settings.infestation_write_false,
            lost_scrap_column: normalize_optional_column(&settings.lost_scrap_column),
            takeoff_time_column: normalize_optional_column(&settings.takeoff_time_column),
            turret_column: normalize_optional_column(&settings.turret_column),
            landmine_column: normalize_optional_column(&settings.landmine_column),
            spiketrap_column: normalize_optional_column(&settings.spiketrap_column),
            app_less_column: normalize_optional_column(&settings.app_less_column),
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
            late_dead_state: settings.late_dead_state.clone(),
            death_notes_enabled: settings.death_notes_enabled,
            player_names_as_notes: settings.player_names_as_notes,
            death_enemy_notes_enabled: settings.death_enemy_notes_enabled,
            enemy_write_false: settings.enemy_write_false,
            enemy_write_zero: settings.enemy_write_zero,
            enemy_columns: resolve_enemy_columns(settings),
            fog_column: normalize_optional_column(&settings.fog_column),
            fog_write_false: settings.fog_write_false,
            meteor_column: normalize_optional_column(&settings.meteor_column),
            meteor_write_false: settings.meteor_write_false,
            gifts_column: normalize_optional_column(&settings.gifts_column),
            gift_boxes_net_only: settings.gift_boxes_net_only,
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum EnemyColumnKind {
    Bool,
    Count,
}

#[derive(Debug, Clone, Copy)]
enum EnemyColumnSource {
    AllSpawns,
    NightTimeSpawns,
}

#[derive(Debug, Clone)]
struct EnemyColumnConfig {
    names: &'static [&'static str],
    column: Option<String>,
    kind: EnemyColumnKind,
    source: EnemyColumnSource,
}

fn resolve_enemy_columns(settings: &CustomLcStatsLayoutSettings) -> Vec<EnemyColumnConfig> {
    vec![
        enemy_column(&["Jester"], &settings.jester_column, EnemyColumnKind::Bool),
        enemy_column(
            &["Clay Surgeon", "ClaySurgeon"],
            &settings.barber_column,
            EnemyColumnKind::Bool,
        ),
        enemy_column(
            &["Bunker Spider", "SandSpider"],
            &settings.bunker_spider_column,
            EnemyColumnKind::Bool,
        ),
        enemy_column(
            &["Flowerman"],
            &settings.bracken_column,
            EnemyColumnKind::Bool,
        ),
        enemy_column(
            &["Cadaver Growths", "Cadaver Growth"],
            &settings.cadaver_column,
            EnemyColumnKind::Bool,
        ),
        enemy_column(
            &["Girl"],
            &settings.ghost_girl_column,
            EnemyColumnKind::Bool,
        ),
        enemy_column(
            &["Maneater", "CaveDweller"],
            &settings.maneater_column,
            EnemyColumnKind::Bool,
        ),
        enemy_column(
            &["Stingray"],
            &settings.backwater_gunkfish_column,
            EnemyColumnKind::Count,
        ),
        enemy_column(
            &["Spring"],
            &settings.coil_head_column,
            EnemyColumnKind::Count,
        ),
        enemy_column(
            &["Hoarding bug", "Hoarding Bug"],
            &settings.hoarding_bug_column,
            EnemyColumnKind::Count,
        ),
        enemy_column(
            &["MaskedPlayerEnemy", "Masked"],
            &settings.masked_column,
            EnemyColumnKind::Count,
        ),
        enemy_column(
            &["Centipede"],
            &settings.snare_flea_column,
            EnemyColumnKind::Count,
        ),
        enemy_column(
            &["Puffer"],
            &settings.spore_lizard_column,
            EnemyColumnKind::Count,
        ),
        enemy_column(
            &["Crawler"],
            &settings.thumper_column,
            EnemyColumnKind::Count,
        ),
        night_enemy_column(
            &["Earth Leviathan"],
            &settings.earth_leviathan_column,
            EnemyColumnKind::Count,
        ),
        night_enemy_column(
            &["ForestGiant"],
            &settings.forest_giant_column,
            EnemyColumnKind::Count,
        ),
        night_enemy_column(
            &["Baboon hawk"],
            &settings.baboon_hawk_column,
            EnemyColumnKind::Count,
        ),
        night_enemy_column(
            &["RadMech", "Old Bird"],
            &settings.old_bird_column,
            EnemyColumnKind::Count,
        ),
        night_enemy_column(
            &["Bush Wolf"],
            &settings.bush_wolf_column,
            EnemyColumnKind::Bool,
        ),
        night_enemy_column(
            &["Feiopar"],
            &settings.feiopar_column,
            EnemyColumnKind::Count,
        ),
        night_enemy_column(
            &["MouthDog", "Eyeless Dog"],
            &settings.eyeless_dog_column,
            EnemyColumnKind::Count,
        ),
    ]
}

fn enemy_column(
    names: &'static [&'static str],
    column: &str,
    kind: EnemyColumnKind,
) -> EnemyColumnConfig {
    enemy_column_with_source(names, column, kind, EnemyColumnSource::AllSpawns)
}

fn night_enemy_column(
    names: &'static [&'static str],
    column: &str,
    kind: EnemyColumnKind,
) -> EnemyColumnConfig {
    enemy_column_with_source(names, column, kind, EnemyColumnSource::NightTimeSpawns)
}

fn enemy_column_with_source(
    names: &'static [&'static str],
    column: &str,
    kind: EnemyColumnKind,
    source: EnemyColumnSource,
) -> EnemyColumnConfig {
    EnemyColumnConfig {
        names,
        column: normalize_optional_column(column),
        kind,
        source,
    }
}

async fn handle_economy_event(
    client: &reqwest::Client,
    token: &str,
    spreadsheet_id: &str,
    sheet_name: &str,
    layout: &ResolvedCustomLayout,
    row: usize,
    payload: &LcStats,
) -> Result<(), String> {
    let mut updates = vec![];
    let value_sold = payload.value_sold();
    let new_quota = payload.new_quota();

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
struct NormalizedEnemyValue {
    column: String,
    value: Value,
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
    app_less: Option<bool>,
    beehive_amount: String,
    beehive_value: String,
    beehive_collected: String,
    beehive_collected_value: NoteCell,
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
    outside_items: NoteCell,
    value_sold: i64,
    sid: NoteCell,
    infestation: NoteCell,
    lost_scrap: i64,
    takeoff_time: String,
    turret_count: i64,
    landmine_count: i64,
    spiketrap_count: i64,
    knife_note: Option<String>,
    shotgun_note: Option<String>,
    players: Vec<NormalizedPlayer>,
    fog: bool,
    meteor: NoteCell,
    gifts: NoteCell,
    enemy_values: Vec<NormalizedEnemyValue>,
}

impl NormalizedStats {
    fn from_stats(stats: &Value, payload: &LcStats, layout: &ResolvedCustomLayout) -> Self {
        let sid_type = non_false_text(payload.sid_type());
        let infestation_type = non_false_text(payload.infestation_type());
        let meteor_time =
            non_false_text(payload.meteor_shower_time()).map(|time| format_time(&time, layout));
        let interior = normalize_interior_name(&strip_apostrophe(&payload.dungeon_interior()));
        let apparatus_spawned = payload.app_spawned();
        Self {
            new_quota: payload.new_quota(),
            seed: strip_apostrophe(&payload.seed_text()),
            moon_name: strip_moon_number(&strip_apostrophe(&payload.moon_name())),
            weather: custom_weather(&payload.moon_weather()),
            interior: interior.clone(),
            item_count: payload.dungeon_item_count(),
            apparatus_spawned,
            app_less: interior
                .eq_ignore_ascii_case("Facility")
                .then_some(!apparatus_spawned),
            beehive_amount: beehive_amount(stats, layout.split_hive_count),
            beehive_value: beehive_value(stats),
            beehive_collected: beehive_collected(stats, payload),
            beehive_collected_value: collected_beehive_value_cell(stats),
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
            collected_total: payload.collected_total(),
            available_total: payload.initial_available_value(),
            real_available_total: payload.total_available_value(),
            collected_no_extra: payload.collected_no_extra(),
            missing: missing_items_cell(stats, layout.filter_collected_gift_scrap_from_missing),
            outside_items: outside_items_cell(stats, payload),
            value_sold: payload.value_sold(),
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
            takeoff_time: format_time(payload.take_off_time(), layout),
            turret_count: payload.turret_count(),
            landmine_count: payload.landmine_count(),
            spiketrap_count: payload.spiketrap_count(),
            knife_note: weapon_missed_note(stats, &["KnifeInfo"], "Knife"),
            shotgun_note: weapon_missed_note(stats, &["ShotgunInfo"], "Shotgun"),
            players: normalize_players(stats, layout),
            fog: payload.indoor_fog(),
            meteor: NoteCell {
                column: String::new(),
                value: json!(meteor_time.is_some()),
                note: meteor_time,
            },
            gifts: gifts_cell(stats, layout.gift_boxes_net_only),
            enemy_values: enemy_values(stats, layout),
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
    if let Some(app_less) = stats.app_less {
        push_value(&mut updates, &layout.app_less_column, row, json!(app_less));
    }
    push_hive_text_value(
        &mut updates,
        &layout.bee_amount_column,
        row,
        &stats.beehive_amount,
        layout.write_zero_for_missing_hives,
    );
    push_value(
        &mut updates,
        &layout.beehive_collected_column,
        row,
        blank_or_x(&stats.beehive_collected),
    );
    if layout.beehive_collected_value_column.is_some() && !layout.beehive_collected_notes_enabled {
        push_value(
            &mut updates,
            &layout.beehive_collected_value_column,
            row,
            stats.beehive_collected_value.value.clone(),
        );
    }
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
        json!(stats.takeoff_time),
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
    for enemy in &stats.enemy_values {
        updates.push((enemy.column.clone(), row, enemy.value.clone()));
    }

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
    if let Some(sid_item) = stats
        .sid
        .note
        .as_ref()
        .filter(|value| !value.trim().is_empty())
    {
        push_value(&mut updates, &layout.sid_item_column, row, json!(sid_item));
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
        if layout.player_names_as_notes {
            continue;
        }
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
    push_note_request(
        &mut requests,
        sheet_id,
        &layout.outside_items_column,
        row,
        &stats.outside_items,
    );
    if layout.nut_notes_enabled {
        push_note_request(
            &mut requests,
            sheet_id,
            &layout.nut_collect_column,
            row,
            &NoteCell {
                column: String::new(),
                value: json!(stats.nutcracker_collected),
                note: stats.shotgun_note.clone(),
            },
        );
    }
    if layout.butler_notes_enabled {
        push_note_request(
            &mut requests,
            sheet_id,
            &layout.butler_collect_column,
            row,
            &NoteCell {
                column: String::new(),
                value: json!(stats.butler_collected),
                note: stats.knife_note.clone(),
            },
        );
    }
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
    if layout.beehive_collected_notes_enabled {
        push_note_request(
            &mut requests,
            sheet_id,
            &layout.beehive_collected_value_column,
            row,
            &stats.beehive_collected_value,
        );
    }
    if stats.sid.value.as_bool().unwrap_or(false) || layout.sid_write_false {
        push_note_request(
            &mut requests,
            sheet_id,
            &layout.sid_column,
            row,
            &NoteCell {
                note: layout
                    .sid_notes_enabled
                    .then(|| stats.sid.note.clone())
                    .flatten(),
                ..stats.sid.clone()
            },
        );
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

    if layout.player_names_as_notes {
        for (index, player) in stats
            .players
            .iter()
            .take(layout.player_name_columns.len())
            .enumerate()
        {
            if !player.name.trim().is_empty() {
                requests.push(value_with_note_request(
                    sheet_id,
                    &NoteCell {
                        column: layout.player_name_columns[index].clone(),
                        value: json!(""),
                        note: Some(player.name.clone()),
                    },
                    layout.player_name_row,
                ));
            }
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

fn player_death_enemy_note(
    stats: &Value,
    death_time: &str,
    layout: &ResolvedCustomLayout,
) -> Option<String> {
    let sections = [
        (
            "Inside spawns before death:",
            spawns_before_death(stats, &["IndoorSpawns"], death_time, layout),
        ),
        (
            "Day outside spawns before death:",
            spawns_before_death(stats, &["DayTimeSpawns"], death_time, layout),
        ),
        (
            "Night outside spawns before death:",
            spawns_before_death(stats, &["NightTimeSpawns"], death_time, layout),
        ),
    ];
    let lines = sections
        .into_iter()
        .filter(|(_, spawns)| !spawns.is_empty())
        .flat_map(|(header, spawns)| {
            let mut lines = vec![header.to_string()];
            lines.extend(spawns);
            lines
        })
        .collect::<Vec<_>>();
    (!lines.is_empty()).then(|| lines.join("\n"))
}

fn spawns_before_death(
    stats: &Value,
    path: &[&str],
    death_time: &str,
    layout: &ResolvedCustomLayout,
) -> Vec<String> {
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
        .map(|spawn| format_spawn_note(spawn, layout))
        .collect()
}

fn format_spawn_note(spawn: &Value, layout: &ResolvedCustomLayout) -> String {
    let enemy = spawn
        .get("Enemy")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let spawn_time = format_time(
        spawn
            .get("SpawnTime")
            .and_then(Value::as_str)
            .unwrap_or_default(),
        layout,
    );
    let mut note = format!("{enemy} - {spawn_time}");
    if let Some(death_time) = spawn
        .get("TimeOfDeath")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
    {
        note.push_str(&format!(" / died {}", format_time(death_time, layout)));
    }
    note
}

fn parse_time_to_minutes(value: &str) -> Option<i64> {
    parse_lcstats_time_to_minutes(value)
}

fn normalize_players(stats: &Value, layout: &ResolvedCustomLayout) -> Vec<NormalizedPlayer> {
    players_at(stats)
        .into_iter()
        .map(|(_, player)| player)
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
            } else if is_late_death(&death_time) && !layout.late_dead_state.trim().is_empty() {
                layout.late_dead_state.as_str()
            } else {
                layout.dead_state.as_str()
            }
            .to_string();

            let note = if status == layout.missing_state {
                None
            } else {
                let mut sections = vec![];
                if layout.death_notes_enabled {
                    let mut parts = vec![];
                    if !death_time.is_empty() {
                        parts.push(format!(
                            "Time of Death: {}",
                            format_time(&death_time, layout)
                        ));
                    }
                    if !cause.is_empty() {
                        parts.push(format!("Cause of Death: {cause}"));
                    }
                    if !parts.is_empty() {
                        sections.push(parts.join("\n"));
                    }
                }
                if layout.death_enemy_notes_enabled {
                    if let Some(enemy_note) = player_death_enemy_note(stats, &death_time, layout) {
                        sections.push(enemy_note);
                    }
                }
                (!sections.is_empty()).then(|| sections.join("\n\n"))
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
    format!("{small}|{large}")
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

fn beehive_collected(stats: &Value, payload: &LcStats) -> String {
    let values = int_values_any(stats, &[&["BeeInfo", "Collected"][..]]);
    if values.is_empty() {
        return String::new();
    }
    if stats_version(payload) >= 70 {
        let small = values.iter().filter(|&&value| value < 100).count();
        let large = values.iter().filter(|&&value| value >= 100).count();
        format!("{small}|{large}")
    } else {
        values.len().to_string()
    }
}

fn collected_beehive_value_cell(stats: &Value) -> NoteCell {
    let mut values = int_values_any(stats, &[&["BeeInfo", "Collected"][..]]);
    values.sort_unstable();
    let total = values.iter().sum::<i64>();
    let value = if values.is_empty() {
        json!("X")
    } else {
        json!(total)
    };
    let note = (!values.is_empty()).then(|| egg_values_note("Collected bees", &values));
    NoteCell {
        column: String::new(),
        value,
        note,
    }
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

fn gifts_cell(stats: &Value, net_only: bool) -> NoteCell {
    let gifts = array_at_any(stats, &[&["GiftBoxesOpened"][..], &["GiftBoxes"][..]]);
    if net_only {
        return gift_net_cell(&gifts);
    }
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
        .map(|gift| gift_new_scrap_value(gift) - gift_scrap_value(gift))
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
                gift_new_scrap_value(gift),
                gift_scrap_value(gift),
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

fn gift_net_cell(gifts: &[Value]) -> NoteCell {
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
    let missed = gifts
        .iter()
        .filter(|gift| {
            !gift
                .get("Collected")
                .and_then(Value::as_bool)
                .unwrap_or(false)
        })
        .collect::<Vec<_>>();
    let net = collected
        .iter()
        .map(|gift| gift_new_scrap_value(gift) - gift_scrap_value(gift))
        .sum::<i64>();
    let sign = if net >= 0 { "+" } else { "" };
    let value = if collected.is_empty() {
        json!("X")
    } else {
        json!(format!("{sign}{net}"))
    };
    let note = (!missed.is_empty()).then(|| {
        missed
            .iter()
            .enumerate()
            .map(|(index, gift)| {
                format!(
                    "Gift {}: Box: {} ; Item: {}",
                    index + 1,
                    gift_scrap_value(gift),
                    gift_new_scrap_value(gift)
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    });
    NoteCell {
        column: String::new(),
        value,
        note,
    }
}

fn gift_new_scrap_value(gift: &Value) -> i64 {
    value_at_any(gift, &[&["NewScrapValue"][..], &["GiftValue"][..]])
        .map(intish_value)
        .unwrap_or(0)
}

fn gift_scrap_value(gift: &Value) -> i64 {
    value_at_any(gift, &[&["GiftScrapValue"][..], &["ScrapValue"][..]])
        .map(intish_value)
        .unwrap_or(0)
}

fn missing_items_cell(stats: &Value, filter_collected_gift_scrap: bool) -> NoteCell {
    let collected_gift_values = if filter_collected_gift_scrap {
        collected_gift_scrap_values(stats)
    } else {
        BTreeSet::new()
    };
    let missing = array_at(stats, &["MissedItems"])
        .iter()
        .filter(|item| {
            !item
                .get("CollectedOnPreviousDay")
                .and_then(Value::as_bool)
                .unwrap_or(false)
        })
        .filter(|item| {
            let gift_value = item
                .get("ScrapInsideGiftValue")
                .map(intish_value)
                .unwrap_or(0);
            gift_value == 0 || !collected_gift_values.contains(&gift_value)
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

fn collected_gift_scrap_values(stats: &Value) -> BTreeSet<i64> {
    array_at_any(stats, &[&["GiftBoxesOpened"][..], &["GiftBoxes"][..]])
        .iter()
        .filter(|gift| {
            gift.get("Collected")
                .and_then(Value::as_bool)
                .unwrap_or(false)
        })
        .map(gift_new_scrap_value)
        .collect()
}

fn outside_items_cell(stats: &Value, payload: &LcStats) -> NoteCell {
    let bee_available = int_values_any(
        stats,
        &[&["BeeInfo", "Available"][..], &["BeeInfo", "Values"][..]],
    );
    let bee_collected = int_values_any(stats, &[&["BeeInfo", "Collected"][..]]);
    let egg_available = int_values_any(
        stats,
        &[
            &["EggInfo", "Available"][..],
            &["BirdInfo", "EggValues"][..],
        ],
    );
    let egg_collected = int_values_any(
        stats,
        &[
            &["EggInfo", "Collected"][..],
            &["BirdInfo", "CollectedEggValues"][..],
        ],
    );

    let total = bee_collected.iter().sum::<i64>() + egg_collected.iter().sum::<i64>();
    let bee_missed_small = bee_available.iter().filter(|&&value| value < 100).count() as i64
        - bee_collected.iter().filter(|&&value| value < 100).count() as i64;
    let bee_missed_large = bee_available.iter().filter(|&&value| value >= 100).count() as i64
        - bee_collected.iter().filter(|&&value| value >= 100).count() as i64;
    let bee_missed_total = bee_available.len() as i64 - bee_collected.len() as i64;

    let mut remaining_eggs = egg_available;
    remaining_eggs.sort_unstable();
    let mut collected_eggs = egg_collected;
    collected_eggs.sort_unstable();
    for value in collected_eggs {
        if let Some(index) = remaining_eggs.iter().position(|egg| *egg == value) {
            remaining_eggs.remove(index);
        }
    }

    let mut note_parts = vec![];
    if stats_version(payload) >= 70 {
        if bee_missed_small > 0 || bee_missed_large > 0 {
            note_parts.push(format!("Bee ({bee_missed_small}|{bee_missed_large})"));
        }
    } else if bee_missed_total > 0 {
        note_parts.push(format!("Bee ({bee_missed_total})"));
    }
    if !remaining_eggs.is_empty() {
        note_parts.push(format!(
            "Egg ({})",
            remaining_eggs
                .iter()
                .map(|value| value.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }

    NoteCell {
        column: String::new(),
        value: if total > 0 { json!(total) } else { json!("X") },
        note: (!note_parts.is_empty()).then(|| format!("Missing: {}", note_parts.join(" "))),
    }
}

fn weapon_missed_note(stats: &Value, path: &[&str], label: &str) -> Option<String> {
    let collected_path = [path, &["Collected"][..]].concat();
    let available_path = [path, &["Available"][..]].concat();
    let collected = int_values_at(stats, &collected_path);
    let available = int_values_at(stats, &available_path);
    let missed = available
        .iter()
        .skip(collected.len())
        .copied()
        .collect::<Vec<_>>();
    (!missed.is_empty()).then(|| {
        missed
            .iter()
            .map(|value| format!("{label}: {value}"))
            .collect::<Vec<_>>()
            .join(" ; ")
    })
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

fn enemy_values(stats: &Value, layout: &ResolvedCustomLayout) -> Vec<NormalizedEnemyValue> {
    layout
        .enemy_columns
        .iter()
        .filter_map(|enemy| {
            let column = enemy.column.as_ref()?;
            let count = enemy_spawn_count(stats, enemy.names, enemy.source);
            let value = match enemy.kind {
                EnemyColumnKind::Bool if count > 0 => Some(json!(true)),
                EnemyColumnKind::Bool if layout.enemy_write_false => Some(json!(false)),
                EnemyColumnKind::Bool => None,
                EnemyColumnKind::Count if count > 0 => Some(json!(count as i64)),
                EnemyColumnKind::Count if layout.enemy_write_zero => Some(json!(0)),
                EnemyColumnKind::Count => None,
            }?;
            Some(NormalizedEnemyValue {
                column: column.clone(),
                value,
            })
        })
        .collect()
}

fn enemy_spawn_count(stats: &Value, enemy_names: &[&str], source: EnemyColumnSource) -> usize {
    let groups: &[&str] = match source {
        EnemyColumnSource::AllSpawns => &["IndoorSpawns", "DayTimeSpawns", "NightTimeSpawns"],
        EnemyColumnSource::NightTimeSpawns => &["NightTimeSpawns"],
    };

    groups
        .iter()
        .map(|path| {
            array_at(stats, &[*path])
                .iter()
                .filter(|spawn| {
                    spawn
                        .get("Enemy")
                        .and_then(Value::as_str)
                        .map(|value| {
                            enemy_names
                                .iter()
                                .any(|enemy| value.eq_ignore_ascii_case(enemy))
                        })
                        .unwrap_or(false)
                })
                .count()
        })
        .sum()
}

fn is_economy_moon(payload: &LcStats) -> bool {
    is_gordion_moon_name(&payload.moon_name())
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

fn normalize_time_format(value: &str) -> String {
    let value = value.trim();
    let compact = value
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect::<String>()
        .to_ascii_lowercase();
    match value {
        "12-hour" | "12-hour compact" | "24-hour" => value.to_string(),
        "7:40 AM" => "12-hour".to_string(),
        "19:40" => "24-hour".to_string(),
        _ if compact == "7:40am" => "12-hour compact".to_string(),
        _ => "12-hour".to_string(),
    }
}

fn format_time(value: &str, layout: &ResolvedCustomLayout) -> String {
    format_time_value(value, &layout.time_format)
}

fn format_time_value(value: &str, time_format: &str) -> String {
    let original = strip_apostrophe(value).trim().to_string();
    match time_format {
        "12-hour" => parse_lcstats_time_to_minutes(value)
            .map(|minutes| format_minutes_12_hour(minutes, true))
            .unwrap_or(original),
        "12-hour compact" => parse_lcstats_time_to_minutes(value)
            .map(|minutes| format_minutes_12_hour(minutes, false))
            .unwrap_or(original),
        "24-hour" => parse_lcstats_time_to_minutes(value)
            .map(format_minutes_24_hour)
            .unwrap_or(original),
        _ => parse_lcstats_time_to_minutes(value)
            .map(|minutes| format_minutes_12_hour(minutes, true))
            .unwrap_or(original),
    }
}

fn format_minutes_12_hour(minutes: i64, spaced: bool) -> String {
    let minutes = minutes.rem_euclid(24 * 60);
    let hour_24 = minutes / 60;
    let minute = minutes % 60;
    let period = if hour_24 < 12 { "AM" } else { "PM" };
    let mut hour = hour_24 % 12;
    if hour == 0 {
        hour = 12;
    }
    if spaced {
        format!("{hour}:{minute:02} {period}")
    } else {
        format!("{hour}:{minute:02}{period}")
    }
}

fn format_minutes_24_hour(minutes: i64) -> String {
    let minutes = minutes.rem_euclid(24 * 60);
    format!("{:02}:{:02}", minutes / 60, minutes % 60)
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

fn int_values_at(stats: &Value, path: &[&str]) -> Vec<i64> {
    array_at(stats, path).iter().map(intish_value).collect()
}

fn int_values_any(stats: &Value, paths: &[&[&str]]) -> Vec<i64> {
    array_at_any(stats, paths)
        .iter()
        .map(intish_value)
        .collect()
}

fn stats_version(payload: &LcStats) -> i64 {
    payload.version()
}

fn intish_at(stats: &Value, path: &[&str]) -> i64 {
    value_at(stats, path).map(intish_value).unwrap_or(0)
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

fn is_late_death(value: &str) -> bool {
    let mut parts = value.split(':');
    let Some(hour) = parts
        .next()
        .and_then(|part| part.trim().parse::<i64>().ok())
    else {
        return false;
    };
    let Some(minute) = parts.next().and_then(|part| {
        let digits = part
            .trim()
            .chars()
            .take_while(|ch| ch.is_ascii_digit())
            .collect::<String>();
        digits.parse::<i64>().ok()
    }) else {
        return false;
    };
    (hour == 22 && minute >= 45) || hour >= 23
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
            "Version": 70,
            "MoonInfo": { "Name": "'68 Artifice", "Weather": "'Mild" },
            "DungeonInfo": { "Interior": "'MineshaftFlow", "ItemCount": "'34" },
            "BeeInfo": { "Available": [64, 132], "Collected": [64] },
            "EggInfo": { "Available": [18, 12] },
            "IndoorSpawns": [{ "Enemy": "Nutcracker" }],
            "CollectedTotal": "'926",
            "InitialAvailableValue": "'2133",
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
            beehive_collected_column: "AQ".to_string(),
            beehive_collected_value_column: "AV".to_string(),
            beehive_collected_notes_enabled: false,
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
            nut_notes_enabled: true,
            butler_column: "N".to_string(),
            butler_collect_column: "AH".to_string(),
            butler_notes_enabled: true,
            collected_column: "O".to_string(),
            available_column: "P".to_string(),
            real_available_column: "AK".to_string(),
            collected_no_extra_column: "AL".to_string(),
            missing_column: "Q".to_string(),
            filter_collected_gift_scrap_from_missing: true,
            outside_items_column: "AR".to_string(),
            sold_column: "R".to_string(),
            sid_column: "S".to_string(),
            sid_item_column: "BD".to_string(),
            sid_notes_enabled: true,
            sid_write_false: true,
            infestation_column: "T".to_string(),
            infestation_write_false: true,
            lost_scrap_column: "AA".to_string(),
            takeoff_time_column: "AM".to_string(),
            turret_column: "AN".to_string(),
            landmine_column: "AO".to_string(),
            spiketrap_column: "AP".to_string(),
            app_less_column: "AU".to_string(),
            death_columns: "U,V,W,X".to_string(),
            player_name_columns: "AB,AC,AD,AE".to_string(),
            player_name_row: 56,
            alive_state: "A".to_string(),
            dead_state: "D".to_string(),
            missing_state: "M".to_string(),
            disconnected_state: "DC".to_string(),
            late_dead_state: "SX".to_string(),
            death_notes_enabled: false,
            player_names_as_notes: false,
            fog_column: "Y".to_string(),
            fog_write_false: true,
            meteor_column: "Z".to_string(),
            meteor_write_false: true,
            gifts_column: "AB".to_string(),
            gift_boxes_net_only: false,
            ..Default::default()
        });
        let normalized = normalized_stats(&stats, &layout);

        let updates = build_value_updates(&normalized, &layout, 7);

        assert_eq!(cell_value(&updates, "C"), Some(&json!(900)));
        assert_eq!(cell_value(&updates, "G"), Some(&json!("ARTIFICE")));
        assert_eq!(cell_value(&updates, "H"), Some(&json!("CLEAR")));
        assert_eq!(cell_value(&updates, "I"), Some(&json!("MINESHAFT")));
        assert_eq!(cell_value(&updates, "J"), Some(&json!(34)));
        assert_eq!(cell_value(&updates, "K"), Some(&json!("1|1")));
        assert_eq!(cell_value(&updates, "AD"), Some(&json!(64)));
        assert_eq!(cell_value(&updates, "AE"), Some(&json!(132)));
        assert_eq!(cell_value(&updates, "AV"), Some(&json!(64)));
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
        assert_eq!(cell_value(&updates, "AQ"), Some(&json!("1|0")));
        assert_eq!(cell_value(&updates, "AU"), None);
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
    fn numeric_seed_is_written_in_custom_layout() {
        let stats = json!({ "Seed": 10183014 });
        let layout = ResolvedCustomLayout::from_settings(&CustomLcStatsLayoutSettings {
            seed_column: "AI".to_string(),
            ..Default::default()
        });
        let normalized = normalized_stats(&stats, &layout);

        let updates = build_value_updates(&normalized, &layout, 7);

        assert_eq!(cell_value(&updates, "AI"), Some(&json!("10183014")));
    }

    #[test]
    fn sid_item_column_writes_item_name_only_for_sid() {
        let layout = ResolvedCustomLayout::from_settings(&CustomLcStatsLayoutSettings {
            sid_item_column: "BA".to_string(),
            sid_notes_enabled: true,
            ..Default::default()
        });

        let stats = json!({ "SIDType": "'Cash register" });
        let normalized = normalized_stats(&stats, &layout);
        let updates = build_value_updates(&normalized, &layout, 7);
        assert_eq!(cell_value(&updates, "BA"), Some(&json!("Cash register")));
        assert_eq!(normalized.sid.note.as_deref(), Some("Cash register"));

        let sid_note_request = value_with_note_request(
            123,
            &NoteCell {
                column: "Y".to_string(),
                value: normalized.sid.value.clone(),
                note: normalized.sid.note.clone(),
            },
            7,
        );
        assert_eq!(
            sid_note_request["updateCells"]["rows"][0]["values"][0]["userEnteredValue"]
                ["boolValue"],
            json!(true)
        );
        assert_eq!(
            sid_note_request["updateCells"]["rows"][0]["values"][0]["note"],
            json!("Cash register")
        );

        let sid_without_note_request = value_with_note_request(
            123,
            &NoteCell {
                column: "Y".to_string(),
                value: normalized.sid.value.clone(),
                note: None,
            },
            7,
        );
        assert!(
            sid_without_note_request["updateCells"]["rows"][0]["values"][0]
                .get("note")
                .is_none()
        );

        let stats = json!({ "SIDType": "" });
        let normalized = normalized_stats(&stats, &layout);
        let updates = build_value_updates(&normalized, &layout, 7);
        assert_eq!(cell_value(&updates, "BA"), None);
    }

    #[test]
    fn enemy_group_writes_bool_and_count_columns() {
        let stats = json!({
            "IndoorSpawns": [
                { "Enemy": "Flowerman", "SpawnTime": "9:00 PM" },
                { "Enemy": "Spring", "SpawnTime": "9:10 PM" },
                { "Enemy": "Spring", "SpawnTime": "9:20 PM" }
            ],
            "DayTimeSpawns": [
                { "Enemy": "Stingray", "SpawnTime": "1:00 PM" }
            ],
            "NightTimeSpawns": [
                { "Enemy": "MaskedPlayerEnemy", "SpawnTime": "10:00 PM" }
            ]
        });
        let settings = CustomLcStatsLayoutSettings {
            jester_column: "BA".to_string(),
            bracken_column: "BB".to_string(),
            coil_head_column: "BC".to_string(),
            backwater_gunkfish_column: "BD".to_string(),
            masked_column: "BE".to_string(),
            snare_flea_column: "BF".to_string(),
            ..Default::default()
        };
        let layout = ResolvedCustomLayout::from_settings(&settings);
        let normalized = normalized_stats(&stats, &layout);
        let updates = build_value_updates(&normalized, &layout, 7);

        assert_eq!(cell_value(&updates, "BA"), None);
        assert_eq!(cell_value(&updates, "BB"), Some(&json!(true)));
        assert_eq!(cell_value(&updates, "BC"), Some(&json!(2)));
        assert_eq!(cell_value(&updates, "BD"), Some(&json!(1)));
        assert_eq!(cell_value(&updates, "BE"), Some(&json!(1)));
        assert_eq!(cell_value(&updates, "BF"), None);

        let layout = ResolvedCustomLayout::from_settings(&CustomLcStatsLayoutSettings {
            enemy_write_false: true,
            enemy_write_zero: true,
            ..settings
        });
        let normalized = normalized_stats(&stats, &layout);
        let updates = build_value_updates(&normalized, &layout, 7);

        assert_eq!(cell_value(&updates, "BA"), Some(&json!(false)));
        assert_eq!(cell_value(&updates, "BF"), Some(&json!(0)));
    }

    #[test]
    fn enemy_group_matches_lcstatstracker_spawn_names() {
        let stats = json!({
            "IndoorSpawns": [
                { "Enemy": "Clay Surgeon", "SpawnTime": "9:00 PM" },
                { "Enemy": "Bunker Spider", "SpawnTime": "9:05 PM" },
                { "Enemy": "Cadaver Growths", "SpawnTime": "9:10 PM" },
                { "Enemy": "Maneater", "SpawnTime": "9:15 PM" },
                { "Enemy": "Hoarding bug", "SpawnTime": "9:20 PM" }
            ]
        });
        let settings = CustomLcStatsLayoutSettings {
            barber_column: "BA".to_string(),
            bunker_spider_column: "BB".to_string(),
            cadaver_column: "BC".to_string(),
            maneater_column: "BD".to_string(),
            hoarding_bug_column: "BE".to_string(),
            ..Default::default()
        };
        let layout = ResolvedCustomLayout::from_settings(&settings);
        let normalized = normalized_stats(&stats, &layout);
        let updates = build_value_updates(&normalized, &layout, 7);

        assert_eq!(cell_value(&updates, "BA"), Some(&json!(true)));
        assert_eq!(cell_value(&updates, "BB"), Some(&json!(true)));
        assert_eq!(cell_value(&updates, "BC"), Some(&json!(true)));
        assert_eq!(cell_value(&updates, "BD"), Some(&json!(true)));
        assert_eq!(cell_value(&updates, "BE"), Some(&json!(1)));
    }

    #[test]
    fn outside_enemy_columns_only_use_night_spawns() {
        let stats = json!({
            "IndoorSpawns": [
                { "Enemy": "MouthDog", "SpawnTime": "9:00 PM" },
                { "Enemy": "Bush Wolf", "SpawnTime": "9:05 PM" }
            ],
            "DayTimeSpawns": [
                { "Enemy": "RadMech", "SpawnTime": "1:00 PM" }
            ],
            "NightTimeSpawns": [
                { "Enemy": "Earth Leviathan", "SpawnTime": "7:39 AM" },
                { "Enemy": "ForestGiant", "SpawnTime": "7:40 AM" },
                { "Enemy": "Baboon hawk", "SpawnTime": "7:41 AM" },
                { "Enemy": "RadMech", "SpawnTime": "7:42 AM" },
                { "Enemy": "Old Bird", "SpawnTime": "7:43 AM" },
                { "Enemy": "Bush Wolf", "SpawnTime": "7:44 AM" },
                { "Enemy": "Feiopar", "SpawnTime": "7:45 AM" },
                { "Enemy": "MouthDog", "SpawnTime": "7:46 AM" },
                { "Enemy": "Eyeless Dog", "SpawnTime": "7:47 AM" }
            ]
        });
        let settings = CustomLcStatsLayoutSettings {
            earth_leviathan_column: "BA".to_string(),
            forest_giant_column: "BB".to_string(),
            baboon_hawk_column: "BC".to_string(),
            old_bird_column: "BD".to_string(),
            bush_wolf_column: "BE".to_string(),
            feiopar_column: "BF".to_string(),
            eyeless_dog_column: "BG".to_string(),
            ..Default::default()
        };
        let layout = ResolvedCustomLayout::from_settings(&settings);
        let normalized = normalized_stats(&stats, &layout);
        let updates = build_value_updates(&normalized, &layout, 7);

        assert_eq!(cell_value(&updates, "BA"), Some(&json!(1)));
        assert_eq!(cell_value(&updates, "BB"), Some(&json!(1)));
        assert_eq!(cell_value(&updates, "BC"), Some(&json!(1)));
        assert_eq!(cell_value(&updates, "BD"), Some(&json!(2)));
        assert_eq!(cell_value(&updates, "BE"), Some(&json!(true)));
        assert_eq!(cell_value(&updates, "BF"), Some(&json!(1)));
        assert_eq!(cell_value(&updates, "BG"), Some(&json!(2)));
    }

    #[test]
    fn death_enemy_notes_append_after_death_reason() {
        let stats = json!({
            "IndoorSpawns": [
                { "Enemy": "Flowerman", "SpawnTime": "9:00 PM" },
                { "Enemy": "Spring", "SpawnTime": "11:00 PM" }
            ],
            "NightTimeSpawns": [
                { "Enemy": "MaskedPlayerEnemy", "SpawnTime": "9:30 PM", "TimeOfDeath": "9:45 PM" }
            ],
            "Players": {
                "1": {
                    "Name": "'Aureo",
                    "Alive": false,
                    "Disconnected": false,
                    "TimeOfDeath": "'10:00 PM",
                    "CauseOfDeath": "'Blunt force trauma"
                }
            }
        });
        let layout = ResolvedCustomLayout::from_settings(&CustomLcStatsLayoutSettings {
            death_enemy_notes_enabled: true,
            death_notes_enabled: true,
            ..Default::default()
        });
        let normalized = normalized_stats(&stats, &layout);
        let note = normalized
            .players
            .first()
            .and_then(|player| player.note.as_deref())
            .unwrap_or_default();

        assert!(note.contains("Cause of Death: Blunt force trauma"));
        assert!(note.contains("Inside spawns before death:\nFlowerman - 9:00 PM"));
        assert!(!note.contains("Spring - 11:00 PM"));
        assert!(note.contains(
            "Night outside spawns before death:\nMaskedPlayerEnemy - 9:30 PM / died 9:45 PM"
        ));
        assert!(note.find("Cause of Death") < note.find("Inside spawns before death"));
    }

    #[test]
    fn time_format_applies_to_custom_layout_time_values_and_notes() {
        let stats = json!({
            "EventInfo": {
                "TakeOffTime": "11:57PM",
                "MeteorShowerTime": "8:30PM"
            },
            "IndoorSpawns": [
                { "Enemy": "Flowerman", "SpawnTime": "9:30PM", "TimeOfDeath": "9:45PM" }
            ],
            "Players": {
                "1": {
                    "Name": "'Aureo",
                    "Alive": false,
                    "Disconnected": false,
                    "TimeOfDeath": "'10:00PM",
                    "CauseOfDeath": "'Blunt force trauma"
                }
            }
        });
        let layout = ResolvedCustomLayout::from_settings(&CustomLcStatsLayoutSettings {
            time_format: "19:40".to_string(),
            takeoff_time_column: "AM".to_string(),
            meteor_column: "AH".to_string(),
            death_enemy_notes_enabled: true,
            death_notes_enabled: true,
            ..Default::default()
        });
        let normalized = normalized_stats(&stats, &layout);
        let updates = build_value_updates(&normalized, &layout, 7);
        let note = normalized
            .players
            .first()
            .and_then(|player| player.note.as_deref())
            .unwrap_or_default();

        assert_eq!(cell_value(&updates, "AM"), Some(&json!("23:57")));
        assert_eq!(normalized.meteor.note.as_deref(), Some("20:30"));
        assert!(note.contains("Time of Death: 22:00"));
        assert!(note.contains("Flowerman - 21:30 / died 21:45"));
    }

    #[test]
    fn gift_boxes_opened_feed_custom_gift_value() {
        let stats = json!({
            "GiftBoxesOpened": [
                { "NewScrapValue": 39, "GiftScrapValue": 12, "Collected": false },
                { "NewScrapValue": 162, "GiftScrapValue": 26, "Collected": true }
            ]
        });
        let layout = ResolvedCustomLayout::from_settings(&CustomLcStatsLayoutSettings {
            gifts_column: "AB".to_string(),
            ..Default::default()
        });
        let normalized = normalized_stats(&stats, &layout);

        assert_eq!(normalized.gifts.value, json!("1|+136"));
        assert!(normalized
            .gifts
            .note
            .as_deref()
            .unwrap_or_default()
            .contains("NewScrapValue=162"));
    }

    #[test]
    fn python_layout_specific_fields_are_available() {
        let stats = json!({
            "Version": 70,
            "MoonInfo": { "Name": "'68 Artifice", "Weather": "'Mild" },
            "DungeonInfo": { "Interior": "'FacilityFlow", "ItemCount": "'34" },
            "AppSpawned": false,
            "BeeInfo": { "Available": [64, 132], "Collected": [64] },
            "EggInfo": { "Available": [12, 18, 30], "Collected": [12] },
            "KnifeInfo": { "Available": [21, 33], "Collected": [21] },
            "ShotgunInfo": { "Available": [40, 52], "Collected": [40] },
            "MissedItems": [
                {
                    "ItemType": "Gift scrap",
                    "Value": 91,
                    "ScrapInsideGiftValue": 111,
                    "CollectedOnPreviousDay": false
                },
                {
                    "ItemType": "Cash register",
                    "Value": 80,
                    "ScrapInsideGiftValue": 0,
                    "CollectedOnPreviousDay": false
                }
            ],
            "GiftBoxesOpened": [
                { "NewScrapValue": 111, "GiftScrapValue": 20, "Collected": true },
                { "NewScrapValue": 9, "GiftScrapValue": 30, "Collected": false }
            ],
            "Players": {
                "1": {
                    "Name": "'Aureo",
                    "Alive": false,
                    "Disconnected": false,
                    "TimeOfDeath": "'23:00",
                    "CauseOfDeath": "'Blunt force trauma"
                }
            }
        });
        let layout = ResolvedCustomLayout::from_settings(&CustomLcStatsLayoutSettings {
            beehive_collected_column: "AV".to_string(),
            beehive_collected_value_column: "BC".to_string(),
            beehive_collected_notes_enabled: false,
            outside_items_column: "AW".to_string(),
            nut_collect_column: "AX".to_string(),
            nut_notes_enabled: true,
            butler_collect_column: "AY".to_string(),
            butler_notes_enabled: true,
            app_less_column: "AZ".to_string(),
            death_columns: "BA".to_string(),
            gift_boxes_net_only: true,
            gifts_column: "BB".to_string(),
            ..Default::default()
        });
        let normalized = normalized_stats(&stats, &layout);
        let updates = build_value_updates(&normalized, &layout, 7);

        assert_eq!(cell_value(&updates, "AV"), Some(&json!("1|0")));
        assert_eq!(cell_value(&updates, "BC"), Some(&json!(64)));
        assert_eq!(
            normalized.beehive_collected_value.note.as_deref(),
            Some("Collected bees: 64")
        );
        assert_eq!(cell_value(&updates, "AZ"), Some(&json!(true)));
        assert_eq!(
            normalized
                .players
                .first()
                .map(|player| player.status.as_str()),
            Some("SX")
        );
        assert_eq!(normalized.outside_items.value, json!(76));
        assert_eq!(
            normalized.outside_items.note.as_deref(),
            Some("Missing: Bee (0|1) Egg (18, 30)")
        );
        assert_eq!(normalized.missing.value, json!("1"));
        assert_eq!(
            normalized.missing.note.as_deref(),
            Some("Cash register: 80")
        );
        assert_eq!(cell_value(&updates, "AX"), Some(&json!(1)));
        assert_eq!(cell_value(&updates, "AY"), Some(&json!(1)));
        assert_eq!(normalized.knife_note.as_deref(), Some("Knife: 33"));
        assert_eq!(normalized.shotgun_note.as_deref(), Some("Shotgun: 52"));
        let shotgun_note_request = value_with_note_request(
            123,
            &NoteCell {
                column: "AX".to_string(),
                value: json!(normalized.nutcracker_collected),
                note: normalized.shotgun_note.clone(),
            },
            7,
        );
        assert_eq!(
            shotgun_note_request["updateCells"]["rows"][0]["values"][0]["note"],
            json!("Shotgun: 52")
        );
        let knife_note_request = value_with_note_request(
            123,
            &NoteCell {
                column: "AY".to_string(),
                value: json!(normalized.butler_collected),
                note: normalized.knife_note.clone(),
            },
            7,
        );
        assert_eq!(
            knife_note_request["updateCells"]["rows"][0]["values"][0]["note"],
            json!("Knife: 33")
        );
        assert_eq!(normalized.gifts.value, json!("+91"));
        assert_eq!(
            normalized.gifts.note.as_deref(),
            Some("Gift 1: Box: 30 ; Item: 9")
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
        let normalized = normalized_stats(&stats, &layout);

        let updates = build_value_updates(&normalized, &layout, 7);

        assert_eq!(cell_value(&updates, "J"), None);
        assert_eq!(cell_value(&updates, "K"), None);
        assert_eq!(cell_value(&updates, "BA"), None);
        assert_eq!(cell_value(&updates, "BB"), None);

        let layout = ResolvedCustomLayout::from_settings(&CustomLcStatsLayoutSettings {
            write_zero_for_missing_hives: true,
            ..settings
        });
        let normalized = normalized_stats(&stats, &layout);
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
        let normalized = normalized_stats(&stats, &layout);
        let updates = build_value_updates(&normalized, &layout, 7);

        assert_eq!(cell_value(&updates, "J"), Some(&json!("5")));

        let layout = ResolvedCustomLayout::from_settings(&CustomLcStatsLayoutSettings {
            split_hive_count: true,
            ..settings
        });
        let normalized = normalized_stats(&stats, &layout);
        let updates = build_value_updates(&normalized, &layout, 7);

        assert_eq!(cell_value(&updates, "J"), Some(&json!("2|3")));
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

    fn normalized_stats(stats: &Value, layout: &ResolvedCustomLayout) -> NormalizedStats {
        let payload = lcstats(stats);
        NormalizedStats::from_stats(stats, &payload, layout)
    }
}

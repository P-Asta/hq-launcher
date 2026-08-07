use serde_json::{json, Value};

use crate::google_oauth::LcStatsSettings;
use crate::lcstats_autosheet::layouts::EVIE_AUTOSHEET_LAYOUT;
use crate::lcstats_autosheet::sheets::{
    batch_update_spreadsheet, batch_write_cells_user_entered, first_empty_row, get_sheet_id,
    number_value, quote_sheet_name, read_number, read_range, write_cells,
};
use crate::lcstats_autosheet::stats::{
    lcstats, parse_lcstats_time_to_minutes, strip_apostrophe, strip_moon_number, LcStats,
};

const QUOTA_COLUMN: &str = "C";
const START_STATS_COLUMN: &str = "H";
const SELL_COLUMN: &str = "U";
const START_PLAYERS_COLUMN: &str = "X";
const PLAYER_NAME_COLUMN: &str = "CA";
const VERSION_CELL: &str = "CA10";
const SALE_CELL: &str = "BN22";
const FURNITURE_CELL_START: &str = "BS13";
const FIRST_DATA_ROW: usize = 4;

pub async fn write(
    client: &reqwest::Client,
    token: &str,
    settings: &LcStatsSettings,
    stats: &Value,
) -> Result<Option<crate::lcstats_autosheet::layouts::WriteReceipt>, String> {
    if !settings.layout.eq_ignore_ascii_case(EVIE_AUTOSHEET_LAYOUT) {
        return Ok(None);
    }
    let spreadsheet_id = settings.spreadsheet_id.trim();
    let sheet_name = settings.active_sheet_name.trim();
    if spreadsheet_id.is_empty() || sheet_name.is_empty() {
        return Err("spreadsheet or sheet is not set".to_string());
    }

    let payload = lcstats(stats);
    write_shop_sales(client, token, spreadsheet_id, sheet_name, &payload).await?;
    write_furniture_state(client, token, spreadsheet_id, sheet_name, &payload).await?;

    if payload.is_quota_event() {
        write_new_quota(client, token, spreadsheet_id, sheet_name, &payload).await?;
        Ok(None)
    } else if !payload.has_dungeon_info() {
        if payload.value_sold() == 0 {
            Ok(None)
        } else {
            update_sold_this_quota(client, token, spreadsheet_id, sheet_name, &payload).await?;
            Ok(None)
        }
    } else {
        let receipt = write_new_day(client, token, spreadsheet_id, sheet_name, stats, &payload).await?;
        Ok(Some(receipt))
    }
}

async fn write_new_quota(
    client: &reqwest::Client,
    token: &str,
    spreadsheet_id: &str,
    sheet_name: &str,
    stats: &LcStats,
) -> Result<(), String> {
    let current_quota_row =
        first_empty_row(client, token, spreadsheet_id, sheet_name, QUOTA_COLUMN)
            .await?
            .saturating_sub(1)
            .max(1);
    let already_sold = read_number(
        client,
        token,
        spreadsheet_id,
        sheet_name,
        &format!("{SELL_COLUMN}{current_quota_row}"),
    )
    .await?;

    batch_write_cells_user_entered(
        client,
        token,
        spreadsheet_id,
        sheet_name,
        vec![
            (
                QUOTA_COLUMN.to_string(),
                current_quota_row + 3,
                json!(stats.new_quota()),
            ),
            (
                SELL_COLUMN.to_string(),
                current_quota_row,
                number_value(already_sold + stats.value_sold() as f64),
            ),
        ],
    )
    .await
}

async fn update_sold_this_quota(
    client: &reqwest::Client,
    token: &str,
    spreadsheet_id: &str,
    sheet_name: &str,
    stats: &LcStats,
) -> Result<(), String> {
    let current_sell_row = first_empty_row(client, token, spreadsheet_id, sheet_name, SELL_COLUMN)
        .await?
        .saturating_sub(1);
    if current_sell_row <= 1 {
        return write_cells(
            client,
            token,
            spreadsheet_id,
            sheet_name,
            &format!("{SELL_COLUMN}{FIRST_DATA_ROW}"),
            vec![vec![json!(stats.value_sold())]],
        )
        .await;
    }

    let already_sold = read_number(
        client,
        token,
        spreadsheet_id,
        sheet_name,
        &format!("{SELL_COLUMN}{current_sell_row}"),
    )
    .await?;
    write_cells(
        client,
        token,
        spreadsheet_id,
        sheet_name,
        &format!("{SELL_COLUMN}{current_sell_row}"),
        vec![vec![number_value(already_sold + stats.value_sold() as f64)]],
    )
    .await
}

async fn write_new_day(
    client: &reqwest::Client,
    token: &str,
    spreadsheet_id: &str,
    sheet_name: &str,
    raw_stats: &Value,
    stats: &LcStats,
) -> Result<crate::lcstats_autosheet::layouts::WriteReceipt, String> {
    let mut player_row = first_empty_row(
        client,
        token,
        spreadsheet_id,
        sheet_name,
        START_PLAYERS_COLUMN,
    )
    .await?;
    if player_row <= FIRST_DATA_ROW {
        write_initial_values(client, token, spreadsheet_id, sheet_name, raw_stats, stats).await?;
        player_row = FIRST_DATA_ROW;
    } else {
        backfill_missing_player_names(client, token, spreadsheet_id, sheet_name, stats).await?;
    }

    let stats_row = first_empty_row(
        client,
        token,
        spreadsheet_id,
        sheet_name,
        START_STATS_COLUMN,
    )
    .await?
    .max(FIRST_DATA_ROW);
    batch_write_cells_user_entered(
        client,
        token,
        spreadsheet_id,
        sheet_name,
        build_day_updates(stats, stats_row, player_row),
    )
    .await?;

    let sheet_id = get_sheet_id(client, token, spreadsheet_id, sheet_name).await?;
    batch_update_spreadsheet(
        client,
        token,
        spreadsheet_id,
        build_note_requests(sheet_id, stats, stats_row),
    )
    .await?;
    Ok(crate::lcstats_autosheet::layouts::WriteReceipt {
        row: stats_row,
        column: START_STATS_COLUMN.to_string(),
    })
}

async fn write_initial_values(
    client: &reqwest::Client,
    token: &str,
    spreadsheet_id: &str,
    sheet_name: &str,
    raw_stats: &Value,
    stats: &LcStats,
) -> Result<(), String> {
    let players = stats.players_sorted();
    let initials = players
        .iter()
        .map(|player| {
            json!(strip_apostrophe(&player.stats.name)
                .chars()
                .take(2)
                .collect::<String>())
        })
        .collect::<Vec<_>>();
    let names = players
        .iter()
        .map(|player| vec![json!(strip_apostrophe(&player.stats.name))])
        .collect::<Vec<_>>();
    let version = raw_stats
        .get("Version")
        .cloned()
        .unwrap_or_else(|| json!(stats.version()));

    write_cells(
        client,
        token,
        spreadsheet_id,
        sheet_name,
        &format!("{START_PLAYERS_COLUMN}3"),
        vec![initials],
    )
    .await?;
    write_cells(
        client,
        token,
        spreadsheet_id,
        sheet_name,
        &format!("{PLAYER_NAME_COLUMN}22"),
        names,
    )
    .await?;
    write_cells(
        client,
        token,
        spreadsheet_id,
        sheet_name,
        VERSION_CELL,
        vec![vec![version]],
    )
    .await
}

/// Backfills any blank player name cells (CA22+) using the current payload.
/// `write_initial_values` only runs on the very first day, so a name that was
/// empty/missing on day one (or a player who joined later) would otherwise stay
/// blank forever. This keeps the existing name cells stable while repairing gaps.
async fn backfill_missing_player_names(
    client: &reqwest::Client,
    token: &str,
    spreadsheet_id: &str,
    sheet_name: &str,
    stats: &LcStats,
) -> Result<(), String> {
    const PLAYER_NAME_START_ROW: usize = 22;
    const PLAYER_NAME_MAX: usize = 4;
    let players = stats.players_sorted();
    if players.is_empty() {
        return Ok(());
    }
    let name_range = format!(
        "{}!{PLAYER_NAME_COLUMN}{PLAYER_NAME_START_ROW}:{PLAYER_NAME_COLUMN}{}",
        quote_sheet_name(sheet_name),
        PLAYER_NAME_START_ROW + PLAYER_NAME_MAX - 1,
    );
    let existing = read_range(client, token, spreadsheet_id, &name_range).await?;
    let existing_row = existing
        .get("values")
        .and_then(Value::as_array)
        .and_then(|rows| rows.first())
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut updates = vec![];
    for (index, player) in players.iter().take(PLAYER_NAME_MAX).enumerate() {
        let current_name = existing_row
            .get(index)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .trim();
        if current_name.is_empty() {
            updates.push((
                format!("{PLAYER_NAME_COLUMN}{}", PLAYER_NAME_START_ROW + index),
                json!(strip_apostrophe(&player.stats.name)),
            ));
        }
    }
    for (cell, value) in updates {
        write_cells(
            client,
            token,
            spreadsheet_id,
            sheet_name,
            &cell,
            vec![vec![value]],
        )
        .await?;
    }
    Ok(())
}

async fn write_shop_sales(
    client: &reqwest::Client,
    token: &str,
    spreadsheet_id: &str,
    sheet_name: &str,
    stats: &LcStats,
) -> Result<(), String> {
    write_cells(
        client,
        token,
        spreadsheet_id,
        sheet_name,
        SALE_CELL,
        shop_sales_rows(stats),
    )
    .await
}

fn shop_sales_rows(stats: &LcStats) -> Vec<Vec<Value>> {
    stats
        .shop_sales_in_source_order()
        .into_iter()
        .flat_map(|(_, value)| [vec![json!(value)], vec![Value::Null]])
        .collect()
}

async fn write_furniture_state(
    client: &reqwest::Client,
    token: &str,
    spreadsheet_id: &str,
    sheet_name: &str,
    stats: &LcStats,
) -> Result<(), String> {
    write_cells(
        client,
        token,
        spreadsheet_id,
        sheet_name,
        FURNITURE_CELL_START,
        furniture_state_rows(stats),
    )
    .await
}

fn furniture_state_rows(stats: &LcStats) -> Vec<Vec<Value>> {
    stats
        .furniture_info_in_source_order()
        .into_iter()
        .flat_map(|(name, furniture)| {
            [
                vec![
                    json!(name),
                    number_value(furniture.luck),
                    json!(furniture.real_price),
                    Value::Null,
                    json!(furniture.owned && !furniture.stored),
                    json!(furniture.in_stock),
                ],
                vec![Value::Null; 6],
            ]
        })
        .collect()
}

fn build_day_updates(
    stats: &LcStats,
    stats_row: usize,
    player_row: usize,
) -> Vec<(String, usize, Value)> {
    let mut updates = process_stats_cells(stats)
        .into_iter()
        .enumerate()
        .filter_map(|(offset, value)| {
            value.map(|value| (index_to_column(7 + offset), stats_row, value))
        })
        .collect::<Vec<_>>();

    for (offset, player) in stats.players_sorted().iter().enumerate() {
        updates.push((
            index_to_column(23 + offset),
            player_row,
            json!(player_status(stats, &player.stats)),
        ));
    }
    updates
}

fn process_stats_cells(stats: &LcStats) -> Vec<Option<Value>> {
    let bee_collected = stats.bee_collected_values();
    let bee_available = stats.bee_available_values();
    let egg_collected = stats.egg_collected_values();
    let egg_available = stats.egg_available_values();
    let shotgun_collected = stats.shotgun_collected_values();
    let shotgun_available = stats.shotgun_available_values();
    let knife_collected = stats.knife_collected_values();
    let knife_available = stats.knife_available_values();
    let gifts = stats.gift_boxes();
    let sid = strip_apostrophe(stats.sid_type());

    vec![
        Some(json!(strip_moon_number(&strip_apostrophe(
            &stats.moon_name()
        )))),
        Some(json!(strip_apostrophe(&stats.moon_weather()))),
        Some(json!(parse_interior(&stats.dungeon_interior()))),
        Some(json!(stats.dungeon_item_count())),
        Some(json!(stats.missed_item_count())),
        Some(json!(stats.collected_total())),
        Some(json!(stats.initial_available_value())),
        Some(json!(stats.total_available_value())),
        None,
        None,
        Some(json!(stats.lost_scrap_value())),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        Some(json!(stats.seed())),
        Some(json!(stats.app_spawned())),
        Some(json!(!sid.is_empty())),
        Some(json!(stats.indoor_fog())),
        Some(json!(strip_apostrophe(stats.infestation_type()))),
        Some(json!(bee_collected.len())),
        Some(json!(bee_collected.iter().sum::<i64>())),
        Some(json!(bee_available.len())),
        Some(json!(bee_available.iter().sum::<i64>())),
        Some(json!(egg_collected.len())),
        Some(json!(egg_collected.iter().sum::<i64>())),
        Some(json!(egg_available.len())),
        Some(json!(egg_available.iter().sum::<i64>())),
        Some(json!(shotgun_collected.len())),
        Some(json!(shotgun_collected.iter().sum::<i64>())),
        Some(json!(shotgun_available.len())),
        Some(json!(shotgun_available.iter().sum::<i64>())),
        Some(json!(knife_collected.len())),
        Some(json!(knife_collected.iter().sum::<i64>())),
        Some(json!(knife_available.len())),
        Some(json!(knife_available.iter().sum::<i64>())),
        Some(json!(stats.turret_count())),
        Some(json!(stats.landmine_count())),
        Some(json!(stats.spiketrap_count())),
        Some(json!(gifts.len())),
        Some(json!(gifts
            .iter()
            .map(|gift| gift.new_scrap_value - gift.gift_scrap_value)
            .sum::<i64>())),
        Some(json!(strip_apostrophe(stats.take_off_time()))),
    ]
}

fn build_note_requests(sheet_id: i64, stats: &LcStats, row: usize) -> Vec<Value> {
    let missed_note = stats
        .missed_items
        .iter()
        .map(|item| format!("{}: {}, ", item.item_type, item.value))
        .collect::<String>();
    let mut requests = vec![
        note_request(sheet_id, 11, row, &missed_note),
        note_request(sheet_id, 30, row, &strip_apostrophe(stats.sid_type())),
    ];

    for (offset, player) in stats.players_sorted().iter().enumerate() {
        if !player.stats.alive {
            requests.push(note_request(
                sheet_id,
                23 + offset,
                row,
                &format!(
                    "{} — {}",
                    strip_apostrophe(&player.stats.cause_of_death),
                    strip_apostrophe(&player.stats.time_of_death)
                ),
            ));
        }
    }
    requests
}

fn note_request(sheet_id: i64, column_index: usize, row: usize, note: &str) -> Value {
    let cell = if note.is_empty() {
        json!({})
    } else {
        json!({ "note": note })
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
            "rows": [{ "values": [cell] }],
            "fields": "note"
        }
    })
}

fn player_status(
    stats: &LcStats,
    player: &crate::lcstats_autosheet::stats::PlayerStats,
) -> &'static str {
    if player.alive {
        return if player.disconnected { "D" } else { "A" };
    }
    if strip_apostrophe(&player.cause_of_death) == "Abandoned" {
        return "M";
    }

    let death_time = parse_lcstats_time_to_minutes(&player.time_of_death).unwrap_or(0);
    let take_off_time = parse_lcstats_time_to_minutes(stats.take_off_time()).unwrap_or(0);
    if death_time + 120 < take_off_time {
        "X"
    } else {
        "S"
    }
}

fn parse_interior(value: &str) -> String {
    let mut interior = strip_apostrophe(value);
    for fragment in ["flow", "Flow", "FLOW", "interior", "Interior", "INTERIOR"] {
        interior = interior.replace(fragment, "");
    }
    match interior.as_str() {
        "AquaticDungeon" => "Citadel".to_string(),
        "Museum" => "Art Gallery".to_string(),
        "DeepSewers" => "Deep Sewers".to_string(),
        "ExpandedFacility" => "Exp Facil".to_string(),
        "Level3ButCool" => "Exp Mines".to_string(),
        "FracturedComplex" => "Complex".to_string(),
        "GrandArmory" => "Grand Armory".to_string(),
        "RubberRooms" => "Rubber Rooms".to_string(),
        "SpookyManor" => "Spooky Manor".to_string(),
        "Toystore" => "Toy Store".to_string(),
        _ => interior,
    }
}

fn index_to_column(mut index: usize) -> String {
    index += 1;
    let mut chars = vec![];
    while index > 0 {
        let offset = (index - 1) % 26;
        chars.push((b'A' + offset as u8) as char);
        index = (index - 1) / 26;
    }
    chars.iter().rev().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_evie_stats_and_players_to_fixed_columns() {
        let stats = lcstats(&json!({
            "Seed": 1234,
            "Version": 70,
            "MoonInfo": { "Name": "68 Artifice", "Weather": "Eclipsed" },
            "DungeonInfo": { "Interior": "AquaticDungeonFlow", "ItemCount": 42 },
            "PerformanceInfo": {
                "CollectedTotal": 700,
                "InitialAvailableValue": 900,
                "TotalAvailableValue": 1000
            },
            "EventInfo": {
                "SIDType": "Mineshaft",
                "IndoorFog": true,
                "InfestationType": "Spiders",
                "TakeOffTime": "8:00 PM"
            },
            "BeeInfo": { "Collected": [60], "Available": [60, 80] },
            "EggInfo": { "Collected": [20], "Available": [20, 30] },
            "ShotgunInfo": { "Collected": [40], "Available": [40, 50] },
            "KnifeInfo": { "Collected": [25], "Available": [25, 35] },
            "HazardInfo": { "TurretCount": 1, "LandmineCount": 2, "SpiketrapCount": 3 },
            "Players": {
                "1": { "Name": "Alpha", "Alive": true, "Disconnected": false },
                "2": {
                    "Name": "Beta",
                    "Alive": false,
                    "CauseOfDeath": "Forest Giant",
                    "TimeOfDeath": "5:00 PM"
                }
            }
        }));

        let updates = build_day_updates(&stats, 4, 4);

        assert_eq!(cell_value(&updates, "H"), Some(&json!("Artifice")));
        assert_eq!(cell_value(&updates, "J"), Some(&json!("Citadel")));
        assert_eq!(cell_value(&updates, "AC"), Some(&json!(1234)));
        assert_eq!(cell_value(&updates, "AE"), Some(&json!(true)));
        assert_eq!(cell_value(&updates, "AH"), Some(&json!(1)));
        assert_eq!(cell_value(&updates, "AK"), Some(&json!(140)));
        assert_eq!(cell_value(&updates, "AW"), Some(&json!(60)));
        assert_eq!(cell_value(&updates, "BC"), Some(&json!("8:00 PM")));
        assert_eq!(cell_value(&updates, "X"), Some(&json!("A")));
        assert_eq!(cell_value(&updates, "Y"), Some(&json!("X")));
        assert!(cell_value(&updates, "U").is_none());
    }

    #[test]
    fn notes_use_l_ae_and_player_columns() {
        let stats = lcstats(&json!({
            "EventInfo": { "SIDType": "Mineshaft" },
            "MissedItems": [{ "ItemType": "Engine", "Value": 50 }],
            "Players": {
                "1": {
                    "Alive": false,
                    "CauseOfDeath": "Forest Giant",
                    "TimeOfDeath": "5:00 PM"
                }
            }
        }));

        let requests = build_note_requests(99, &stats, 7);

        assert_eq!(
            requests[0]["updateCells"]["range"]["startColumnIndex"],
            json!(11)
        );
        assert_eq!(
            requests[1]["updateCells"]["range"]["startColumnIndex"],
            json!(30)
        );
        assert_eq!(
            requests[2]["updateCells"]["range"]["startColumnIndex"],
            json!(23)
        );
        assert_eq!(
            requests[2]["updateCells"]["range"]["startRowIndex"],
            json!(6)
        );
    }

    #[test]
    fn evie_interior_aliases_are_preserved() {
        assert_eq!(parse_interior("ExpandedFacilityInterior"), "Exp Facil");
        assert_eq!(parse_interior("SpookyManorFlow"), "Spooky Manor");
    }

    #[test]
    fn fixed_cells_and_ordered_shop_rows_match_evie_config() {
        let stats = lcstats(&json!({
            "ShopSales": {
                "Walkie-talkie": 20,
                "Flashlight": 30
            },
            "FurnitureInfo": {
                "Cozy lights": {
                    "Luck": 1.5,
                    "RealPrice": 100,
                    "Owned": true,
                    "Stored": false,
                    "InStock": true
                }
            }
        }));

        assert_eq!(QUOTA_COLUMN, "C");
        assert_eq!(START_STATS_COLUMN, "H");
        assert_eq!(SELL_COLUMN, "U");
        assert_eq!(START_PLAYERS_COLUMN, "X");
        assert_eq!(PLAYER_NAME_COLUMN, "CA");
        assert_eq!(VERSION_CELL, "CA10");
        assert_eq!(SALE_CELL, "BN22");
        assert_eq!(FURNITURE_CELL_START, "BS13");
        assert_eq!(
            shop_sales_rows(&stats),
            vec![
                vec![json!(20)],
                vec![Value::Null],
                vec![json!(30)],
                vec![Value::Null]
            ]
        );
        assert_eq!(
            furniture_state_rows(&stats),
            vec![
                vec![
                    json!("Cozy lights"),
                    json!(1.5),
                    json!(100),
                    Value::Null,
                    json!(true),
                    json!(true)
                ],
                vec![Value::Null; 6]
            ]
        );
    }

    fn cell_value<'a>(values: &'a [(String, usize, Value)], column: &str) -> Option<&'a Value> {
        values
            .iter()
            .find(|(value_column, _, _)| value_column == column)
            .map(|(_, _, value)| value)
    }
}

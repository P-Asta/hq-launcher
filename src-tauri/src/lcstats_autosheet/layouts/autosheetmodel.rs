use serde_json::{json, Value};

use crate::google_oauth::LcStatsSettings;
use crate::lcstats_autosheet::layouts::AUTOSHEETMODEL_LAYOUT;
use crate::lcstats_autosheet::sheets::{
    batch_write_cells_user_entered, first_empty_row, number_value, read_number, value_as_f64,
    write_cells,
};
use crate::lcstats_autosheet::stats::{
    array_at, array_at_any, bool_at, enemy_count, int_at, intish_value, missed_item_count,
    normalize_column, string_at, strip_moon_number, sum_array_any, value_at,
};

pub async fn write(
    client: &reqwest::Client,
    token: &str,
    settings: &LcStatsSettings,
    stats: &Value,
) -> Result<(), String> {
    if !settings.layout.eq_ignore_ascii_case(AUTOSHEETMODEL_LAYOUT) {
        return Ok(());
    }
    let spreadsheet_id = settings.spreadsheet_id.trim();
    let sheet_name = settings.active_sheet_name.trim();
    let start_column = normalize_column(&settings.start_column, "D");
    let quota_column = normalize_column(&settings.quota_column, "B");
    let sell_column = normalize_column(&settings.sell_column, "AE");
    if spreadsheet_id.is_empty() || sheet_name.is_empty() {
        return Err("spreadsheet or sheet is not set".to_string());
    }

    let row = process_stats(stats);
    match row.len() {
        1 => {
            let current_sell_count =
                first_empty_row(client, token, spreadsheet_id, sheet_name, &sell_column).await?;
            if current_sell_count == 1 {
                write_cells(
                    client,
                    token,
                    spreadsheet_id,
                    sheet_name,
                    &format!("{sell_column}2"),
                    vec![row],
                )
                .await?;
            } else {
                let target_row = current_sell_count + 2;
                let sell_amount = read_number(
                    client,
                    token,
                    spreadsheet_id,
                    sheet_name,
                    &format!("{sell_column}{target_row}"),
                )
                .await?;
                let value = value_as_f64(&row[0]) + sell_amount;
                write_cells(
                    client,
                    token,
                    spreadsheet_id,
                    sheet_name,
                    &format!("{sell_column}{target_row}"),
                    vec![vec![number_value(value)]],
                )
                .await?;
            }
        }
        2 => {
            let current_quota_count =
                first_empty_row(client, token, spreadsheet_id, sheet_name, &quota_column).await?;
            let sell_row = current_quota_count.saturating_sub(1).max(1);
            let sell_this_quota_amount = read_number(
                client,
                token,
                spreadsheet_id,
                sheet_name,
                &format!("{sell_column}{sell_row}"),
            )
            .await?;
            let value = value_as_f64(&row[1]) + sell_this_quota_amount;
            batch_write_cells_user_entered(
                client,
                token,
                spreadsheet_id,
                sheet_name,
                vec![
                    (sell_column.clone(), sell_row, number_value(value)),
                    (
                        quota_column.clone(),
                        current_quota_count + 2,
                        row[0].clone(),
                    ),
                ],
            )
            .await?;
        }
        _ => {
            let first_empty_row =
                first_empty_row(client, token, spreadsheet_id, sheet_name, &start_column).await?;
            write_cells(
                client,
                token,
                spreadsheet_id,
                sheet_name,
                &format!("{start_column}{first_empty_row}"),
                vec![row],
            )
            .await?;
        }
    }
    Ok(())
}

fn process_stats(stats: &Value) -> Vec<Value> {
    let new_quota = intish_at(stats, &["NewQuota"]);
    let value_sold = intish_at(stats, &["ValueSold"]);
    if new_quota != 0 {
        return vec![json!(new_quota), json!(value_sold)];
    }
    if !has_dungeon_info(stats) && value_sold != 0 {
        return vec![json!(value_sold)];
    }

    vec![
        json!(int_at(stats, &["Seed"])),
        json!(strip_moon_number(&string_at(stats, &["MoonInfo", "Name"]))),
        json!(string_at(stats, &["MoonInfo", "Weather"])),
        json!(string_at(stats, &["DungeonInfo", "Interior"])),
        json!(int_at(stats, &["DungeonInfo", "ItemCount"])),
        json!(missed_item_count(stats)),
        json!(bool_at(stats, &["AppSpawned"])),
        json!(array_at_any(
            stats,
            &[&["BeeInfo", "Available"][..], &["BeeInfo", "Values"][..],],
        )
        .len()),
        json!(sum_array_any(
            stats,
            &[&["BeeInfo", "Available"][..], &["BeeInfo", "Values"][..],],
        )),
        json!(sum_array_any(
            stats,
            &[
                &["EggInfo", "Available"][..],
                &["BirdInfo", "EggValues"][..],
            ],
        )),
        json!(enemy_count(stats, "Nutcracker")),
        json!(enemy_count(stats, "Butler")),
        json!(collected_count_or_legacy_int(
            stats,
            &["ShotgunInfo", "Collected"],
            &["ShotgunsCollected"],
        )),
        json!(collected_count_or_legacy_int(
            stats,
            &["KnifeInfo", "Collected"],
            &["KnivesCollected"],
        )),
        json!(int_at(stats, &["CollectedNoExtra"])),
        json!(int_at(stats, &["BottomLine"])),
        json!(int_at(stats, &["CollectedTotal"])),
        json!(int_at(stats, &["BottomLineTrue"])),
        json!(string_at(stats, &["TakeOffTime"])),
        json!(int_at(stats, &["HazardInfo", "TurretCount"])),
        json!(int_at(stats, &["HazardInfo", "LandmineCount"])),
        json!(int_at(stats, &["HazardInfo", "SpiketrapCount"])),
        json!(bool_at(stats, &["IndoorFog"])),
        json!(string_at(stats, &["SIDType"])),
        json!(string_at(stats, &["InfestationType"])),
        json!(string_at(stats, &["MeteorShowerTime"])),
        json!(lost_scrap(stats)),
    ]
}

fn collected_count_or_legacy_int(
    stats: &Value,
    collected_path: &[&str],
    legacy_path: &[&str],
) -> i64 {
    if let Some(collected) = value_at(stats, collected_path).and_then(Value::as_array) {
        collected.len() as i64
    } else {
        int_at(stats, legacy_path)
    }
}

fn intish_at(stats: &Value, path: &[&str]) -> i64 {
    value_at(stats, path).map(intish_value).unwrap_or(0)
}

fn has_dungeon_info(stats: &Value) -> bool {
    value_at(stats, &["DungeonInfo"])
        .map(|value| !value.is_null())
        .unwrap_or(false)
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn new_stat_arrays_feed_autosheet_model_values() {
        let stats = json!({
            "Seed": 10183014,
            "MoonInfo": { "Name": "68 Artifice", "Weather": "Eclipsed" },
            "DungeonInfo": { "Interior": "Mineshaft", "ItemCount": 34 },
            "BeeInfo": { "Available": [64, 88] },
            "EggInfo": { "Available": [12, 18] },
            "ShotgunInfo": { "Collected": [60] },
            "KnifeInfo": { "Collected": [35, 35] },
            "MissedItems": [
                { "Value": "'40", "CollectedOnPreviousDay": true }
            ]
        });

        let row = process_stats(&stats);

        assert_eq!(row[7], json!(2));
        assert_eq!(row[8], json!(152));
        assert_eq!(row[9], json!(30));
        assert_eq!(row[12], json!(1));
        assert_eq!(row[13], json!(2));
        assert_eq!(row[26], json!(40));
    }

    #[test]
    fn value_sold_with_day_stats_does_not_skip_day_row() {
        let stats = json!({
            "ValueSold": "'130",
            "MoonInfo": { "Name": "68 Artifice", "Weather": "Mild" },
            "DungeonInfo": { "Interior": "Mineshaft", "ItemCount": 34 }
        });

        let row = process_stats(&stats);

        assert!(row.len() > 2);
        assert_eq!(row[1], json!("Artifice"));
    }

    #[test]
    fn sell_only_payload_uses_string_number() {
        let stats = json!({
            "ValueSold": "'130",
            "DungeonInfo": null
        });

        assert_eq!(process_stats(&stats), vec![json!(130)]);
    }
}

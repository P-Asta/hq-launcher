use serde_json::{json, Value};

use crate::google_oauth::LcStatsSettings;
use crate::lcstats_autosheet::layouts::AUTOSHEETMODEL_LAYOUT;
use crate::lcstats_autosheet::sheets::{
    batch_write_cells_user_entered, first_empty_row, number_value, read_number, value_as_f64,
    write_cells,
};
use crate::lcstats_autosheet::stats::{lcstats, normalize_column, strip_moon_number};

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
    let lc_stats = lcstats(stats);
    let new_quota = lc_stats.new_quota();
    let value_sold = lc_stats.value_sold();
    if new_quota != 0 {
        return vec![json!(new_quota), json!(value_sold)];
    }
    if lc_stats.is_sell_event_without_day_stats() {
        return vec![json!(value_sold)];
    }

    vec![
        json!(lc_stats.seed()),
        json!(strip_moon_number(&lc_stats.moon_name())),
        json!(lc_stats.moon_weather()),
        json!(lc_stats.dungeon_interior()),
        json!(lc_stats.dungeon_item_count()),
        json!(lc_stats.missed_item_count()),
        json!(lc_stats.app_spawned()),
        json!(lc_stats.bee_available_count()),
        json!(lc_stats.bee_available_total()),
        json!(lc_stats.egg_available_total()),
        json!(lc_stats.indoor_enemy_count("Nutcracker")),
        json!(lc_stats.indoor_enemy_count("Butler")),
        json!(lc_stats.shotgun_collected_count()),
        json!(lc_stats.knife_collected_count()),
        json!(lc_stats.collected_no_extra()),
        json!(lc_stats.initial_available_value()),
        json!(lc_stats.collected_total()),
        json!(lc_stats.total_available_value()),
        json!(lc_stats.take_off_time()),
        json!(lc_stats.turret_count()),
        json!(lc_stats.landmine_count()),
        json!(lc_stats.spiketrap_count()),
        json!(lc_stats.indoor_fog()),
        json!(lc_stats.sid_type()),
        json!(lc_stats.infestation_type()),
        json!(lc_stats.meteor_shower_time()),
        json!(lc_stats.lost_scrap_value()),
    ]
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

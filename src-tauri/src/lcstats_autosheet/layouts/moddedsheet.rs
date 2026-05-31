use serde_json::{json, Value};

use crate::google_oauth::LcStatsSettings;
use crate::lcstats_autosheet::layouts::MODDEDSHEET_LAYOUT;
use crate::lcstats_autosheet::sheets::{
    batch_write_cells_user_entered, first_empty_row_from, number_value, read_number,
};
use crate::lcstats_autosheet::stats::{
    array_at, int_at, intish_value, lcstats_payload, string_at, strip_moon_number,
    total_available_value,
};

const QUOTA_COLUMN: &str = "B";
const MOON_COLUMN: &str = "H";
const WEATHER_COLUMN: &str = "I";
const INTERIOR_COLUMN: &str = "J";
const COLLECTED_COLUMN: &str = "K";
const BOTTOMLINE_COLUMN: &str = "L";
const SHIP_TOTAL_COLUMN: &str = "O";
const LOST_SOLD_COLUMN: &str = "P";
const START_ROW: usize = 2;

pub async fn write(
    client: &reqwest::Client,
    token: &str,
    settings: &LcStatsSettings,
    stats: &Value,
) -> Result<(), String> {
    if !settings.layout.eq_ignore_ascii_case(MODDEDSHEET_LAYOUT) {
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
        COLLECTED_COLUMN,
        START_ROW,
    )
    .await?;
    if is_economy_stats(stats) {
        return handle_economy(client, token, spreadsheet_id, sheet_name, row, stats).await;
    }
    let stats_kind = stats_kind(stats);
    if !stats_kind.has_day_stats() {
        return handle_economy(client, token, spreadsheet_id, sheet_name, row, stats).await;
    }
    let day_row = row;
    let economy_row = economy_row_for_stats(row, stats_kind);
    let mut values = build_values(stats, day_row, economy_row, stats_kind);
    add_sold_value(
        client,
        token,
        spreadsheet_id,
        sheet_name,
        stats,
        economy_row,
        &mut values,
    )
    .await?;
    batch_write_cells_user_entered(client, token, spreadsheet_id, sheet_name, values).await
}

async fn handle_economy(
    client: &reqwest::Client,
    token: &str,
    spreadsheet_id: &str,
    sheet_name: &str,
    target_row: usize,
    stats: &Value,
) -> Result<(), String> {
    let sell_row = target_row.saturating_sub(1).max(START_ROW);
    let mut updates = build_economy_values(stats, target_row, sell_row);
    add_sold_value(
        client,
        token,
        spreadsheet_id,
        sheet_name,
        stats,
        sell_row,
        &mut updates,
    )
    .await?;
    batch_write_cells_user_entered(client, token, spreadsheet_id, sheet_name, updates).await
}

fn build_economy_values(
    stats: &Value,
    quota_row: usize,
    sell_row: usize,
) -> Vec<(String, usize, Value)> {
    let new_quota = lcstats_payload(stats).new_quota();
    let mut updates = vec![];

    if new_quota != 0 {
        updates.push((QUOTA_COLUMN.to_string(), quota_row, json!(new_quota)));
    }
    let lost = lost_scrap(stats);
    if lost != 0 {
        updates.push((LOST_SOLD_COLUMN.to_string(), sell_row, json!(lost)));
    }
    updates
}

fn build_values(
    stats: &Value,
    day_row: usize,
    economy_row: usize,
    stats_kind: StatsKind,
) -> Vec<(String, usize, Value)> {
    let mut values = vec![];

    if stats_kind.has_day_stats() {
        values.extend([
            (
                MOON_COLUMN.to_string(),
                day_row,
                json!(strip_moon_number(&string_at(stats, &["MoonInfo", "Name"]))),
            ),
            (
                WEATHER_COLUMN.to_string(),
                day_row,
                json!(moddedsheet_weather(&string_at(
                    stats,
                    &["MoonInfo", "Weather"]
                ))),
            ),
            (
                INTERIOR_COLUMN.to_string(),
                day_row,
                json!(moddedsheet_interior(&string_at(
                    stats,
                    &["DungeonInfo", "Interior"]
                ))),
            ),
            (
                COLLECTED_COLUMN.to_string(),
                day_row,
                json!(int_at(stats, &["CollectedTotal"])),
            ),
            (
                BOTTOMLINE_COLUMN.to_string(),
                day_row,
                json!(total_available_value(stats)),
            ),
        ]);
    }

    let new_quota = lcstats_payload(stats).new_quota();
    if new_quota != 0 {
        values.push((QUOTA_COLUMN.to_string(), day_row, json!(new_quota)));
    }

    let lost = lost_scrap(stats);
    if lost != 0 {
        values.push((LOST_SOLD_COLUMN.to_string(), economy_row, json!(lost)));
    }

    values
}

async fn add_sold_value(
    client: &reqwest::Client,
    token: &str,
    spreadsheet_id: &str,
    sheet_name: &str,
    stats: &Value,
    row: usize,
    values: &mut Vec<(String, usize, Value)>,
) -> Result<(), String> {
    let payload = lcstats_payload(stats);
    let mut value_sold = payload.value_sold();
    if value_sold == 0 && payload.new_quota() != 0 {
        value_sold = read_number(
            client,
            token,
            spreadsheet_id,
            sheet_name,
            &format!("{SHIP_TOTAL_COLUMN}{row}"),
        )
        .await? as i64;
    }
    if value_sold == 0 {
        return Ok(());
    }
    values.retain(|(column, value_row, _)| column != LOST_SOLD_COLUMN || *value_row != row);
    let current_value = read_number(
        client,
        token,
        spreadsheet_id,
        sheet_name,
        &format!("{LOST_SOLD_COLUMN}{row}"),
    )
    .await?;
    values.push((
        LOST_SOLD_COLUMN.to_string(),
        row,
        number_value(current_value + value_sold as f64),
    ));
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StatsKind {
    Day,
    Economy,
}

impl StatsKind {
    fn has_day_stats(self) -> bool {
        matches!(self, Self::Day)
    }
}

fn stats_kind(stats: &Value) -> StatsKind {
    let moon = string_at(stats, &["MoonInfo", "Name"]);
    let collected = int_at(stats, &["CollectedTotal"]);
    let bottomline = total_available_value(stats);
    let interior = string_at(stats, &["DungeonInfo", "Interior"]);
    let weather = string_at(stats, &["MoonInfo", "Weather"]);
    if !moon.trim().is_empty()
        || !interior.trim().is_empty()
        || !weather.trim().is_empty()
        || collected != 0
        || bottomline != 0
    {
        StatsKind::Day
    } else {
        StatsKind::Economy
    }
}

fn is_economy_stats(stats: &Value) -> bool {
    let payload = lcstats_payload(stats);
    payload.is_gordion_moon() || payload.is_sell_or_quota_event()
}

fn economy_row_for_stats(first_empty_collected_row: usize, stats_kind: StatsKind) -> usize {
    if stats_kind.has_day_stats() {
        first_empty_collected_row
    } else {
        first_empty_collected_row.saturating_sub(1).max(START_ROW)
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

fn moddedsheet_weather(value: &str) -> String {
    if value.eq_ignore_ascii_case("Mild") {
        "Clear".to_string()
    } else {
        value.to_string()
    }
}

fn moddedsheet_interior(value: &str) -> String {
    match value.trim() {
        "ToystoreFlow" => "Toy Store".to_string(),
        "DeepSewersFlow" => "Sewers".to_string(),
        "GreenhouseFlow" => "Greenhouse".to_string(),
        "MuseumInteriorFlow" => "Gallery".to_string(),
        "ExpandedFacility" => "Exp. Fac".to_string(),
        "FracturedComplexFlow" => "Complex".to_string(),
        "SpookyManorFlow" => "Manor".to_string(),
        "RubberRoomsFlow" => "Rubbor".to_string(),
        "Level3ButCoolFlow" => "Exp. Mine".to_string(),
        _ => value.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lcstats_autosheet::stats::{is_gordion_stats, lcstats_payload};

    #[test]
    fn maps_moddedsheet_columns_from_row_two() {
        let stats = json!({
            "MoonInfo": {
                "Name": "71 March",
                "Weather": "Mild"
            },
            "DungeonInfo": {
                "Interior": "ToystoreFlow"
            },
            "CollectedTotal": 420,
            "TotalAvailableValue": 680,
            "MissedItems": [
                { "Value": "'20", "CollectedOnPreviousDay": true },
                { "Value": 30, "CollectedOnPreviousDay": false }
            ]
        });

        let values = build_values(&stats, START_ROW, START_ROW, stats_kind(&stats));

        assert_eq!(cell_value(&values, QUOTA_COLUMN), None);
        assert_eq!(cell_value(&values, MOON_COLUMN), Some(&json!("March")));
        assert_eq!(cell_value(&values, WEATHER_COLUMN), Some(&json!("Clear")));
        assert_eq!(
            cell_value(&values, INTERIOR_COLUMN),
            Some(&json!("Toy Store"))
        );
        assert_eq!(cell_value(&values, COLLECTED_COLUMN), Some(&json!(420)));
        assert_eq!(cell_value(&values, BOTTOMLINE_COLUMN), Some(&json!(680)));
        assert_eq!(cell_value(&values, LOST_SOLD_COLUMN), Some(&json!(20)));
        assert_eq!(cell_row(&values, COLLECTED_COLUMN), Some(2));
    }

    #[test]
    fn reads_sold_value_from_string_numbers() {
        let stats = json!({ "ValueSold": "'55" });

        assert_eq!(lcstats_payload(&stats).value_sold(), 55);
    }

    #[test]
    fn economy_events_target_previous_collected_row() {
        assert_eq!(economy_row_for_stats(7, StatsKind::Economy), 6);
        assert_eq!(
            economy_row_for_stats(START_ROW, StatsKind::Economy),
            START_ROW
        );
    }

    #[test]
    fn economy_events_do_not_write_blank_day_values() {
        let stats = json!({ "ValueSold": 55 });

        let values = build_values(&stats, 7, 6, stats_kind(&stats));

        assert_eq!(cell_value(&values, MOON_COLUMN), None);
        assert_eq!(cell_value(&values, COLLECTED_COLUMN), None);
    }

    #[test]
    fn sold_or_quota_payloads_are_economy_even_with_stale_day_fields() {
        let stats = json!({
            "NewQuota": "'196",
            "ValueSold": 130,
            "MoonInfo": {
                "Name": "71 Experimentation",
                "Weather": "Mild"
            },
            "DungeonInfo": {
                "Interior": "Factory"
            },
            "CollectedTotal": 0,
            "TotalAvailableValue": 446
        });

        let values = build_economy_values(&stats, 5, 4);

        assert!(is_economy_stats(&stats));
        assert_eq!(cell_value(&values, MOON_COLUMN), None);
        assert_eq!(cell_value(&values, COLLECTED_COLUMN), None);
        assert_eq!(cell_value(&values, QUOTA_COLUMN), Some(&json!(196)));
        assert_eq!(cell_row(&values, QUOTA_COLUMN), Some(5));
    }

    #[test]
    fn gordion_events_add_sold_and_write_next_quota_without_day_values() {
        let stats = json!({
            "NewQuota": 900,
            "ValueSold": 130,
            "MoonInfo": {
                "Name": "71 Gordion",
                "Weather": "Mild"
            },
            "CollectedTotal": 120
        });

        let values = build_economy_values(&stats, 8, 7);

        assert!(is_gordion_stats(&stats));
        assert_eq!(cell_value(&values, MOON_COLUMN), None);
        assert_eq!(cell_value(&values, WEATHER_COLUMN), None);
        assert_eq!(cell_value(&values, COLLECTED_COLUMN), None);
        assert_eq!(cell_value(&values, LOST_SOLD_COLUMN), None);
        assert_eq!(cell_value(&values, QUOTA_COLUMN), Some(&json!(900)));
        assert_eq!(cell_row(&values, QUOTA_COLUMN), Some(8));
    }

    #[test]
    fn galetry_events_are_economy_events() {
        let stats = json!({
            "MoonInfo": {
                "Name": "'Galetry"
            },
            "CollectedTotal": 120
        });

        assert!(is_gordion_stats(&stats));
        assert!(is_economy_stats(&stats));
    }

    #[test]
    fn economy_quota_and_lost_use_different_rows() {
        let stats = json!({
            "NewQuota": 900,
            "MissedItems": [
                { "Value": 40, "CollectedOnPreviousDay": true }
            ]
        });

        let values = build_values(&stats, 8, 7, stats_kind(&stats));

        assert_eq!(cell_value(&values, QUOTA_COLUMN), Some(&json!(900)));
        assert_eq!(cell_row(&values, QUOTA_COLUMN), Some(8));
        assert_eq!(cell_value(&values, LOST_SOLD_COLUMN), Some(&json!(40)));
        assert_eq!(cell_row(&values, LOST_SOLD_COLUMN), Some(7));
    }

    fn cell_value<'a>(values: &'a [(String, usize, Value)], column: &str) -> Option<&'a Value> {
        values
            .iter()
            .find(|(value_column, _, _)| value_column == column)
            .map(|(_, _, value)| value)
    }

    fn cell_row(values: &[(String, usize, Value)], column: &str) -> Option<usize> {
        values
            .iter()
            .find(|(value_column, _, _)| value_column == column)
            .map(|(_, row, _)| *row)
    }
}

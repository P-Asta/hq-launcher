use serde_json::{json, Value};

use crate::google_oauth::LcStatsSettings;
use crate::lcstats_autosheet::layouts::BREADSHEET_LAYOUT;
use crate::lcstats_autosheet::sheets::{
    batch_write_cells_user_entered, first_empty_row_from, number_value, read_number,
};
use crate::lcstats_autosheet::stats::{int_at, string_at, strip_moon_number};

const QUOTA_COLUMN: &str = "B";
const MOON_COLUMN: &str = "G";
const WEATHER_COLUMN: &str = "H";
const COLLECTED_COLUMN: &str = "I";
const SOLD_COLUMN: &str = "M";
const MOON_START_ROW: usize = 3;

pub async fn write(
    client: &reqwest::Client,
    token: &str,
    settings: &LcStatsSettings,
    stats: &Value,
) -> Result<(), String> {
    if !settings.layout.eq_ignore_ascii_case(BREADSHEET_LAYOUT) {
        return Ok(());
    }
    let spreadsheet_id = settings.spreadsheet_id.trim();
    let sheet_name = settings.active_sheet_name.trim();
    if spreadsheet_id.is_empty() || sheet_name.is_empty() {
        return Err("spreadsheet or sheet is not set".to_string());
    }

    let target_row = first_empty_row_from(
        client,
        token,
        spreadsheet_id,
        sheet_name,
        COLLECTED_COLUMN,
        MOON_START_ROW,
    )
    .await?;
    let summary_row = summary_row_for_day(target_row);
    let mut values = build_values(stats, target_row, summary_row);
    add_sold_value(
        client,
        token,
        spreadsheet_id,
        sheet_name,
        stats,
        summary_row,
        &mut values,
    )
    .await?;
    if values.is_empty() {
        return Ok(());
    }
    batch_write_cells_user_entered(client, token, spreadsheet_id, sheet_name, values).await
}

fn build_values(stats: &Value, moon_row: usize, summary_row: usize) -> Vec<(String, usize, Value)> {
    let mut values = vec![];
    let moon = strip_moon_number(&string_at(stats, &["MoonInfo", "Name"]));
    if !moon.trim().is_empty() {
        values.push((MOON_COLUMN.to_string(), moon_row, json!(moon)));
        values.push((
            WEATHER_COLUMN.to_string(),
            moon_row,
            json!(breadsheet_weather(&string_at(
                stats,
                &["MoonInfo", "Weather"]
            ))),
        ));
        values.push((
            COLLECTED_COLUMN.to_string(),
            moon_row,
            json!(int_at(stats, &["CollectedTotal"])),
        ));
    }

    let new_quota = int_at(stats, &["NewQuota"]);
    if new_quota != 0 {
        values.push((QUOTA_COLUMN.to_string(), summary_row, json!(new_quota)));
    }

    values
}

async fn add_sold_value(
    client: &reqwest::Client,
    token: &str,
    spreadsheet_id: &str,
    sheet_name: &str,
    stats: &Value,
    summary_row: usize,
    values: &mut Vec<(String, usize, Value)>,
) -> Result<(), String> {
    let value_sold = int_at(stats, &["ValueSold"]);
    if value_sold == 0 {
        return Ok(());
    }
    let current_value = read_number(
        client,
        token,
        spreadsheet_id,
        sheet_name,
        &format!("{SOLD_COLUMN}{summary_row}"),
    )
    .await?;
    values.push((
        SOLD_COLUMN.to_string(),
        summary_row,
        number_value(current_value + value_sold as f64),
    ));
    Ok(())
}

fn summary_row_for_day(day_row: usize) -> usize {
    let block_start = day_row.saturating_sub(MOON_START_ROW) / 3 * 3 + MOON_START_ROW;
    block_start + 1
}

fn breadsheet_weather(value: &str) -> String {
    if value.eq_ignore_ascii_case("Mild") {
        "Clear".to_string()
    } else {
        value.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_breadsheet_columns() {
        let stats = json!({
            "NewQuota": 130,
            "ValueSold": 55,
            "MoonInfo": {
                "Name": "71 March",
                "Weather": "Mild"
            },
            "CollectedTotal": 420
        });

        let values = build_values(&stats, 7, 5);

        assert_eq!(cell_value(&values, QUOTA_COLUMN), Some(&json!(130)));
        assert_eq!(cell_value(&values, MOON_COLUMN), Some(&json!("March")));
        assert_eq!(cell_value(&values, WEATHER_COLUMN), Some(&json!("Clear")));
        assert_eq!(cell_value(&values, COLLECTED_COLUMN), Some(&json!(420)));
        assert_eq!(cell_row(&values, MOON_COLUMN), Some(7));
        assert_eq!(cell_row(&values, QUOTA_COLUMN), Some(5));
        assert_eq!(cell_value(&values, SOLD_COLUMN), None);
    }

    #[test]
    fn summary_row_targets_middle_of_three_day_block() {
        assert_eq!(summary_row_for_day(3), 4);
        assert_eq!(summary_row_for_day(4), 4);
        assert_eq!(summary_row_for_day(5), 4);
        assert_eq!(summary_row_for_day(6), 7);
        assert_eq!(summary_row_for_day(9), 10);
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

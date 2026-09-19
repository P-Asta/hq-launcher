//! Layout-aware resets. Clear run data in one batch without creating backups.
use super::{layouts, sheets};
use crate::google_oauth::LcStatsSettings;
use serde_json::{json, Value};
use std::collections::BTreeSet;

fn col(text: &str) -> Result<usize, String> {
    if text.is_empty() || text.len() > 3 || !text.bytes().all(|b| b.is_ascii_alphabetic()) {
        return Err(format!("Invalid reset column: {text}"));
    }
    Ok(text.bytes().fold(0, |n, b| {
        n * 26 + (b.to_ascii_uppercase() - b'A' + 1) as usize
    }) - 1)
}

#[derive(Default)]
struct Plan {
    cells: BTreeSet<(usize, usize)>,
    notes: BTreeSet<(usize, usize)>,
}
impl Plan {
    fn range(&mut self, columns: &str, start: usize, end: usize) -> Result<(), String> {
        for part in columns.split(',').filter(|s| !s.is_empty()) {
            let (a, b) = part.split_once(':').unwrap_or((part, part));
            for c in col(a)?..=col(b)? {
                for r in start..=end {
                    self.cells.insert((r - 1, c));
                }
            }
        }
        Ok(())
    }
}

fn plan(settings: &LcStatsSettings, end: usize) -> Result<Plan, String> {
    let mut p = Plan::default();
    let name = settings.layout.to_ascii_lowercase();
    let (columns, start) = match name.as_str() {
        "wafrodyautosheet" => ("H:I,K:N,P,R:Y,AJ:AL,BF", 4),
        "breadsheet" => ("G:I,M", 3),
        "charlyautosheet" => ("F:Q,X:Z,AB:AI", 3),
        "evieautosheet" => ("H:O,R,U,X:AA,AC:BC", 4),
        "evilsheet" => ("G:M,R:S,U:X", 4),
        "makusheet 1.0" => ("F:L,R,U:Y", 3),
        "moddedsheet" => ("B,H:L,O:P", 2),
        "serenadesheet" => ("N:AJ,AT:AU,AW,BA:BD", 4),
        "autosheetmodel" => ("", 2),
        "custom layout" => ("", settings.custom_layout.start_row.max(1)),
        _ => return Err(format!("Unsupported reset layout: {}", settings.layout)),
    };
    if end < start || end > 10000 {
        return Err("Reset end row must be between the layout start row and 10000".into());
    }
    p.range(columns, start, end)?;
    match name.as_str() {
        "wafrodyautosheet" => {
            if end != 93 {
                return Err("Wafrody reset uses rows 4–93".into());
            }
            p.range("C", 7, 93)?;
            p.range("AD:AG", 3, 93)?;
            p.range("AD:AG", 199, 199)?;
            for c in ["I", "J", "P", "U", "W", "AA", "AK"] {
                for r in 4..=93 {
                    p.notes.insert((r - 1, col(c)?));
                }
            }
        }
        "breadsheet" => p.range("B", 4, end)?,
        "charlyautosheet" => p.range("B", 6, end)?,
        "evieautosheet" => {
            p.range("C", 7, end)?;
            p.range("X:AA", 3, 3)?;
            p.range("CA", 22, 25)?;
        }
        "evilsheet" => {
            p.range("C", 7, end)?;
            p.range("AB:AE", 56, 56)?;
        }
        "makusheet 1.0" => {
            p.range("B", 6, end)?;
            p.range("V:Y", 2, 2)?;
            p.range("V:Y", 199, 199)?;
        }
        "serenadesheet" => {
            p.range("C", 7, end)?;
            p.range("BJ", 42, 45)?;
        }
        "autosheetmodel" => {
            let start = col(&super::stats::normalize_column(&settings.start_column, "D"))?;
            for c in start..start + 27 {
                p.range(&sheets::index_to_column(c), 2, end)?;
            }
            p.range(
                &super::stats::normalize_column(&settings.quota_column, "B"),
                5,
                end,
            )?;
            p.range(
                &super::stats::normalize_column(&settings.sell_column, "AE"),
                2,
                end,
            )?;
        }
        "custom layout" => {
            let config =
                serde_json::to_value(&settings.custom_layout).map_err(|e| e.to_string())?;
            for (key, value) in config.as_object().ok_or("Invalid custom layout")? {
                if key == "checkColumn" || key == "playerNameColumns" {
                    continue;
                }
                if key.ends_with("Column") || key == "deathColumns" {
                    let columns = value.as_str().unwrap_or("").trim().to_ascii_uppercase();
                    p.range(&columns, start, end)?;
                }
            }
            p.range(
                &settings
                    .custom_layout
                    .player_name_columns
                    .trim()
                    .to_ascii_uppercase(),
                settings.custom_layout.player_name_row.max(1),
                settings.custom_layout.player_name_row.max(1),
            )?;
        }
        _ => {}
    }
    Ok(p)
}

fn cell_at(sheet: &Value, row: usize, column: usize) -> Value {
    for data in sheet["data"].as_array().into_iter().flatten() {
        let sr = data["startRow"].as_u64().unwrap_or(0) as usize;
        let sc = data["startColumn"].as_u64().unwrap_or(0) as usize;
        if row >= sr && column >= sc {
            if let Some(cell) = data
                .get("rowData")
                .and_then(|v| v.get(row - sr))
                .and_then(|v| v.get("values"))
                .and_then(|v| v.get(column - sc))
            {
                return cell.clone();
            }
        }
    }
    json!({})
}
fn effective(cell: &Value) -> Value {
    let v = &cell["effectiveValue"];
    v.get("numberValue")
        .or_else(|| v.get("stringValue"))
        .or_else(|| v.get("boolValue"))
        .cloned()
        .unwrap_or(json!(""))
}
fn user_value(v: Value) -> Value {
    if v.is_number() {
        json!({"numberValue":v})
    } else if v.is_boolean() {
        json!({"boolValue":v})
    } else {
        json!({"stringValue":v.as_str().unwrap_or("")})
    }
}
fn update(id: i64, r: usize, c: usize, cell: Value, fields: &str) -> Value {
    json!({"repeatCell":{"range":{"sheetId":id,"startRowIndex":r,"endRowIndex":r+1,
        "startColumnIndex":c,"endColumnIndex":c+1},"cell":cell,"fields":fields}})
}
fn reset_cell(cell: &Value) -> Value {
    if cell["userEnteredValue"].get("formulaValue").is_some() {
        return json!({"userEnteredValue":cell["userEnteredValue"]});
    }
    if cell["dataValidation"]["condition"]["type"] == "BOOLEAN" {
        let values = &cell["dataValidation"]["condition"]["values"];
        return match values.as_array().map(Vec::len).unwrap_or(0) {
            0 => json!({"userEnteredValue":{"boolValue":false}}),
            1 => json!({}),
            _ => json!({"userEnteredValue":user_value(values[1]["userEnteredValue"].clone())}),
        };
    }
    json!({})
}

async fn get(
    client: &reqwest::Client,
    token: &str,
    id: &str,
    ranges: &[String],
) -> Result<Value, String> {
    let url = format!(
        "https://sheets.googleapis.com/v4/spreadsheets/{}",
        crate::util::url_encode(id)
    );
    let mut query = vec![("fields","namedRanges,sheets(properties,data(startRow,startColumn,rowData(values(userEnteredValue,effectiveValue,note,dataValidation))))".to_string())];
    for range in ranges {
        query.push(("ranges", range.clone()));
    }
    let query = query
        .iter()
        .map(|(key, value)| format!("{key}={}", crate::util::url_encode(value)))
        .collect::<Vec<_>>()
        .join("&");
    let response = client
        .get(format!("{url}?{query}"))
        .bearer_auth(token)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let status = response.status();
    if !status.is_success() {
        return Err(format!(
            "Failed to read reset data ({status}): {}",
            response.text().await.unwrap_or_default()
        ));
    }
    response.json().await.map_err(|e| e.to_string())
}

pub async fn execute(
    app: tauri::AppHandle,
    settings: LcStatsSettings,
    end: usize,
) -> Result<(), String> {
    let p = plan(&settings, end)?;
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(90))
        .build()
        .map_err(|e| e.to_string())?;
    let spreadsheet = settings.spreadsheet_id.trim();
    if spreadsheet.is_empty() || settings.active_sheet_name.trim().is_empty() {
        return Err("Select a spreadsheet and sheet first".into());
    }
    crate::google_oauth::assert_spreadsheet_can_edit(app.clone(), &client, spreadsheet).await?;
    let token = crate::google_oauth::access_token(app).await?;
    let infos = sheets::get_sheet_infos(&client, &token, spreadsheet).await?;
    // Destructive operations never use fuzzy matching or another-tab fallback.
    let selected = infos
        .iter()
        .find(|s| {
            s.title == settings.active_sheet_name
                && (settings.active_sheet_id.is_empty()
                    || s.id.to_string() == settings.active_sheet_id)
        })
        .ok_or("The selected sheet changed. Refresh the sheet list and select it again.")?;
    let id = selected.id;
    let wafrody = settings
        .layout
        .eq_ignore_ascii_case(layouts::WAFRODY_LAYOUT);
    let last_row = p
        .cells
        .iter()
        .map(|(r, _)| r + 1)
        .max()
        .unwrap_or(end)
        .max(end);
    let last_col = p
        .cells
        .iter()
        .map(|(_, c)| *c)
        .max()
        .unwrap_or(0)
        .max(if wafrody { 57 } else { 0 });
    let range = format!(
        "{}!A1:{}{}",
        sheets::quote_sheet_name(&selected.title),
        sheets::index_to_column(last_col),
        last_row
    );
    let data = get(&client, &token, spreadsheet, &[range]).await?;
    let sheet = data["sheets"]
        .as_array()
        .and_then(|a| a.iter().find(|s| s["properties"]["sheetId"] == id))
        .ok_or("Selected sheet data missing")?;
    let rows = sheet["properties"]["gridProperties"]["rowCount"]
        .as_u64()
        .unwrap_or(0) as usize;
    let cols = sheet["properties"]["gridProperties"]["columnCount"]
        .as_u64()
        .unwrap_or(0) as usize;
    if p.cells.iter().any(|(r, c)| *r >= rows || *c >= cols) {
        return Err("Reset range exceeds this sheet's grid. Check the layout and end row.".into());
    }
    let mut requests = vec![];
    // Group adjacent cells so large runs stay well within API request sizes.
    let mut row_values = std::collections::BTreeMap::<usize, Vec<(usize, Value)>>::new();
    for &(r, c) in &p.cells {
        row_values.entry(r).or_default().push((
            c,
            if wafrody {
                json!({})
            } else {
                reset_cell(&cell_at(sheet, r, c))
            },
        ));
    }
    for (r, cells) in row_values {
        let mut i = 0;
        while i < cells.len() {
            let start = cells[i].0;
            let mut values = vec![cells[i].1.clone()];
            i += 1;
            while i < cells.len() && cells[i].0 == start + values.len() {
                values.push(cells[i].1.clone());
                i += 1;
            }
            requests.push(
                json!({"updateCells":{"start":{"sheetId":id,"rowIndex":r,"columnIndex":start},
                "rows":[{"values":values}],"fields":"userEnteredValue,note"}}),
            );
        }
    }
    for &(r, c) in &p.notes {
        requests.push(update(id, r, c, json!({}), "note"));
    }
    if wafrody {
        wafrody_requests(&data, sheet, id, &mut requests)?;
    }
    sheets::batch_update_spreadsheet(&client,&token,spreadsheet,requests).await.map_err(|e|
        format!("Reset could not be confirmed. Inspect the sheet before retrying. {e}"))?;
    Ok(())
}

fn wafrody_requests(
    data: &Value,
    sheet: &Value,
    id: i64,
    requests: &mut Vec<Value>,
) -> Result<(), String> {
    for r in 3..93 {
        for c in [9, 14, 16] {
            requests.push(update(
                id,
                r,
                c,
                json!({"userEnteredValue":{"boolValue":false}}),
                "userEnteredValue",
            ));
        }
    }
    requests.push(update(
        id,
        30,
        44,
        json!({"userEnteredValue":{"numberValue":0}}),
        "userEnteredValue",
    ));
    for r in [15, 16, 17] {
        requests.push(update(
            id,
            r,
            44,
            json!({"userEnteredValue":{"numberValue":0}}),
            "userEnteredValue",
        ));
    }
    let named = data["namedRanges"].as_array().cloned().unwrap_or_default();
    let own = |name: &str| {
        named
            .iter()
            .find(|n| n["name"] == name && n["range"]["sheetId"] == id)
    };
    for name in [
        "Steam_ID",
        "Gift",
        "Furnitures1",
        "Furnitures2",
        "Deaths",
        "ValueQ2wipes",
    ] {
        let Some(n) = own(name) else { continue };
        let mut range = n["range"].clone();
        let start = range["startRowIndex"].as_u64().unwrap_or(0) as usize;
        if name != "Steam_ID" {
            if start >= 93 {
                continue;
            }
            range["endRowIndex"] = json!(range["endRowIndex"].as_u64().unwrap_or(93).min(93));
        }
        let value = match name {
            "Gift" => json!({"numberValue":0}),
            "Furnitures1" | "Furnitures2" => json!({"boolValue":false}),
            "ValueQ2wipes" => {
                let Some(q) = own("Quota2") else { continue };
                let qr = q["range"]["startRowIndex"].as_u64().unwrap_or(0) as usize;
                let qc = q["range"]["startColumnIndex"].as_u64().unwrap_or(0) as usize;
                if effective(&cell_at(sheet, qr, qc)) == json!("") {
                    continue;
                }
                let c = range["startColumnIndex"].as_u64().unwrap_or(0) as usize;
                json!({"numberValue":sheets::value_as_f64(&effective(&cell_at(sheet,start,c)))+1.0})
            }
            _ => Value::Null,
        };
        let cell = if value.is_null() {
            json!({})
        } else {
            json!({"userEnteredValue":value})
        };
        requests.push(json!({"repeatCell":{"range":range,"cell":cell,"fields":if name=="Deaths"{"userEnteredValue,note"}else{"userEnteredValue"}}}));
    }
    let first = include_str!("reset_first_moon.txt").trim();
    let remaining = include_str!("reset_remaining_moon.txt").trim();
    let weather = include_str!("reset_weather.txt").trim();
    for r in 3..93 {
        requests.push(update(
            id,
            r,
            6,
            json!({"userEnteredValue":{"formulaValue":if r<6 {first}else{remaining}}}),
            "userEnteredValue",
        ));
        // Weather is intentionally blank after day one, even if it previously contained a formula.
        requests.push(update(
            id,
            r,
            7,
            if r == 3 {
                json!({"userEnteredValue":{"formulaValue":weather}})
            } else {
                json!({})
            },
            "userEnteredValue",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn supports_every_layout_and_preserves_wafrody_boundaries() {
        for name in [
            "AutoSheetModel",
            "BreadSheet",
            "CharlyAutoSheet",
            "Custom Layout",
            "EvieAutoSheet",
            "Evilsheet",
            "MakuSheet 1.0",
            "ModdedSheet",
            "SerenadeSheet",
            "WafrodyAutoSheet",
        ] {
            let s = LcStatsSettings {
                layout: name.into(),
                ..Default::default()
            };
            assert!(!plan(&s, 93).unwrap().cells.is_empty());
        }
        let s = LcStatsSettings {
            layout: "WafrodyAutoSheet".into(),
            ..Default::default()
        };
        let p = plan(&s, 93).unwrap();
        assert!(!p.cells.contains(&(3, 2)));
        assert!(p.cells.contains(&(6, 2)));
        assert!(!p.cells.contains(&(93, 57)));
        assert!(p.cells.contains(&(92, 57)));
        assert!(!p.cells.contains(&(3, 26)));
        assert!(p.notes.contains(&(3, 26)));
        assert!(plan(&s, 94).is_err());
    }
    #[test]
    fn formulas_and_checkbox_validation_survive() {
        assert_eq!(
            reset_cell(&json!({"userEnteredValue":{"formulaValue":"=SUM(A1:A3)"},"note":"old"})),
            json!({"userEnteredValue":{"formulaValue":"=SUM(A1:A3)"}})
        );
        assert_eq!(
            reset_cell(&json!({"userEnteredValue":{"stringValue":"old"}})),
            json!({})
        );
        assert_eq!(
            reset_cell(&json!({"dataValidation":{"condition":{"type":"BOOLEAN"}}})),
            json!({"userEnteredValue":{"boolValue":false}})
        );
    }
    #[test]
    fn wafrody_reset_does_not_archive_or_require_seeds() {
        let mut requests = vec![];
        wafrody_requests(&json!({}), &json!({}), 42, &mut requests).unwrap();
        assert!(!requests.is_empty());
        for request in requests {
            assert!(request.get("duplicateSheet").is_none());
            assert_eq!(request["repeatCell"]["range"]["sheetId"], json!(42));
        }
    }

    #[test]
    fn grid_offsets_and_seed_zero_are_preserved() {
        let s = json!({"data":[{"startRow":5,"startColumn":1,"rowData":[{"values":[{"effectiveValue":{"numberValue":0}}]}]}]});
        assert_eq!(effective(&cell_at(&s, 5, 1)), json!(0));
        assert_eq!(effective(&cell_at(&s, 4, 1)), json!(""));
    }
}

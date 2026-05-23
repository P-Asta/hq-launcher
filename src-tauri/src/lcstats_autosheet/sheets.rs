use serde_json::{json, Value};
use std::collections::BTreeMap;

pub async fn first_empty_row(
    client: &reqwest::Client,
    token: &str,
    spreadsheet_id: &str,
    sheet_name: &str,
    column: &str,
) -> Result<usize, String> {
    let range = format!("{}!{column}:{column}", quote_sheet_name(sheet_name));
    let data = read_range(client, token, spreadsheet_id, &range).await?;
    Ok(data
        .get("values")
        .and_then(Value::as_array)
        .map(|values| values.len())
        .unwrap_or(0)
        + 1)
}

pub async fn first_empty_row_from(
    client: &reqwest::Client,
    token: &str,
    spreadsheet_id: &str,
    sheet_name: &str,
    column: &str,
    start_row: usize,
) -> Result<usize, String> {
    let range = format!(
        "{}!{column}{start_row}:{column}",
        quote_sheet_name(sheet_name)
    );
    let data = read_range(client, token, spreadsheet_id, &range).await?;
    let rows = data
        .get("values")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    for (index, row) in rows.iter().enumerate() {
        let empty = row
            .as_array()
            .and_then(|cells| cells.first())
            .map(|value| value.as_str().unwrap_or_default().is_empty())
            .unwrap_or(true);
        if empty {
            return Ok(start_row + index);
        }
    }
    Ok(start_row + rows.len())
}

pub async fn read_number(
    client: &reqwest::Client,
    token: &str,
    spreadsheet_id: &str,
    sheet_name: &str,
    cell: &str,
) -> Result<f64, String> {
    let range = format!("{}!{cell}", quote_sheet_name(sheet_name));
    let data = read_range(client, token, spreadsheet_id, &range).await?;
    Ok(data
        .get("values")
        .and_then(Value::as_array)
        .and_then(|rows| rows.first())
        .and_then(Value::as_array)
        .and_then(|cells| cells.first())
        .map(value_as_f64)
        .unwrap_or(0.0))
}

pub async fn read_range(
    client: &reqwest::Client,
    token: &str,
    spreadsheet_id: &str,
    range: &str,
) -> Result<Value, String> {
    let url = format!(
        "https://sheets.googleapis.com/v4/spreadsheets/{}/values/{}?fields=values",
        url_encode(spreadsheet_id),
        url_encode(range)
    );
    let response = client
        .get(url)
        .header(reqwest::header::AUTHORIZATION, format!("Bearer {token}"))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    parse_google_response(response, "read Google Sheets range").await
}

pub async fn batch_read_ranges(
    client: &reqwest::Client,
    token: &str,
    spreadsheet_id: &str,
    ranges: &[&str],
) -> Result<Vec<Value>, String> {
    if ranges.is_empty() {
        return Ok(vec![]);
    }
    let range_query = ranges
        .iter()
        .map(|range| format!("ranges={}", url_encode(range)))
        .collect::<Vec<_>>()
        .join("&");
    let url = format!(
        "https://sheets.googleapis.com/v4/spreadsheets/{}/values:batchGet?{}&fields=valueRanges(values)",
        url_encode(spreadsheet_id),
        range_query
    );
    let response = client
        .get(url)
        .header(reqwest::header::AUTHORIZATION, format!("Bearer {token}"))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let data = parse_google_response(response, "read Google Sheets ranges").await?;
    Ok(data
        .get("valueRanges")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default())
}

pub async fn write_cells(
    client: &reqwest::Client,
    token: &str,
    spreadsheet_id: &str,
    sheet_name: &str,
    start_cell: &str,
    values: Vec<Vec<Value>>,
) -> Result<(), String> {
    if values.is_empty() {
        return Ok(());
    }
    let sheet_id = get_sheet_id(client, token, spreadsheet_id, sheet_name).await?;
    let (start_column_index, start_row) = parse_cell_reference(start_cell)
        .ok_or_else(|| format!("Invalid sheet cell reference: {start_cell}"))?;
    let note_clear_requests =
        rows_note_clear_requests(sheet_id, start_column_index, start_row, &values);
    let range = format!("{}!{start_cell}", quote_sheet_name(sheet_name));
    let url = format!(
        "https://sheets.googleapis.com/v4/spreadsheets/{}/values/{}?valueInputOption=RAW&fields=updatedCells",
        url_encode(spreadsheet_id),
        url_encode(&range)
    );
    let response = client
        .put(url)
        .header(reqwest::header::AUTHORIZATION, format!("Bearer {token}"))
        .json(&json!({ "values": values }))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let _ = parse_google_response(response, "write Google Sheets range").await?;
    batch_update_spreadsheet(client, token, spreadsheet_id, note_clear_requests).await
}

pub async fn batch_write_cells_user_entered(
    client: &reqwest::Client,
    token: &str,
    spreadsheet_id: &str,
    sheet_name: &str,
    values: Vec<(String, usize, Value)>,
) -> Result<(), String> {
    if values.is_empty() {
        return Ok(());
    }
    let sheet_id = get_sheet_id(client, token, spreadsheet_id, sheet_name).await?;
    let note_clear_requests = compact_note_clear_requests(sheet_id, &values)?;
    let data = compact_value_ranges(sheet_name, values)?;
    if data.is_empty() {
        return Ok(());
    }
    let url = format!(
        "https://sheets.googleapis.com/v4/spreadsheets/{}/values:batchUpdate?fields=totalUpdatedCells",
        url_encode(spreadsheet_id)
    );
    let response = client
        .post(url)
        .header(reqwest::header::AUTHORIZATION, format!("Bearer {token}"))
        .json(&json!({ "valueInputOption": "USER_ENTERED", "data": data }))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let _ = parse_google_response(response, "batch write Google Sheets values").await?;
    batch_update_spreadsheet(client, token, spreadsheet_id, note_clear_requests).await
}

pub async fn batch_update_spreadsheet(
    client: &reqwest::Client,
    token: &str,
    spreadsheet_id: &str,
    requests: Vec<Value>,
) -> Result<(), String> {
    if requests.is_empty() {
        return Ok(());
    }
    let url = format!(
        "https://sheets.googleapis.com/v4/spreadsheets/{}:batchUpdate?fields=spreadsheetId",
        url_encode(spreadsheet_id)
    );
    let response = client
        .post(url)
        .header(reqwest::header::AUTHORIZATION, format!("Bearer {token}"))
        .json(&json!({ "requests": requests }))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let _ = parse_google_response(response, "batch update Google Sheets").await?;
    Ok(())
}

pub async fn get_sheet_id(
    client: &reqwest::Client,
    token: &str,
    spreadsheet_id: &str,
    sheet_name: &str,
) -> Result<i64, String> {
    let url = format!(
        "https://sheets.googleapis.com/v4/spreadsheets/{}?fields=sheets.properties(sheetId,title)",
        url_encode(spreadsheet_id)
    );
    let response = client
        .get(url)
        .header(reqwest::header::AUTHORIZATION, format!("Bearer {token}"))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let data = parse_google_response(response, "read Google spreadsheet metadata").await?;
    data.get("sheets")
        .and_then(Value::as_array)
        .and_then(|sheets| {
            sheets.iter().find_map(|sheet| {
                let props = sheet.get("properties")?;
                let title = props.get("title").and_then(Value::as_str)?;
                if title == sheet_name {
                    props.get("sheetId").and_then(Value::as_i64)
                } else {
                    None
                }
            })
        })
        .ok_or_else(|| format!("Sheet not found: {sheet_name}"))
}

async fn parse_google_response(response: reqwest::Response, label: &str) -> Result<Value, String> {
    if response.status().is_success() {
        return response.json::<Value>().await.map_err(|e| e.to_string());
    }
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    Err(format!("Failed to {label} ({status}): {body}"))
}

pub fn quote_sheet_name(sheet_name: &str) -> String {
    format!("'{}'", sheet_name.replace('\'', "''"))
}

pub fn number_value(value: f64) -> Value {
    if value.fract() == 0.0 {
        json!(value as i64)
    } else {
        json!(value)
    }
}

pub fn value_as_f64(value: &Value) -> f64 {
    value
        .as_f64()
        .or_else(|| value.as_str().and_then(|text| text.parse::<f64>().ok()))
        .unwrap_or(0.0)
}

fn compact_value_ranges(
    sheet_name: &str,
    values: Vec<(String, usize, Value)>,
) -> Result<Vec<Value>, String> {
    let mut cells = BTreeMap::new();
    for (column, row, value) in values {
        let column_index =
            column_to_index(&column).ok_or_else(|| format!("Invalid sheet column: {column}"))?;
        cells.insert((row, column_index), value);
    }

    let mut data = vec![];
    let mut current_row = 0;
    let mut start_column = 0;
    let mut next_column = 0;
    let mut row_values: Vec<Value> = vec![];

    for ((row, column), value) in cells {
        if row_values.is_empty() {
            current_row = row;
            start_column = column;
            next_column = column + 1;
            row_values.push(value);
            continue;
        }
        if row == current_row && column == next_column {
            next_column += 1;
            row_values.push(value);
            continue;
        }
        data.push(value_range(
            sheet_name,
            current_row,
            start_column,
            next_column,
            std::mem::take(&mut row_values),
        ));
        current_row = row;
        start_column = column;
        next_column = column + 1;
        row_values.push(value);
    }

    if !row_values.is_empty() {
        data.push(value_range(
            sheet_name,
            current_row,
            start_column,
            next_column,
            row_values,
        ));
    }

    Ok(data)
}

fn value_range(
    sheet_name: &str,
    row: usize,
    start_column: usize,
    next_column: usize,
    values: Vec<Value>,
) -> Value {
    let start = index_to_column(start_column);
    let range = if next_column == start_column + 1 {
        format!("{}!{start}{row}", quote_sheet_name(sheet_name))
    } else {
        let end = index_to_column(next_column - 1);
        format!("{}!{start}{row}:{end}{row}", quote_sheet_name(sheet_name))
    };
    json!({ "range": range, "values": [values] })
}

fn compact_note_clear_requests(
    sheet_id: i64,
    values: &[(String, usize, Value)],
) -> Result<Vec<Value>, String> {
    let mut cells = BTreeMap::new();
    for (column, row, _) in values {
        let column_index =
            column_to_index(column).ok_or_else(|| format!("Invalid sheet column: {column}"))?;
        cells.insert((*row, column_index), ());
    }
    Ok(group_note_clear_requests(sheet_id, cells.into_keys()))
}

fn rows_note_clear_requests(
    sheet_id: i64,
    start_column_index: usize,
    start_row: usize,
    values: &[Vec<Value>],
) -> Vec<Value> {
    values
        .iter()
        .enumerate()
        .filter(|(_, row_values)| !row_values.is_empty())
        .map(|(row_index, row_values)| {
            row_note_clear_request(
                sheet_id,
                start_row + row_index,
                start_column_index,
                row_values.len(),
            )
        })
        .collect()
}

fn group_note_clear_requests<I>(sheet_id: i64, cells: I) -> Vec<Value>
where
    I: IntoIterator<Item = (usize, usize)>,
{
    let mut requests = vec![];
    let mut current_row = 0;
    let mut start_column = 0;
    let mut next_column = 0;

    for (row, column) in cells {
        if next_column == 0 {
            current_row = row;
            start_column = column;
            next_column = column + 1;
            continue;
        }
        if row == current_row && column == next_column {
            next_column += 1;
            continue;
        }
        requests.push(row_note_clear_request(
            sheet_id,
            current_row,
            start_column,
            next_column - start_column,
        ));
        current_row = row;
        start_column = column;
        next_column = column + 1;
    }

    if next_column != 0 {
        requests.push(row_note_clear_request(
            sheet_id,
            current_row,
            start_column,
            next_column - start_column,
        ));
    }

    requests
}

fn row_note_clear_request(
    sheet_id: i64,
    row: usize,
    start_column_index: usize,
    column_count: usize,
) -> Value {
    let cell_values = (0..column_count).map(|_| json!({})).collect::<Vec<_>>();
    json!({
        "updateCells": {
            "range": {
                "sheetId": sheet_id,
                "startRowIndex": row.saturating_sub(1),
                "endRowIndex": row,
                "startColumnIndex": start_column_index,
                "endColumnIndex": start_column_index + column_count
            },
            "rows": [{ "values": cell_values }],
            "fields": "note"
        }
    })
}

fn parse_cell_reference(cell: &str) -> Option<(usize, usize)> {
    let mut column = String::new();
    let mut row = String::new();
    for ch in cell.trim().chars().filter(|ch| *ch != '$') {
        if ch.is_ascii_alphabetic() && row.is_empty() {
            column.push(ch);
        } else if ch.is_ascii_digit() {
            row.push(ch);
        } else {
            return None;
        }
    }
    let column_index = column_to_index(&column)?;
    let row = row.parse::<usize>().ok().filter(|row| *row > 0)?;
    Some((column_index, row))
}

fn column_to_index(column: &str) -> Option<usize> {
    let mut index = 0usize;
    let mut seen = false;
    for ch in column.chars() {
        if !ch.is_ascii_alphabetic() {
            return None;
        }
        seen = true;
        index = index * 26 + (ch.to_ascii_uppercase() as usize - 'A' as usize + 1);
    }
    seen.then_some(index - 1)
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

fn url_encode(value: &str) -> String {
    let mut out = String::new();
    for b in value.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            b' ' => out.push_str("%20"),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn note_clear_requests_clear_notes_without_touching_values() {
        let request = row_note_clear_request(123, 7, 2, 2);
        let update = &request["updateCells"];

        assert_eq!(update["fields"], json!("note"));
        assert_eq!(update["range"]["startRowIndex"], json!(6));
        assert_eq!(update["range"]["startColumnIndex"], json!(2));
        assert_eq!(update["range"]["endColumnIndex"], json!(4));
        assert_eq!(update["rows"][0]["values"], json!([{}, {}]));
        assert!(update["rows"][0]["values"][0].get("note").is_none());
    }

    #[test]
    fn compact_note_clear_requests_group_contiguous_cells() {
        let values = vec![
            ("B".to_string(), 4, json!(1)),
            ("C".to_string(), 4, json!(true)),
            ("E".to_string(), 4, json!("x")),
        ];
        let requests = compact_note_clear_requests(123, &values).unwrap();

        assert_eq!(requests.len(), 2);
        assert_eq!(
            requests[0]["updateCells"]["range"]["endColumnIndex"],
            json!(3)
        );
        assert_eq!(
            requests[1]["updateCells"]["range"]["startColumnIndex"],
            json!(4)
        );
    }

    #[test]
    fn parses_start_cell_reference() {
        assert_eq!(parse_cell_reference("AA12"), Some((26, 12)));
        assert_eq!(parse_cell_reference("$B$3"), Some((1, 3)));
        assert_eq!(parse_cell_reference("12AA"), None);
    }
}

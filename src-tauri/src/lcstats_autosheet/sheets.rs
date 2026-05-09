use serde_json::{json, Value};

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
        "https://sheets.googleapis.com/v4/spreadsheets/{}/values/{}",
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

pub async fn write_cells(
    client: &reqwest::Client,
    token: &str,
    spreadsheet_id: &str,
    sheet_name: &str,
    start_cell: &str,
    values: Vec<Vec<Value>>,
) -> Result<(), String> {
    let range = format!("{}!{start_cell}", quote_sheet_name(sheet_name));
    let url = format!(
        "https://sheets.googleapis.com/v4/spreadsheets/{}/values/{}?valueInputOption=RAW",
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
    Ok(())
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
    let data = values
        .into_iter()
        .map(|(column, row, value)| {
            json!({
                "range": format!("{}!{column}{row}", quote_sheet_name(sheet_name)),
                "values": [[value]]
            })
        })
        .collect::<Vec<_>>();
    let url = format!(
        "https://sheets.googleapis.com/v4/spreadsheets/{}/values:batchUpdate",
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
    Ok(())
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
        "https://sheets.googleapis.com/v4/spreadsheets/{}:batchUpdate",
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

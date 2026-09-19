//! Helpers shared by every sheet layout.
//!
//! These were duplicated verbatim across the layout modules; each copy was
//! byte-identical and depended only on items shared by all of them.

use serde_json::{json, Value};

use crate::lcstats_autosheet::stats::strip_apostrophe;

pub(super) fn column_to_index(column: &str) -> usize {
    column.chars().fold(0, |index, ch| {
        index * 26 + (ch.to_ascii_uppercase() as usize - 'A' as usize + 1)
    }) - 1
}

pub(super) fn google_user_value(value: Value) -> Value {
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

pub(super) fn non_false_text(value: &str) -> Option<String> {
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

pub(super) fn normalize_interior_name(value: &str) -> String {
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

pub(super) fn blank_or_x(value: &str) -> Value {
    if value.trim().is_empty() {
        json!("X")
    } else {
        json!(value)
    }
}

/// Builds an `updateCells` request for a single cell with an optional note.
///
/// `to_user_value` is a parameter for the same reason as in
/// [`note_cell_request`]: the layouts encode numbers differently.
pub(super) fn value_with_note_request(
    sheet_id: i64,
    column: &str,
    row: usize,
    value: Value,
    note: &str,
    to_user_value: fn(Value) -> Value,
) -> Value {
    let column_index = column_to_index(column);
    let mut cell = json!({ "userEnteredValue": to_user_value(value) });
    if !note.is_empty() {
        cell["note"] = json!(note);
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
            "rows": [{ "values": [cell] }],
            "fields": "userEnteredValue,note"
        }
    })
}

#[derive(Debug, Clone)]
pub(super) struct NoteCell {
    pub(super) column: &'static str,
    pub(super) value: Value,
    pub(super) note: Option<String>,
}

/// Builds an `updateCells` request for one [`NoteCell`].
///
/// `to_user_value` is a parameter because the layouts do not agree on how to
/// encode numbers: `serenadesheet` preserves `u64` values above `i64::MAX`
/// exactly, while the others fall back to `f64`. Passing each layout's own
/// converter keeps that difference intact.
pub(super) fn note_cell_request(
    sheet_id: i64,
    cell: &NoteCell,
    row: usize,
    to_user_value: fn(Value) -> Value,
) -> Value {
    let column_index = column_to_index(cell.column);
    let mut value = json!({ "userEnteredValue": to_user_value(cell.value.clone()) });
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

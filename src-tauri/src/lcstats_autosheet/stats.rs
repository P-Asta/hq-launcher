use serde_json::Value;

pub fn value_at<'a>(stats: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut value = stats;
    for key in path {
        value = value.get(*key)?;
    }
    Some(value)
}

pub fn int_at(stats: &Value, path: &[&str]) -> i64 {
    value_at(stats, path).and_then(Value::as_i64).unwrap_or(0)
}

pub fn bool_at(stats: &Value, path: &[&str]) -> bool {
    value_at(stats, path)
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

pub fn string_at(stats: &Value, path: &[&str]) -> String {
    value_at(stats, path)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

pub fn array_at<'a>(stats: &'a Value, path: &[&str]) -> &'a [Value] {
    value_at(stats, path)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[])
}

pub fn sum_array(stats: &Value, path: &[&str]) -> i64 {
    array_at(stats, path).iter().filter_map(Value::as_i64).sum()
}

pub fn object_at(stats: &Value, path: &[&str]) -> std::collections::BTreeMap<String, Value> {
    value_at(stats, path)
        .and_then(Value::as_object)
        .map(|object| {
            object
                .iter()
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect()
        })
        .unwrap_or_default()
}

pub fn missed_item_count(stats: &Value) -> usize {
    array_at(stats, &["MissedItems"])
        .iter()
        .filter(|item| {
            !item
                .get("CollectedOnPreviousDay")
                .and_then(Value::as_bool)
                .unwrap_or(false)
        })
        .count()
}

pub fn enemy_count(stats: &Value, enemy: &str) -> usize {
    array_at(stats, &["IndoorSpawns"])
        .iter()
        .filter(|spawn| spawn.get("Enemy").and_then(Value::as_str) == Some(enemy))
        .count()
}

pub fn strip_moon_number(name: &str) -> String {
    name.trim_start_matches(|ch: char| ch.is_ascii_digit() || ch.is_whitespace())
        .to_string()
}

pub fn normalize_column(value: &str, fallback: &str) -> String {
    let column = value
        .trim()
        .chars()
        .filter(|ch| ch.is_ascii_alphabetic())
        .collect::<String>()
        .to_ascii_uppercase();
    if column.is_empty() {
        fallback.to_string()
    } else {
        column
    }
}

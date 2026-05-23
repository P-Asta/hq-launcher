use serde_json::Value;

pub fn value_at<'a>(stats: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut value = stats;
    for key in path {
        value = value.get(*key)?;
    }
    Some(value)
}

pub fn int_at(stats: &Value, path: &[&str]) -> i64 {
    value_at(stats, path).map(intish_value).unwrap_or(0)
}

pub fn bool_at(stats: &Value, path: &[&str]) -> bool {
    value_at(stats, path)
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

pub fn string_at(stats: &Value, path: &[&str]) -> String {
    value_at(stats, path).map(value_text).unwrap_or_default()
}

pub fn array_at<'a>(stats: &'a Value, path: &[&str]) -> &'a [Value] {
    value_at(stats, path)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[])
}

pub fn array_at_any<'a>(stats: &'a Value, paths: &[&[&str]]) -> &'a [Value] {
    for path in paths {
        if let Some(values) = value_at(stats, path).and_then(Value::as_array) {
            return values;
        }
    }
    &[]
}

pub fn value_at_any<'a>(stats: &'a Value, paths: &[&[&str]]) -> Option<&'a Value> {
    paths.iter().find_map(|path| value_at(stats, path))
}

pub fn intish_value(value: &Value) -> i64 {
    value
        .as_i64()
        .or_else(|| {
            value
                .as_str()
                .and_then(|text| text.trim_start_matches('\'').trim().parse::<i64>().ok())
        })
        .unwrap_or(0)
}

pub fn value_text(value: &Value) -> String {
    if let Some(text) = value.as_str() {
        text.to_string()
    } else if let Some(number) = value.as_i64() {
        number.to_string()
    } else if let Some(number) = value.as_u64() {
        number.to_string()
    } else if let Some(number) = value.as_f64() {
        number.to_string()
    } else if let Some(flag) = value.as_bool() {
        flag.to_string()
    } else {
        String::new()
    }
}

pub fn sum_array_any(stats: &Value, paths: &[&[&str]]) -> i64 {
    array_at_any(stats, paths).iter().map(intish_value).sum()
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

pub fn players_at(stats: &Value) -> Vec<(String, Value)> {
    let mut players = object_at(stats, &["Players"])
        .into_iter()
        .collect::<Vec<_>>();
    players.sort_by(|(left_key, left), (right_key, right)| {
        player_id_sort_key(left_key, left)
            .cmp(&player_id_sort_key(right_key, right))
            .then_with(|| left_key.cmp(right_key))
    });
    players
}

fn player_id_sort_key(key: &str, player: &Value) -> (bool, i64) {
    player
        .get("PlayerID")
        .or_else(|| player.get("PlayerId"))
        .or_else(|| player.get("PlayerIndex"))
        .and_then(|value| {
            value.as_i64().or_else(|| {
                value
                    .as_str()
                    .and_then(|text| text.trim_start_matches('\'').trim().parse::<i64>().ok())
            })
        })
        .or_else(|| key.trim_start_matches('\'').trim().parse::<i64>().ok())
        .map(|id| (false, id))
        .unwrap_or((true, 0))
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn strings_can_read_numeric_payload_values() {
        let stats = json!({ "Seed": 30494987, "IndoorFog": false });

        assert_eq!(string_at(&stats, &["Seed"]), "30494987");
        assert_eq!(string_at(&stats, &["IndoorFog"]), "false");
    }

    #[test]
    fn ints_can_read_quoted_payload_values() {
        let stats = json!({ "CollectedTotal": "'225" });

        assert_eq!(int_at(&stats, &["CollectedTotal"]), 225);
    }

    #[test]
    fn players_are_sorted_by_player_id_before_object_key() {
        let stats = json!({
            "Players": {
                "steam-c": { "Name": "C", "PlayerID": 2 },
                "steam-a": { "Name": "A", "PlayerID": 0 },
                "steam-b": { "Name": "B", "PlayerID": 1 }
            }
        });

        let names = players_at(&stats)
            .into_iter()
            .map(|(_, player)| {
                player
                    .get("Name")
                    .and_then(Value::as_str)
                    .unwrap()
                    .to_string()
            })
            .collect::<Vec<_>>();

        assert_eq!(names, vec!["A", "B", "C"]);
    }

    #[test]
    fn players_fall_back_to_numeric_object_keys() {
        let stats = json!({
            "Players": {
                "10": { "Name": "C" },
                "2": { "Name": "B" },
                "1": { "Name": "A" }
            }
        });

        let names = players_at(&stats)
            .into_iter()
            .map(|(_, player)| {
                player
                    .get("Name")
                    .and_then(Value::as_str)
                    .unwrap()
                    .to_string()
            })
            .collect::<Vec<_>>();

        assert_eq!(names, vec!["A", "B", "C"]);
    }
}

use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct LcStatsPayload {
    pub seed: i64,
    pub version: i64,
    pub moon_info: MoonInfo,
    pub dungeon_info: Option<DungeonInfo>,
    pub hazard_info: Option<HazardInfo>,
    pub performance_info: PerformanceInfo,
    pub bee_info: SpecialItemInfo,
    pub egg_info: SpecialItemInfo,
    pub knife_info: SpecialItemInfo,
    pub shotgun_info: SpecialItemInfo,
    pub quota_info: QuotaInfo,
    pub event_info: EventInfo,
    pub players: BTreeMap<String, PlayerStats>,
    pub indoor_spawns: Vec<SpawnInfo>,
    pub day_time_spawns: Vec<SpawnInfo>,
    pub night_time_spawns: Vec<SpawnInfo>,
    pub shop_sales: BTreeMap<String, i64>,
    pub furniture_info: BTreeMap<String, FurnitureInfo>,
    pub gift_boxes_opened: Vec<GiftBoxInfo>,
    pub missed_items: Vec<MissingItemInfo>,
    #[serde(skip)]
    fallback_source: Option<Value>,
}

impl LcStatsPayload {
    pub fn from_value(stats: &Value) -> Self {
        let mut payload: Self = serde_json::from_value(stats.clone()).unwrap_or_default();
        payload.fallback_source = Some(stats.clone());
        payload
    }

    pub fn moon_name(&self) -> String {
        self.fallback_string(&["MoonInfo", "Name"])
            .unwrap_or_else(|| self.moon_info.name.clone())
    }

    pub fn new_quota(&self) -> i64 {
        self.fallback_int(&["NewQuota"])
            .unwrap_or(self.quota_info.new_quota)
    }

    pub fn value_sold(&self) -> i64 {
        self.fallback_int(&["ValueSold"])
            .unwrap_or(self.quota_info.value_sold)
    }

    pub fn collected_no_extra(&self) -> i64 {
        self.fallback_int(&["CollectedNoExtra"])
            .unwrap_or(self.performance_info.collected_no_extra)
    }

    pub fn collected_total(&self) -> i64 {
        self.fallback_int(&["CollectedTotal"])
            .unwrap_or(self.performance_info.collected_total)
    }

    pub fn initial_available_value(&self) -> i64 {
        self.fallback_int(&["InitialAvailableValue"])
            .unwrap_or(self.performance_info.initial_available_value)
    }

    pub fn total_available_value(&self) -> i64 {
        self.fallback_int(&["TotalAvailableValue"])
            .unwrap_or(self.performance_info.total_available_value)
    }

    pub fn extra_from_old_gift(&self) -> i64 {
        self.fallback_int(&["ExtraFromOldGift"])
            .unwrap_or(self.performance_info.extra_from_old_gift)
    }

    pub fn app_spawned(&self) -> bool {
        self.fallback_bool(&["AppSpawned"])
            .unwrap_or(self.event_info.app_spawned)
    }

    pub fn indoor_fog(&self) -> bool {
        self.fallback_bool(&["IndoorFog"])
            .unwrap_or(self.event_info.indoor_fog)
    }

    pub fn take_off_time(&self) -> &str {
        self.fallback_str(&["TakeOffTime"])
            .unwrap_or(&self.event_info.take_off_time)
    }

    pub fn sid_type(&self) -> &str {
        self.fallback_str(&["SIDType"])
            .unwrap_or(&self.event_info.s_i_d_type)
    }

    pub fn infestation_type(&self) -> &str {
        self.fallback_str(&["InfestationType"])
            .unwrap_or(&self.event_info.infestation_type)
    }

    pub fn meteor_shower_time(&self) -> &str {
        self.fallback_str(&["MeteorShowerTime"])
            .unwrap_or(&self.event_info.meteor_shower_time)
    }

    pub fn has_dungeon_info(&self) -> bool {
        self.fallback_source
            .as_ref()
            .and_then(|stats| value_at(stats, &["DungeonInfo"]))
            .map(|value| !value.is_null())
            .unwrap_or_else(|| self.dungeon_info.is_some())
    }

    pub fn is_sell_or_quota_event(&self) -> bool {
        self.value_sold() != 0 || self.new_quota() != 0
    }

    pub fn is_quota_event(&self) -> bool {
        self.new_quota() != 0
    }

    pub fn is_sell_event_without_day_stats(&self) -> bool {
        !self.has_dungeon_info() && self.value_sold() != 0
    }

    pub fn is_gordion_moon(&self) -> bool {
        is_gordion_moon_name(&self.moon_name())
    }

    fn fallback_int(&self, path: &[&str]) -> Option<i64> {
        self.fallback_source
            .as_ref()
            .and_then(|stats| value_at(stats, path))
            .map(intish_value)
    }

    fn fallback_bool(&self, path: &[&str]) -> Option<bool> {
        self.fallback_source
            .as_ref()
            .and_then(|stats| value_at(stats, path))
            .and_then(Value::as_bool)
    }

    fn fallback_string(&self, path: &[&str]) -> Option<String> {
        self.fallback_source
            .as_ref()
            .and_then(|stats| value_at(stats, path))
            .map(value_text)
    }

    fn fallback_str(&self, path: &[&str]) -> Option<&str> {
        self.fallback_source
            .as_ref()
            .and_then(|stats| value_at(stats, path))
            .and_then(Value::as_str)
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct MoonInfo {
    pub name: String,
    pub weather: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct DungeonInfo {
    pub item_count: i64,
    pub interior: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct HazardInfo {
    pub turret_count: i64,
    pub landmine_count: i64,
    pub spiketrap_count: i64,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct PerformanceInfo {
    pub collected_no_extra: i64,
    pub collected_total: i64,
    pub initial_available_value: i64,
    pub total_available_value: i64,
    pub extra_from_old_gift: i64,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct SpecialItemInfo {
    pub available: Vec<i64>,
    pub collected: Vec<i64>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct QuotaInfo {
    pub value_sold: i64,
    pub new_quota: i64,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct EventInfo {
    pub app_spawned: bool,
    pub indoor_fog: bool,
    pub take_off_time: String,
    pub s_i_d_type: String,
    pub infestation_type: String,
    pub meteor_shower_time: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct PlayerStats {
    pub name: String,
    pub alive: bool,
    pub disconnected: bool,
    pub time_of_death: String,
    pub cause_of_death: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct SpawnInfo {
    pub enemy: String,
    pub spawn_time: String,
    pub time_of_death: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct FurnitureInfo {
    pub in_stock: bool,
    pub owned: bool,
    pub stored: bool,
    pub apparent_price: i64,
    pub real_price: i64,
    pub luck: f64,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct GiftBoxInfo {
    pub new_scrap_value: i64,
    pub gift_scrap_value: i64,
    pub gift_box_age: i64,
    pub collected: bool,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct MissingItemInfo {
    pub value: i64,
    pub item_type: String,
    pub spawn_position: Option<Vec<f64>>,
    pub despawn_position: Vec<f64>,
    pub collected_on_previous_day: bool,
    pub scrap_inside_gift_value: i64,
}

pub fn lcstats_payload(stats: &Value) -> LcStatsPayload {
    LcStatsPayload::from_value(stats)
}

pub fn is_gordion_stats(stats: &Value) -> bool {
    lcstats_payload(stats).is_gordion_moon()
}

pub fn is_gordion_moon_name(value: &str) -> bool {
    let moon = strip_moon_number(&strip_apostrophe(value));
    let normalized = moon
        .trim()
        .chars()
        .filter(|ch| ch.is_ascii_alphabetic())
        .collect::<String>()
        .to_ascii_uppercase();
    normalized == "GORDION" || normalized == "GORION" || normalized == "GALETRY"
}

pub fn strip_apostrophe(value: &str) -> String {
    value.trim_start_matches('\'').to_string()
}

pub fn value_at<'a>(stats: &'a Value, path: &[&str]) -> Option<&'a Value> {
    if path.len() == 1 {
        if let Some(value) = aliased_value_at(stats, path[0]) {
            return Some(value);
        }
    }

    raw_value_at(stats, path)
}

fn raw_value_at<'a>(stats: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut value = stats;
    for key in path {
        value = value.get(*key)?;
    }
    Some(value)
}

fn aliased_value_at<'a>(stats: &'a Value, key: &str) -> Option<&'a Value> {
    match key {
        "CollectedNoExtra" => raw_value_at(stats, &["PerformanceInfo", "CollectedNoExtra"])
            .or_else(|| raw_value_at(stats, &["CollectedNoExtra"])),
        "CollectedTotal" => raw_value_at(stats, &["PerformanceInfo", "CollectedTotal"])
            .or_else(|| raw_value_at(stats, &["CollectedTotal"])),
        "InitialAvailableValue" | "BottomLine" => {
            raw_value_at(stats, &["PerformanceInfo", "InitialAvailableValue"])
                .or_else(|| raw_value_at(stats, &["InitialAvailableValue"]))
                .or_else(|| raw_value_at(stats, &["BottomLine"]))
        }
        "TotalAvailableValue" | "BottomLineTrue" => {
            raw_value_at(stats, &["PerformanceInfo", "TotalAvailableValue"])
                .or_else(|| raw_value_at(stats, &["TotalAvailableValue"]))
                .or_else(|| raw_value_at(stats, &["BottomLineTrue"]))
        }
        "ExtraFromOldGift" | "ExtraFromOldGiftbox" => {
            raw_value_at(stats, &["PerformanceInfo", "ExtraFromOldGift"])
                .or_else(|| raw_value_at(stats, &["ExtraFromOldGift"]))
                .or_else(|| raw_value_at(stats, &["ExtraFromOldGiftbox"]))
        }
        "ValueSold" => raw_value_at(stats, &["QuotaInfo", "ValueSold"])
            .or_else(|| raw_value_at(stats, &["ValueSold"])),
        "NewQuota" => raw_value_at(stats, &["QuotaInfo", "NewQuota"])
            .or_else(|| raw_value_at(stats, &["NewQuota"])),
        "AppSpawned" => raw_value_at(stats, &["EventInfo", "AppSpawned"])
            .or_else(|| raw_value_at(stats, &["AppSpawned"])),
        "IndoorFog" => raw_value_at(stats, &["EventInfo", "IndoorFog"])
            .or_else(|| raw_value_at(stats, &["IndoorFog"])),
        "TakeOffTime" => raw_value_at(stats, &["EventInfo", "TakeOffTime"])
            .or_else(|| raw_value_at(stats, &["TakeOffTime"])),
        "SIDType" => raw_value_at(stats, &["EventInfo", "SIDType"])
            .or_else(|| raw_value_at(stats, &["SIDType"])),
        "InfestationType" => raw_value_at(stats, &["EventInfo", "InfestationType"])
            .or_else(|| raw_value_at(stats, &["InfestationType"])),
        "MeteorShowerTime" => raw_value_at(stats, &["EventInfo", "MeteorShowerTime"])
            .or_else(|| raw_value_at(stats, &["MeteorShowerTime"])),
        _ => None,
    }
}

pub fn int_at(stats: &Value, path: &[&str]) -> i64 {
    if path.len() == 1 {
        let payload = lcstats_payload(stats);
        match path[0] {
            "CollectedNoExtra" => return payload.collected_no_extra(),
            "CollectedTotal" => return payload.collected_total(),
            "InitialAvailableValue" | "BottomLine" => return payload.initial_available_value(),
            "TotalAvailableValue" | "BottomLineTrue" => return payload.total_available_value(),
            "ExtraFromOldGift" | "ExtraFromOldGiftbox" => return payload.extra_from_old_gift(),
            "ValueSold" => return payload.value_sold(),
            "NewQuota" => return payload.new_quota(),
            _ => {}
        }
    }
    value_at(stats, path).map(intish_value).unwrap_or(0)
}

pub fn initial_available_value(stats: &Value) -> i64 {
    lcstats_payload(stats).initial_available_value()
}

pub fn total_available_value(stats: &Value) -> i64 {
    lcstats_payload(stats).total_available_value()
}

pub fn bool_at(stats: &Value, path: &[&str]) -> bool {
    if path.len() == 1 {
        let payload = lcstats_payload(stats);
        match path[0] {
            "AppSpawned" => return payload.app_spawned(),
            "IndoorFog" => return payload.indoor_fog(),
            _ => {}
        }
    }
    value_at(stats, path)
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

pub fn string_at(stats: &Value, path: &[&str]) -> String {
    if path.len() == 1 {
        let payload = lcstats_payload(stats);
        match path[0] {
            "TakeOffTime" => return payload.take_off_time().to_string(),
            "SIDType" => return payload.sid_type().to_string(),
            "InfestationType" => return payload.infestation_type().to_string(),
            "MeteorShowerTime" => return payload.meteor_shower_time().to_string(),
            _ => {}
        }
    }
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
    fn available_values_prefer_new_payload_keys_and_fall_back_to_old_keys() {
        let new_stats = json!({
            "PerformanceInfo": {
                "InitialAvailableValue": "'300",
                "TotalAvailableValue": "'400"
            },
            "InitialAvailableValue": "'200",
            "TotalAvailableValue": "'250",
            "BottomLine": "'30",
            "BottomLineTrue": "'40"
        });
        let flat_new_stats = json!({
            "InitialAvailableValue": "'200",
            "TotalAvailableValue": "'250",
            "BottomLine": "'30",
            "BottomLineTrue": "'40"
        });
        let old_stats = json!({
            "BottomLine": "'30",
            "BottomLineTrue": "'40"
        });

        assert_eq!(initial_available_value(&new_stats), 300);
        assert_eq!(total_available_value(&new_stats), 400);
        assert_eq!(initial_available_value(&flat_new_stats), 200);
        assert_eq!(total_available_value(&flat_new_stats), 250);
        assert_eq!(initial_available_value(&old_stats), 30);
        assert_eq!(total_available_value(&old_stats), 40);
    }

    #[test]
    fn aliased_root_keys_prefer_nested_latest_payload_values() {
        let stats = json!({
            "PerformanceInfo": {
                "CollectedTotal": "'100",
                "CollectedNoExtra": "'80",
                "ExtraFromOldGift": "'15"
            },
            "QuotaInfo": {
                "ValueSold": "'200",
                "NewQuota": "'900"
            },
            "EventInfo": {
                "AppSpawned": true,
                "IndoorFog": true,
                "TakeOffTime": "'11:00 PM",
                "SIDType": "'Mineshaft",
                "InfestationType": "'Spiders",
                "MeteorShowerTime": "'8:30 PM"
            },
            "CollectedTotal": "'1",
            "CollectedNoExtra": "'2",
            "ExtraFromOldGift": "'3",
            "ValueSold": "'4",
            "NewQuota": "'5",
            "AppSpawned": false,
            "IndoorFog": false,
            "TakeOffTime": "'old",
            "SIDType": "'old",
            "InfestationType": "'old",
            "MeteorShowerTime": "'old"
        });

        assert_eq!(int_at(&stats, &["CollectedTotal"]), 100);
        assert_eq!(int_at(&stats, &["CollectedNoExtra"]), 80);
        assert_eq!(int_at(&stats, &["ExtraFromOldGiftbox"]), 15);
        assert_eq!(int_at(&stats, &["ValueSold"]), 200);
        assert_eq!(int_at(&stats, &["NewQuota"]), 900);
        assert!(bool_at(&stats, &["AppSpawned"]));
        assert!(bool_at(&stats, &["IndoorFog"]));
        assert_eq!(string_at(&stats, &["TakeOffTime"]), "'11:00 PM");
        assert_eq!(string_at(&stats, &["SIDType"]), "'Mineshaft");
        assert_eq!(string_at(&stats, &["InfestationType"]), "'Spiders");
        assert_eq!(string_at(&stats, &["MeteorShowerTime"]), "'8:30 PM");
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

use serde::{Deserialize, Deserializer};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct LcStats {
    #[serde(deserialize_with = "deserialize_intish")]
    pub seed: i64,
    #[serde(deserialize_with = "deserialize_intish")]
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
    #[serde(deserialize_with = "deserialize_map_intish")]
    pub shop_sales: BTreeMap<String, i64>,
    pub furniture_info: BTreeMap<String, FurnitureInfo>,
    pub gift_boxes_opened: Vec<GiftBoxInfo>,
    pub missed_items: Vec<MissingItemInfo>,
    #[serde(skip)]
    fallback_source: Option<Value>,
}

impl LcStats {
    pub fn from_value(stats: &Value) -> Self {
        let mut lc_stats: Self = serde_json::from_value(stats.clone()).unwrap_or_default();
        lc_stats.fallback_source = Some(stats.clone());
        lc_stats
    }

    pub fn moon_name(&self) -> String {
        self.fallback_string(&["MoonInfo", "Name"])
            .unwrap_or_else(|| self.moon_info.name.clone())
    }

    pub fn seed(&self) -> i64 {
        self.fallback_int(&["Seed"]).unwrap_or(self.seed)
    }

    pub fn seed_text(&self) -> String {
        self.fallback_string(&["Seed"])
            .unwrap_or_else(|| self.seed.to_string())
    }

    pub fn version(&self) -> i64 {
        self.fallback_int(&["Version"]).unwrap_or(self.version)
    }

    pub fn version_text(&self) -> String {
        self.fallback_string(&["Version"])
            .unwrap_or_else(|| self.version.to_string())
    }

    pub fn moon_weather(&self) -> String {
        self.fallback_string(&["MoonInfo", "Weather"])
            .unwrap_or_else(|| self.moon_info.weather.clone())
    }

    pub fn dungeon_interior(&self) -> String {
        self.fallback_string(&["DungeonInfo", "Interior"])
            .or_else(|| self.dungeon_info.as_ref().map(|info| info.interior.clone()))
            .unwrap_or_default()
    }

    pub fn dungeon_item_count(&self) -> i64 {
        self.fallback_int(&["DungeonInfo", "ItemCount"])
            .or_else(|| self.dungeon_info.as_ref().map(|info| info.item_count))
            .unwrap_or(0)
    }

    pub fn turret_count(&self) -> i64 {
        self.fallback_int(&["HazardInfo", "TurretCount"])
            .or_else(|| self.hazard_info.as_ref().map(|info| info.turret_count))
            .unwrap_or(0)
    }

    pub fn landmine_count(&self) -> i64 {
        self.fallback_int(&["HazardInfo", "LandmineCount"])
            .or_else(|| self.hazard_info.as_ref().map(|info| info.landmine_count))
            .unwrap_or(0)
    }

    pub fn spiketrap_count(&self) -> i64 {
        self.fallback_int(&["HazardInfo", "SpiketrapCount"])
            .or_else(|| self.hazard_info.as_ref().map(|info| info.spiketrap_count))
            .unwrap_or(0)
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

    pub fn bee_available_values(&self) -> Vec<i64> {
        self.fallback_int_array_any(&[&["BeeInfo", "Available"][..], &["BeeInfo", "Values"][..]])
            .unwrap_or_else(|| self.bee_info.available.clone())
    }

    pub fn bee_collected_values(&self) -> Vec<i64> {
        self.fallback_int_array_any(&[&["BeeInfo", "Collected"][..]])
            .unwrap_or_else(|| self.bee_info.collected.clone())
    }

    pub fn egg_available_values(&self) -> Vec<i64> {
        self.fallback_int_array_any(&[
            &["EggInfo", "Available"][..],
            &["BirdInfo", "EggValues"][..],
        ])
        .unwrap_or_else(|| self.egg_info.available.clone())
    }

    pub fn egg_collected_values(&self) -> Vec<i64> {
        self.fallback_int_array_any(&[
            &["EggInfo", "Collected"][..],
            &["BirdInfo", "CollectedEggValues"][..],
        ])
        .unwrap_or_else(|| self.egg_info.collected.clone())
    }

    pub fn knife_collected_values(&self) -> Vec<i64> {
        self.fallback_int_array_any(&[&["KnifeInfo", "Collected"][..]])
            .unwrap_or_else(|| self.knife_info.collected.clone())
    }

    pub fn shotgun_available_values(&self) -> Vec<i64> {
        self.fallback_int_array_any(&[&["ShotgunInfo", "Available"][..]])
            .unwrap_or_else(|| self.shotgun_info.available.clone())
    }

    pub fn shotgun_collected_values(&self) -> Vec<i64> {
        self.fallback_int_array_any(&[&["ShotgunInfo", "Collected"][..]])
            .unwrap_or_else(|| self.shotgun_info.collected.clone())
    }

    pub fn gift_boxes(&self) -> Vec<GiftBoxInfo> {
        self.fallback_source
            .as_ref()
            .and_then(|stats| {
                value_at_any(stats, &[&["GiftBoxesOpened"][..], &["GiftBoxes"][..]])
                    .and_then(Value::as_array)
                    .map(|gifts| gifts.iter().map(GiftBoxInfo::from_value).collect())
            })
            .unwrap_or_else(|| self.gift_boxes_opened.clone())
    }

    pub fn active_missed_items(&self) -> impl Iterator<Item = &MissingItemInfo> {
        self.missed_items
            .iter()
            .filter(|item| !item.collected_on_previous_day)
    }

    pub fn lost_missed_items(&self) -> impl Iterator<Item = &MissingItemInfo> {
        self.missed_items
            .iter()
            .filter(|item| item.collected_on_previous_day)
    }

    pub fn missed_item_count(&self) -> usize {
        self.active_missed_items().count()
    }

    pub fn lost_scrap_value(&self) -> i64 {
        self.lost_missed_items().map(|item| item.value).sum()
    }

    pub fn bee_available_count(&self) -> usize {
        self.bee_available_values().len()
    }

    pub fn bee_available_total(&self) -> i64 {
        self.bee_available_values().iter().sum()
    }

    pub fn egg_available_total(&self) -> i64 {
        self.egg_available_values().iter().sum()
    }

    pub fn shotgun_collected_count(&self) -> i64 {
        self.collected_count_or_legacy_int(&["ShotgunInfo", "Collected"], &["ShotgunsCollected"])
    }

    pub fn knife_collected_count(&self) -> i64 {
        self.collected_count_or_legacy_int(&["KnifeInfo", "Collected"], &["KnivesCollected"])
    }

    pub fn indoor_enemy_count(&self, enemy: &str) -> usize {
        self.indoor_spawns
            .iter()
            .filter(|spawn| spawn.enemy.eq_ignore_ascii_case(enemy))
            .count()
    }

    pub fn players_sorted(&self) -> Vec<PlayerEntry> {
        let mut players = self
            .players
            .iter()
            .map(|(steam_id, player)| PlayerEntry {
                steam_id: steam_id.clone(),
                stats: player.clone(),
            })
            .collect::<Vec<_>>();
        players.sort_by(|left, right| {
            player_sort_key(&left.steam_id, &left.stats)
                .cmp(&player_sort_key(&right.steam_id, &right.stats))
                .then_with(|| left.steam_id.cmp(&right.steam_id))
        });
        players
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

    fn fallback_int_array_any(&self, paths: &[&[&str]]) -> Option<Vec<i64>> {
        self.fallback_source.as_ref().and_then(|stats| {
            paths.iter().find_map(|path| {
                value_at(stats, path).and_then(|value| {
                    value
                        .as_array()
                        .map(|items| items.iter().map(intish_value).collect())
                })
            })
        })
    }

    fn collected_count_or_legacy_int(&self, collected_path: &[&str], legacy_path: &[&str]) -> i64 {
        self.fallback_source
            .as_ref()
            .and_then(|stats| value_at(stats, collected_path))
            .and_then(Value::as_array)
            .map(|items| items.len() as i64)
            .or_else(|| self.fallback_int(legacy_path))
            .unwrap_or(0)
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
    #[serde(deserialize_with = "deserialize_intish")]
    pub item_count: i64,
    pub interior: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct HazardInfo {
    #[serde(deserialize_with = "deserialize_intish")]
    pub turret_count: i64,
    #[serde(deserialize_with = "deserialize_intish")]
    pub landmine_count: i64,
    #[serde(deserialize_with = "deserialize_intish")]
    pub spiketrap_count: i64,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct PerformanceInfo {
    #[serde(deserialize_with = "deserialize_intish")]
    pub collected_no_extra: i64,
    #[serde(deserialize_with = "deserialize_intish")]
    pub collected_total: i64,
    #[serde(deserialize_with = "deserialize_intish")]
    pub initial_available_value: i64,
    #[serde(deserialize_with = "deserialize_intish")]
    pub total_available_value: i64,
    #[serde(deserialize_with = "deserialize_intish")]
    pub extra_from_old_gift: i64,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct SpecialItemInfo {
    #[serde(deserialize_with = "deserialize_vec_intish")]
    pub available: Vec<i64>,
    #[serde(deserialize_with = "deserialize_vec_intish")]
    pub collected: Vec<i64>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct QuotaInfo {
    #[serde(deserialize_with = "deserialize_intish")]
    pub value_sold: i64,
    #[serde(deserialize_with = "deserialize_intish")]
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
    #[serde(default, alias = "PlayerID", alias = "PlayerId", alias = "PlayerIndex")]
    #[serde(deserialize_with = "deserialize_option_intish")]
    pub player_id: Option<i64>,
    pub name: String,
    pub alive: bool,
    pub disconnected: bool,
    pub time_of_death: String,
    pub cause_of_death: String,
}

#[derive(Debug, Clone)]
pub struct PlayerEntry {
    pub steam_id: String,
    pub stats: PlayerStats,
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
    #[serde(deserialize_with = "deserialize_intish")]
    pub apparent_price: i64,
    #[serde(deserialize_with = "deserialize_intish")]
    pub real_price: i64,
    #[serde(deserialize_with = "deserialize_floatish")]
    pub luck: f64,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct GiftBoxInfo {
    #[serde(deserialize_with = "deserialize_intish")]
    pub new_scrap_value: i64,
    #[serde(deserialize_with = "deserialize_intish")]
    pub gift_scrap_value: i64,
    #[serde(deserialize_with = "deserialize_intish")]
    pub gift_box_age: i64,
    pub collected: bool,
}

impl GiftBoxInfo {
    fn from_value(value: &Value) -> Self {
        Self {
            new_scrap_value: value_at_any(value, &[&["NewScrapValue"][..], &["GiftValue"][..]])
                .map(intish_value)
                .unwrap_or(0),
            gift_scrap_value: value_at_any(value, &[&["GiftScrapValue"][..], &["ScrapValue"][..]])
                .map(intish_value)
                .unwrap_or_else(|| value.get("Value").map(intish_value).unwrap_or(0)),
            gift_box_age: value.get("GiftBoxAge").map(intish_value).unwrap_or(0),
            collected: value
                .get("Collected")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct MissingItemInfo {
    #[serde(deserialize_with = "deserialize_intish")]
    pub value: i64,
    pub item_type: String,
    pub spawn_position: Option<Vec<f64>>,
    pub despawn_position: Vec<f64>,
    pub collected_on_previous_day: bool,
    #[serde(deserialize_with = "deserialize_intish")]
    pub scrap_inside_gift_value: i64,
}

pub fn lcstats(stats: &Value) -> LcStats {
    LcStats::from_value(stats)
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

fn deserialize_intish<'de, D>(deserializer: D) -> Result<i64, D::Error>
where
    D: Deserializer<'de>,
{
    Ok(intish_value(&Value::deserialize(deserializer)?))
}

fn deserialize_option_intish<'de, D>(deserializer: D) -> Result<Option<i64>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Value::deserialize(deserializer)?;
    if value.is_null() {
        Ok(None)
    } else {
        Ok(Some(intish_value(&value)))
    }
}

fn deserialize_floatish<'de, D>(deserializer: D) -> Result<f64, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Value::deserialize(deserializer)?;
    Ok(value
        .as_f64()
        .or_else(|| value.as_i64().map(|number| number as f64))
        .or_else(|| value.as_u64().map(|number| number as f64))
        .or_else(|| {
            value
                .as_str()
                .and_then(|text| text.trim_start_matches('\'').trim().parse::<f64>().ok())
        })
        .unwrap_or(0.0))
}

fn deserialize_vec_intish<'de, D>(deserializer: D) -> Result<Vec<i64>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Value::deserialize(deserializer)?;
    Ok(value
        .as_array()
        .map(|items| items.iter().map(intish_value).collect())
        .unwrap_or_default())
}

fn deserialize_map_intish<'de, D>(deserializer: D) -> Result<BTreeMap<String, i64>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Value::deserialize(deserializer)?;
    Ok(value
        .as_object()
        .map(|object| {
            object
                .iter()
                .map(|(key, value)| (key.clone(), intish_value(value)))
                .collect()
        })
        .unwrap_or_default())
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

fn player_sort_key(key: &str, player: &PlayerStats) -> (bool, i64) {
    player
        .player_id
        .or_else(|| key.trim_start_matches('\'').trim().parse::<i64>().ok())
        .map(|id| (false, id))
        .unwrap_or((true, 0))
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
    fn lcstats_keeps_numeric_values_available_as_text() {
        let stats = json!({ "Seed": 30494987, "IndoorFog": false });
        let lc_stats = lcstats(&stats);

        assert_eq!(lc_stats.seed_text(), "30494987");
        assert!(!lc_stats.indoor_fog());
    }

    #[test]
    fn lcstats_reads_quoted_numbers() {
        let stats = json!({ "CollectedTotal": "'225" });

        assert_eq!(lcstats(&stats).collected_total(), 225);
    }

    #[test]
    fn lcstats_deserializes_quoted_numbers_without_defaulting_whole_stats() {
        let stats = json!({
            "Seed": "'123",
            "Version": "'70",
            "MoonInfo": { "Name": "'71 March", "Weather": "Mild" },
            "DungeonInfo": { "Interior": "'Mineshaft", "ItemCount": "'34" },
            "HazardInfo": {
                "TurretCount": "'1",
                "LandmineCount": "'2",
                "SpiketrapCount": "'3"
            },
            "PerformanceInfo": {
                "CollectedTotal": "'225",
                "InitialAvailableValue": "'300",
                "TotalAvailableValue": "'400"
            }
        });

        let lc_stats = lcstats(&stats);

        assert_eq!(lc_stats.seed(), 123);
        assert_eq!(lc_stats.version(), 70);
        assert_eq!(lc_stats.moon_name(), "'71 March");
        assert_eq!(lc_stats.dungeon_interior(), "'Mineshaft");
        assert_eq!(lc_stats.dungeon_item_count(), 34);
        assert_eq!(lc_stats.turret_count(), 1);
        assert_eq!(lc_stats.landmine_count(), 2);
        assert_eq!(lc_stats.spiketrap_count(), 3);
        assert_eq!(lc_stats.collected_total(), 225);
        assert_eq!(lc_stats.initial_available_value(), 300);
        assert_eq!(lc_stats.total_available_value(), 400);
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

        assert_eq!(lcstats(&new_stats).initial_available_value(), 300);
        assert_eq!(lcstats(&new_stats).total_available_value(), 400);
        assert_eq!(lcstats(&flat_new_stats).initial_available_value(), 200);
        assert_eq!(lcstats(&flat_new_stats).total_available_value(), 250);
        assert_eq!(lcstats(&old_stats).initial_available_value(), 30);
        assert_eq!(lcstats(&old_stats).total_available_value(), 40);
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

        let lc_stats = lcstats(&stats);
        assert_eq!(lc_stats.collected_total(), 100);
        assert_eq!(lc_stats.collected_no_extra(), 80);
        assert_eq!(lc_stats.value_sold(), 200);
        assert_eq!(lc_stats.new_quota(), 900);
        assert!(lc_stats.app_spawned());
        assert!(lc_stats.indoor_fog());
        assert_eq!(lc_stats.take_off_time(), "'11:00 PM");
        assert_eq!(lc_stats.sid_type(), "'Mineshaft");
        assert_eq!(lc_stats.infestation_type(), "'Spiders");
        assert_eq!(lc_stats.meteor_shower_time(), "'8:30 PM");
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

        let names = lcstats(&stats)
            .players_sorted()
            .into_iter()
            .map(|player| player.stats.name)
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

        let names = lcstats(&stats)
            .players_sorted()
            .into_iter()
            .map(|player| player.stats.name)
            .collect::<Vec<_>>();

        assert_eq!(names, vec!["A", "B", "C"]);
    }
}

//! Pure validation used by the Android bridge. Kept independent of Tauri for tests.
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Debug, PartialEq, Eq)]
pub struct SteamManifest {
    pub state_flags: u32,
    pub install_dir: String,
    pub depot_manifest: String,
}

#[derive(Debug)]
enum Vdf {
    Text(String),
    Object(BTreeMap<String, Vdf>),
}

impl Vdf {
    fn object(&self) -> Result<&BTreeMap<String, Vdf>, String> {
        match self {
            Self::Object(value) => Ok(value),
            _ => Err("Expected Steam manifest object".into()),
        }
    }
    fn text(&self) -> Result<&str, String> {
        match self {
            Self::Text(value) => Ok(value),
            _ => Err("Expected Steam manifest value".into()),
        }
    }
}

fn field<'a>(object: &'a BTreeMap<String, Vdf>, key: &str) -> Result<&'a Vdf, String> {
    object
        .get(key)
        .ok_or_else(|| format!("Steam manifest is missing {key}"))
}

pub fn parse_steam_manifest(text: &str) -> Result<SteamManifest, String> {
    if text.len() > 1024 * 1024 {
        return Err("Steam manifest is too large".into());
    }
    let mut chars = text.chars().peekable();
    let mut tokens = Vec::new();
    while let Some(ch) = chars.next() {
        if ch.is_whitespace() || ch == '\u{feff}' {
            continue;
        }
        if ch == '/' && chars.peek() == Some(&'/') {
            for next in chars.by_ref() {
                if next == '\n' {
                    break;
                }
            }
            continue;
        }
        if matches!(ch, '{' | '}') {
            tokens.push(ch.to_string());
            continue;
        }
        let mut token = String::new();
        if ch == '"' {
            let mut closed = false;
            while let Some(next) = chars.next() {
                if next == '"' {
                    closed = true;
                    break;
                }
                if next == '\\' {
                    let escaped = chars.next().ok_or("Truncated Steam manifest escape")?;
                    // Preserve unknown escapes, including Windows path separators.
                    if !matches!(escaped, '"' | '\\') {
                        token.push('\\');
                    }
                    token.push(escaped);
                } else {
                    token.push(next);
                }
            }
            if !closed {
                return Err("Truncated Steam manifest string".into());
            }
        } else {
            token.push(ch);
            while chars
                .peek()
                .is_some_and(|next| !next.is_whitespace() && !matches!(next, '{' | '}'))
            {
                token.push(chars.next().unwrap());
            }
        }
        tokens.push(token);
    }
    fn object(
        tokens: &[String],
        at: &mut usize,
        nested: bool,
        depth: usize,
    ) -> Result<BTreeMap<String, Vdf>, String> {
        if depth > 24 {
            return Err("Steam manifest nesting is too deep".into());
        }
        let mut result = BTreeMap::new();
        while *at < tokens.len() {
            let key = tokens[*at].to_ascii_lowercase();
            *at += 1;
            if key == "}" {
                return if nested {
                    Ok(result)
                } else {
                    Err("Unexpected Steam manifest brace".into())
                };
            }
            if key == "{" || key.is_empty() {
                return Err("Invalid Steam manifest key".into());
            }
            let value = tokens.get(*at).ok_or("Truncated Steam manifest value")?;
            *at += 1;
            let value = if value == "{" {
                Vdf::Object(object(tokens, at, true, depth + 1)?)
            } else if value == "}" {
                return Err("Missing Steam manifest value".into());
            } else {
                Vdf::Text(value.clone())
            };
            if result.insert(key, value).is_some() {
                return Err("Duplicate Steam manifest key".into());
            }
        }
        if nested {
            Err("Truncated Steam manifest object".into())
        } else {
            Ok(result)
        }
    }
    let top = object(&tokens, &mut 0, false, 0)?;
    let state = field(&top, "appstate")?.object()?;
    if field(state, "appid")?.text()? != "1966720" {
        return Err("Steam manifest is not Lethal Company".into());
    }
    let state_flags = field(state, "stateflags")?
        .text()?
        .parse::<u32>()
        .map_err(|_| "Invalid Steam StateFlags")?;
    let install_dir = field(state, "installdir")?.text()?.to_string();
    if install_dir.is_empty()
        || matches!(install_dir.as_str(), "." | "..")
        || install_dir
            .chars()
            .any(|ch| ch.is_control() || matches!(ch, '/' | '\\' | ':'))
    {
        return Err("Invalid Steam install directory".into());
    }
    let depot = field(field(state, "installeddepots")?.object()?, "1966721")?.object()?;
    let depot_manifest = field(depot, "manifest")?.text()?.to_string();
    if depot_manifest.is_empty() || !depot_manifest.bytes().all(|ch| ch.is_ascii_digit()) {
        return Err("Invalid Steam depot manifest ID".into());
    }
    Ok(SteamManifest {
        state_flags,
        install_dir,
        depot_manifest,
    })
}

pub fn game_version(versions: &BTreeMap<u32, String>, depot: &str) -> Option<u32> {
    let mut matching = versions.iter().filter(|(_, value)| value.as_str() == depot);
    let version = *matching.next()?.0;
    if matching.next().is_some() {
        None
    } else {
        Some(version)
    }
}

pub fn safe_mod_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == b'_')
}

pub fn android_mod_block_reason(dev: &str, name: &str, version: u32) -> Option<&'static str> {
    // PathfindingLib 2.4.1 alone stalls startup in the tested ARM64 runtime.
    // Disabling its optional OffMesh hook does not avoid the native detours.
    // Older library revisions and desktop selection are outside this policy.
    if version >= 73 && dev.eq_ignore_ascii_case("Zaggy1024") &&
        (name.eq_ignore_ascii_case("PathfindingLib") || name.eq_ignore_ascii_case("PathfindingLagFix")) {
        Some("Disabled on Android: PathfindingLib 2.4.1 stalls startup in the current runtime. PathfindingLagFix requires it.")
    } else { None }
}

pub fn safe_archive_entry(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with(['/', '\\'])
        && !name.contains(['\0', ':'])
        && !name.split(['/', '\\']).any(|part| part == "..")
}

pub fn checked_internal_path(root: &Path, path: &Path) -> Result<PathBuf, String> {
    use std::path::Component;
    if !root.is_absolute()
        || !path.is_absolute()
        || path
            .components()
            .any(|part| matches!(part, Component::ParentDir))
    {
        return Err("Expected an absolute internal path without traversal".into());
    }
    let relative = path
        .strip_prefix(root)
        .map_err(|_| "Path is outside HQ private storage")?;
    let mut cursor = root.to_path_buf();
    if std::fs::symlink_metadata(&cursor)
        .map_err(|e| e.to_string())?
        .file_type()
        .is_symlink()
    {
        return Err("HQ private root is a symlink".into());
    }
    for part in relative.components() {
        cursor.push(part);
        if std::fs::symlink_metadata(&cursor)
            .map_err(|e| e.to_string())?
            .file_type()
            .is_symlink()
        {
            return Err("Symlinks are not allowed in the internal game path".into());
        }
    }
    let canonical_root = root.canonicalize().map_err(|e| e.to_string())?;
    let canonical = path.canonicalize().map_err(|e| e.to_string())?;
    if !canonical.starts_with(&canonical_root) {
        return Err("Path escapes HQ private storage".into());
    }
    Ok(canonical)
}

pub struct GamePaths {
    pub private_root: PathBuf,
    pub game_dir: PathBuf,
    pub steam_manifest: PathBuf,
    pub manifest: SteamManifest,
    pub managed: Option<ManagedInstall>,
}

pub struct ManagedInstall {
    pub version: u32,
    pub depot_manifest: String,
    pub receipt: PathBuf,
}

impl GamePaths {
    pub fn depot_manifest(&self) -> &str {
        self.managed.as_ref().map_or(self.manifest.depot_manifest.as_str(), |value| value.depot_manifest.as_str())
    }
    pub fn mod_state_path(&self) -> PathBuf {
        self.managed.as_ref().map_or_else(|| self.private_root.join("hq-android-mods.json"), |value| value.receipt.parent().unwrap().join("mods.json"))
    }
}

/// A managed version has its own verified receipt. Keep the original Steam
/// manifest unchanged: it still describes Steam's separate current install.
pub fn validate_managed_game_paths(
    private: &Path, runtime: &Path, game: &Path, steam_manifest: &Path,
    receipt: &Path, version: u32, depot: &str,
) -> Result<GamePaths, String> {
    if !(1..=100000).contains(&version) || depot.is_empty() || depot.len() > 32 || !depot.bytes().all(|value| value.is_ascii_digit()) {
        return Err("Invalid managed game version".into());
    }
    let original = parse_steam_manifest(&std::fs::read_to_string(checked_internal_path(private, steam_manifest)?).map_err(|error| error.to_string())?)?;
    let mut paths = validate_game_paths(private, runtime, &steam_manifest.parent().ok_or("Missing Steam directory")?.join("common").join(original.install_dir), steam_manifest)?;
    let folder = runtime.join("root/.local/share/hq-launcher/versions").join(format!("{version}-{depot}"));
    if receipt != folder.join("install.json") || game != folder.join("game") {
        return Err("Managed game paths do not match their version".into());
    }
    let receipt = checked_internal_path(private, receipt)?;
    let game = checked_internal_path(private, game)?;
    if std::fs::metadata(&receipt).map_err(|error| error.to_string())?.len() > 64 * 1024 {
        return Err("Managed install receipt is too large".into());
    }
    let record: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&receipt).map_err(|error| error.to_string())?).map_err(|error| error.to_string())?;
    if record["format"].as_u64() != Some(1) || record["source"] != "steam-download-depot" ||
        record["complete"].as_bool() != Some(true) || record["version"].as_u64() != Some(version as u64) || record["depot_manifest"].as_str() != Some(depot) ||
        !game.join("Lethal Company.exe").is_file() || !game.join("Lethal Company_Data").is_dir() {
        return Err("Managed game installation is incomplete or does not match its receipt".into());
    }
    reject_symlink_tree(&game)?;
    paths.game_dir = game;
    paths.managed = Some(ManagedInstall { version, depot_manifest: depot.into(), receipt });
    Ok(paths)
}

pub fn validate_game_paths(
    private: &Path,
    runtime: &Path,
    game: &Path,
    manifest_path: &Path,
) -> Result<GamePaths, String> {
    let private_root = private.canonicalize().map_err(|e| e.to_string())?;
    let runtime = checked_internal_path(private, runtime)?;
    if runtime != private_root.join("linuxfs") {
        return Err("Runtime is not HQ's internal Linux installation".into());
    }
    let game_dir = checked_internal_path(private, game)?;
    let steam_manifest = checked_internal_path(private, manifest_path)?;
    let steamapps = steam_manifest
        .parent()
        .ok_or("Missing Steam library directory")?;
    let allowed_roots = [
        "root/.local/share/Steam/steamapps",
        "root/.droiddeck-x64/.local/share/Steam/steamapps",
    ];
    if !allowed_roots
        .iter()
        .any(|root| steamapps == runtime.join(root))
        || steam_manifest.file_name().and_then(|name| name.to_str())
            != Some("appmanifest_1966720.acf")
    {
        return Err("Steam manifest is outside HQ's internal Steam library".into());
    }
    let metadata = std::fs::metadata(&steam_manifest).map_err(|e| e.to_string())?;
    if metadata.len() > 1024 * 1024 {
        return Err("Steam manifest is too large".into());
    }
    let manifest = parse_steam_manifest(
        &std::fs::read_to_string(&steam_manifest).map_err(|e| e.to_string())?,
    )?;
    if game_dir != steamapps.join("common").join(&manifest.install_dir)
        || !game_dir.join("Lethal Company.exe").is_file()
        || !game_dir.join("Lethal Company_Data").is_dir()
    {
        return Err(
            "Game directory does not match the internal Lethal Company installation".into(),
        );
    }
    checked_internal_path(private, &game.join("Lethal Company.exe"))?;
    checked_internal_path(private, &game.join("Lethal Company_Data"))?;
    Ok(GamePaths {
        private_root,
        game_dir,
        steam_manifest,
        manifest,
        managed: None,
    })
}

pub fn reject_symlink_tree(root: &Path) -> Result<(), String> {
    let mut pending = vec![root.to_path_buf()];
    while let Some(path) = pending.pop() {
        let metadata = std::fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
        if metadata.file_type().is_symlink() {
            return Err(format!("Symlink in game files: {}", path.display()));
        }
        if metadata.is_dir() {
            for entry in std::fs::read_dir(path).map_err(|e| e.to_string())? {
                pending.push(entry.map_err(|e| e.to_string())?.path());
            }
        }
    }
    Ok(())
}

pub fn install_is_current(
    complete: bool,
    saved_depot: &str,
    current_depot: &str,
    all_mods_installed: bool,
) -> bool {
    complete && saved_depot == current_depot && all_mods_installed
}

pub fn validate_launch_state(
    saved: Option<(bool, &str)>,
    current_depot: &str,
    state_flags: u32,
) -> Result<(), String> {
    if state_flags != 4 {
        return Err("Steam 게임 설치 또는 업데이트가 끝난 뒤 실행하세요.".into());
    }
    if saved
        .is_some_and(|(complete, depot)| !install_is_current(complete, depot, current_depot, true))
    {
        return Err("게임이 업데이트되었거나 HQ 모드 적용이 완료되지 않았습니다. HQ 모드를 다시 적용한 뒤 실행하세요.".into());
    }
    Ok(())
}

pub const DEFAULT_DISABLED_MODS: &[(&str, &str)] = &[
    ("asta", "Replay"),
    ("asta", "StreamChats"),
    ("SlushyRH", "FreeeeeeMoooooons"),
    ("stormytuna", "EclipseOnly"),
];

pub fn is_run_mode_tag(tag: &str) -> bool {
    ["brutal", "wesley", "smhq", "eclipsed", "c.moons"]
        .iter()
        .any(|value| tag.eq_ignore_ascii_case(value))
}

#[derive(Clone)]
pub struct ModPolicy {
    pub key: String,
    pub remote_enabled: bool,
    pub switch_group: Option<String>,
}

pub fn effective_choices(
    mods: &[ModPolicy],
    saved: &BTreeMap<String, bool>,
) -> BTreeMap<String, bool> {
    let defaults: Vec<String> = DEFAULT_DISABLED_MODS
        .iter()
        .map(|(dev, name)| format!("{}::{}", dev.to_lowercase(), name.to_lowercase()))
        .collect();
    let mut result = BTreeMap::new();
    let mut groups = std::collections::BTreeSet::new();
    for spec in mods {
        let mut enabled = spec.remote_enabled
            && saved
                .get(&spec.key)
                .copied()
                .unwrap_or(!defaults.contains(&spec.key));
        if let Some(group) = spec
            .switch_group
            .as_deref()
            .map(str::trim)
            .filter(|group| !group.is_empty())
        {
            if enabled && !groups.insert(group.to_lowercase()) {
                enabled = false;
            }
        }
        result.insert(spec.key.clone(), enabled);
    }
    if result.get("asta::replay") == Some(&true) {
        result.insert("fumiko::cullfactory".into(), false);
    }
    result
}

pub fn toggle_choice(
    mods: &[ModPolicy],
    saved: &BTreeMap<String, bool>,
    key: &str,
    enabled: bool,
) -> Result<BTreeMap<String, bool>, String> {
    let spec = mods
        .iter()
        .find(|spec| spec.key == key)
        .ok_or("Unknown or incompatible HQ mod")?;
    if enabled && !spec.remote_enabled {
        return Err("This mod is disabled by the HQ manifest".into());
    }
    let mut choices = saved.clone();
    choices.extend(effective_choices(mods, saved));
    if enabled && key == "fumiko::cullfactory" && choices.get("asta::replay") == Some(&true) {
        return Err("CullFactory cannot be enabled while Replay is enabled".into());
    }
    choices.insert(key.to_string(), enabled);
    if enabled {
        if let Some(group) = spec
            .switch_group
            .as_deref()
            .map(str::trim)
            .filter(|group| !group.is_empty())
        {
            for other in mods {
                if other.key != key
                    && other
                        .switch_group
                        .as_deref()
                        .is_some_and(|other| other.trim().eq_ignore_ascii_case(group))
                {
                    choices.insert(other.key.clone(), false);
                }
            }
        }
        if key == "asta::replay" {
            choices.insert("fumiko::cullfactory".into(), false);
        }
    }
    Ok(choices)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    const ACF: &str = r#""AppState" {
        "appid" "1966720" "StateFlags" "4" "installdir" "Lethal Company"
        "buildid" "999" "manifest" "wrong-root"
        "InstalledDepots" {
            "123" { "manifest" "wrong-depot" }
            "1966721" { "manifest" "1749099131234587692" }
        }
    }"#;

    #[test]
    fn scoped_depot_manifest_ignores_other_manifest_keys() {
        assert_eq!(
            parse_steam_manifest(ACF).unwrap(),
            SteamManifest {
                state_flags: 4,
                install_dir: "Lethal Company".into(),
                depot_manifest: "1749099131234587692".into(),
            }
        );
    }

    #[test]
    fn rejects_wrong_app_duplicate_keys_and_traversing_install_dir() {
        for text in [
            ACF.replace("1966720", "123"),
            ACF.replace(
                "\"StateFlags\" \"4\"",
                "\"StateFlags\" \"4\" \"StateFlags\" \"4\"",
            ),
            ACF.replace("Lethal Company", "../outside"),
            ACF.replace("Lethal Company", "..\\outside"),
            ACF[..ACF.len() - 1].to_string(),
        ] {
            assert!(parse_steam_manifest(&text).is_err(), "accepted {text}");
        }
    }

    #[test]
    fn maps_depot_to_game_version_without_using_schema_or_build_version() {
        let versions = BTreeMap::from([
            (73, "1749099131234587692".into()),
            (81, "6423525044216269478".into()),
        ]);
        assert_eq!(game_version(&versions, "1749099131234587692"), Some(73));
        assert_eq!(game_version(&versions, "999"), None);
        assert_eq!(game_version(&versions, "unknown"), None);
    }

    #[test]
    fn package_ids_cannot_become_paths() {
        for id in ["HQHQTeam", "HQoL", "BepInExPack", "Some_Mod2"] {
            assert!(safe_mod_id(id));
        }
        for id in ["", ".", "..", "../outside", "x/y", "x\\y", "C:foo", "x\0y"] {
            assert!(!safe_mod_id(id));
        }
    }

    #[test]
    fn archive_paths_remain_safe_after_wrapper_or_section_prefix_is_removed() {
        for path in [
            "BepInExPack/BepInEx/core/BepInEx.dll",
            "plugins/Example/plugin.dll",
            "manifest.json",
        ] {
            assert!(safe_archive_entry(path), "{path}");
        }
        for path in [
            "BepInExPack/../outside",
            "plugins/../../outside",
            "wrapper/plugins/../outside",
            "wrapper\\..\\outside",
            "/absolute",
            "C:\\absolute",
            "x\0y",
        ] {
            assert!(!safe_archive_entry(path), "{path}");
        }
    }

    fn policies() -> Vec<ModPolicy> {
        [
            ("asta::replay", true, None),
            ("fumiko::cullfactory", true, None),
            ("asta::streamchats", true, None),
            ("one::a", true, Some("choice")),
            ("two::b", true, Some("CHOICE")),
            ("disabled::remote", false, None),
        ]
        .into_iter()
        .map(|(key, remote_enabled, group)| ModPolicy {
            key: key.into(),
            remote_enabled,
            switch_group: group.map(String::from),
        })
        .collect()
    }

    #[test]
    fn hq_defaults_keep_opt_in_and_remote_disabled_mods_off_and_choose_one_switch() {
        let result = effective_choices(&policies(), &BTreeMap::new());
        for (key, expected) in [
            ("asta::replay", false),
            ("asta::streamchats", false),
            ("fumiko::cullfactory", true),
            ("one::a", true),
            ("two::b", false),
            ("disabled::remote", false),
        ] {
            assert_eq!(result.get(key), Some(&expected), "{key}");
        }
    }

    #[test]
    fn android_startup_compatibility_is_limited_to_the_reproduced_library_and_its_dependent() {
        assert!(android_mod_block_reason("Zaggy1024", "PathfindingLib", 81).is_some());
        assert!(android_mod_block_reason("Zaggy1024", "PathfindingLagFix", 73).is_some());
        assert!(android_mod_block_reason("Zaggy1024", "PathfindingLib", 72).is_none());
        assert!(android_mod_block_reason("fumiko", "CullFactory", 81).is_none());
        assert!(android_mod_block_reason("HQHQTeam", "VLog", 81).is_none());
    }

    #[test]
    fn replay_disables_cullfactory_and_cullfactory_cannot_be_enabled_while_replay_is_on() {
        let choices = toggle_choice(&policies(), &BTreeMap::new(), "asta::replay", true).unwrap();
        assert_eq!(choices.get("fumiko::cullfactory"), Some(&false));
        assert_eq!(choices.get("asta::replay"), Some(&true));
        assert!(toggle_choice(&policies(), &choices, "fumiko::cullfactory", true).is_err());
    }

    #[test]
    fn explicit_group_choices_and_all_off_are_preserved() {
        let selected = toggle_choice(&policies(), &BTreeMap::new(), "two::b", true).unwrap();
        assert_eq!(selected.get("one::a"), Some(&false));
        assert_eq!(selected.get("two::b"), Some(&true));
        let off = toggle_choice(&policies(), &selected, "two::b", false).unwrap();
        let effective = effective_choices(&policies(), &off);
        assert_eq!(effective.get("one::a"), Some(&false));
        assert_eq!(effective.get("two::b"), Some(&false));
        assert!(toggle_choice(&policies(), &off, "disabled::remote", true).is_err());
        assert!(toggle_choice(&policies(), &off, "unknown::mod", true).is_err());
    }

    struct Temp(PathBuf);
    impl Temp {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "hq-android-path-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn owned_paths_require_real_component_boundaries() {
        let temp = Temp::new();
        let root = temp.0.join("private");
        let child = root.join("linuxfs/game");
        let outside = temp.0.join("private-other/game");
        std::fs::create_dir_all(&child).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        assert_eq!(
            checked_internal_path(&root, &child).unwrap(),
            child.canonicalize().unwrap()
        );
        assert!(checked_internal_path(&root, &outside).is_err());
        assert!(checked_internal_path(&root, &root.join("linuxfs/../linuxfs/game")).is_err());
    }

    #[test]
    fn game_paths_must_match_owned_runtime_steam_manifest_and_install_directory() {
        let temp = Temp::new();
        let private = temp.0.join("private");
        let runtime = private.join("linuxfs");
        let apps = runtime.join("root/.local/share/Steam/steamapps");
        let game = apps.join("common/Lethal Company");
        std::fs::create_dir_all(game.join("Lethal Company_Data")).unwrap();
        std::fs::write(game.join("Lethal Company.exe"), b"exe").unwrap();
        let manifest = apps.join("appmanifest_1966720.acf");
        std::fs::write(&manifest, ACF).unwrap();
        assert!(validate_game_paths(&private, &runtime, &game, &manifest).is_ok());
        let other = apps.join("common/Other Game");
        std::fs::create_dir_all(&other).unwrap();
        assert!(validate_game_paths(&private, &runtime, &other, &manifest).is_err());
        assert!(validate_game_paths(&private, &private, &game, &manifest).is_err());
    }

    #[test]
    fn managed_versions_require_complete_matching_receipts_and_keep_real_steam_metadata() {
        let temp = Temp::new();
        let private = temp.0.join("private");
        let runtime = private.join("linuxfs");
        let apps = runtime.join("root/.local/share/Steam/steamapps");
        let steam_game = apps.join("common/Lethal Company");
        std::fs::create_dir_all(steam_game.join("Lethal Company_Data")).unwrap();
        std::fs::write(steam_game.join("Lethal Company.exe"), b"exe").unwrap();
        let manifest = apps.join("appmanifest_1966720.acf");
        std::fs::write(&manifest, ACF).unwrap();
        let folder = runtime.join("root/.local/share/hq-launcher/versions/72-4861510547912001926");
        let game = folder.join("game");
        std::fs::create_dir_all(game.join("Lethal Company_Data")).unwrap();
        std::fs::write(game.join("Lethal Company.exe"), b"exe").unwrap();
        let receipt = folder.join("install.json");
        let record = serde_json::json!({"format":1,"source":"steam-download-depot","version":72,"depot_manifest":"4861510547912001926","complete":true});
        std::fs::write(&receipt, record.to_string()).unwrap();
        let paths = validate_managed_game_paths(&private, &runtime, &game, &manifest, &receipt, 72, "4861510547912001926").unwrap();
        assert_eq!(paths.depot_manifest(), "4861510547912001926");
        assert_eq!(paths.manifest.depot_manifest, "1749099131234587692");
        assert_eq!(paths.mod_state_path(), receipt.canonicalize().unwrap().parent().unwrap().join("mods.json"));
        assert!(validate_managed_game_paths(&private, &runtime, &game, &manifest, &receipt, 73, "4861510547912001926").is_err());
        let mut incomplete = record.clone();
        incomplete["complete"] = false.into();
        std::fs::write(&receipt, incomplete.to_string()).unwrap();
        assert!(validate_managed_game_paths(&private, &runtime, &game, &manifest, &receipt, 72, "4861510547912001926").is_err());
        std::fs::write(&receipt, record.to_string()).unwrap();
        assert!(validate_managed_game_paths(&private, &runtime, &steam_game, &manifest, &receipt, 72, "4861510547912001926").is_err());
        std::fs::remove_file(game.join("Lethal Company.exe")).unwrap();
        assert!(validate_managed_game_paths(&private, &runtime, &game, &manifest, &receipt, 72, "4861510547912001926").is_err());
    }

    #[test]
    fn completion_marker_cannot_survive_a_steam_update_or_partial_install() {
        assert!(install_is_current(true, "old", "old", true));
        assert!(!install_is_current(true, "old", "new", true));
        assert!(!install_is_current(false, "old", "old", true));
        assert!(!install_is_current(true, "old", "old", false));
    }

    #[test]
    fn launch_allows_fresh_base_game_but_rejects_partial_or_outdated_hq_state_offline() {
        assert!(validate_launch_state(None, "new", 4).is_ok());
        assert!(validate_launch_state(Some((true, "old")), "old", 4).is_ok());
        assert!(validate_launch_state(Some((false, "old")), "old", 4).is_err());
        assert!(validate_launch_state(Some((true, "old")), "new", 4).is_err());
        assert!(validate_launch_state(None, "new", 36).is_err());
        assert!(validate_launch_state(Some((true, "new")), "new", 36).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlink_ancestors_even_when_the_final_path_exists() {
        let temp = Temp::new();
        let root = temp.0.join("private");
        std::fs::create_dir_all(&root).unwrap();
        let outside = temp.0.join("outside");
        std::fs::create_dir_all(outside.join("game")).unwrap();
        std::os::unix::fs::symlink(&outside, root.join("escape")).unwrap();
        assert!(checked_internal_path(&root, &root.join("escape/game")).is_err());
    }
}

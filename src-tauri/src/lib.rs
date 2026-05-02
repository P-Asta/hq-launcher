mod bepinex_cfg;
mod discord_presence;
mod downloader;
mod installer;
mod logger;
mod mod_config;
mod mods;
mod progress;
mod thunderstore;
mod variable;
mod zip_utils;

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::ffi::OsString;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::sync::Mutex;
use tauri::{Manager, State};

#[cfg(target_os = "windows")]
use std::ffi::CString;
#[cfg(target_os = "windows")]
use windows_sys::Win32::Foundation::{CloseHandle, GetLastError};
#[cfg(target_os = "windows")]
use windows_sys::Win32::System::Diagnostics::Debug::WriteProcessMemory;
#[cfg(target_os = "windows")]
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Thread32First, Thread32Next, TH32CS_SNAPTHREAD, THREADENTRY32,
};
#[cfg(target_os = "windows")]
use windows_sys::Win32::System::LibraryLoader::{GetModuleHandleA, GetProcAddress};
#[cfg(target_os = "windows")]
use windows_sys::Win32::System::Memory::{
    VirtualAllocEx, VirtualFreeEx, MEM_COMMIT, MEM_RELEASE, MEM_RESERVE, PAGE_READWRITE,
};
#[cfg(target_os = "windows")]
use windows_sys::Win32::System::Threading::{
    CreateRemoteThread, OpenProcess, OpenThread, ResumeThread, WaitForSingleObject, INFINITE,
    PROCESS_CREATE_THREAD, PROCESS_QUERY_INFORMATION, PROCESS_VM_OPERATION, PROCESS_VM_READ,
    PROCESS_VM_WRITE, THREAD_SUSPEND_RESUME,
};
#[cfg(target_os = "windows")]
use winreg::enums::{HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_32KEY, KEY_WOW64_64KEY};
#[cfg(target_os = "windows")]
use winreg::RegKey;

use crate::bepinex_cfg::read_manifest;
use crate::progress::{TaskErrorPayload, TaskProgressPayload};
use crate::{
    mod_config::ModsConfig,
    progress::{TaskFinishedPayload, TaskUpdatableProgressPayload},
};

const INSTALL_COMPLETE_MARKER: &str = ".hq_install_complete";

fn manifest_state_has_version(app: &tauri::AppHandle, version: u32) -> bool {
    let Ok(app_data) = app.path().app_data_dir() else {
        return false;
    };
    let path = app_data.join("config").join("manifest_state.json");
    let Ok(text) = std::fs::read_to_string(path) else {
        return false;
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else {
        return false;
    };
    value
        .get("depot_manifests")
        .and_then(|v| v.get(version.to_string()))
        .is_some()
}

fn has_legacy_complete_files(path: &Path) -> bool {
    path.join("Lethal Company.exe").is_file()
        && path.join("UnityPlayer.dll").is_file()
        && path.join("Lethal Company_Data").is_dir()
        && path.join("winhttp.dll").is_file()
        && path.join("BepInEx").join("core").is_dir()
}

fn is_complete_version_dir(app: &tauri::AppHandle, version: u32, path: &Path) -> bool {
    path.join(INSTALL_COMPLETE_MARKER).is_file()
        || (manifest_state_has_version(app, version) && has_legacy_complete_files(path))
}

fn preset_tags_for_name(preset: &str) -> Vec<String> {
    let p = preset.trim().to_lowercase();
    match p.as_str() {
        "brutal" | "bc" => vec!["Brutal".to_string()],
        "brutal_smhq" | "bcsmhq" | "brutal-smhq" => {
            vec!["Brutal".to_string(), "SMHQ".to_string()]
        }
        "brutal_eclipsed" | "brutal-eclipsed" => {
            vec!["Brutal".to_string(), "Eclipsed".to_string()]
        }
        "wesley" | "wesley's" | "wesleys" => vec!["Wesley".to_string()],
        "wesley_eclipsed" | "wesleys_eclipsed" | "wesley-eclipsed" | "wesleys-eclipsed" => {
            vec!["Wesley".to_string(), "Eclipsed".to_string()]
        }
        "smhq" => vec!["SMHQ".to_string()],
        "eclipsed" | "eclipsed_hq" | "eclipsed-hq" => vec!["Eclipsed".to_string()],
        "c_moons" | "cmoons" | "c.moons" => vec!["C.Moons".to_string()],
        "c_moons_eclipsed" | "cmoons_eclipsed" | "c.moons_eclipsed" | "c-moons-eclipsed" => {
            vec!["C.Moons".to_string(), "Eclipsed".to_string()]
        }
        "c_moons_smhq" | "cmoons_smhq" | "c.moons_smhq" | "c-moons-smhq" => {
            vec!["C.Moons".to_string(), "SMHQ".to_string()]
        }
        "wesley_smhq" | "wesleys_smhq" | "wesley-smhq" | "wesleys-smhq" => {
            vec!["Wesley".to_string(), "SMHQ".to_string()]
        }
        _ => vec![],
    }
}

fn preset_and_practice_for_run_mode(run_mode: &str) -> (String, bool) {
    let mode = run_mode.trim().to_lowercase();
    match mode.as_str() {
        "practice" => ("hq".to_string(), true),
        "brutal" => ("brutal".to_string(), false),
        "brutal_smhq" => ("brutal_smhq".to_string(), false),
        "brutal_eclipsed" => ("brutal_eclipsed".to_string(), false),
        "brutal_practice" => ("brutal".to_string(), true),
        "wesley" => ("wesley".to_string(), false),
        "wesley_practice" => ("wesley".to_string(), true),
        "wesley_smhq" => ("wesley_smhq".to_string(), false),
        "wesley_eclipsed" => ("wesley_eclipsed".to_string(), false),
        "smhq" => ("smhq".to_string(), false),
        "eclipsed_hq" => ("eclipsed_hq".to_string(), false),
        "c_moons" => ("c_moons".to_string(), false),
        "c_moons_practice" => ("c_moons".to_string(), true),
        "c_moons_eclipsed" => ("c_moons_eclipsed".to_string(), false),
        "c_moons_smhq" => ("c_moons_smhq".to_string(), false),
        _ => ("hq".to_string(), false),
    }
}

fn is_run_mode_tag(tag: &str) -> bool {
    tag.eq_ignore_ascii_case("brutal")
        || tag.eq_ignore_ascii_case("wesley")
        || tag.eq_ignore_ascii_case("smhq")
        || tag.eq_ignore_ascii_case("eclipsed")
        || tag.eq_ignore_ascii_case("c.moons")
}

fn mod_has_run_mode_affinity(spec: &mod_config::ModEntry) -> bool {
    spec.tags.iter().any(|tag| is_run_mode_tag(tag))
        || spec.tag_constraints.keys().any(|tag| is_run_mode_tag(tag))
}

fn is_wesley_base_run(tags: &[String], practice: bool) -> bool {
    !practice
        && tags.iter().any(|t| t.eq_ignore_ascii_case("wesley"))
        && !tags.iter().any(|t| t.eq_ignore_ascii_case("smhq"))
}

const HQOL_DONT_STORE_CFG_FILES: [&str; 2] = ["OreoM.HQoL.72.cfg", "OreoM.HQoL.73.cfg"];

const WESLEY_HQOL_DONT_STORE_ITEMS: [&str; 18] = [
    "Royal apparatus",
    "Bloody apparatus",
    "Cosmic apparatus",
    "Atlantica videotape",
    "Acidir videotape",
    "Asteroid-13 videotape",
    "Junic videotape",
    "Hyx videotape",
    "Floppy disk",
    "Infernis videotape",
    "Etern videotape",
    "Empra videotape",
    "Filitrios videotape",
    "Motra videotape",
    "Hyve videotape",
    "Utril videotape",
    "Gratar videotape",
    "Gloom videotape",
];

const LETHAL_COMPANY_STEAM_APP_ID: &str = "1966720";

#[cfg(target_os = "windows")]
fn inject_dll_into_process(pid: u32, dll_path: &std::path::Path) -> Result<(), String> {
    use std::ptr::{null, null_mut};

    let dll_path = dll_path.to_str().ok_or_else(|| {
        format!(
            "dll path contains non-utf8 characters: {}",
            dll_path.display()
        )
    })?;
    let dll_path_cstr = CString::new(dll_path)
        .map_err(|_| format!("dll path contains interior NUL: {dll_path}"))?;
    let process = unsafe {
        OpenProcess(
            PROCESS_CREATE_THREAD
                | PROCESS_QUERY_INFORMATION
                | PROCESS_VM_OPERATION
                | PROCESS_VM_WRITE
                | PROCESS_VM_READ,
            0,
            pid,
        )
    };
    if process.is_null() {
        return Err(format!(
            "failed to open process {pid} for injection (Win32 error {})",
            unsafe { GetLastError() }
        ));
    }

    let result = (|| {
        let alloc_size = dll_path_cstr.as_bytes_with_nul().len();
        let remote_memory = unsafe {
            VirtualAllocEx(
                process,
                null_mut(),
                alloc_size,
                MEM_COMMIT | MEM_RESERVE,
                PAGE_READWRITE,
            )
        };
        if remote_memory.is_null() {
            return Err(format!(
                "failed to allocate remote memory for injection (Win32 error {})",
                unsafe { GetLastError() }
            ));
        }

        let write_ok = unsafe {
            WriteProcessMemory(
                process,
                remote_memory,
                dll_path_cstr.as_ptr().cast(),
                alloc_size,
                null_mut(),
            )
        };
        if write_ok == 0 {
            unsafe {
                VirtualFreeEx(process, remote_memory, 0, MEM_RELEASE);
            }
            return Err(format!(
                "failed to write DLL path into target process (Win32 error {})",
                unsafe { GetLastError() }
            ));
        }

        let kernel32 = unsafe { GetModuleHandleA(c"kernel32.dll".as_ptr().cast()) };
        if kernel32.is_null() {
            unsafe {
                VirtualFreeEx(process, remote_memory, 0, MEM_RELEASE);
            }
            return Err(format!(
                "failed to resolve kernel32.dll handle (Win32 error {})",
                unsafe { GetLastError() }
            ));
        }

        let Some(load_library) =
            (unsafe { GetProcAddress(kernel32, c"LoadLibraryA".as_ptr().cast()) })
        else {
            unsafe {
                VirtualFreeEx(process, remote_memory, 0, MEM_RELEASE);
            }
            return Err(format!(
                "failed to resolve LoadLibraryA (Win32 error {})",
                unsafe { GetLastError() }
            ));
        };

        let remote_thread = unsafe {
            CreateRemoteThread(
                process,
                null(),
                0,
                Some(std::mem::transmute(load_library)),
                remote_memory,
                0,
                null_mut(),
            )
        };
        if remote_thread.is_null() {
            unsafe {
                VirtualFreeEx(process, remote_memory, 0, MEM_RELEASE);
            }
            return Err(format!(
                "failed to create remote thread for injection (Win32 error {})",
                unsafe { GetLastError() }
            ));
        }

        unsafe {
            WaitForSingleObject(remote_thread, INFINITE);
            CloseHandle(remote_thread);
            VirtualFreeEx(process, remote_memory, 0, MEM_RELEASE);
        }
        Ok(())
    })();

    unsafe {
        CloseHandle(process);
    }
    result
}

#[cfg(not(target_os = "windows"))]
fn inject_dll_into_process(_pid: u32, _dll_path: &std::path::Path) -> Result<(), String> {
    Err("DLL injection is only supported on Windows".to_string())
}

#[cfg(target_os = "windows")]
fn get_windows_steam_install_path(configured_path: Option<&str>) -> Option<std::path::PathBuf> {
    if let Some(configured) = configured_path
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let candidate = std::path::PathBuf::from(configured);
        if candidate.exists() {
            return Some(candidate);
        }
    }

    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let subkeys = [
        ("SOFTWARE\\Valve\\Steam", KEY_READ | KEY_WOW64_64KEY),
        (
            "SOFTWARE\\WOW6432Node\\Valve\\Steam",
            KEY_READ | KEY_WOW64_64KEY,
        ),
        ("SOFTWARE\\Valve\\Steam", KEY_READ | KEY_WOW64_32KEY),
        (
            "SOFTWARE\\WOW6432Node\\Valve\\Steam",
            KEY_READ | KEY_WOW64_32KEY,
        ),
    ];

    for (subkey, flags) in subkeys {
        let Ok(key) = hklm.open_subkey_with_flags(subkey, flags) else {
            continue;
        };
        let Ok(path) = key.get_value::<String, _>("InstallPath") else {
            continue;
        };
        let candidate = std::path::PathBuf::from(path);
        if candidate.exists() {
            return Some(candidate);
        }
    }

    [
        std::env::var_os("ProgramFiles(x86)")
            .map(std::path::PathBuf::from)
            .map(|base| base.join("Steam")),
        std::env::var_os("ProgramFiles")
            .map(std::path::PathBuf::from)
            .map(|base| base.join("Steam")),
        Some(std::path::PathBuf::from(r"C:\Program Files (x86)\Steam")),
    ]
    .into_iter()
    .flatten()
    .find(|path| path.exists())
}

#[cfg(target_os = "windows")]
fn steam_overlay_dlls(steam_path: &std::path::Path) -> Vec<std::path::PathBuf> {
    vec![
        steam_path.join("tier0_s64.dll"),
        steam_path.join("vstdlib_s64.dll"),
        steam_path.join("steamclient64.dll"),
        // steam_path.join("win64").join("gameoverlayui.dll"),
        steam_path.join("GameOverlayRenderer64.dll"),
    ]
}

#[cfg(target_os = "windows")]
fn resume_main_thread(pid: u32) -> Result<(), String> {
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) };
    if snapshot == windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE {
        return Err(format!(
            "failed to enumerate process threads (Win32 error {})",
            unsafe { GetLastError() }
        ));
    }

    let result = (|| {
        let mut entry = THREADENTRY32 {
            dwSize: std::mem::size_of::<THREADENTRY32>() as u32,
            ..Default::default()
        };

        let mut has_entry = unsafe { Thread32First(snapshot, &mut entry) } != 0;
        while has_entry {
            if entry.th32OwnerProcessID == pid {
                let thread = unsafe { OpenThread(THREAD_SUSPEND_RESUME, 0, entry.th32ThreadID) };
                if thread.is_null() {
                    return Err(format!(
                        "failed to open suspended main thread (Win32 error {})",
                        unsafe { GetLastError() }
                    ));
                }

                let resume_result = unsafe { ResumeThread(thread) };
                unsafe {
                    CloseHandle(thread);
                }
                if resume_result == u32::MAX {
                    return Err(format!(
                        "failed to resume suspended process thread (Win32 error {})",
                        unsafe { GetLastError() }
                    ));
                }

                return Ok(());
            }

            has_entry = unsafe { Thread32Next(snapshot, &mut entry) } != 0;
        }

        Err(format!("no thread found for suspended process {pid}"))
    })();

    unsafe {
        CloseHandle(snapshot);
    }

    result
}

fn merge_mod_entries_prefer_later(
    base: Vec<mod_config::ModEntry>,
    overlay: Vec<mod_config::ModEntry>,
) -> Vec<mod_config::ModEntry> {
    let overlay_names: std::collections::HashSet<String> =
        overlay.iter().map(|m| m.name.to_lowercase()).collect();

    let mut merged: Vec<mod_config::ModEntry> = base
        .into_iter()
        .filter(|m| !overlay_names.contains(&m.name.to_lowercase()))
        .collect();
    merged.extend(overlay);
    merged
}

fn is_ui_hidden_mod(m: &mod_config::ModEntry) -> bool {
    m.tags.iter().any(|t| t.eq_ignore_ascii_case("ui_hidden"))
}

async fn effective_mods_config_for_run_mode(
    client: &reqwest::Client,
    version: u32,
    run_mode: &str,
    include_ui_hidden: bool,
    include_practice_mods: bool,
) -> Result<ModsConfig, String> {
    let (_remote_manifest_version, mods_cfg, _chain_config, _manifests, preset_tag_constraints) =
        ModsConfig::fetch_manifest(client).await?;
    let (preset, practice) = preset_and_practice_for_run_mode(run_mode);
    let tags = preset_tags_for_name(&preset);
    let want: Vec<String> = tags.iter().map(|t| t.to_lowercase()).collect();
    let preset_tags_supported = tags
        .iter()
        .all(|tag| preset_tag_supported_for_version(version, &preset_tag_constraints, tag));

    let mut base: Vec<mod_config::ModEntry> = vec![];
    let mut selected_tagged: Vec<mod_config::ModEntry> = vec![];

    for m in mods_cfg.mods {
        let has_run_mode_affinity = mod_has_run_mode_affinity(&m);
        let applies_to_preset = !want.is_empty() && want.iter().any(|tag| m.applies_to_tag(tag));
        let can_apply_as_base = !has_run_mode_affinity || m.tags.is_empty();

        if can_apply_as_base && m.is_compatible(version) {
            base.push(m);
            continue;
        }

        if !has_run_mode_affinity {
            continue;
        }

        if !applies_to_preset {
            continue;
        }

        if !preset_tags_supported {
            continue;
        }

        let mut mm = m;
        mm.enabled = true;
        if mm.is_compatible_for_tags(version, &tags) {
            selected_tagged.push(mm);
        }
    }

    let mut effective = base;
    if practice {
        let practice_mods: Vec<mod_config::ModEntry> = variable::get_practice_mod_list()
            .into_iter()
            .filter(|m| m.is_compatible(version))
            .collect();
        let practice_names: std::collections::HashSet<String> = practice_mods
            .iter()
            .map(|m| m.name.to_lowercase())
            .collect();
        effective.retain(|m| !practice_names.contains(&m.name.to_lowercase()));
        if include_practice_mods {
            effective.extend(practice_mods);
        }
    }
    if !selected_tagged.is_empty() {
        effective = merge_mod_entries_prefer_later(effective, selected_tagged);
    }

    if !include_ui_hidden {
        effective.retain(|m| !is_ui_hidden_mod(m));
    }

    Ok(ModsConfig { mods: effective })
}

async fn find_mod_entry_for_install(
    version: u32,
    dev: &str,
    name: &str,
) -> Result<Option<mod_config::ModEntry>, String> {
    let practice_match = variable::get_practice_mod_list().into_iter().find(|m| {
        m.is_compatible(version)
            && m.dev.eq_ignore_ascii_case(dev)
            && m.name.eq_ignore_ascii_case(name)
    });
    if practice_match.is_some() {
        return Ok(practice_match);
    }

    let client = reqwest::Client::new();
    let (_remote_manifest_version, mods_cfg, _chain_config, _manifests, _preset_tag_constraints) =
        ModsConfig::fetch_manifest(&client).await?;

    Ok(mods_cfg.mods.into_iter().find(|m| {
        m.enabled
            && m.is_compatible(version)
            && m.dev.eq_ignore_ascii_case(dev)
            && m.name.eq_ignore_ascii_case(name)
    }))
}

async fn prepare_tagged_mods_for_version(
    app: &tauri::AppHandle,
    version: u32,
    tags: &[String],
    step_name: &str,
    cancel: Option<Arc<AtomicBool>>,
) -> Result<Vec<(String, String)>, String> {
    if tags.is_empty() {
        return Ok(vec![]);
    }
    let game_root = version_dir(app, version)?;
    if !game_root.exists() {
        return Err(format!(
            "version folder not found: {}",
            game_root.to_string_lossy()
        ));
    }

    let client = reqwest::Client::new();
    let (_remote_manifest_version, mods_cfg, _chain_config, _manifests, preset_tag_constraints) =
        ModsConfig::fetch_manifest_with_cancel(&client, cancel.as_ref()).await?;
    validate_preset_tags_for_version(version, tags, &preset_tag_constraints)?;

    let want: Vec<String> = tags.iter().map(|t| t.to_lowercase()).collect();
    let mut tagged: Vec<mod_config::ModEntry> = vec![];
    for m in mods_cfg.mods {
        let has = want.iter().any(|tag| m.applies_to_tag(tag));
        if !has {
            continue;
        }
        // Override enabled=false for "optional" tagged mods.
        let mut mm = m;
        mm.enabled = true;
        tagged.push(mm);
    }

    // Only install compatible subset (same semantics as practice list).
    let tagged: Vec<mod_config::ModEntry> = tagged
        .into_iter()
        .filter(|m| m.is_compatible_for_tags(version, tags))
        .collect();
    if tagged.is_empty() {
        return Ok(vec![]);
    }

    // Return ids so the caller can force-enable these mods for this run (even if they overlap practice-disable rules).
    let tagged_ids: Vec<(String, String)> = tagged
        .iter()
        .map(|m| (m.dev.clone(), m.name.clone()))
        .collect();

    let missing_tagged_mods = filter_missing_mods_for_version(app, version, &tagged)?;
    if !missing_tagged_mods.is_empty() {
        const STEPS_TOTAL: u32 = 1;
        progress::emit_progress(
            app,
            TaskProgressPayload {
                version,
                steps_total: STEPS_TOTAL,
                step: 1,
                step_name: step_name.to_string(),
                step_progress: 0.0,
                overall_percent: 0.0,
                detail: Some(format!(
                    "Installing missing tagged mods: {}",
                    tags.join(", ")
                )),
                downloaded_bytes: None,
                total_bytes: None,
                extracted_files: Some(0),
                total_files: Some(missing_tagged_mods.len() as u64),
            },
        );

        let cfg = ModsConfig {
            mods: missing_tagged_mods,
        };
        mods::install_mods_with_progress(
            app,
            &game_root,
            version,
            &cfg,
            tags,
            cancel,
            |done, total, progress_info| {
                let step_progress = if total == 0 {
                    1.0
                } else {
                    (done as f64 / total as f64).clamp(0.0, 1.0)
                };
                progress::emit_progress(
                    app,
                    TaskProgressPayload {
                        version,
                        steps_total: STEPS_TOTAL,
                        step: 1,
                        step_name: step_name.to_string(),
                        step_progress,
                        overall_percent: overall_from_step(1, step_progress, STEPS_TOTAL),
                        detail: progress_info.detail,
                        downloaded_bytes: progress_info.downloaded_bytes,
                        total_bytes: progress_info.total_bytes,
                        extracted_files: progress_info.extracted_files.or(Some(done)),
                        total_files: progress_info.total_files.or(Some(total)),
                    },
                );
            },
        )
        .await?;
    }

    Ok(tagged_ids)
}

async fn run_mode_tagged_mod_ids(
    version: u32,
    cancel: Option<&Arc<AtomicBool>>,
) -> Result<Vec<(String, String)>, String> {
    let client = reqwest::Client::new();
    let (_remote_manifest_version, mods_cfg, _chain_config, _manifests, _preset_tag_constraints) =
        ModsConfig::fetch_manifest_with_cancel(&client, cancel).await?;

    let mut seen: HashSet<String> = HashSet::new();
    let mut tagged_ids: Vec<(String, String)> = vec![];
    for m in mods_cfg.mods {
        if !mod_has_run_mode_affinity(&m) {
            continue;
        }
        if m.tags.is_empty() && m.is_compatible(version) {
            continue;
        }
        let key = normalize_mod_key(&m.dev, &m.name);
        if seen.insert(key) {
            tagged_ids.push((m.dev, m.name));
        }
    }

    Ok(tagged_ids)
}

fn force_enable_mods_for_version(
    app: &tauri::AppHandle,
    version: u32,
    mods_to_enable: &[(String, String)],
) -> Result<(), String> {
    if mods_to_enable.is_empty() {
        return Ok(());
    }

    // Remove from disablemod.json (global source of truth used by UI).
    let mut list = read_disablemod(app)?;
    for (dev, name) in mods_to_enable {
        let id = normalize_mod_id(dev, name);
        list.mods.retain(|m| m != &id);
    }
    // Keep deterministic ordering.
    list.mods
        .sort_by(|a, b| a.dev.cmp(&b.dev).then(a.name.cmp(&b.name)));
    list.mods.dedup();
    write_disablemod(app, &list)?;

    // Apply filesystem state for this version immediately: remove `.old` suffixes.
    let plugins = plugins_dir(app, version)?;
    let patchers = patchers_dir(app, version)?;
    for (dev, name) in mods_to_enable {
        if let Some(dir) = mod_dir_for(&plugins, dev, name) {
            let _ = set_mod_files_old_suffix(&dir, true);
        }
        if let Some(dir) = mod_dir_for(&patchers, dev, name) {
            let _ = set_mod_files_old_suffix(&dir, true);
        }
    }

    Ok(())
}

fn overall_from_step(step: u32, step_progress: f64, steps_total: u32) -> f64 {
    if steps_total == 0 {
        return 0.0;
    }
    let step0 = step.saturating_sub(1) as f64;
    let steps = steps_total as f64;
    (((step0 + step_progress.clamp(0.0, 1.0)) / steps) * 100.0).clamp(0.0, 100.0)
}

#[derive(Debug, Clone, Serialize)]
struct ManifestDto {
    version: u32,
    chain_config: Vec<Vec<String>>,
    mods: Vec<mod_config::ModEntry>,
    manifests: BTreeMap<u32, String>,
    preset_tag_constraints: BTreeMap<String, mod_config::TagConstraint>,
}

fn preset_tag_constraint_for_name<'a>(
    constraints: &'a BTreeMap<String, mod_config::TagConstraint>,
    tag: &str,
) -> Option<&'a mod_config::TagConstraint> {
    constraints
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(tag))
        .map(|(_, value)| value)
}

fn preset_tag_supported_for_version(
    version: u32,
    constraints: &BTreeMap<String, mod_config::TagConstraint>,
    tag: &str,
) -> bool {
    preset_tag_constraint_for_name(constraints, tag)
        .is_none_or(|rule| mod_config::ModEntry::matches_caps(version, rule.low_cap, rule.high_cap))
}

fn preset_range_text(rule: &mod_config::TagConstraint) -> String {
    match (rule.low_cap, rule.high_cap) {
        (Some(low), Some(high)) => format!("v{low}-v{high}"),
        (Some(low), None) => format!("v{low}+"),
        (None, Some(high)) => format!("up to v{high}"),
        (None, None) => "all versions".to_string(),
    }
}

fn validate_preset_tags_for_version(
    version: u32,
    tags: &[String],
    constraints: &BTreeMap<String, mod_config::TagConstraint>,
) -> Result<(), String> {
    for tag in tags {
        let Some(rule) = preset_tag_constraint_for_name(constraints, tag) else {
            continue;
        };
        if mod_config::ModEntry::matches_caps(version, rule.low_cap, rule.high_cap) {
            continue;
        }
        return Err(format!(
            "{} preset supports {} (current: v{})",
            tag,
            preset_range_text(rule),
            version
        ));
    }
    Ok(())
}

fn shared_config_dir(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    Ok(app
        .path()
        .app_data_dir()
        .map_err(|e| format!("failed to resolve app data dir: {e}"))?
        .join("config")
        .join("shared"))
}

fn collect_installed_mod_pairs(
    root: &std::path::Path,
    seen: &mut HashSet<String>,
    out: &mut Vec<(String, String)>,
) -> Result<(), String> {
    if !root.exists() {
        return Ok(());
    }

    for e in std::fs::read_dir(root).map_err(|e| e.to_string())? {
        let e = e.map_err(|e| e.to_string())?;
        let path = e.path();
        if !path.is_dir() {
            continue;
        }

        let folder = e.file_name().to_string_lossy().to_string();
        let Some((dev, name)) = folder.split_once('-') else {
            continue;
        };

        let key = normalize_mod_key(dev, name);
        if seen.insert(key) {
            out.push((dev.to_string(), name.to_string()));
        }
    }

    Ok(())
}

fn remote_mod_has_active_cap(spec: &mod_config::ModEntry, active_tags: &[String]) -> bool {
    if active_tags.is_empty() {
        return spec.low_cap.is_some() || spec.high_cap.is_some();
    }

    active_tags.iter().any(|active_tag| {
        if !spec.applies_to_tag(active_tag) {
            return false;
        }

        let constraint = spec.constraint_for_tag(active_tag);
        let low_cap = constraint.and_then(|rule| rule.low_cap).or(spec.low_cap);
        let high_cap = constraint.and_then(|rule| rule.high_cap).or(spec.high_cap);
        low_cap.is_some() || high_cap.is_some()
    })
}

async fn purge_capped_incompatible_installed_mods(
    app: &tauri::AppHandle,
    version: u32,
    run_mode: Option<&str>,
) -> Result<u64, String> {
    let plugins = plugins_dir(app, version)?;
    let patchers = patchers_dir(app, version)?;

    let mut seen: HashSet<String> = HashSet::new();
    let mut installed: Vec<(String, String)> = vec![];
    collect_installed_mod_pairs(&plugins, &mut seen, &mut installed)?;
    collect_installed_mod_pairs(&patchers, &mut seen, &mut installed)?;
    if installed.is_empty() {
        return Ok(0);
    }

    let client = reqwest::Client::new();
    let (_remote_manifest_version, mods_cfg, _chain_config, _manifests, _preset_tag_constraints) =
        ModsConfig::fetch_manifest(&client).await?;
    let active_tags = run_mode
        .map(|mode| {
            let (preset, _practice) = preset_and_practice_for_run_mode(mode);
            preset_tags_for_name(&preset)
        })
        .unwrap_or_default();

    let mut capped_incompatible: HashMap<String, mod_config::ModEntry> = HashMap::new();
    for spec in mods_cfg.mods {
        if !spec.enabled {
            continue;
        }
        if active_tags.is_empty() && mod_has_run_mode_affinity(&spec) {
            continue;
        }

        let is_incompatible = if spec.tags.is_empty() {
            if active_tags
                .iter()
                .any(|active_tag| spec.applies_to_tag(active_tag))
            {
                !spec.is_compatible_for_tags(version, &active_tags)
            } else {
                !spec.is_compatible(version)
            }
        } else {
            if run_mode.is_none() {
                continue;
            }

            active_tags
                .iter()
                .any(|active_tag| spec.applies_to_tag(active_tag))
                && !spec.is_compatible_for_tags(version, &active_tags)
        };

        if !is_incompatible || !remote_mod_has_active_cap(&spec, &active_tags) {
            continue;
        }

        capped_incompatible.insert(normalize_mod_key(&spec.dev, &spec.name), spec);
    }

    if capped_incompatible.is_empty() {
        return Ok(0);
    }

    let mut purged: u64 = 0;

    for (installed_dev, installed_name) in installed {
        let key = normalize_mod_key(&installed_dev, &installed_name);
        let Some(spec) = capped_incompatible.get(&key) else {
            continue;
        };

        let mut removed_any = false;

        if let Some(dir) = mod_dir_for(&plugins, &spec.dev, &spec.name) {
            std::fs::remove_dir_all(&dir).map_err(|e| {
                format!(
                    "failed to remove incompatible capped mod {} ({}): {e}",
                    mod_folder_name(&spec.dev, &spec.name),
                    dir.to_string_lossy()
                )
            })?;
            removed_any = true;
        }

        if let Some(dir) = mod_dir_for(&patchers, &spec.dev, &spec.name) {
            std::fs::remove_dir_all(&dir).map_err(|e| {
                format!(
                    "failed to remove incompatible capped patcher {} ({}): {e}",
                    mod_folder_name(&spec.dev, &spec.name),
                    dir.to_string_lossy()
                )
            })?;
            removed_any = true;
        }

        if !removed_any {
            continue;
        }

        purged = purged.saturating_add(1);
        let mod_label = mod_folder_name(&spec.dev, &spec.name);
        log::info!(
            "Purged incompatible capped mod {mod_label} from v{version} while checking installed mods"
        );
    }

    Ok(purged)
}

fn is_safe_rel_path(rel: &std::path::Path) -> bool {
    use std::path::Component;
    rel.components().all(|c| match c {
        Component::Normal(_) => true,
        _ => false, // reject Prefix/RootDir/CurDir/ParentDir
    })
}

fn version_dir(app: &tauri::AppHandle, version: u32) -> Result<std::path::PathBuf, String> {
    Ok(app
        .path()
        .app_data_dir()
        .map_err(|e| format!("failed to resolve app data dir: {e}"))?
        .join("versions")
        .join(format!("v{version}")))
}

fn version_config_dir(app: &tauri::AppHandle, version: u32) -> Result<std::path::PathBuf, String> {
    Ok(version_dir(app, version)?.join("BepInEx").join("config"))
}

fn parse_font_assets_path_from_cfg(cfg_text: &str) -> Option<String> {
    let mut in_path_section = false;

    for raw_line in cfg_text.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
            continue;
        }

        if line.starts_with('[') && line.ends_with(']') {
            in_path_section = line[1..line.len() - 1].trim().eq_ignore_ascii_case("Path");
            continue;
        }

        if !in_path_section {
            continue;
        }

        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        if !key.trim().eq_ignore_ascii_case("FontAssetsPath") {
            continue;
        }

        let value = value.trim().trim_matches('"');
        if !value.is_empty() {
            return Some(value.to_string());
        }
    }

    None
}

fn fontpatcher_assets_dir_for_version(
    app: &tauri::AppHandle,
    version: u32,
) -> Result<std::path::PathBuf, String> {
    let cfg_dir = version_config_dir(app, version)?;
    let default_dir = cfg_dir.join("FontPatcher").join("default");
    let cfg_path = cfg_dir.join("lekakid.lcfontpatcher.cfg");

    if !cfg_path.exists() {
        return Ok(default_dir);
    }

    let cfg_text = std::fs::read_to_string(&cfg_path).map_err(|e| {
        format!(
            "failed to read fontpatcher config {}: {e}",
            cfg_path.to_string_lossy()
        )
    })?;

    let Some(raw_path) = parse_font_assets_path_from_cfg(&cfg_text) else {
        return Ok(default_dir);
    };

    if raw_path.eq_ignore_ascii_case("FontPatcher\\default")
        || raw_path.eq_ignore_ascii_case("FontPatcher/default")
    {
        return Ok(default_dir);
    }

    let rel_path = std::path::Path::new(&raw_path);
    if rel_path.is_absolute() {
        return Ok(rel_path.to_path_buf());
    }

    Ok(cfg_dir.join(rel_path))
}

fn sync_fontpatcher_with_assets_for_version(
    app: &tauri::AppHandle,
    version: u32,
) -> Result<(), String> {
    let assets_dir = fontpatcher_assets_dir_for_version(app, version)?;
    let assets_available = assets_dir.is_dir();
    let disabled = read_disablemod(app)?
        .mods
        .contains(&normalize_mod_id("LeKAKiD", "FontPatcher"));

    let enabled = assets_available && !disabled;
    let plugins = plugins_dir(app, version)?;
    if let Some(dir) = mod_dir_for(&plugins, "LeKAKiD", "FontPatcher") {
        let _ = set_mod_files_old_suffix(&dir, enabled);
    }

    let patchers = patchers_dir(app, version)?;
    if let Some(dir) = mod_dir_for(&patchers, "LeKAKiD", "FontPatcher") {
        let _ = set_mod_files_old_suffix(&dir, enabled);
    }

    Ok(())
}

fn ensure_wesley_moonscripts_cfg(
    app: &tauri::AppHandle,
    version: u32,
    lock_moons: bool,
) -> Result<(), String> {
    // Only meaningful for the Wesley preset (v69+).
    if version < 69 {
        return Ok(());
    }

    let cfg_dir = version_config_dir(app, version)?;
    std::fs::create_dir_all(&cfg_dir).map_err(|e| e.to_string())?;

    let cfg_path = cfg_dir.join("JacobG5.WesleyMoonScripts.cfg");
    if cfg_path.exists() {
        // Preserve everything else in the file and only update LockMoons.
        let bytes = std::fs::read(&cfg_path).map_err(|e| e.to_string())?;
        let text = String::from_utf8_lossy(&bytes);

        let mut changed = false;
        let mut out = String::with_capacity(text.len());
        let desired_value = if lock_moons { "true" } else { "false" };

        for seg in text.split_inclusive(['\n', '\r']) {
            // Preserve original newline sequence; only edit the assignment line.
            let (line, nl) = if seg.ends_with("\r\n") {
                (seg.trim_end_matches("\r\n"), "\r\n")
            } else if seg.ends_with('\n') {
                (seg.trim_end_matches('\n'), "\n")
            } else if seg.ends_with('\r') {
                (seg.trim_end_matches('\r'), "\r")
            } else {
                (seg, "")
            };

            let trimmed = line.trim_start();
            let indent_len = line.len().saturating_sub(trimmed.len());
            let indent = &line[..indent_len];

            if trimmed.starts_with("LockMoons") {
                if let Some(eq_idx) = trimmed.find('=') {
                    let (left, right_all) = trimmed.split_at(eq_idx);
                    let right_all = right_all.trim_start_matches('=');

                    // Keep inline comments if any.
                    let (right, comment) = match right_all.find('#') {
                        Some(i) => (&right_all[..i], &right_all[i..]),
                        None => (right_all, ""),
                    };

                    if left.trim() == "LockMoons" {
                        if !right.trim().eq_ignore_ascii_case(desired_value) {
                            out.push_str(indent);
                            out.push_str("LockMoons = ");
                            out.push_str(desired_value);
                            out.push_str(comment);
                            out.push_str(nl);
                            changed = true;
                            continue;
                        }
                    }
                }
            }

            out.push_str(line);
            out.push_str(nl);
        }

        if changed {
            std::fs::write(&cfg_path, out).map_err(|e| e.to_string())?;
        }
        return Ok(());
    }

    let content = format!(
        "## Settings file was created by plugin WesleyMoonScripts v1.1.6\n## Plugin GUID: JacobG5.WesleyMoonScripts\n\n[Core]\n\n## Locks moons that have progression integration set up to enable playing the campaign.\n# Setting type: Boolean\n# Default value: true\nLockMoons = {}\n\n",
        if lock_moons { "true" } else { "false" }
    );

    std::fs::write(&cfg_path, content).map_err(|e| e.to_string())?;
    Ok(())
}

pub(crate) fn ensure_reverb_trigger_fix_cfg(
    app: &tauri::AppHandle,
    version: u32,
) -> Result<(), String> {
    let plugins = plugins_dir(app, version)?;
    if mod_dir_for(&plugins, "JacobG5", "ReverbTriggerFix").is_none() {
        return Ok(());
    }

    let cfg_dir = version_config_dir(app, version)?;
    std::fs::create_dir_all(&cfg_dir).map_err(|e| e.to_string())?;

    let cfg_path = cfg_dir.join("JacobG5.ReverbTriggerFix.cfg");
    let baseline = "## Settings file was created by plugin ReverbTriggerFix v0.3.0\n## Plugin GUID: JacobG5.ReverbTriggerFix\n\n[Core]\n\n## Disables all reverb trigger modifications.\n## Requires a lobby restart to apply.\n## Game restart *not* required.\n# Setting type: Boolean\n# Default value: false\ndisableMod = false\n\n[Debug]\n\n## Logs more info to the console when enabled.\n## \n## *THIS WILL SPAM YOUR CONSOLE DEPENDING ON YOUR OTHER SETTINGS*\n# Setting type: Boolean\n# Default value: false\nextendedLogging = false\n\n[Experimental]\n\n## I'm not sure why reverb triggers run their calculations every frame when as far as I can tell they only need to run their changes when something enters their collider.\n## I'm leaving this as an experimental toggle because it seems to be very buggy atm.\n## \n## Feel free to try it if you wish. If you're experiencing problems then turn it back off.\n# Setting type: Boolean\n# Default value: false\nTriggerOnEnter = true\n";

    if !cfg_path.exists() {
        std::fs::write(&cfg_path, baseline).map_err(|e| e.to_string())?;
        return Ok(());
    }

    let bytes = std::fs::read(&cfg_path).map_err(|e| e.to_string())?;
    let text = String::from_utf8_lossy(&bytes);

    let mut changed = false;
    let mut saw_key = false;
    let mut out = String::with_capacity(text.len() + 128);

    for seg in text.split_inclusive(['\n', '\r']) {
        let (line, nl) = if seg.ends_with("\r\n") {
            (seg.trim_end_matches("\r\n"), "\r\n")
        } else if seg.ends_with('\n') {
            (seg.trim_end_matches('\n'), "\n")
        } else if seg.ends_with('\r') {
            (seg.trim_end_matches('\r'), "\r")
        } else {
            (seg, "")
        };

        let trimmed = line.trim_start();
        let indent_len = line.len().saturating_sub(trimmed.len());
        let indent = &line[..indent_len];

        if let Some(eq_idx) = trimmed.find('=') {
            let (left, right_all) = trimmed.split_at(eq_idx);
            if left.trim().eq_ignore_ascii_case("triggerOnEnter") {
                saw_key = true;
                let right_all = right_all.trim_start_matches('=');
                let (right, comment) = match right_all.find('#') {
                    Some(i) => (&right_all[..i], &right_all[i..]),
                    None => (right_all, ""),
                };

                if !left.trim().eq("TriggerOnEnter") || !right.trim().eq_ignore_ascii_case("true") {
                    out.push_str(indent);
                    out.push_str("TriggerOnEnter = true");
                    out.push_str(comment);
                    out.push_str(nl);
                    changed = true;
                    continue;
                }
            }
        }

        out.push_str(line);
        out.push_str(nl);
    }

    if !saw_key {
        if !out.ends_with('\n') && !out.ends_with("\r\n") && !out.ends_with('\r') {
            out.push('\n');
        }
        out.push_str("\n[Experimental]\n\n");
        out.push_str("## I'm not sure why reverb triggers run their calculations every frame when as far as I can tell they only need to run their changes when something enters their collider.\n");
        out.push_str("## I'm leaving this as an experimental toggle because it seems to be very buggy atm.\n");
        out.push_str("## \n");
        out.push_str("## Feel free to try it if you wish. If you're experiencing problems then turn it back off.\n");
        out.push_str("# Setting type: Boolean\n");
        out.push_str("# Default value: false\n");
        out.push_str("TriggerOnEnter = true\n");
        changed = true;
    }

    if changed {
        std::fs::write(&cfg_path, out).map_err(|e| e.to_string())?;
    }

    Ok(())
}

#[derive(Clone, Serialize, Deserialize)]
struct HqolDontStoreBackupEntry {
    file_name: String,
    rhs: String,
}

#[derive(Clone, Serialize, Deserialize)]
struct HqolDontStoreBackup {
    version: u32,
    files: Vec<HqolDontStoreBackupEntry>,
}

fn hqol_dont_store_backup_path(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    Ok(app
        .path()
        .app_data_dir()
        .map_err(|e| format!("failed to resolve app data dir: {e}"))?
        .join("hqol_wesley_dont_store_backup.json"))
}

fn read_hqol_dont_store_backup(
    app: &tauri::AppHandle,
) -> Result<Option<HqolDontStoreBackup>, String> {
    let path = hqol_dont_store_backup_path(app)?;
    if !path.exists() {
        return Ok(None);
    }

    let text = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    match serde_json::from_str::<HqolDontStoreBackup>(&text) {
        Ok(backup) => Ok(Some(backup)),
        Err(e) => {
            log::warn!("Failed to parse HQoL Wesley dont-store backup, clearing stale file: {e}");
            let _ = std::fs::remove_file(&path);
            Ok(None)
        }
    }
}

fn write_hqol_dont_store_backup(
    app: &tauri::AppHandle,
    backup: Option<&HqolDontStoreBackup>,
) -> Result<(), String> {
    let path = hqol_dont_store_backup_path(app)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    if let Some(backup) = backup {
        let json = serde_json::to_string_pretty(backup).map_err(|e| e.to_string())?;
        std::fs::write(&path, json).map_err(|e| e.to_string())?;
    } else if path.exists() {
        std::fs::remove_file(&path).map_err(|e| e.to_string())?;
    }

    Ok(())
}

fn extract_hqol_dont_store_rhs(text: &str) -> Option<String> {
    let mut in_general = false;

    for seg in text.split_inclusive(['\n', '\r']) {
        let line = if seg.ends_with("\r\n") {
            seg.trim_end_matches("\r\n")
        } else if seg.ends_with('\n') {
            seg.trim_end_matches('\n')
        } else if seg.ends_with('\r') {
            seg.trim_end_matches('\r')
        } else {
            seg
        };

        let trimmed = line.trim_start();
        let trimmed_all = line.trim();

        if trimmed_all.starts_with('[') && trimmed_all.ends_with(']') {
            in_general = trimmed_all.eq_ignore_ascii_case("[General]");
        }

        if !in_general {
            continue;
        }

        let Some(eq_idx) = trimmed.find('=') else {
            continue;
        };
        let (left, _) = trimmed.split_at(eq_idx);
        if left.trim() == "Dont store list" {
            return Some(trimmed[eq_idx + 1..].to_string());
        }
    }

    None
}

fn replace_hqol_dont_store_rhs(text: &str, replacement_rhs: &str) -> (String, bool) {
    let mut changed = false;
    let mut in_general = false;
    let mut out = String::with_capacity(text.len() + replacement_rhs.len());

    for seg in text.split_inclusive(['\n', '\r']) {
        let (line, nl) = if seg.ends_with("\r\n") {
            (seg.trim_end_matches("\r\n"), "\r\n")
        } else if seg.ends_with('\n') {
            (seg.trim_end_matches('\n'), "\n")
        } else if seg.ends_with('\r') {
            (seg.trim_end_matches('\r'), "\r")
        } else {
            (seg, "")
        };

        let trimmed = line.trim_start();
        let trimmed_all = line.trim();

        if trimmed_all.starts_with('[') && trimmed_all.ends_with(']') {
            in_general = trimmed_all.eq_ignore_ascii_case("[General]");
        }

        if in_general {
            if let Some(eq_idx) = trimmed.find('=') {
                let (left, _) = trimmed.split_at(eq_idx);
                if left.trim() == "Dont store list" {
                    let indent_len = line.len().saturating_sub(trimmed.len());
                    let indent = &line[..indent_len];
                    let current_rhs = &trimmed[eq_idx + 1..];
                    changed |= current_rhs != replacement_rhs;
                    out.push_str(indent);
                    out.push_str("Dont store list =");
                    out.push_str(replacement_rhs);
                    out.push_str(nl);
                    continue;
                }
            }
        }

        out.push_str(line);
        out.push_str(nl);
    }

    (out, changed)
}

fn wesley_hqol_dont_store_rhs(current_rhs: &str) -> String {
    let comment = current_rhs
        .find('#')
        .map(|idx| &current_rhs[idx..])
        .unwrap_or("");
    let leading_ws_len = current_rhs.len() - current_rhs.trim_start().len();
    let leading_ws = &current_rhs[..leading_ws_len];
    let prefix = if leading_ws.is_empty() {
        " "
    } else {
        leading_ws
    };

    format!(
        "{}{}{}",
        prefix,
        WESLEY_HQOL_DONT_STORE_ITEMS.join(", "),
        comment
    )
}

fn apply_wesley_hqol_dont_store_override(
    app: &tauri::AppHandle,
    version: u32,
) -> Result<(), String> {
    let _ = restore_hqol_wesley_dont_store_backup_if_present(app)?;

    let cfg_dir = version_config_dir(app, version)?;
    let mut backup = HqolDontStoreBackup {
        version,
        files: vec![],
    };

    for file_name in HQOL_DONT_STORE_CFG_FILES {
        let cfg_path = cfg_dir.join(file_name);
        if !cfg_path.exists() {
            continue;
        }

        let bytes = std::fs::read(&cfg_path).map_err(|e| e.to_string())?;
        let text = String::from_utf8_lossy(&bytes);
        let Some(current_rhs) = extract_hqol_dont_store_rhs(&text) else {
            continue;
        };

        backup.files.push(HqolDontStoreBackupEntry {
            file_name: file_name.to_string(),
            rhs: current_rhs.clone(),
        });

        let override_rhs = wesley_hqol_dont_store_rhs(&current_rhs);
        let (updated, changed) = replace_hqol_dont_store_rhs(&text, &override_rhs);
        if changed {
            std::fs::write(&cfg_path, updated).map_err(|e| e.to_string())?;
        }
    }

    if backup.files.is_empty() {
        write_hqol_dont_store_backup(app, None)?;
    } else {
        write_hqol_dont_store_backup(app, Some(&backup))?;
    }

    Ok(())
}

pub(crate) fn restore_hqol_wesley_dont_store_backup_if_present(
    app: &tauri::AppHandle,
) -> Result<bool, String> {
    let Some(backup) = read_hqol_dont_store_backup(app)? else {
        return Ok(false);
    };

    let cfg_dir = version_config_dir(app, backup.version)?;
    let mut restored = false;

    for entry in &backup.files {
        let cfg_path = cfg_dir.join(&entry.file_name);
        if !cfg_path.exists() {
            continue;
        }

        let bytes = std::fs::read(&cfg_path).map_err(|e| e.to_string())?;
        let text = String::from_utf8_lossy(&bytes);
        let (updated, changed) = replace_hqol_dont_store_rhs(&text, &entry.rhs);
        if changed {
            std::fs::write(&cfg_path, updated).map_err(|e| e.to_string())?;
            restored = true;
        }
    }

    write_hqol_dont_store_backup(app, None)?;
    Ok(restored)
}

fn ensure_weather_registry_cfg(app: &tauri::AppHandle, version: u32) -> Result<(), String> {
    // Only meaningful for the Wesley preset (v69+). Never overwrite entire configs.
    if version < 69 {
        return Ok(());
    }

    let cfg_dir = version_config_dir(app, version)?;
    std::fs::create_dir_all(&cfg_dir).map_err(|e| e.to_string())?;

    let cfg_path = cfg_dir.join("mrov.WeatherRegistry.cfg");
    let desired_first_day = "true";
    let desired_algo = "Hybrid";

    // If missing, create using the provided baseline.
    if !cfg_path.exists() {
        let content = r#"## Settings file was created by plugin WeatherRegistry v0.7.5
## Plugin GUID: mrov.WeatherRegistry

[|General]

## Enable colored weathers on map screen
# Setting type: Boolean
# Default value: true
Colored Weathers = true

## Display planet videos on map screen
# Setting type: Boolean
# Default value: true
Planet Videos = true

## Show weather multipliers on map screen
# Setting type: Boolean
# Default value: false
Show Weather Multipliers = false

## Use Registry's scrap multipliers. Disable if you prefer to use other mod's multiplier settings.
# Setting type: Boolean
# Default value: true
Scrap multipliers = true

[|Logging]

## Select which logs to show.
# Setting type: LoggingType
# Default value: Basic
# Acceptable values: Basic, Debug, Developer
Display Log Levels = Basic

[|WeatherSelection]

## Select the algorithm to use during weather selection.
# Setting type: WeatherAlgorithm
# Default value: Registry
# Acceptable values: Registry, Vanilla, Hybrid
Weather Selection Algorithm = Hybrid

## If enabled, the first day will always have clear weather, on all planets, regardless of the selected algorithm.
# Setting type: Boolean
# Default value: false
First Day Clear Weather = true

[Modded Weather: Earthquakes]

## The default weight of this weather
# Setting type: Int32
# Default value: 40
# Acceptable value range: From 0 to 10000
Default weight = 40

## Multiplier for the amount of scrap spawned
# Setting type: Single
# Default value: 1.1
# Acceptable value range: From 0 to 100
Scrap amount multiplier = 1.1

## Multiplier for the value of scrap spawned
# Setting type: Single
# Default value: 1
# Acceptable value range: From 0 to 100
Scrap value multiplier = 1

## Whether to make the filter a whitelist (false is blacklist, true is whitelist)
# Setting type: Boolean
# Default value: false
Filtering option = false

## Semicolon-separated list of level names to filter (use `Filtering Option` config to select filter type)
# Setting type: String
# Default value: Company;Galetry;Calist;Berunah
Level filter = Company;Galetry;Calist;Berunah

## Semicolon-separated list of level weights
# Setting type: String
# Default value: MoonName@50
Level weights = MoonName@50

## Semicolon-separated list of weather-to-weather weights - if previous day was Earthquakes, next day should have weights:
# Setting type: String
# Default value: WeatherName@50
WeatherToWeather weights = WeatherName@50

[Modded Weather: Forsaken]

## The default weight of this weather
# Setting type: Int32
# Default value: 30
# Acceptable value range: From 0 to 10000
Default weight = 30

## Multiplier for the amount of scrap spawned
# Setting type: Single
# Default value: 1.2
# Acceptable value range: From 0 to 100
Scrap amount multiplier = 1.2

## Multiplier for the value of scrap spawned
# Setting type: Single
# Default value: 1.2
# Acceptable value range: From 0 to 100
Scrap value multiplier = 1.2

## Whether to make the filter a whitelist (false is blacklist, true is whitelist)
# Setting type: Boolean
# Default value: false
Filtering option = false

## Semicolon-separated list of level names to filter (use `Filtering Option` config to select filter type)
# Setting type: String
# Default value: Company;Galetry;Berunah;Calist;Repress;Cosmocos
Level filter = Company;Galetry;Berunah;Calist;Repress;Cosmocos

## Semicolon-separated list of level weights
# Setting type: String
# Default value: MoonName@20
Level weights = MoonName@20

## Semicolon-separated list of weather-to-weather weights - if previous day was Forsaken, next day should have weights:
# Setting type: String
# Default value: WeatherName@20
WeatherToWeather weights = WeatherName@20

[Modded Weather: Hallowed]

## The default weight of this weather
# Setting type: Int32
# Default value: 8
# Acceptable value range: From 0 to 10000
Default weight = 8

## Multiplier for the amount of scrap spawned
# Setting type: Single
# Default value: 1
# Acceptable value range: From 0 to 100
Scrap amount multiplier = 1

## Multiplier for the value of scrap spawned
# Setting type: Single
# Default value: 1
# Acceptable value range: From 0 to 100
Scrap value multiplier = 1

## Whether to make the filter a whitelist (false is blacklist, true is whitelist)
# Setting type: Boolean
# Default value: false
Filtering option = false

## Semicolon-separated list of level names to filter (use `Filtering Option` config to select filter type)
# Setting type: String
# Default value: Company;Galetry;Calist;Berunah;Asteroid-13;Thalasso;Roart;Repress;Cosmocos
Level filter = Company;Galetry;Calist;Berunah;Asteroid-13;Thalasso;Roart;Repress;Cosmocos

## Semicolon-separated list of level weights
# Setting type: String
# Default value: MoonName@8
Level weights = MoonName@8

## Semicolon-separated list of weather-to-weather weights - if previous day was Hallowed, next day should have weights:
# Setting type: String
# Default value: WeatherName@8
WeatherToWeather weights = WeatherName@8

[Modded Weather: Hurricane]

## The default weight of this weather
# Setting type: Int32
# Default value: 80
# Acceptable value range: From 0 to 10000
Default weight = 80

## Multiplier for the amount of scrap spawned
# Setting type: Single
# Default value: 1
# Acceptable value range: From 0 to 100
Scrap amount multiplier = 1

## Multiplier for the value of scrap spawned
# Setting type: Single
# Default value: 1
# Acceptable value range: From 0 to 100
Scrap value multiplier = 1

## Whether to make the filter a whitelist (false is blacklist, true is whitelist)
# Setting type: Boolean
# Default value: false
Filtering option = false

## Semicolon-separated list of level names to filter (use `Filtering Option` config to select filter type)
# Setting type: String
# Default value: Company;Galetry;Calist;Berunah;Asteroid-13,Roart;Repress;Cosmocos
Level filter = Company;Galetry;Calist;Berunah;Asteroid-13,Roart;Repress;Cosmocos

## Semicolon-separated list of level weights
# Setting type: String
# Default value: WeatherName@20
Level weights = WeatherName@20

## Semicolon-separated list of weather-to-weather weights - if previous day was Hurricane, next day should have weights:
# Setting type: String
# Default value: WeatherName@20
WeatherToWeather weights = WeatherName@20

[Vanilla Weather: DustClouds]

## The default weight of this weather
# Setting type: Int32
# Default value: 100
# Acceptable value range: From 0 to 10000
Default weight = 100

## Multiplier for the amount of scrap spawned
# Setting type: Single
# Default value: 1
# Acceptable value range: From 0 to 100
Scrap amount multiplier = 1

## Multiplier for the value of scrap spawned
# Setting type: Single
# Default value: 1
# Acceptable value range: From 0 to 100
Scrap value multiplier = 1

## Whether to make the filter a whitelist (false is blacklist, true is whitelist)
# Setting type: Boolean
# Default value: false
Filtering option = false

## Semicolon-separated list of level names to filter (use `Filtering Option` config to select filter type)
# Setting type: String
# Default value: Company;
Level filter = Company;

## Semicolon-separated list of level weights
# Setting type: String
# Default value: MoonName@50;
Level weights = MoonName@50;

## Semicolon-separated list of weather-to-weather weights - if previous day was Dust Clouds, next day should have weights:
# Setting type: String
# Default value: WeatherName@50
WeatherToWeather weights = WeatherName@50

[Vanilla Weather: Eclipsed]

## The default weight of this weather
# Setting type: Int32
# Default value: 100
# Acceptable value range: From 0 to 10000
Default weight = 100

## Multiplier for the amount of scrap spawned
# Setting type: Single
# Default value: 1
# Acceptable value range: From 0 to 100
Scrap amount multiplier = 1

## Multiplier for the value of scrap spawned
# Setting type: Single
# Default value: 1
# Acceptable value range: From 0 to 100
Scrap value multiplier = 1

## Whether to make the filter a whitelist (false is blacklist, true is whitelist)
# Setting type: Boolean
# Default value: false
Filtering option = false

## Semicolon-separated list of level names to filter (use `Filtering Option` config to select filter type)
# Setting type: String
# Default value: Company;
Level filter = Company;

## Semicolon-separated list of level weights
# Setting type: String
# Default value: MoonName@50;
Level weights = MoonName@50;

## Semicolon-separated list of weather-to-weather weights - if previous day was Eclipsed, next day should have weights:
# Setting type: String
# Default value: None@300; Rainy@40; Stormy@16; Flooded@20; Foggy@60; Eclipsed@10;
WeatherToWeather weights = None@300; Rainy@40; Stormy@16; Flooded@20; Foggy@60; Eclipsed@10;

[Vanilla Weather: Flooded]

## The default weight of this weather
# Setting type: Int32
# Default value: 100
# Acceptable value range: From 0 to 10000
Default weight = 100

## Multiplier for the amount of scrap spawned
# Setting type: Single
# Default value: 1
# Acceptable value range: From 0 to 100
Scrap amount multiplier = 1

## Multiplier for the value of scrap spawned
# Setting type: Single
# Default value: 1
# Acceptable value range: From 0 to 100
Scrap value multiplier = 1

## Whether to make the filter a whitelist (false is blacklist, true is whitelist)
# Setting type: Boolean
# Default value: false
Filtering option = false

## Semicolon-separated list of level names to filter (use `Filtering Option` config to select filter type)
# Setting type: String
# Default value: Company;
Level filter = Company;

## Semicolon-separated list of level weights
# Setting type: String
# Default value: MoonName@50;
Level weights = MoonName@50;

## Semicolon-separated list of weather-to-weather weights - if previous day was Flooded, next day should have weights:
# Setting type: String
# Default value: None@160; Rainy@60; Stormy@50; Flooded@10; Foggy@60; Eclipsed@40;
WeatherToWeather weights = None@160; Rainy@60; Stormy@50; Flooded@10; Foggy@60; Eclipsed@40;

[Vanilla Weather: Foggy]

## The default weight of this weather
# Setting type: Int32
# Default value: 100
# Acceptable value range: From 0 to 10000
Default weight = 100

## Multiplier for the amount of scrap spawned
# Setting type: Single
# Default value: 1
# Acceptable value range: From 0 to 100
Scrap amount multiplier = 1

## Multiplier for the value of scrap spawned
# Setting type: Single
# Default value: 1
# Acceptable value range: From 0 to 100
Scrap value multiplier = 1

## Whether to make the filter a whitelist (false is blacklist, true is whitelist)
# Setting type: Boolean
# Default value: false
Filtering option = false

## Semicolon-separated list of level names to filter (use `Filtering Option` config to select filter type)
# Setting type: String
# Default value: Company;
Level filter = Company;

## Semicolon-separated list of level weights
# Setting type: String
# Default value: MoonName@50;
Level weights = MoonName@50;

## Semicolon-separated list of weather-to-weather weights - if previous day was Foggy, next day should have weights:
# Setting type: String
# Default value: None@200; Rainy@60; Stormy@50; Flooded@10; Foggy@30; Eclipsed@20;
WeatherToWeather weights = None@200; Rainy@60; Stormy@50; Flooded@10; Foggy@30; Eclipsed@20;

[Vanilla Weather: None]

## The default weight of this weather
# Setting type: Int32
# Default value: 100
# Acceptable value range: From 0 to 10000
Default weight = 100

## Multiplier for the amount of scrap spawned
# Setting type: Single
# Default value: 1
# Acceptable value range: From 0 to 100
Scrap amount multiplier = 1

## Multiplier for the value of scrap spawned
# Setting type: Single
# Default value: 1
# Acceptable value range: From 0 to 100
Scrap value multiplier = 1

## Whether to make the filter a whitelist (false is blacklist, true is whitelist)
# Setting type: Boolean
# Default value: false
Filtering option = false

## Semicolon-separated list of level names to filter (use `Filtering Option` config to select filter type)
# Setting type: String
# Default value: Company;
Level filter = Company;

## Semicolon-separated list of level weights
# Setting type: String
# Default value: MoonName@50;
Level weights = MoonName@50;

## Semicolon-separated list of weather-to-weather weights - if previous day was None, next day should have weights:
# Setting type: String
# Default value: None@160; Rainy@100; Stormy@70; Flooded@20; Foggy@40; Eclipsed@10;
WeatherToWeather weights = None@160; Rainy@100; Stormy@70; Flooded@20; Foggy@40; Eclipsed@10;

[Vanilla Weather: Rainy]

## The default weight of this weather
# Setting type: Int32
# Default value: 100
# Acceptable value range: From 0 to 10000
Default weight = 100

## Multiplier for the amount of scrap spawned
# Setting type: Single
# Default value: 1
# Acceptable value range: From 0 to 100
Scrap amount multiplier = 1

## Multiplier for the value of scrap spawned
# Setting type: Single
# Default value: 1
# Acceptable value range: From 0 to 100
Scrap value multiplier = 1

## Whether to make the filter a whitelist (false is blacklist, true is whitelist)
# Setting type: Boolean
# Default value: false
Filtering option = false

## Semicolon-separated list of level names to filter (use `Filtering Option` config to select filter type)
# Setting type: String
# Default value: Company;
Level filter = Company;

## Semicolon-separated list of level weights
# Setting type: String
# Default value: MoonName@50;
Level weights = MoonName@50;

## Semicolon-separated list of weather-to-weather weights - if previous day was Rainy, next day should have weights:
# Setting type: String
# Default value: None@100; Rainy@60; Stormy@40; Flooded@30; Foggy@50; Eclipsed@20;
WeatherToWeather weights = None@100; Rainy@60; Stormy@40; Flooded@30; Foggy@50; Eclipsed@20;

[Vanilla Weather: Stormy]

## The default weight of this weather
# Setting type: Int32
# Default value: 100
# Acceptable value range: From 0 to 10000
Default weight = 100

## Multiplier for the amount of scrap spawned
# Setting type: Single
# Default value: 1
# Acceptable value range: From 0 to 100
Scrap amount multiplier = 1

## Multiplier for the value of scrap spawned
# Setting type: Single
# Default value: 1
# Acceptable value range: From 0 to 100
Scrap value multiplier = 1

## Whether to make the filter a whitelist (false is blacklist, true is whitelist)
# Setting type: Boolean
# Default value: false
Filtering option = false

## Semicolon-separated list of level names to filter (use `Filtering Option` config to select filter type)
# Setting type: String
# Default value: Company;
Level filter = Company;

## Semicolon-separated list of level weights
# Setting type: String
# Default value: MoonName@50;
Level weights = MoonName@50;

## Semicolon-separated list of weather-to-weather weights - if previous day was Stormy, next day should have weights:
# Setting type: String
# Default value: None@160; Rainy@110; Stormy@10; Flooded@120; Foggy@20; Eclipsed@80;
WeatherToWeather weights = None@160; Rainy@110; Stormy@10; Flooded@120; Foggy@20; Eclipsed@80;




[|WeatherSelection]
First Day Clear Weather = true
Weather Selection Algorithm = Hybrid
"#;

        std::fs::write(&cfg_path, content).map_err(|e| e.to_string())?;
        return Ok(());
    }

    // If exists: enforce only these two keys (preserve everything else).
    let bytes = std::fs::read(&cfg_path).map_err(|e| e.to_string())?;
    let text = String::from_utf8_lossy(&bytes);

    let mut changed = false;
    let mut saw_first_day = false;
    let mut saw_algo = false;
    let mut out = String::with_capacity(text.len());

    for seg in text.split_inclusive(['\n', '\r']) {
        let (line, nl) = if seg.ends_with("\r\n") {
            (seg.trim_end_matches("\r\n"), "\r\n")
        } else if seg.ends_with('\n') {
            (seg.trim_end_matches('\n'), "\n")
        } else if seg.ends_with('\r') {
            (seg.trim_end_matches('\r'), "\r")
        } else {
            (seg, "")
        };

        let trimmed = line.trim_start();
        let indent_len = line.len().saturating_sub(trimmed.len());
        let indent = &line[..indent_len];

        let mut handled = false;
        for (key, desired, flag) in [
            (
                "First Day Clear Weather",
                desired_first_day,
                &mut saw_first_day,
            ),
            ("Weather Selection Algorithm", desired_algo, &mut saw_algo),
        ] {
            if trimmed.starts_with(key) {
                if let Some(eq_idx) = trimmed.find('=') {
                    let (left, right_all) = trimmed.split_at(eq_idx);
                    if left.trim() == key {
                        let right_all = right_all.trim_start_matches('=');
                        let (right, comment) = match right_all.find('#') {
                            Some(i) => (&right_all[..i], &right_all[i..]),
                            None => (right_all, ""),
                        };
                        *flag = true;
                        if right.trim() != desired {
                            out.push_str(indent);
                            out.push_str(key);
                            out.push_str(" = ");
                            out.push_str(desired);
                            out.push_str(comment);
                            out.push_str(nl);
                            changed = true;
                        } else {
                            out.push_str(line);
                            out.push_str(nl);
                        }
                        handled = true;
                        break;
                    }
                }
            }
        }
        if handled {
            continue;
        }

        out.push_str(line);
        out.push_str(nl);
    }

    if !saw_first_day || !saw_algo {
        // Append a minimal section at end if missing.
        if !out.ends_with('\n') && !out.ends_with("\r\n") && !out.ends_with('\r') {
            out.push('\n');
        }
        out.push_str("\n[|WeatherSelection]\n");
        if !saw_first_day {
            out.push_str("First Day Clear Weather = true\n");
        }
        if !saw_algo {
            out.push_str("Weather Selection Algorithm = Hybrid\n");
        }
        changed = true;
    }

    if changed {
        std::fs::write(&cfg_path, out).map_err(|e| e.to_string())?;
    }

    Ok(())
}

fn find_file_named(
    root: &std::path::Path,
    target_name: &str,
    max_depth: usize,
) -> Option<std::path::PathBuf> {
    let target_lower = target_name.to_lowercase();
    let mut stack: Vec<(std::path::PathBuf, usize)> = vec![(root.to_path_buf(), 0)];

    while let Some((dir, depth)) = stack.pop() {
        if depth > max_depth {
            continue;
        }
        let rd = std::fs::read_dir(&dir).ok()?;
        for e in rd.flatten() {
            let path = e.path();
            let ft = e.file_type().ok()?;
            if ft.is_dir() {
                stack.push((path, depth + 1));
                continue;
            }
            if !ft.is_file() {
                continue;
            }
            let name = path.file_name().and_then(|s| s.to_str())?;
            if name.to_lowercase() == target_lower {
                return Some(path);
            }
        }
    }
    None
}

fn plugins_dir(app: &tauri::AppHandle, version: u32) -> Result<std::path::PathBuf, String> {
    Ok(version_dir(app, version)?.join("BepInEx").join("plugins"))
}

fn patchers_dir(app: &tauri::AppHandle, version: u32) -> Result<std::path::PathBuf, String> {
    Ok(version_dir(app, version)?.join("BepInEx").join("patchers"))
}

fn mod_folder_name(dev: &str, name: &str) -> String {
    format!("{dev}-{name}")
}

fn mod_dir_for(plugins_dir: &std::path::Path, dev: &str, name: &str) -> Option<std::path::PathBuf> {
    // Fast paths (common on Windows; case-insensitive FS).
    let direct = plugins_dir.join(mod_folder_name(dev, name));
    if direct.exists() {
        return Some(direct);
    }

    let dev_l = dev.trim().to_lowercase();
    let name_l = name.trim().to_lowercase();
    let lowered = plugins_dir.join(mod_folder_name(&dev_l, &name_l));
    if lowered.exists() {
        return Some(lowered);
    }

    // Fallback: scan directories and match case-insensitively.
    let target = mod_folder_name(&dev_l, &name_l);
    let Ok(rd) = std::fs::read_dir(plugins_dir) else {
        return None;
    };
    for e in rd.flatten() {
        let path = e.path();
        if !path.is_dir() {
            continue;
        }
        let Some(folder) = path.file_name().and_then(|s| s.to_str()) else {
            continue;
        };
        if folder.to_lowercase() == target {
            return Some(path);
        }
    }
    None
}

fn find_mod_icon_path(mod_dir: &std::path::Path) -> Option<String> {
    for file_name in ["icon.png", "icon.png.old"] {
        let path = mod_dir.join(file_name);
        if path.is_file() {
            return Some(path.to_string_lossy().to_string());
        }
    }
    None
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct DisabledMod {
    dev: String,
    name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct InstalledModVersion {
    dev: String,
    name: String,
    version: String,
    icon_path: Option<String>,
    description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DisableModFile {
    version: u32,
    mods: Vec<DisabledMod>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SteamOverlayConfig {
    enabled: bool,
    steam_path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct SteamOverlayConfigDto {
    enabled: bool,
    steam_path: Option<String>,
    resolved_steam_path: Option<String>,
}

impl Default for SteamOverlayConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            steam_path: None,
        }
    }
}

impl SteamOverlayConfig {
    fn into_dto(self) -> SteamOverlayConfigDto {
        #[cfg(target_os = "windows")]
        let resolved_steam_path = get_windows_steam_install_path(self.steam_path.as_deref())
            .map(|path| path.to_string_lossy().to_string());
        #[cfg(not(target_os = "windows"))]
        let resolved_steam_path = None;

        SteamOverlayConfigDto {
            enabled: self.enabled,
            steam_path: self.steam_path,
            resolved_steam_path,
        }
    }
}

fn disablemod_path(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    Ok(app
        .path()
        .app_data_dir()
        .map_err(|e| format!("failed to resolve app data dir: {e}"))?
        .join("config")
        .join("disablemod.json"))
}

fn steam_overlay_config_path(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    Ok(app
        .path()
        .app_data_dir()
        .map_err(|e| format!("failed to resolve app data dir: {e}"))?
        .join("config")
        .join("steam_overlay.json"))
}

pub(crate) fn thunderstore_cache_path(
    app: &tauri::AppHandle,
) -> Result<std::path::PathBuf, String> {
    Ok(app
        .path()
        .app_data_dir()
        .map_err(|e| format!("failed to resolve app data dir: {e}"))?
        .join("cache")
        .join("thunderstore.json"))
}

fn read_disablemod(app: &tauri::AppHandle) -> Result<DisableModFile, String> {
    let path = disablemod_path(app)?;
    let mut default_mods = vec![
        normalize_mod_id("SlushyRH", "FreeeeeeMoooooons"),
        normalize_mod_id("stormytuna", "EclipseOnly"),
    ];
    default_mods.sort_by(|a, b| a.dev.cmp(&b.dev).then(a.name.cmp(&b.name)));
    if !path.exists() {
        // v3 (migration): include default disabled layer mods.
        let f = DisableModFile {
            version: 3,
            mods: default_mods,
        };
        // best-effort persist so frontend sees stable state
        let _ = write_disablemod(app, &f);
        return Ok(f);
    }
    let text = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let mut f = match serde_json::from_str::<DisableModFile>(&text) {
        Ok(v) => v,
        Err(e) => {
            // If the file is corrupted, recover with defaults rather than breaking the UI.
            log::warn!("Failed to parse disablemod.json, resetting: {e}");
            let f = DisableModFile {
                version: 3,
                mods: default_mods,
            };
            let _ = write_disablemod(app, &f);
            return Ok(f);
        }
    };

    // Migration: v1 -> v2
    if f.version == 1 {
        f.version = 2;
        f.mods.push(normalize_mod_id("SlushyRH", "FreeeeeeMoooooons"));
        f.mods
            .sort_by(|a, b| a.dev.cmp(&b.dev).then(a.name.cmp(&b.name)));
        f.mods.dedup();
        let _ = write_disablemod(app, &f);
    }
    if f.version < 3 {
        f.version = 3;
        f.mods.push(normalize_mod_id("stormytuna", "EclipseOnly"));
        f.mods
            .sort_by(|a, b| a.dev.cmp(&b.dev).then(a.name.cmp(&b.name)));
        f.mods.dedup();
        let _ = write_disablemod(app, &f);
    }

    Ok(f)
}

fn write_disablemod(app: &tauri::AppHandle, f: &DisableModFile) -> Result<(), String> {
    let path = disablemod_path(app)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(f).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| e.to_string())
}

fn read_steam_overlay_config(app: &tauri::AppHandle) -> Result<SteamOverlayConfig, String> {
    let path = steam_overlay_config_path(app)?;
    if !path.exists() {
        let cfg = SteamOverlayConfig::default();
        let _ = write_steam_overlay_config(app, &cfg);
        return Ok(cfg);
    }

    let text = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    match serde_json::from_str::<SteamOverlayConfig>(&text) {
        Ok(cfg) => Ok(cfg),
        Err(e) => {
            log::warn!("Failed to parse steam_overlay.json, resetting: {e}");
            let cfg = SteamOverlayConfig::default();
            let _ = write_steam_overlay_config(app, &cfg);
            Ok(cfg)
        }
    }
}

fn write_steam_overlay_config(
    app: &tauri::AppHandle,
    cfg: &SteamOverlayConfig,
) -> Result<(), String> {
    let path = steam_overlay_config_path(app)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(cfg).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| e.to_string())
}

fn normalize_mod_id(dev: &str, name: &str) -> DisabledMod {
    DisabledMod {
        dev: dev.trim().to_lowercase(),
        name: name.trim().to_lowercase(),
    }
}

fn normalize_mod_key(dev: &str, name: &str) -> String {
    let id = normalize_mod_id(dev, name);
    format!("{}::{}", id.dev, id.name)
}

fn mod_keys_from_pairs(mods: &[(String, String)]) -> HashSet<String> {
    mods.iter()
        .map(|(dev, name)| normalize_mod_key(dev, name))
        .collect()
}

fn disabled_mod_keys(disabled_mods: &[DisabledMod]) -> HashSet<String> {
    disabled_mods
        .iter()
        .map(|m| normalize_mod_key(&m.dev, &m.name))
        .collect()
}

fn collect_mod_entries_recursive(
    root: &std::path::Path,
) -> Result<Vec<std::path::PathBuf>, String> {
    let mut out: Vec<std::path::PathBuf> = vec![];
    let mut stack: Vec<std::path::PathBuf> = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for e in std::fs::read_dir(&dir).map_err(|e| e.to_string())? {
            let e = e.map_err(|e| e.to_string())?;
            let path = e.path();
            let ty = e.file_type().map_err(|e| e.to_string())?;
            out.push(path.clone());
            if ty.is_dir() {
                stack.push(path);
            }
        }
    }
    out.sort_by_key(|path| std::cmp::Reverse(path.components().count()));
    Ok(out)
}

fn set_mod_files_old_suffix(mod_dir: &std::path::Path, enabled: bool) -> Result<(), String> {
    if !mod_dir.exists() {
        return Ok(());
    }

    let entries = collect_mod_entries_recursive(mod_dir)?;

    if enabled {
        // Remove .old suffix
        for path in entries {
            let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
            if !name.to_lowercase().ends_with(".old") {
                continue;
            }
            let mut new_name = name.to_string();
            new_name.truncate(new_name.len().saturating_sub(4));
            let new_path = path.with_file_name(new_name);
            if new_path.exists() {
                // Don't overwrite; keep the .old file.
                continue;
            }
            std::fs::rename(&path, new_path).map_err(|e| e.to_string())?;
        }
    } else {
        // Add .old suffix
        for path in entries {
            let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
            if name.to_lowercase().ends_with(".old") {
                continue;
            }
            let new_path = path.with_file_name(format!("{name}.old"));
            if new_path.exists() {
                continue;
            }
            std::fs::rename(&path, new_path).map_err(|e| e.to_string())?;
        }
    }

    Ok(())
}

fn sync_mod_dirs_with_effective_state(
    root: &std::path::Path,
    disabled_keys: &HashSet<String>,
    forced_disabled_keys: &HashSet<String>,
    forced_enabled_keys: &HashSet<String>,
) -> Result<(), String> {
    if !root.exists() {
        return Ok(());
    }

    let rd = std::fs::read_dir(root).map_err(|e| e.to_string())?;
    for e in rd {
        let e = e.map_err(|e| e.to_string())?;
        let path = e.path();
        if !path.is_dir() {
            continue;
        }
        let folder = e.file_name().to_string_lossy().to_string();
        let Some((dev, name)) = folder.split_once('-') else {
            continue;
        };
        let key = normalize_mod_key(dev, name);
        let enabled = if forced_enabled_keys.contains(&key) {
            true
        } else if forced_disabled_keys.contains(&key) {
            false
        } else {
            !disabled_keys.contains(&key)
        };
        set_mod_files_old_suffix(&path, enabled)?;
    }

    Ok(())
}

fn apply_effective_mod_states_for_version(
    app: &tauri::AppHandle,
    version: u32,
    forced_disabled_mods: &[(String, String)],
    forced_enabled_mods: &[(String, String)],
) -> Result<(), String> {
    let list = read_disablemod(app)?;
    let disabled_keys = disabled_mod_keys(&list.mods);
    let forced_disabled_keys = mod_keys_from_pairs(forced_disabled_mods);
    let forced_enabled_keys = mod_keys_from_pairs(forced_enabled_mods);

    let plugins = plugins_dir(app, version)?;
    let patchers = patchers_dir(app, version)?;
    sync_mod_dirs_with_effective_state(
        &plugins,
        &disabled_keys,
        &forced_disabled_keys,
        &forced_enabled_keys,
    )?;
    sync_mod_dirs_with_effective_state(
        &patchers,
        &disabled_keys,
        &forced_disabled_keys,
        &forced_enabled_keys,
    )?;
    let _ = sync_fontpatcher_with_assets_for_version(app, version);
    Ok(())
}

fn wait_for_mod_file_renames_to_settle() {
    // Renames are synchronous, but a short pause helps avoid launch races on some systems
    // where the game starts while plugin DLL name changes are still propagating.
    std::thread::sleep(std::time::Duration::from_millis(350));
}

fn emit_basic_mod_files_progress(
    app: &tauri::AppHandle,
    version: u32,
    working: bool,
    detail: &str,
) {
    let (step_progress, overall_percent, extracted_files) = if working {
        (0.0, 0.0, 0)
    } else {
        (1.0, 100.0, 1)
    };

    progress::emit_progress(
        app,
        TaskProgressPayload {
            version,
            steps_total: 1,
            step: 1,
            step_name: "Mod Files".to_string(),
            step_progress,
            overall_percent,
            detail: Some(detail.to_string()),
            downloaded_bytes: None,
            total_bytes: None,
            extracted_files: Some(extracted_files),
            total_files: Some(1),
        },
    );
}

fn describe_mode_file_changes(practice: bool, tags: &[String]) -> String {
    let mut parts: Vec<String> = vec![];
    if practice {
        parts.push("practice mods".to_string());
        parts.push("VLog".to_string());
    } else {
        parts.push("run-mode mods".to_string());
        parts.push("VLog".to_string());
    }
    if !tags.is_empty() {
        parts.push(format!("preset mods ({})", tags.join(", ")));
    }
    format!("Applying mod file changes for {}...", parts.join(", "))
}

// (intentionally no "is_disabled"/"is_mod_enabled" helpers; frontend uses disablemod list as source of truth)

fn apply_disabled_mods_for_version(app: &tauri::AppHandle, version: u32) -> Result<(), String> {
    apply_effective_mod_states_for_version(app, version, &[], &[])
}

fn hqol_mod_dir(plugins_dir: &std::path::Path) -> Option<std::path::PathBuf> {
    mod_dir_for(plugins_dir, "HQHQTeam", "HQoL")
        .or_else(|| mod_dir_for(plugins_dir, "HQHQTeam", "HQOL"))
}

fn sync_hqol_with_disablemod_for_version(
    app: &tauri::AppHandle,
    version: u32,
) -> Result<(), String> {
    let list = read_disablemod(app)?;
    let id1 = normalize_mod_id("HQHQTeam", "HQoL");
    let id2 = normalize_mod_id("HQHQTeam", "HQOL");
    let disabled = list.mods.iter().any(|m| m == &id1 || m == &id2);

    let plugins = plugins_dir(app, version)?;
    if let Some(dir) = hqol_mod_dir(&plugins) {
        let _ = set_mod_files_old_suffix(&dir, !disabled);
    }
    Ok(())
}

fn sync_named_mod_with_disablemod_for_version(
    app: &tauri::AppHandle,
    version: u32,
    dev: &str,
    name: &str,
) -> Result<(), String> {
    let disabled = read_disablemod(app)?
        .mods
        .contains(&normalize_mod_id(dev, name));

    let plugins = plugins_dir(app, version)?;
    if let Some(dir) = mod_dir_for(&plugins, dev, name) {
        let _ = set_mod_files_old_suffix(&dir, !disabled);
    }

    let patchers = patchers_dir(app, version)?;
    if let Some(dir) = mod_dir_for(&patchers, dev, name) {
        let _ = set_mod_files_old_suffix(&dir, !disabled);
    }

    Ok(())
}

fn sync_vlog_with_disablemod_for_version(
    app: &tauri::AppHandle,
    version: u32,
) -> Result<(), String> {
    sync_named_mod_with_disablemod_for_version(app, version, "HQHQTeam", "VLog")
}

fn practice_mode_mod_ids() -> Vec<(String, String)> {
    variable::get_practice_mod_list()
        .into_iter()
        .map(|m| (m.dev, m.name))
        .collect()
}

fn practice_mode_forced_disabled_ids() -> Vec<(String, String)> {
    let mut mods = practice_mode_mod_ids();
    mods.push(("HQHQTeam".to_string(), "VLog".to_string()));
    mods
}

fn sync_practice_locked_mods_for_version(
    version_plugins_dir: &std::path::Path,
) -> Result<(), String> {
    for (dev, name) in [("HQHQTeam", "VLog")] {
        if let Some(dir) = mod_dir_for(version_plugins_dir, dev, name) {
            let _ = set_mod_files_old_suffix(&dir, false);
        }
    }
    Ok(())
}

fn filter_missing_mods_for_version(
    app: &tauri::AppHandle,
    version: u32,
    mods: &[mod_config::ModEntry],
) -> Result<Vec<mod_config::ModEntry>, String> {
    let plugins = plugins_dir(app, version)?;
    let patchers = patchers_dir(app, version)?;

    Ok(mods
        .iter()
        .filter(|m| {
            mod_dir_for(&plugins, &m.dev, &m.name).is_none()
                && mod_dir_for(&patchers, &m.dev, &m.name).is_none()
        })
        .cloned()
        .collect())
}

fn ensure_practice_mods_disabled_for_version(
    app: &tauri::AppHandle,
    version: u32,
) -> Result<(), String> {
    let practice = variable::get_practice_mod_list();
    // Apply non-practice runtime state for this version immediately, without
    // overwriting the user's persisted practice toggle preferences.
    let plugins = plugins_dir(app, version)?;
    let patchers = patchers_dir(app, version)?;
    for m in practice {
        if let Some(dir) = mod_dir_for(&plugins, &m.dev, &m.name) {
            let _ = set_mod_files_old_suffix(&dir, false);
        }
        if let Some(dir) = mod_dir_for(&patchers, &m.dev, &m.name) {
            let _ = set_mod_files_old_suffix(&dir, false);
        }
    }

    Ok(())
}

async fn prepare_practice_mods_for_version(
    app: &tauri::AppHandle,
    version: u32,
    cancel: Option<Arc<AtomicBool>>,
) -> Result<Vec<(String, String)>, String> {
    let game_root = version_dir(app, version)?;
    if !game_root.exists() {
        return Err(format!(
            "version folder not found: {}",
            game_root.to_string_lossy()
        ));
    }

    let practice_all = variable::get_practice_mod_list();
    let disabled_list = read_disablemod(app)?;
    let practice_enabled: Vec<mod_config::ModEntry> = practice_all
        .iter()
        .cloned()
        .filter(|m| {
            m.is_compatible(version)
                && (is_ui_hidden_mod(m)
                    || !disabled_list
                        .mods
                        .contains(&normalize_mod_id(&m.dev, &m.name)))
        })
        .collect();
    let practice_ids: Vec<(String, String)> = practice_enabled
        .iter()
        .map(|m| (m.dev.clone(), m.name.clone()))
        .collect();

    let missing_practice_mods = filter_missing_mods_for_version(app, version, &practice_enabled)?;
    if !missing_practice_mods.is_empty() {
        const STEPS_TOTAL: u32 = 1;
        progress::emit_progress(
            app,
            TaskProgressPayload {
                version,
                steps_total: STEPS_TOTAL,
                step: 1,
                step_name: "Practice Mods".to_string(),
                step_progress: 0.0,
                overall_percent: 0.0,
                detail: Some("Installing missing practice mods...".to_string()),
                downloaded_bytes: None,
                total_bytes: None,
                extracted_files: Some(0),
                total_files: Some(missing_practice_mods.len() as u64),
            },
        );

        let cfg = ModsConfig {
            mods: missing_practice_mods,
        };

        let install_res: Result<(), String> = mods::install_mods_with_progress(
            app,
            &game_root,
            version,
            &cfg,
            &[],
            cancel,
            |done, total, progress_info| {
                let step_progress = if total == 0 {
                    1.0
                } else {
                    (done as f64 / total as f64).clamp(0.0, 1.0)
                };
                progress::emit_progress(
                    app,
                    TaskProgressPayload {
                        version,
                        steps_total: STEPS_TOTAL,
                        step: 1,
                        step_name: "Practice Mods".to_string(),
                        step_progress,
                        overall_percent: overall_from_step(1, step_progress, STEPS_TOTAL),
                        detail: progress_info.detail,
                        downloaded_bytes: progress_info.downloaded_bytes,
                        total_bytes: progress_info.total_bytes,
                        extracted_files: progress_info.extracted_files.or(Some(done)),
                        total_files: progress_info.total_files.or(Some(total)),
                    },
                );
            },
        )
        .await;

        if let Err(e) = &install_res {
            progress::emit_error(
                app,
                TaskErrorPayload {
                    version,
                    run_mode: None,
                    message: e.clone(),
                },
            );
            return Err(e.clone());
        }
    }

    let plugins = plugins_dir(app, version)?;
    let patchers = patchers_dir(app, version)?;
    let mut rename_ops: Vec<(std::path::PathBuf, bool, String)> = vec![];
    for m in &practice_all {
        if let Some(dir) = mod_dir_for(&plugins, &m.dev, &m.name) {
            rename_ops.push((dir, false, format!("{}-{}", m.dev, m.name)));
        }
        if let Some(dir) = mod_dir_for(&patchers, &m.dev, &m.name) {
            rename_ops.push((dir, false, format!("{}-{}", m.dev, m.name)));
        }
    }
    for m in &practice_enabled {
        if let Some(dir) = mod_dir_for(&plugins, &m.dev, &m.name) {
            rename_ops.push((dir, true, format!("{}-{}", m.dev, m.name)));
        }
        if let Some(dir) = mod_dir_for(&patchers, &m.dev, &m.name) {
            rename_ops.push((dir, true, format!("{}-{}", m.dev, m.name)));
        }
    }
    for (dev, name) in [("HQHQTeam", "VLog")] {
        if let Some(dir) = mod_dir_for(&plugins, dev, name) {
            rename_ops.push((dir, false, format!("{dev}-{name}")));
        }
    }
    for (dir, enabled, _label) in rename_ops {
        let _ = set_mod_files_old_suffix(&dir, enabled);
    }

    Ok(practice_ids)
}

#[derive(Default)]
struct GameState {
    active: Mutex<Vec<ActiveGame>>,
    next_id: AtomicU64,
    launch_lock: Mutex<()>,
}

struct ActiveGame {
    id: u64,
    child: std::process::Child,
    version: u32,
    mode_label: String,
    launch_options: Vec<String>,
    launch_command_template: Option<String>,
}

#[derive(Default)]
struct DownloadState {
    active: Mutex<Option<ActiveDownload>>,
}

struct ActiveDownload {
    version: u32,
    cancel: Arc<AtomicBool>,
}

#[derive(Default)]
struct PrepareState {
    next_id: std::sync::atomic::AtomicU64,
    active: Mutex<Option<ActivePrepare>>,
}

fn wait_for_prepare_to_finish(
    state: &PrepareState,
    version: u32,
    timeout: std::time::Duration,
) -> Result<(), String> {
    let start = std::time::Instant::now();
    loop {
        let is_active_for_version = {
            let guard = state
                .active
                .lock()
                .map_err(|_| "prepare state lock poisoned".to_string())?;
            guard.as_ref().is_some_and(|a| a.version == version)
        };

        if !is_active_for_version {
            return Ok(());
        }

        if start.elapsed() >= timeout {
            return Err(
                "mod file changes are still in progress; please wait a moment and try again"
                    .to_string(),
            );
        }

        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}

struct ActivePrepare {
    id: u64,
    version: u32,
    cancel: Arc<AtomicBool>,
}

#[derive(Debug, Clone, Serialize)]
struct GameStatus {
    running: bool,
    pid: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
struct RunningGameDto {
    id: u64,
    order: usize,
    pid: Option<u32>,
    version: u32,
    mode_label: String,
    launch_options: Vec<String>,
    launch_command_template: Option<String>,
}

#[tauri::command]
async fn download(
    app: tauri::AppHandle,
    version: u32,
    state: State<'_, DownloadState>,
) -> Result<bool, String> {
    // Only allow one active download at a time (simplifies cancel + UI state).
    let cancel = Arc::new(AtomicBool::new(false));
    {
        let mut guard = state
            .active
            .lock()
            .map_err(|_| "download state lock poisoned".to_string())?;
        if let Some(active) = guard.as_ref() {
            if !active.cancel.load(Ordering::Relaxed) {
                return Err(format!(
                    "download already in progress (v{}). Please cancel it first.",
                    active.version
                ));
            }
        }
        *guard = Some(ActiveDownload {
            version,
            cancel: cancel.clone(),
        });
    }

    let res = installer::download_and_setup(app.clone(), version, cancel.clone()).await;

    // Clear active download state (best-effort).
    {
        let mut guard = state
            .active
            .lock()
            .map_err(|_| "download state lock poisoned".to_string())?;
        if guard.as_ref().is_some_and(|a| a.version == version) {
            *guard = None;
        }
    }
    res
}

#[tauri::command]
fn cancel_download(
    app: tauri::AppHandle,
    version: u32,
    state: State<'_, DownloadState>,
) -> Result<bool, String> {
    let mut did_signal = false;
    {
        let guard = state
            .active
            .lock()
            .map_err(|_| "download state lock poisoned".to_string())?;
        if let Some(active) = guard.as_ref() {
            if active.version == version {
                active.cancel.store(true, Ordering::Relaxed);
                did_signal = true;
            }
        }
    }

    // Best-effort cleanup: delete the partial version folder.
    // If files are still in use, the installer will retry cleanup on exit.
    if did_signal {
        if let Ok(dir) = version_dir(&app, version) {
            let _ = std::fs::remove_dir_all(&dir);
        }
    }

    Ok(did_signal)
}

#[tauri::command]
async fn sync_latest_install_from_manifest(
    app: tauri::AppHandle,
    version: Option<u32>,
) -> Result<bool, String> {
    if let Some(version) = version {
        let game_root = app
            .path()
            .app_data_dir()
            .map_err(|e| format!("failed to resolve app data dir: {e}"))?
            .join("versions")
            .join(format!("v{version}"));
        installer::sync_install_from_manifest_for_version(&app, version, game_root).await?;
    } else {
        installer::sync_latest_install_from_manifest(app).await?;
    }
    Ok(true)
}

#[tauri::command]
async fn check_latest_install_manifest_update(
    app: tauri::AppHandle,
    version: Option<u32>,
) -> Result<installer::ManifestUpdateCheck, String> {
    if let Some(version) = version {
        installer::check_manifest_update_for_version(&app, version).await
    } else {
        installer::check_latest_install_manifest_update(&app).await
    }
}

#[tauri::command]
async fn open_version_folder(app: tauri::AppHandle) -> Result<bool, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("failed to resolve app data dir: {e}"))?
        .join("versions");
    std::fs::create_dir_all(&dir).map_err(|e| format!("failed to create versions dir: {e}"))?;
    open_folder_path(&dir)?;
    Ok(true)
}

fn open_folder_path(path: &Path) -> Result<(), String> {
    open_path_with_fallbacks(path, true)
}

fn open_file_path(path: &Path) -> Result<(), String> {
    open_path_with_fallbacks(path, false)
}

#[cfg(target_os = "linux")]
fn host_open_command(program: &str) -> std::process::Command {
    let mut command = std::process::Command::new(program);
    for key in [
        "APPDIR",
        "APPIMAGE",
        "ARGV0",
        "GIO_MODULE_DIR",
        "GTK_PATH",
        "LD_PRELOAD",
        "PYTHONHOME",
        "PYTHONPATH",
        "PYTHONPLATLIBDIR",
        "PYTHONSAFEPATH",
        "QT_PLUGIN_PATH",
        "QT_QPA_PLATFORM_PLUGIN_PATH",
    ] {
        command.env_remove(key);
    }
    if let Some(original_ld_library_path) = std::env::var_os("APPIMAGE_ORIGINAL_LD_LIBRARY_PATH") {
        command.env("LD_LIBRARY_PATH", original_ld_library_path);
    } else {
        command.env_remove("LD_LIBRARY_PATH");
    }
    command
}

#[cfg(target_os = "linux")]
fn command_status_ok(program: &str, args: &[&std::ffi::OsStr]) -> bool {
    let mut command = host_open_command(program);
    for arg in args {
        command.arg(arg);
    }
    command.status().is_ok_and(|status| status.success())
}

#[cfg(target_os = "linux")]
fn spawn_open_command(program: &str, path: &Path) -> bool {
    host_open_command(program).arg(path).spawn().is_ok()
}

#[cfg(target_os = "linux")]
fn open_path_with_fallbacks(path: &Path, is_folder: bool) -> Result<(), String> {
    if command_status_ok("xdg-open", &[path.as_os_str()])
        || command_status_ok("gio", &[std::ffi::OsStr::new("open"), path.as_os_str()])
        || (is_folder
            && (command_status_ok(
                "kioclient6",
                &[std::ffi::OsStr::new("exec"), path.as_os_str()],
            ) || command_status_ok(
                "kioclient5",
                &[std::ffi::OsStr::new("exec"), path.as_os_str()],
            )))
    {
        return Ok(());
    }

    if !is_folder {
        if let Some(editor) = std::env::var_os("VISUAL")
            .or_else(|| std::env::var_os("EDITOR"))
            .and_then(|value| {
                let value = value.to_string_lossy();
                shlex::split(&value).and_then(|parts| parts.into_iter().next())
            })
        {
            if host_open_command(&editor).arg(path).spawn().is_ok() {
                return Ok(());
            }
        }

        for program in [
            "kate",
            "kwrite",
            "gedit",
            "mousepad",
            "xed",
            "code",
            "codium",
        ] {
            if spawn_open_command(program, path) {
                return Ok(());
            }
        }

        if let Some(parent) = path.parent() {
            if open_folder_path(parent).is_ok() {
                return Ok(());
            }
        }
    }

    if is_folder {
        for program in [
            "dolphin",
            "nautilus",
            "thunar",
            "nemo",
            "pcmanfm",
            "pcmanfm-qt",
            "caja",
        ] {
            if spawn_open_command(program, path) {
                return Ok(());
            }
        }
    }

    opener::open(path).map_err(|e| e.to_string())
}

#[cfg(not(target_os = "linux"))]
fn open_path_with_fallbacks(path: &Path, _is_folder: bool) -> Result<(), String> {
    opener::open(path).map_err(|e| e.to_string())
}

fn collect_delete_targets(
    path: &std::path::Path,
    out: &mut Vec<std::path::PathBuf>,
) -> Result<(), String> {
    let rd = std::fs::read_dir(path).map_err(|e| e.to_string())?;
    for entry in rd {
        let entry = entry.map_err(|e| e.to_string())?;
        let child = entry.path();
        let ty = entry.file_type().map_err(|e| e.to_string())?;
        if ty.is_dir() && !ty.is_symlink() {
            collect_delete_targets(&child, out)?;
            out.push(child);
        } else {
            out.push(child);
        }
    }
    Ok(())
}

fn remove_delete_target(path: &std::path::Path) -> Result<(), String> {
    let ty = std::fs::symlink_metadata(path)
        .map_err(|e| e.to_string())?
        .file_type();

    if ty.is_dir() && !ty.is_symlink() {
        std::fs::remove_dir(path).map_err(|e| e.to_string())?;
        return Ok(());
    }

    if let Err(file_err) = std::fs::remove_file(path) {
        std::fs::remove_dir(path).map_err(|dir_err| format!("{file_err}; {dir_err}"))?;
    }
    Ok(())
}

#[tauri::command]
fn delete_installed_version(
    app: tauri::AppHandle,
    version: u32,
    game_state: State<'_, GameState>,
    download_state: State<'_, DownloadState>,
    prepare_state: State<'_, PrepareState>,
) -> Result<bool, String> {
    {
        let mut guard = game_state
            .active
            .lock()
            .map_err(|_| "game state lock poisoned".to_string())?;
        for active in guard.iter_mut() {
            if active.version == version
                && active.child.try_wait().map_err(|e| e.to_string())?.is_none()
            {
                return Err("Cannot delete a version while the game is running.".to_string());
            }
            if active.version == version && linux_lingering_game_pid(&app, version).is_some() {
                return Err("Cannot delete a version while the game is running.".to_string());
            }
        }
        guard.retain(|active| active.version != version);
    }

    {
        let guard = download_state
            .active
            .lock()
            .map_err(|_| "download state lock poisoned".to_string())?;
        if let Some(active) = guard.as_ref() {
            if active.version == version && !active.cancel.load(Ordering::Relaxed) {
                return Err("Cannot delete a version while it is downloading.".to_string());
            }
        }
    }

    {
        let guard = prepare_state
            .active
            .lock()
            .map_err(|_| "prepare state lock poisoned".to_string())?;
        if let Some(active) = guard.as_ref() {
            if active.version == version && !active.cancel.load(Ordering::Relaxed) {
                return Err("Cannot delete a version while it is being prepared.".to_string());
            }
        }
    }

    let dir = version_dir(&app, version)?;
    if !dir.exists() {
        return Ok(false);
    }

    let mut targets = Vec::new();
    collect_delete_targets(&dir, &mut targets)?;
    targets.push(dir.clone());

    let total = targets.len() as u64;
    progress::emit_progress(
        &app,
        TaskProgressPayload {
            version,
            steps_total: 1,
            step: 1,
            step_name: "Delete Version".to_string(),
            step_progress: 0.0,
            overall_percent: 0.0,
            detail: Some(format!("Preparing to delete v{version}...")),
            downloaded_bytes: None,
            total_bytes: None,
            extracted_files: Some(0),
            total_files: Some(total),
        },
    );

    for (idx, target) in targets.iter().enumerate() {
        remove_delete_target(target)?;
        let done = idx as u64 + 1;
        let step_progress = if total == 0 {
            1.0
        } else {
            done as f64 / total as f64
        };
        let detail = target
            .file_name()
            .and_then(|name| name.to_str())
            .map(|name| format!("Deleting {name}"))
            .unwrap_or_else(|| "Deleting files...".to_string());
        progress::emit_progress(
            &app,
            TaskProgressPayload {
                version,
                steps_total: 1,
                step: 1,
                step_name: "Delete Version".to_string(),
                step_progress,
                overall_percent: step_progress * 100.0,
                detail: Some(detail),
                downloaded_bytes: None,
                total_bytes: None,
                extracted_files: Some(done),
                total_files: Some(total),
            },
        );
    }

    Ok(true)
}

#[tauri::command]
async fn open_downloader_folder(app: tauri::AppHandle) -> Result<bool, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("failed to resolve app data dir: {e}"))?
        .join("downloader");
    std::fs::create_dir_all(&dir).map_err(|e| format!("failed to create downloader dir: {e}"))?;
    open_folder_path(&dir)?;
    Ok(true)
}

#[tauri::command]
async fn open_mod_folder(
    app: tauri::AppHandle,
    version: u32,
    dev: String,
    name: String,
) -> Result<bool, String> {
    let plugins = plugins_dir(&app, version)?;
    let patchers = patchers_dir(&app, version)?;

    let Some(dir) =
        mod_dir_for(&plugins, &dev, &name).or_else(|| mod_dir_for(&patchers, &dev, &name))
    else {
        return Err(format!(
            "mod folder not found for {dev}-{name} on v{version}"
        ));
    };

    open_folder_path(&dir)?;
    Ok(true)
}

#[tauri::command]
async fn check_mod_updates(
    app: tauri::AppHandle,
    version: u32,
    run_mode: Option<String>,
) -> Result<bool, String> {
    let client = reqwest::Client::new();
    let run_mode_name = run_mode.as_deref().unwrap_or("hq");

    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("failed to resolve app data dir: {e}"))?
        .join("versions");
    let extract_dir = dir.join(format!("v{version}"));
    let mods_cfg =
        effective_mods_config_for_run_mode(&client, version, run_mode_name, false, true).await?;
    let (preset, _practice) = preset_and_practice_for_run_mode(run_mode_name);
    let active_tags = preset_tags_for_name(&preset);

    let mut updatable_mods: Vec<String> = vec![];

    let res = mods::updatable_mods_with_progress(
        &app,
        &extract_dir,
        version,
        &mods_cfg,
        &active_tags,
        |checked, total, detail, mod_name| {
            if let Some(mod_name) = mod_name {
                if !updatable_mods.contains(&mod_name) {
                    updatable_mods.push(mod_name.clone());
                }
            }

            progress::emit_updatable_progress(
                &app,
                TaskUpdatableProgressPayload {
                    version,
                    run_mode: Some(run_mode_name.to_string()),
                    total,
                    checked,
                    updatable_mods: updatable_mods.clone(),
                    detail,
                },
            );
        },
    )
    .await;

    if let Err(e) = res {
        progress::emit_updatable_error(
            &app,
            TaskErrorPayload {
                version,
                run_mode: Some(run_mode_name.to_string()),
                message: e.clone(),
            },
        );
        return Err(e);
    }

    match purge_capped_incompatible_installed_mods(&app, version, Some(run_mode_name)).await {
        Ok(purged) => {
            if purged > 0 {
                log::info!(
                    "Purged {purged} incompatible capped mods after checking updates for v{version}"
                );
            }
        }
        Err(e) => {
            log::warn!(
                "Failed to purge incompatible capped mods after checking updates for v{version}: {e}"
            );
        }
    }

    progress::emit_updatable_finished(
        &app,
        TaskFinishedPayload {
            version,
            run_mode: Some(run_mode_name.to_string()),
            path: extract_dir.to_string_lossy().to_string(),
        },
    );
    Ok(true)
}

#[tauri::command]
async fn apply_mod_updates(
    app: tauri::AppHandle,
    version: u32,
    run_mode: Option<String>,
) -> Result<bool, String> {
    let run_mode_name = run_mode.as_deref().unwrap_or("hq").to_string();
    let res: Result<(), String> = async {
        let client = reqwest::Client::new();

        let dir = app
            .path()
            .app_data_dir()
            .map_err(|e| format!("failed to resolve app data dir: {e}"))?
            .join("versions");
        let game_root = dir.join(format!("v{version}"));
        if !game_root.exists() {
            return Err(format!(
                "version folder not found: {}",
                game_root.to_string_lossy()
            ));
        }

        let mods_cfg = effective_mods_config_for_run_mode(
            &client,
            version,
            &run_mode_name,
            false,
            true,
        )
        .await?;
        let (preset, _practice) = preset_and_practice_for_run_mode(&run_mode_name);
        let active_tags = preset_tags_for_name(&preset);

        const STEPS_TOTAL: u32 = 2;
        progress::emit_progress(
            &app,
            TaskProgressPayload {
                version,
                steps_total: STEPS_TOTAL,
                step: 1,
                step_name: "Check Updates".to_string(),
                step_progress: 0.0,
                overall_percent: overall_from_step(1, 0.0, STEPS_TOTAL),
                detail: Some("Checking updatable mods...".to_string()),
                downloaded_bytes: None,
                total_bytes: None,
                extracted_files: None,
                total_files: None,
            },
        );

        let mut updatable: Vec<String> = vec![];
        mods::updatable_mods_with_progress(
            &app,
            &game_root,
            version,
            &mods_cfg,
            &active_tags,
            |checked, total, detail, mod_name| {
                if let Some(m) = mod_name {
                    if !updatable.contains(&m) {
                        updatable.push(m);
                    }
                }
                let step_progress = if total == 0 {
                    1.0
                } else {
                    (checked as f64 / total as f64).clamp(0.0, 1.0)
                };
                progress::emit_progress(
                    &app,
                    TaskProgressPayload {
                        version,
                        steps_total: STEPS_TOTAL,
                        step: 1,
                        step_name: "Check Updates".to_string(),
                        step_progress,
                        overall_percent: overall_from_step(1, step_progress, STEPS_TOTAL),
                        detail,
                        downloaded_bytes: None,
                        total_bytes: None,
                        extracted_files: Some(checked),
                        total_files: Some(total),
                    },
                );
            },
        )
        .await?;

        match purge_capped_incompatible_installed_mods(&app, version, Some(&run_mode_name)).await {
            Ok(purged) => {
                if purged > 0 {
                    log::info!(
                        "Purged {purged} incompatible capped mods during update check for v{version}"
                    );
                }
            }
            Err(e) => {
                log::warn!(
                    "Failed to purge incompatible capped mods during update check for v{version}: {e}"
                );
            }
        }

        if updatable.is_empty() {
            progress::emit_progress(
                &app,
                TaskProgressPayload {
                    version,
                    steps_total: STEPS_TOTAL,
                    step: 2,
                    step_name: "Update Mods".to_string(),
                    step_progress: 1.0,
                    overall_percent: 100.0,
                    detail: Some("No updates available".to_string()),
                    downloaded_bytes: None,
                    total_bytes: None,
                    extracted_files: None,
                    total_files: None,
                },
            );
            return Ok(());
        }

        progress::emit_progress(
            &app,
            TaskProgressPayload {
                version,
                steps_total: STEPS_TOTAL,
                step: 2,
                step_name: "Update Mods".to_string(),
                step_progress: 0.0,
                overall_percent: overall_from_step(2, 0.0, STEPS_TOTAL),
                detail: Some(format!("Updating {} mods...", updatable.len())),
                downloaded_bytes: None,
                total_bytes: None,
                extracted_files: Some(0),
                total_files: Some(updatable.len() as u64),
            },
        );

        mods::update_mods_with_progress(
            &app,
            &game_root,
            version,
            &mods_cfg,
            updatable.clone(),
            |done, total, progress_info| {
                let step_progress = if total == 0 {
                    1.0
                } else {
                    (done as f64 / total as f64).clamp(0.0, 1.0)
                };
                progress::emit_progress(
                    &app,
                    TaskProgressPayload {
                        version,
                        steps_total: STEPS_TOTAL,
                        step: 2,
                        step_name: "Update Mods".to_string(),
                        step_progress,
                        overall_percent: overall_from_step(2, step_progress, STEPS_TOTAL),
                        detail: progress_info.detail,
                        downloaded_bytes: progress_info.downloaded_bytes,
                        total_bytes: progress_info.total_bytes,
                        extracted_files: progress_info.extracted_files.or(Some(done)),
                        total_files: progress_info.total_files.or(Some(total)),
                    },
                );
            },
        )
        .await?;

        let _ = ensure_reverb_trigger_fix_cfg(&app, version);

        Ok(())
    }
    .await;

    match res {
        Ok(()) => {
            progress::emit_finished(
                &app,
                TaskFinishedPayload {
                    version,
                    run_mode: Some(run_mode_name.clone()),
                    path: version_dir(&app, version)?.to_string_lossy().to_string(),
                },
            );
            Ok(true)
        }
        Err(e) => {
            progress::emit_error(
                &app,
                TaskErrorPayload {
                    version,
                    run_mode: Some(run_mode_name.clone()),
                    message: e.clone(),
                },
            );
            Err(e)
        }
    }
}

#[cfg(target_os = "linux")]
fn get_steam_client_path(
    launcher_root: &std::path::Path,
    configured_path: Option<&str>,
) -> std::path::PathBuf {
    if let Some(configured) = configured_path
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let candidate = std::path::PathBuf::from(configured);
        if candidate.exists() {
            return candidate;
        }
    }

    if let Some(home_dir) = dirs::home_dir() {
        let steam_paths = [
            home_dir.join(".steam/steam"),
            home_dir.join(".local/share/Steam"),
            home_dir.join(".var/app/com.valvesoftware.Steam/.local/share/Steam"),
        ];
        for path in steam_paths.iter() {
            if path.exists() && path.join("steamapps").exists() {
                println!("Found real Steam installation at: {:?}", path);
                return path.clone();
            }
        }
    }

    println!("Steam not found. Mocking client path.");
    launcher_root.to_path_buf()
}

#[cfg(target_os = "linux")]
fn linux_overlay_preload_value(steam_path: &std::path::Path) -> Option<String> {
    let mut libs: Vec<String> = vec![];
    for rel in [
        std::path::Path::new("ubuntu12_32").join("gameoverlayrenderer.so"),
        std::path::Path::new("ubuntu12_64").join("gameoverlayrenderer.so"),
    ] {
        let candidate = steam_path.join(rel);
        if candidate.exists() {
            libs.push(candidate.to_string_lossy().to_string());
        }
    }

    if libs.is_empty() {
        None
    } else {
        Some(libs.join(":"))
    }
}

fn resolve_game_launch_paths(
    app: &tauri::AppHandle,
    version: u32,
) -> Result<(std::path::PathBuf, std::path::PathBuf, std::path::PathBuf), String> {
    let dir = version_dir(app, version)?;
    if !dir.exists() {
        return Err(format!(
            "version folder not found: {}",
            dir.to_string_lossy()
        ));
    }

    let exe_name = "Lethal Company.exe";
    let exe_path = dir.join(exe_name);
    let exe_path = if exe_path.exists() {
        exe_path
    } else {
        find_file_named(&dir, exe_name, 3)
            .ok_or_else(|| format!("{exe_name} not found under {}", dir.to_string_lossy()))?
    };

    let exe_dir = exe_path
        .parent()
        .ok_or_else(|| "invalid exe path".to_string())?
        .to_path_buf();

    Ok((dir, exe_path, exe_dir))
}

fn ensure_game_not_running(app: &tauri::AppHandle, state: &State<'_, GameState>) -> Result<(), String> {
    let mut guard = state
        .active
        .lock()
        .map_err(|_| "game state lock poisoned".to_string())?;
    let mut has_running = false;
    for active in guard.iter_mut() {
        if active.child.try_wait().map_err(|e| e.to_string())?.is_none() {
            has_running = true;
        } else if linux_lingering_game_pid(app, active.version).is_some() {
            has_running = true;
        }
    }
    if has_running {
        return Err("game is already running".to_string());
    }
    guard.clear();
    Ok(())
}

fn cleanup_active_games(
    app: &tauri::AppHandle,
    active_games: &mut Vec<ActiveGame>,
) -> Result<bool, String> {
    let mut any_finished = false;
    let mut kept = Vec::with_capacity(active_games.len());
    for mut active in active_games.drain(..) {
        match active.child.try_wait().map_err(|e| e.to_string())? {
            None => kept.push(active),
            Some(_) => {
                if linux_lingering_game_pid(app, active.version).is_some() {
                    kept.push(active);
                } else {
                    any_finished = true;
                }
            }
        }
    }
    *active_games = kept;
    Ok(any_finished)
}

fn active_game_dto(order: usize, active: &ActiveGame) -> RunningGameDto {
    RunningGameDto {
        id: active.id,
        order,
        pid: Some(active.child.id()),
        version: active.version,
        mode_label: active.mode_label.clone(),
        launch_options: active.launch_options.clone(),
        launch_command_template: active.launch_command_template.clone(),
    }
}

#[cfg(target_os = "windows")]
fn inject_launch_dlls(
    pid: u32,
    version_dir: &std::path::Path,
    steam_path_override: Option<&str>,
) -> Result<(), String> {
    let Some(steam_path) = get_windows_steam_install_path(steam_path_override) else {
        log::warn!("Steam install path not found; skipping Steam overlay DLL injection");
        return Ok(());
    };

    for dll in steam_overlay_dlls(&steam_path) {
        if !dll.exists() {
            log::warn!(
                "Steam overlay DLL not found, skipping injection for {}",
                dll.to_string_lossy()
            );
            continue;
        }
        inject_dll_into_process(pid, &dll)?;
    }

    let dll_path = version_dir.join("winhttp.dll");
    if !dll_path.exists() {
        log::warn!("winhttp.dll not found: {}", dll_path.to_string_lossy());
        return Ok(());
    }
    inject_dll_into_process(pid, &dll_path)?;
    Ok(())
}

fn parse_launch_env_assignment(entry: &str) -> Option<(&str, &str)> {
    let trimmed = entry.trim();
    let (name, value) = trimmed.split_once('=')?;
    if name.is_empty() {
        return None;
    }

    let mut chars = name.chars();
    let first = chars.next()?;
    if !(first == '_' || first.is_ascii_alphabetic()) {
        return None;
    }
    if !chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric()) {
        return None;
    }

    Some((name, value))
}

fn apply_custom_launch_options(command: &mut std::process::Command, launch_options: &[String]) {
    for raw_entry in launch_options {
        let entry = raw_entry.trim();
        if entry.is_empty() {
            continue;
        }

        if let Some((name, value)) = parse_launch_env_assignment(entry) {
            command.env(name, value);
        } else {
            command.arg(entry);
        }
    }
}

#[cfg(target_os = "linux")]
fn put_command_in_new_process_group(command: &mut std::process::Command) {
    use std::os::unix::process::CommandExt;

    unsafe {
        command.pre_exec(|| {
            if libc::setsid() == -1 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
}

#[cfg(not(target_os = "linux"))]
fn put_command_in_new_process_group(_command: &mut std::process::Command) {}

#[cfg(target_os = "linux")]
fn terminate_child_process_tree(child: &mut std::process::Child) {
    let pgid = child.id() as libc::pid_t;
    unsafe {
        let _ = libc::kill(-pgid, libc::SIGTERM);
    }

    for _ in 0..15 {
        if child.try_wait().ok().flatten().is_some() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    unsafe {
        let _ = libc::kill(-pgid, libc::SIGKILL);
    }
    let _ = child.kill();
}

#[cfg(not(target_os = "linux"))]
fn terminate_child_process_tree(child: &mut std::process::Child) {
    let _ = child.kill();
}

#[cfg(target_os = "linux")]
fn read_proc_bytes(pid: libc::pid_t, name: &str) -> Option<Vec<u8>> {
    std::fs::read(format!("/proc/{pid}/{name}")).ok()
}

#[cfg(target_os = "linux")]
fn bytes_contain(haystack: &[u8], needle: &std::path::Path) -> bool {
    let needle = needle.to_string_lossy();
    haystack
        .windows(needle.len())
        .any(|window| window == needle.as_bytes())
}

#[cfg(target_os = "linux")]
fn collect_linux_game_processes(app: &tauri::AppHandle, version: u32) -> Vec<libc::pid_t> {
    let self_pid = std::process::id() as libc::pid_t;
    let Ok(game_root) = version_dir(app, version) else {
        return vec![];
    };
    let compat_prefix = installer::proton_env_dir(app)
        .ok()
        .map(|path| path.join("wine_prefix"));
    let compat_env = compat_prefix
        .as_ref()
        .map(|path| format!("STEAM_COMPAT_DATA_PATH={}", path.to_string_lossy()));

    let Ok(entries) = std::fs::read_dir("/proc") else {
        return vec![];
    };

    let mut pids = Vec::new();
    for entry in entries.flatten() {
        let Some(pid) = entry
            .file_name()
            .to_str()
            .and_then(|name| name.parse::<libc::pid_t>().ok())
        else {
            continue;
        };
        if pid <= 1 || pid == self_pid {
            continue;
        }

        let cmdline = read_proc_bytes(pid, "cmdline").unwrap_or_default();
        let environ = read_proc_bytes(pid, "environ").unwrap_or_default();

        let matches_version = bytes_contain(&cmdline, &game_root)
            || bytes_contain(&environ, &game_root);
        let matches_prefix = compat_prefix
            .as_ref()
            .is_some_and(|prefix| bytes_contain(&cmdline, prefix) || bytes_contain(&environ, prefix))
            || compat_env
                .as_ref()
                .is_some_and(|env| environ.windows(env.len()).any(|window| window == env.as_bytes()));

        if matches_version || matches_prefix {
            pids.push(pid);
        }
    }

    pids.sort_unstable();
    pids.dedup();
    pids
}

#[cfg(target_os = "linux")]
fn pid_is_alive(pid: libc::pid_t) -> bool {
    unsafe { libc::kill(pid, 0) == 0 }
}

#[cfg(target_os = "linux")]
fn terminate_linux_game_processes_for_version(app: &tauri::AppHandle, version: u32) {
    let pids = collect_linux_game_processes(app, version);
    if pids.is_empty() {
        return;
    }

    log::info!(
        "Stopping {} lingering Linux game processes for v{}: {:?}",
        pids.len(),
        version,
        pids
    );

    for pid in &pids {
        unsafe {
            let _ = libc::kill(*pid, libc::SIGTERM);
        }
    }

    for _ in 0..15 {
        if pids.iter().all(|pid| !pid_is_alive(*pid)) {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    for pid in pids {
        if pid_is_alive(pid) {
            unsafe {
                let _ = libc::kill(pid, libc::SIGKILL);
            }
        }
    }
}

#[cfg(target_os = "linux")]
fn linux_lingering_game_pid(app: &tauri::AppHandle, version: u32) -> Option<u32> {
    collect_linux_game_processes(app, version)
        .into_iter()
        .next()
        .map(|pid| pid as u32)
}

#[cfg(not(target_os = "linux"))]
fn linux_lingering_game_pid(_app: &tauri::AppHandle, _version: u32) -> Option<u32> {
    None
}

#[cfg(not(target_os = "linux"))]
fn terminate_linux_game_processes_for_version(_app: &tauri::AppHandle, _version: u32) {}

fn build_wrapped_launch_command(
    template: Option<&str>,
    default_program: &std::ffi::OsStr,
    default_args: &[OsString],
) -> Result<(OsString, Vec<OsString>), String> {
    let default_tokens: Vec<OsString> = std::iter::once(default_program.to_os_string())
        .chain(default_args.iter().cloned())
        .collect();

    let Some(template) = template.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok((default_program.to_os_string(), default_args.to_vec()));
    };

    let parsed = shlex::split(template)
        .ok_or_else(|| "failed to parse launch command template".to_string())?;
    if parsed.is_empty() {
        return Ok((default_program.to_os_string(), default_args.to_vec()));
    }

    let mut wrapped_tokens = Vec::new();
    let mut inserted_command = false;
    for token in parsed {
        if token == "%command%" {
            wrapped_tokens.extend(default_tokens.iter().cloned());
            inserted_command = true;
        } else {
            wrapped_tokens.push(OsString::from(token));
        }
    }

    if !inserted_command {
        wrapped_tokens.extend(default_tokens);
    }

    let mut parts = wrapped_tokens.into_iter();
    let program = parts
        .next()
        .ok_or_else(|| "launch command template produced an empty command".to_string())?;
    Ok((program, parts.collect()))
}

fn spawn_game_process(
    _app: &tauri::AppHandle,
    _version_dir: &std::path::Path,
    exe_path: &std::path::Path,
    exe_dir: &std::path::Path,
    launch_options: &[String],
    launch_command_template: Option<&str>,
) -> Result<std::process::Child, String> {
    #[cfg(target_os = "windows")]
    let overlay_config = read_steam_overlay_config(_app)?;
    #[cfg(target_os = "linux")]
    let overlay_config = read_steam_overlay_config(_app)?;

    #[cfg(target_os = "windows")]
    let mut command = {
        use std::os::windows::process::CommandExt;
        use windows_sys::Win32::System::Threading::CREATE_SUSPENDED;

        let default_program = exe_path.as_os_str().to_os_string();
        let default_args = Vec::new();
        let (program, args) =
            build_wrapped_launch_command(launch_command_template, &default_program, &default_args)?;
        let mut cmd = std::process::Command::new(program);
        cmd.args(args);
        if overlay_config.enabled {
            cmd.creation_flags(CREATE_SUSPENDED);
            cmd.env("SteamGameId", LETHAL_COMPANY_STEAM_APP_ID);
            cmd.env("SteamAppId", LETHAL_COMPANY_STEAM_APP_ID);
        }
        cmd
    };

    #[cfg(target_os = "macos")]
    let mut command = {
        let default_program = OsString::from("open");
        let default_args = vec![OsString::from("-a"), exe_path.as_os_str().to_os_string()];
        let (program, args) =
            build_wrapped_launch_command(launch_command_template, &default_program, &default_args)?;
        let mut cmd = std::process::Command::new(program);
        cmd.args(args);
        cmd
    };

    #[cfg(target_os = "linux")]
    let mut command = {
        let app_path = _app
            .path()
            .app_data_dir()
            .map_err(|e| format!("app path not found: {e}"))?;
        let proton_env_path = installer::proton_env_dir(_app)
            .map_err(|e| format!("proton_env path not found: {e}"))?;
        let proton_bin_path = installer::get_current_proton_dir_impl(_app)
            .map_err(|e| format!("proton path not found: {e}"))?
            .ok_or("found proton path but is None")?;
        let compat_pre_path = proton_env_path.join("wine_prefix");
        if !compat_pre_path.exists() {
            std::fs::create_dir(&compat_pre_path)
                .map_err(|e| format!("could not make prefix: {e}"))?;
        }
        let steam_path = get_steam_client_path(&app_path, overlay_config.steam_path.as_deref());
        let default_program = proton_bin_path.join("proton").into_os_string();
        let default_args = vec![OsString::from("run"), exe_path.as_os_str().to_os_string()];
        let (program, args) =
            build_wrapped_launch_command(launch_command_template, &default_program, &default_args)?;
        let mut cmd = std::process::Command::new(program);
        cmd.args(args);
        cmd.env("STEAM_COMPAT_DATA_PATH", &compat_pre_path);
        cmd.env("STEAM_COMPAT_CLIENT_INSTALL_PATH", &steam_path);
        cmd.env("WINEDLLOVERRIDES", "winhttp=n,b");
        if overlay_config.enabled {
            cmd.env("SteamGameId", LETHAL_COMPANY_STEAM_APP_ID);
            cmd.env("SteamAppId", LETHAL_COMPANY_STEAM_APP_ID);
            if let Some(overlay_preload) = linux_overlay_preload_value(&steam_path) {
                let preload = std::env::var("LD_PRELOAD")
                    .ok()
                    .filter(|value| !value.trim().is_empty())
                    .map(|existing| format!("{overlay_preload}:{existing}"))
                    .unwrap_or(overlay_preload);
                cmd.env("LD_PRELOAD", preload);
            } else {
                log::warn!(
                    "Steam overlay enabled, but no Linux overlay renderer found under {}",
                    steam_path.to_string_lossy()
                );
            }
        }
        cmd.env_remove("PYTHONPATH");
        cmd.env_remove("PYTHONHOME");
        cmd
    };

    apply_custom_launch_options(&mut command, launch_options);
    put_command_in_new_process_group(&mut command);

    #[allow(unused_mut)]
    let mut child = command
        .current_dir(exe_dir)
        .spawn()
        .map_err(|e| format!("failed to launch: {e}"))?;

    #[cfg(target_os = "windows")]
    {
        if overlay_config.enabled {
            if let Err(e) = inject_launch_dlls(
                child.id(),
                _version_dir,
                overlay_config.steam_path.as_deref(),
            ) {
                log::error!(
                    "failed to inject launch DLLs into pid {}: {}",
                    child.id(),
                    e
                );
                let _ = child.kill();
                let _ = child.wait();
                return Err(e);
            }
            if let Err(e) = resume_main_thread(child.id()) {
                log::error!(
                    "failed to resume suspended game process {}: {}",
                    child.id(),
                    e
                );
                let _ = child.kill();
                let _ = child.wait();
                return Err(e);
            }
        }
    }

    Ok(child)
}

#[tauri::command]
fn get_steam_overlay_config(app: tauri::AppHandle) -> Result<SteamOverlayConfigDto, String> {
    Ok(read_steam_overlay_config(&app)?.into_dto())
}

#[tauri::command]
fn set_steam_overlay_config(
    app: tauri::AppHandle,
    enabled: bool,
    steam_path: Option<String>,
) -> Result<SteamOverlayConfigDto, String> {
    let normalized_path = steam_path
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    if let Some(path) = normalized_path.as_deref() {
        let candidate = std::path::Path::new(path);
        if !candidate.exists() {
            return Err(format!(
                "Steam path does not exist: {}",
                candidate.to_string_lossy()
            ));
        }
    }

    let cfg = SteamOverlayConfig {
        enabled,
        steam_path: normalized_path,
    };
    write_steam_overlay_config(&app, &cfg)?;
    Ok(cfg.into_dto())
}

#[tauri::command]
fn pick_steam_overlay_path(initial_path: Option<String>) -> Result<Option<String>, String> {
    let mut dialog = rfd::FileDialog::new();
    if let Some(path) = initial_path
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
    {
        let candidate = std::path::PathBuf::from(&path);
        if candidate.exists() {
            dialog = dialog.set_directory(candidate);
        }
    }

    Ok(dialog
        .pick_folder()
        .map(|path| path.to_string_lossy().to_string()))
}

#[tauri::command]
async fn launch_game(
    app: tauri::AppHandle,
    version: u32,
    launch_options: Option<Vec<String>>,
    launch_command_template: Option<String>,
    allow_multiple: Option<bool>,
    state: State<'_, GameState>,
    prepare_state: State<'_, PrepareState>,
) -> Result<u32, String> {
    wait_for_prepare_to_finish(&prepare_state, version, std::time::Duration::from_secs(30))?;
    let (dir, exe_path, exe_dir) = resolve_game_launch_paths(&app, version)?;

    let mut forced_disabled_ids = practice_mode_mod_ids();
    forced_disabled_ids.extend(run_mode_tagged_mod_ids(version, None).await?);

    // Non-practice launch: mode-required disabled mods win over the saved disabled list.
    let _ = apply_effective_mod_states_for_version(&app, version, &forced_disabled_ids, &[]);
    // For HQoL specifically, also ensure `.old` matches disablemod.json on normal runs.
    let _ = sync_hqol_with_disablemod_for_version(&app, version);
    let _ = sync_vlog_with_disablemod_for_version(&app, version);
    let _ = restore_hqol_wesley_dont_store_backup_if_present(&app);
    let _ = ensure_reverb_trigger_fix_cfg(&app, version);
    wait_for_mod_file_renames_to_settle();

    let _launch_guard = state
        .launch_lock
        .lock()
        .map_err(|_| "game launch lock poisoned".to_string())?;
    if !allow_multiple.unwrap_or(false) {
        ensure_game_not_running(&app, &state)?;
    }

    let launch_options = launch_options.unwrap_or_default();
    let launch_command_template_for_state = launch_command_template.clone();
    let child = spawn_game_process(
        &app,
        &dir,
        &exe_path,
        &exe_dir,
        &launch_options,
        launch_command_template.as_deref(),
    )?;
    let pid = child.id();
    let id = state.next_id.fetch_add(1, Ordering::Relaxed) + 1;
    let mut guard = state
        .active
        .lock()
        .map_err(|_| "game state lock poisoned".to_string())?;
    guard.push(ActiveGame {
        id,
        child,
        version,
        mode_label: "HQ".to_string(),
        launch_options,
        launch_command_template: launch_command_template_for_state,
    });
    Ok(pid)
}

#[tauri::command]
async fn launch_game_practice(
    app: tauri::AppHandle,
    version: u32,
    launch_options: Option<Vec<String>>,
    launch_command_template: Option<String>,
    allow_multiple: Option<bool>,
    state: State<'_, GameState>,
    prepare_state: State<'_, PrepareState>,
) -> Result<u32, String> {
    wait_for_prepare_to_finish(&prepare_state, version, std::time::Duration::from_secs(30))?;
    let (dir, exe_path, exe_dir) = resolve_game_launch_paths(&app, version)?;

    // Practice run: install + enable practice mods (compatible with this game version).
    let practice_ids = prepare_practice_mods_for_version(&app, version, None).await?;
    let mut forced_disabled_ids = practice_mode_forced_disabled_ids();
    forced_disabled_ids.extend(run_mode_tagged_mod_ids(version, None).await?);

    // Practice mode state wins over the saved disabled list on launch.
    let _ =
        apply_effective_mod_states_for_version(&app, version, &forced_disabled_ids, &practice_ids);
    if let Ok(plugins) = plugins_dir(&app, version) {
        let _ = sync_practice_locked_mods_for_version(&plugins);
    }
    let _ = restore_hqol_wesley_dont_store_backup_if_present(&app);
    wait_for_mod_file_renames_to_settle();

    let _launch_guard = state
        .launch_lock
        .lock()
        .map_err(|_| "game launch lock poisoned".to_string())?;
    if !allow_multiple.unwrap_or(false) {
        ensure_game_not_running(&app, &state)?;
    }

    let launch_options = launch_options.unwrap_or_default();
    let launch_command_template_for_state = launch_command_template.clone();
    let child = spawn_game_process(
        &app,
        &dir,
        &exe_path,
        &exe_dir,
        &launch_options,
        launch_command_template.as_deref(),
    )?;
    let pid = child.id();
    let id = state.next_id.fetch_add(1, Ordering::Relaxed) + 1;
    let mut guard = state
        .active
        .lock()
        .map_err(|_| "game state lock poisoned".to_string())?;
    guard.push(ActiveGame {
        id,
        child,
        version,
        mode_label: "Practice".to_string(),
        launch_options,
        launch_command_template: launch_command_template_for_state,
    });
    Ok(pid)
}

#[tauri::command]
async fn launch_game_preset(
    app: tauri::AppHandle,
    version: u32,
    preset: String,
    practice: bool,
    launch_options: Option<Vec<String>>,
    launch_command_template: Option<String>,
    allow_multiple: Option<bool>,
    state: State<'_, GameState>,
    prepare_state: State<'_, PrepareState>,
) -> Result<u32, String> {
    wait_for_prepare_to_finish(&prepare_state, version, std::time::Duration::from_secs(30))?;
    // Normalize preset and map to manifest tags.
    let tags = preset_tags_for_name(&preset);

    let practice_ids = if practice {
        // Practice run: install + enable practice mods (compatible with this game version).
        prepare_practice_mods_for_version(&app, version, None).await?
    } else {
        vec![]
    };

    // Install preset-tagged mods additively (no overwrite).
    // For practice runs, install these AFTER practice mods so preset-specific pins can win.
    // (Practice list has its own pinning, e.g. for LethalNetworkAPI.)
    let preset_ids =
        prepare_tagged_mods_for_version(&app, version, &tags, "Preset Mods", None).await?;
    // Ensure preset mods are enabled for this run (even if they overlap practice-disable rules).
    // For practice runs we can apply immediately; for non-practice runs we must apply after
    // `ensure_practice_mods_disabled_for_version` which rewrites disablemod.json.
    if practice {
        let _ = force_enable_mods_for_version(&app, version, &preset_ids);
    }

    let (dir, exe_path, exe_dir) = resolve_game_launch_paths(&app, version)?;

    if !practice {
        // Non-practice run: force-disable practice mods.
        ensure_practice_mods_disabled_for_version(&app, version)?;
        // Re-enable preset mods for this run (Wesley includes LethalNetworkAPI which is otherwise forced off).
        let _ = force_enable_mods_for_version(&app, version, &preset_ids);
    }

    let tagged_disabled_ids = run_mode_tagged_mod_ids(version, None).await?;
    let mut forced_enabled_ids = preset_ids.clone();
    forced_enabled_ids.extend(practice_ids.clone());
    let forced_disabled_ids = if practice {
        let mut ids = practice_mode_forced_disabled_ids();
        ids.extend(tagged_disabled_ids);
        ids
    } else {
        let mut ids = practice_mode_mod_ids();
        ids.extend(tagged_disabled_ids);
        ids
    };

    // Mode-required state must win over the saved disabled list at launch time too.
    let _ = apply_effective_mod_states_for_version(
        &app,
        version,
        &forced_disabled_ids,
        &forced_enabled_ids,
    );
    if practice {
        if let Ok(plugins) = plugins_dir(&app, version) {
            let _ = sync_practice_locked_mods_for_version(&plugins);
        }
    } else {
        // For HQoL specifically, also ensure `.old` matches disablemod.json on normal runs.
        let _ = sync_hqol_with_disablemod_for_version(&app, version);
        let _ = sync_vlog_with_disablemod_for_version(&app, version);
    }
    let _ = ensure_reverb_trigger_fix_cfg(&app, version);
    if is_wesley_base_run(&tags, practice) {
        apply_wesley_hqol_dont_store_override(&app, version)?;
    } else {
        let _ = restore_hqol_wesley_dont_store_backup_if_present(&app);
    }
    if tags.iter().any(|t| t.eq_ignore_ascii_case("wesley")) {
        let lock_moons = !practice && !tags.iter().any(|t| t.eq_ignore_ascii_case("smhq"));
        let _ = ensure_wesley_moonscripts_cfg(&app, version, lock_moons);
    }
    wait_for_mod_file_renames_to_settle();

    let _launch_guard = state
        .launch_lock
        .lock()
        .map_err(|_| "game launch lock poisoned".to_string())?;
    if !allow_multiple.unwrap_or(false) {
        ensure_game_not_running(&app, &state)?;
    }

    let launch_options = launch_options.unwrap_or_default();
    let launch_command_template_for_state = launch_command_template.clone();
    let child = spawn_game_process(
        &app,
        &dir,
        &exe_path,
        &exe_dir,
        &launch_options,
        launch_command_template.as_deref(),
    )?;
    let pid = child.id();
    let id = state.next_id.fetch_add(1, Ordering::Relaxed) + 1;
    let mut guard = state
        .active
        .lock()
        .map_err(|_| "game state lock poisoned".to_string())?;
    let mode_label = if practice {
        format!("{preset} Practice")
    } else {
        preset.clone()
    };
    guard.push(ActiveGame {
        id,
        child,
        version,
        mode_label,
        launch_options,
        launch_command_template: launch_command_template_for_state,
    });
    Ok(pid)
}

async fn prepare_preset_for_version(
    app: &tauri::AppHandle,
    version: u32,
    preset: &str,
    practice: bool,
    cancel: Arc<AtomicBool>,
) -> Result<bool, String> {
    let tags = preset_tags_for_name(preset);

    if cancel.load(Ordering::Relaxed) {
        return Err("Cancelled".to_string());
    }

    // Wesley and Classic Moons presets: ensure companion configs exist during prepare.
    if tags.iter().any(|t| t.eq_ignore_ascii_case("wesley")) {
        let _ = ensure_weather_registry_cfg(app, version);
        let lock_moons = !practice && !tags.iter().any(|t| t.eq_ignore_ascii_case("smhq"));
        let _ = ensure_wesley_moonscripts_cfg(app, version, lock_moons);
    }
    if tags
        .iter()
        .any(|t| t.eq_ignore_ascii_case("wesley") || t.eq_ignore_ascii_case("c.moons"))
    {
        let _ = ensure_reverb_trigger_fix_cfg(app, version);
    }

    let practice_ids = if practice {
        prepare_practice_mods_for_version(app, version, Some(cancel.clone())).await?
    } else {
        vec![]
    };

    if cancel.load(Ordering::Relaxed) {
        return Err("Cancelled".to_string());
    }

    let preset_ids =
        prepare_tagged_mods_for_version(app, version, &tags, "Preset Mods", Some(cancel.clone()))
            .await?;

    let mod_files_detail = describe_mode_file_changes(practice, &tags);
    emit_basic_mod_files_progress(app, version, true, &mod_files_detail);

    if !practice {
        // Selecting a non-practice run should disable practice mods now (not at launch).
        ensure_practice_mods_disabled_for_version(app, version)?;
    }
    // Ensure preset mods are enabled (can override practice-disable overlap).
    let _ = force_enable_mods_for_version(app, version, &preset_ids);

    if cancel.load(Ordering::Relaxed) {
        return Err("Cancelled".to_string());
    }

    let tagged_disabled_ids = run_mode_tagged_mod_ids(version, Some(&cancel)).await?;
    let mut forced_enabled_ids = preset_ids.clone();
    forced_enabled_ids.extend(practice_ids.clone());
    let forced_disabled_ids = if practice {
        let mut ids = practice_mode_forced_disabled_ids();
        ids.extend(tagged_disabled_ids);
        ids
    } else {
        let mut ids = practice_mode_mod_ids();
        ids.extend(tagged_disabled_ids);
        ids
    };

    // Apply the effective state for this mode now. Mode-required changes have priority.
    let _ = apply_effective_mod_states_for_version(
        app,
        version,
        &forced_disabled_ids,
        &forced_enabled_ids,
    );
    if practice {
        if let Ok(plugins) = plugins_dir(app, version) {
            let _ = sync_practice_locked_mods_for_version(&plugins);
        }
    } else {
        let _ = sync_hqol_with_disablemod_for_version(app, version);
        let _ = sync_vlog_with_disablemod_for_version(app, version);
    }
    let _ = ensure_reverb_trigger_fix_cfg(app, version);
    if !is_wesley_base_run(&tags, practice) {
        let _ = restore_hqol_wesley_dont_store_backup_if_present(app);
    }
    wait_for_mod_file_renames_to_settle();
    emit_basic_mod_files_progress(
        app,
        version,
        false,
        "Finished applying mod file changes, including VLog state updates.",
    );

    Ok(true)
}

#[tauri::command]
async fn prepare_preset(
    app: tauri::AppHandle,
    version: u32,
    preset: String,
    practice: bool,
    state: State<'_, PrepareState>,
) -> Result<bool, String> {
    // Only allow one active prepare at a time; new prepares cancel previous ones.
    let cancel = Arc::new(AtomicBool::new(false));
    let id = state.next_id.fetch_add(1, Ordering::Relaxed) + 1;
    {
        let mut guard = state
            .active
            .lock()
            .map_err(|_| "prepare state lock poisoned".to_string())?;
        if let Some(active) = guard.as_ref() {
            active.cancel.store(true, Ordering::Relaxed);
        }
        *guard = Some(ActivePrepare {
            id,
            version,
            cancel: cancel.clone(),
        });
    }

    let res = prepare_preset_for_version(&app, version, &preset, practice, cancel.clone()).await;

    // Clear active prepare if it is still ours.
    {
        let mut guard = state
            .active
            .lock()
            .map_err(|_| "prepare state lock poisoned".to_string())?;
        if guard.as_ref().is_some_and(|a| a.id == id) {
            *guard = None;
        }
    }

    res
}

#[tauri::command]
fn cancel_prepare(version: u32, state: State<'_, PrepareState>) -> Result<bool, String> {
    let mut did = false;
    let guard = state
        .active
        .lock()
        .map_err(|_| "prepare state lock poisoned".to_string())?;
    if let Some(active) = guard.as_ref() {
        if active.version == version {
            active.cancel.store(true, Ordering::Relaxed);
            did = true;
        }
    }
    Ok(did)
}

#[tauri::command]
fn get_game_status(
    app: tauri::AppHandle,
    state: State<'_, GameState>,
) -> Result<GameStatus, String> {
    let mut guard = state
        .active
        .lock()
        .map_err(|_| "game state lock poisoned".to_string())?;
    let mut running_pid = None;
    let mut any_finished = false;
    let mut kept = Vec::with_capacity(guard.len());
    for mut active in guard.drain(..) {
        match active.child.try_wait().map_err(|e| e.to_string())? {
            None => {
                running_pid.get_or_insert_with(|| active.child.id());
                kept.push(active);
            }
            Some(_) => {
                any_finished = true;
                if let Some(pid) = linux_lingering_game_pid(&app, active.version) {
                    running_pid.get_or_insert(pid);
                    kept.push(active);
                }
            }
        }
    }
    *guard = kept;

    if let Some(pid) = running_pid {
        return Ok(GameStatus {
            running: true,
            pid: Some(pid),
        });
    }

    if any_finished {
        if let Err(e) = restore_hqol_wesley_dont_store_backup_if_present(&app) {
            log::warn!("Failed to restore HQoL Wesley dont-store backup after exit: {e}");
        }
    }

    Ok(GameStatus {
        running: false,
        pid: None,
    })
}

#[tauri::command]
fn list_running_games(
    app: tauri::AppHandle,
    state: State<'_, GameState>,
) -> Result<Vec<RunningGameDto>, String> {
    let mut guard = state
        .active
        .lock()
        .map_err(|_| "game state lock poisoned".to_string())?;
    let any_finished = cleanup_active_games(&app, &mut guard)?;
    if any_finished {
        if let Err(e) = restore_hqol_wesley_dont_store_backup_if_present(&app) {
            log::warn!("Failed to restore HQoL Wesley dont-store backup after exit: {e}");
        }
    }

    Ok(guard
        .iter()
        .enumerate()
        .map(|(idx, active)| active_game_dto(idx + 1, active))
        .collect())
}

#[tauri::command]
fn stop_game_instance(
    app: tauri::AppHandle,
    id: u64,
    state: State<'_, GameState>,
) -> Result<bool, String> {
    let _launch_guard = state
        .launch_lock
        .lock()
        .map_err(|_| "game launch lock poisoned".to_string())?;
    let mut guard = state
        .active
        .lock()
        .map_err(|_| "game state lock poisoned".to_string())?;
    let Some(index) = guard.iter().position(|active| active.id == id) else {
        return Ok(false);
    };

    let mut active = guard.remove(index);
    terminate_child_process_tree(&mut active.child);
    let _ = active.child.wait();

    if guard.is_empty() {
        terminate_linux_game_processes_for_version(&app, active.version);
        if let Err(e) = restore_hqol_wesley_dont_store_backup_if_present(&app) {
            log::warn!("Failed to restore HQoL Wesley dont-store backup after stopping instance: {e}");
        }
    }

    Ok(true)
}

#[tauri::command]
fn stop_game(app: tauri::AppHandle, state: State<'_, GameState>) -> Result<bool, String> {
    let _launch_guard = state
        .launch_lock
        .lock()
        .map_err(|_| "game launch lock poisoned".to_string())?;
    let mut guard = state
        .active
        .lock()
        .map_err(|_| "game state lock poisoned".to_string())?;
    if !guard.is_empty() {
        let mut active_games = std::mem::take(&mut *guard);
        let mut versions = Vec::new();
        for active in &mut active_games {
            versions.push(active.version);
            terminate_child_process_tree(&mut active.child);
        }
        for mut active in active_games {
            let _ = active.child.wait();
        }
        versions.sort_unstable();
        versions.dedup();
        for version in versions {
            terminate_linux_game_processes_for_version(&app, version);
        }
        if let Err(e) = restore_hqol_wesley_dont_store_backup_if_present(&app) {
            log::warn!("Failed to restore HQoL Wesley dont-store backup after stop: {e}");
        }
        Ok(true)
    } else {
        if let Err(e) = restore_hqol_wesley_dont_store_backup_if_present(&app) {
            log::warn!("Failed to restore HQoL Wesley dont-store backup without active child: {e}");
        }
        Ok(false)
    }
}

#[tauri::command]
fn get_disabled_mods(app: tauri::AppHandle) -> Result<Vec<DisabledMod>, String> {
    Ok(read_disablemod(&app)?.mods)
}

#[tauri::command]
fn apply_disabled_mods(app: tauri::AppHandle, version: u32) -> Result<bool, String> {
    apply_disabled_mods_for_version(&app, version)?;
    Ok(true)
}

#[tauri::command]
async fn set_mod_enabled(
    app: tauri::AppHandle,
    version: u32,
    dev: String,
    name: String,
    enabled: bool,
) -> Result<bool, String> {
    let mut list = read_disablemod(&app)?;

    // Use normalized ids in the file.
    let id = normalize_mod_id(&dev, &name);
    list.mods.retain(|m| m != &id);
    if !enabled {
        list.mods.push(id);
        list.mods
            .sort_by(|a, b| a.dev.cmp(&b.dev).then(a.name.cmp(&b.name)));
        list.mods.dedup();
    }
    write_disablemod(&app, &list)?;

    let plugins = plugins_dir(&app, version)?;
    let patchers = patchers_dir(&app, version)?;

    if enabled
        && mod_dir_for(&plugins, &dev, &name).is_none()
        && mod_dir_for(&patchers, &dev, &name).is_none()
    {
        let game_root = version_dir(&app, version)?;
        if !game_root.exists() {
            return Err(format!(
                "version folder not found: {}",
                game_root.to_string_lossy()
            ));
        }

        let Some(spec) = find_mod_entry_for_install(version, &dev, &name).await? else {
            return Err(format!("mod not found in available configs: {dev}-{name}"));
        };

        let cfg = ModsConfig { mods: vec![spec] };
        progress::emit_progress(
            &app,
            TaskProgressPayload {
                version,
                steps_total: 1,
                step: 1,
                step_name: "Enable Mod".to_string(),
                step_progress: 0.0,
                overall_percent: 0.0,
                detail: Some(format!("Installing {dev}-{name}...")),
                downloaded_bytes: None,
                total_bytes: None,
                extracted_files: Some(0),
                total_files: Some(1),
            },
        );
        mods::install_mods_with_progress(
            &app,
            &game_root,
            version,
            &cfg,
            &[],
            None,
            |done, total, progress_info| {
                let step_progress = if total == 0 {
                    1.0
                } else {
                    (done as f64 / total as f64).clamp(0.0, 1.0)
                };
                progress::emit_progress(
                    &app,
                    TaskProgressPayload {
                        version,
                        steps_total: 1,
                        step: 1,
                        step_name: "Enable Mod".to_string(),
                        step_progress,
                        overall_percent: overall_from_step(1, step_progress, 1),
                        detail: progress_info
                            .detail
                            .or_else(|| Some(format!("Installing {dev}-{name}..."))),
                        downloaded_bytes: progress_info.downloaded_bytes,
                        total_bytes: progress_info.total_bytes,
                        extracted_files: progress_info.extracted_files.or(Some(done)),
                        total_files: progress_info.total_files.or(Some(total)),
                    },
                );
            },
        )
        .await?;
    }

    // Apply to current version immediately.
    if let Some(dir) = mod_dir_for(&plugins, &dev, &name) {
        let _ = set_mod_files_old_suffix(&dir, enabled);
    }
    if let Some(dir) = mod_dir_for(&patchers, &dev, &name) {
        let _ = set_mod_files_old_suffix(&dir, enabled);
    }
    Ok(true)
}

#[tauri::command]
async fn list_installed_mod_versions(
    app: tauri::AppHandle,
    version: u32,
) -> Result<Vec<InstalledModVersion>, String> {
    match purge_capped_incompatible_installed_mods(&app, version, None).await {
        Ok(purged) => {
            if purged > 0 {
                log::info!(
                    "Purged {purged} incompatible capped mods before listing installed mods for v{version}"
                );
            }
        }
        Err(e) => {
            log::warn!(
                "Failed to purge incompatible capped mods before listing installed mods for v{version}: {e}"
            );
        }
    }

    let plugins = plugins_dir(&app, version)?;
    if !plugins.exists() {
        return Ok(vec![]);
    }

    let mut out: Vec<InstalledModVersion> = vec![];
    let Ok(rd) = std::fs::read_dir(&plugins) else {
        return Ok(out);
    };

    for e in rd.flatten() {
        let path = e.path();
        if !path.is_dir() {
            continue;
        }

        // Plugin folder naming is deterministic: "{dev}-{name}"
        let folder = e.file_name().to_string_lossy().to_string();
        let Some((dev, name)) = folder.split_once('-') else {
            continue;
        };

        // When disabled, we suffix every file with `.old`, including manifest.json.
        let manifest_path = path.join("manifest.json");
        let manifest_old_path = path.join("manifest.json.old");

        let manifest = if manifest_path.exists() {
            read_manifest(&manifest_path)
        } else if manifest_old_path.exists() {
            read_manifest(&manifest_old_path)
        } else {
            continue;
        };

        match manifest {
            Ok(m) => {
                out.push(InstalledModVersion {
                    dev: dev.to_string(),
                    name: name.to_string(),
                    version: m.version_number,
                    icon_path: find_mod_icon_path(&path),
                    description: if m.description.trim().is_empty() {
                        None
                    } else {
                        Some(m.description)
                    },
                });
            }
            Err(err) => {
                log::warn!(
                    "Failed to read plugin manifest for {} (v{}): {}",
                    folder,
                    version,
                    err
                );
            }
        }
    }

    out.sort_by(|a, b| a.dev.cmp(&b.dev).then(a.name.cmp(&b.name)));
    Ok(out)
}

#[tauri::command]
async fn get_manifest() -> Result<ManifestDto, String> {
    let client = reqwest::Client::new();
    let (version, cfg, chain_config, manifests, preset_tag_constraints) =
        mod_config::ModsConfig::fetch_manifest(&client).await?;
    Ok(ManifestDto {
        version,
        chain_config,
        mods: cfg.mods,
        manifests,
        preset_tag_constraints,
    })
}

#[tauri::command]
fn get_practice_mod_list() -> Vec<mod_config::ModEntry> {
    variable::get_practice_mod_list()
}

#[tauri::command]
fn list_installed_versions(app: tauri::AppHandle) -> Result<Vec<u32>, String> {
    let base = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("failed to resolve app data dir: {e}"))?
        .join("versions");

    let mut out: Vec<u32> = vec![];
    let Ok(rd) = std::fs::read_dir(&base) else {
        return Ok(out);
    };

    for e in rd {
        let Ok(e) = e else { continue };
        let path = e.path();
        if !path.is_dir() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
            continue;
        };
        let Some(num) = name.strip_prefix('v') else {
            continue;
        };
        if let Ok(v) = num.parse::<u32>() {
            if !is_complete_version_dir(&app, v, &path) {
                continue;
            }
            out.push(v);
        }
    }
    out.sort_unstable();
    Ok(out)
}

#[tauri::command]
fn list_config_files(app: tauri::AppHandle) -> Result<Vec<String>, String> {
    let base = shared_config_dir(&app)?;
    if !base.exists() {
        return Ok(vec![]);
    }

    let mut out: Vec<String> = vec![];
    let base_canon = std::fs::canonicalize(&base).map_err(|e| e.to_string())?;

    let mut stack: Vec<std::path::PathBuf> = vec![base.clone()];
    while let Some(dir) = stack.pop() {
        for e in std::fs::read_dir(&dir).map_err(|e| e.to_string())? {
            let e = e.map_err(|e| e.to_string())?;
            let path = e.path();
            let ty = e.file_type().map_err(|e| e.to_string())?;
            if ty.is_dir() {
                stack.push(path);
                continue;
            }
            if !ty.is_file() {
                continue;
            }
            let canon = std::fs::canonicalize(&path).map_err(|e| e.to_string())?;
            if !canon.starts_with(&base_canon) {
                continue;
            }
            let rel = canon
                .strip_prefix(&base_canon)
                .unwrap_or(&canon)
                .to_string_lossy()
                .replace('\\', "/");
            out.push(rel.trim_start_matches('/').to_string());
        }
    }

    out.sort();
    Ok(out)
}

fn list_config_files_for_version_impl(
    app: &tauri::AppHandle,
    version: u32,
) -> Result<Vec<String>, String> {
    let base = version_config_dir(app, version)?;
    if !base.exists() {
        return Ok(vec![]);
    }

    let mut out: Vec<String> = vec![];
    let base_canon = std::fs::canonicalize(&base).map_err(|e| e.to_string())?;

    let mut stack: Vec<std::path::PathBuf> = vec![base.clone()];
    while let Some(dir) = stack.pop() {
        for e in std::fs::read_dir(&dir).map_err(|e| e.to_string())? {
            let e = e.map_err(|e| e.to_string())?;
            let path = e.path();
            let ty = e.file_type().map_err(|e| e.to_string())?;
            if ty.is_dir() {
                stack.push(path);
                continue;
            }
            if !ty.is_file() {
                continue;
            }
            let canon = std::fs::canonicalize(&path).map_err(|e| e.to_string())?;
            if !canon.starts_with(&base_canon) {
                continue;
            }
            let rel = canon
                .strip_prefix(&base_canon)
                .unwrap_or(&canon)
                .to_string_lossy()
                .replace('\\', "/");
            out.push(rel.trim_start_matches('/').to_string());
        }
    }

    out.sort();
    Ok(out)
}

#[tauri::command]
fn get_config_link_state(app: tauri::AppHandle) -> Result<installer::ConfigLinkState, String> {
    installer::get_config_link_state(&app)
}

#[tauri::command]
fn get_config_link_state_for_version(
    app: tauri::AppHandle,
    version: u32,
) -> Result<installer::VersionConfigLinkState, String> {
    installer::get_config_link_state_for_version(&app, version)
}

#[tauri::command]
fn link_config(app: tauri::AppHandle) -> Result<installer::ConfigLinkState, String> {
    let _ = installer::link_config_for_all_versions(&app)?;
    installer::get_config_link_state(&app)
}

#[tauri::command]
fn link_config_for_version(
    app: tauri::AppHandle,
    version: u32,
) -> Result<installer::VersionConfigLinkState, String> {
    installer::link_config_for_version(&app, version)
}

#[tauri::command]
fn unlink_config(app: tauri::AppHandle) -> Result<installer::ConfigLinkState, String> {
    let _ = installer::unlink_config_for_all_versions(&app)?;
    installer::get_config_link_state(&app)
}

#[tauri::command]
fn unlink_config_for_version(
    app: tauri::AppHandle,
    version: u32,
) -> Result<installer::VersionConfigLinkState, String> {
    installer::unlink_config_for_version(&app, version)
}

#[tauri::command]
fn list_config_files_for_mod(
    app: tauri::AppHandle,
    dev: String,
    name: String,
) -> Result<Vec<String>, String> {
    let all = list_config_files(app)?;
    let d = dev.to_lowercase();
    let n = name.to_lowercase();
    Ok(all
        .into_iter()
        .filter(|p| {
            let lp = p.to_lowercase();
            lp.contains(&d) || lp.contains(&n)
        })
        .collect())
}

#[tauri::command]
fn list_config_files_for_mod_for_version(
    app: tauri::AppHandle,
    version: u32,
    dev: String,
    name: String,
) -> Result<Vec<String>, String> {
    let all = list_config_files_for_version_impl(&app, version)?;
    let d = dev.to_lowercase();
    let n = name.to_lowercase();
    Ok(all
        .into_iter()
        .filter(|p| {
            let lp = p.to_lowercase();
            lp.contains(&d) || lp.contains(&n)
        })
        .collect())
}

#[tauri::command]
fn read_config_file(app: tauri::AppHandle, rel_path: String) -> Result<String, String> {
    let base = shared_config_dir(&app)?;
    let rel = std::path::Path::new(&rel_path);
    if !is_safe_rel_path(rel) {
        return Err("invalid path".to_string());
    }
    let path = base.join(rel);
    std::fs::read_to_string(&path).map_err(|e| e.to_string())
}

#[tauri::command]
fn open_config_file_for_version(
    app: tauri::AppHandle,
    version: u32,
    rel_path: String,
) -> Result<bool, String> {
    let base = version_config_dir(&app, version)?;
    let rel = std::path::Path::new(&rel_path);
    if !is_safe_rel_path(rel) {
        return Err("invalid path".to_string());
    }
    let path = base.join(rel);
    if !path.exists() {
        return Err(format!("config file not found: {}", rel_path));
    }
    open_file_path(&path)?;
    Ok(true)
}

#[tauri::command]
fn read_bepinex_cfg(
    app: tauri::AppHandle,
    rel_path: String,
) -> Result<bepinex_cfg::FileData, String> {
    let base = shared_config_dir(&app)?;
    let rel = std::path::Path::new(&rel_path);
    if !is_safe_rel_path(rel) {
        return Err("invalid path".to_string());
    }
    let path = base.join(rel);
    let text = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    bepinex_cfg::parse(&text)
}

#[tauri::command]
fn read_bepinex_cfg_for_version(
    app: tauri::AppHandle,
    version: u32,
    rel_path: String,
) -> Result<bepinex_cfg::FileData, String> {
    let base = version_config_dir(&app, version)?;
    let rel = std::path::Path::new(&rel_path);
    if !is_safe_rel_path(rel) {
        return Err("invalid path".to_string());
    }
    let path = base.join(rel);
    let text = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    bepinex_cfg::parse(&text)
}

#[derive(Debug, Clone, Deserialize)]
struct SetBepInExEntryArgs {
    rel_path: String,
    section: String,
    entry: String,
    value: bepinex_cfg::Value,
}

#[tauri::command]
fn set_bepinex_cfg_entry(app: tauri::AppHandle, args: SetBepInExEntryArgs) -> Result<bool, String> {
    let base = shared_config_dir(&app)?;
    let rel = std::path::Path::new(&args.rel_path);

    log::info!("set_bepinex_cfg_entry: {:?}", args);

    if !is_safe_rel_path(rel) {
        return Err("invalid path".to_string());
    }
    let path = base.join(rel);

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    // If the cfg doesn't exist yet, start from an empty file and create the
    // requested section/entry.
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(e.to_string()),
    };
    let mut file = bepinex_cfg::parse(&text)?;

    let section = match file.sections.iter_mut().find(|s| s.name == args.section) {
        Some(s) => s,
        None => {
            file.sections.push(bepinex_cfg::Section {
                name: args.section.clone(),
                entries: vec![],
            });
            file.sections
                .iter_mut()
                .find(|s| s.name == args.section)
                .ok_or("failed to create section".to_string())?
        }
    };

    match section.entries.iter_mut().find(|e| e.name == args.entry) {
        Some(e) => {
            e.value = args.value;
        }
        None => {
            section.entries.push(bepinex_cfg::Entry {
                name: args.entry,
                description: None,
                default: None,
                value: args.value,
            });
        }
    }

    let new_text = bepinex_cfg::write(&file)?;
    std::fs::write(&path, new_text).map_err(|e| e.to_string())?;
    Ok(true)
}

#[tauri::command]
fn set_bepinex_cfg_entry_for_version(
    app: tauri::AppHandle,
    version: u32,
    args: SetBepInExEntryArgs,
) -> Result<bool, String> {
    let base = version_config_dir(&app, version)?;
    let rel = std::path::Path::new(&args.rel_path);

    log::info!("set_bepinex_cfg_entry_for_version(v{version}): {:?}", args);

    if !is_safe_rel_path(rel) {
        return Err("invalid path".to_string());
    }
    let path = base.join(rel);

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    // If the cfg doesn't exist yet, start from an empty file and create the
    // requested section/entry.
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(e.to_string()),
    };
    let mut file = bepinex_cfg::parse(&text)?;

    let section = match file.sections.iter_mut().find(|s| s.name == args.section) {
        Some(s) => s,
        None => {
            file.sections.push(bepinex_cfg::Section {
                name: args.section.clone(),
                entries: vec![],
            });
            file.sections
                .iter_mut()
                .find(|s| s.name == args.section)
                .ok_or("failed to create section".to_string())?
        }
    };

    match section.entries.iter_mut().find(|e| e.name == args.entry) {
        Some(e) => {
            e.value = args.value;
        }
        None => {
            section.entries.push(bepinex_cfg::Entry {
                name: args.entry,
                description: None,
                default: None,
                value: args.value,
            });
        }
    }

    let new_text = bepinex_cfg::write(&file)?;
    std::fs::write(&path, new_text).map_err(|e| e.to_string())?;
    Ok(true)
}

#[derive(Debug, Clone, Deserialize)]
struct WriteConfigArgs {
    rel_path: String,
    contents: String,
}

#[tauri::command]
fn write_config_file(app: tauri::AppHandle, args: WriteConfigArgs) -> Result<bool, String> {
    let base = shared_config_dir(&app)?;
    let rel = std::path::Path::new(&args.rel_path);
    if !is_safe_rel_path(rel) {
        return Err("invalid path".to_string());
    }
    let path = base.join(rel);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(&path, args.contents).map_err(|e| e.to_string())?;
    Ok(true)
}

// =========================
// 🔹 AUTO-UPDATE COMMANDS
// =========================

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    name: String,
    published_at: String,
    body: Option<String>,
    assets: Vec<GitHubAsset>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GitHubAsset {
    name: String,
    browser_download_url: String,
    size: u64,
}

#[derive(Debug, Clone, Serialize)]
struct UpdateInfo {
    available: bool,
    current_version: String,
    version: Option<String>,
    date: Option<String>,
    body: Option<String>,
}

#[tauri::command]
async fn check_app_update(app: tauri::AppHandle) -> Result<UpdateInfo, String> {
    use semver::Version;

    let current_version_str = app.package_info().version.to_string();

    // GitHub Releases API에서 최신 릴리즈 가져오기
    let client = reqwest::Client::new();
    let github_release_url = "https://api.github.com/repos/p-asta/hq-launcher/releases/latest";

    let github_release: GitHubRelease = client
        .get(github_release_url)
        .header("User-Agent", "hq-launcher-updater")
        .send()
        .await
        .map_err(|e| format!("Failed to fetch GitHub release: {e}"))?
        .json()
        .await
        .map_err(|e| format!("Failed to parse GitHub release: {e}"))?;

    // 버전 비교 (tag_name에서 v 제거)
    let latest_version_str = github_release.tag_name.trim_start_matches('v').to_string();
    let current_version = Version::parse(&current_version_str)
        .map_err(|e| format!("Failed to parse current version: {e}"))?;
    let latest_version = Version::parse(&latest_version_str)
        .map_err(|e| format!("Failed to parse latest version: {e}"))?;

    let available = latest_version > current_version;

    Ok(UpdateInfo {
        available,
        current_version: current_version_str,
        version: if available {
            Some(latest_version_str.clone())
        } else {
            None
        },
        date: Some(github_release.published_at),
        body: github_release.body,
    })
}

#[derive(Debug, Clone, Serialize)]
struct UpdateProgress {
    downloaded: u64,
    total: u64,
    percent: f64,
}

#[tauri::command]
async fn download_app_update(app: tauri::AppHandle) -> Result<bool, String> {
    use tauri_plugin_updater::UpdaterExt;

    // Tauri updater 사용 (엔드포인트는 tauri.conf.json에서 설정, GitHub Releases latest.json)
    let updater = app
        .updater_builder()
        .build()
        .map_err(|e| format!("Failed to initialize updater: {e}"))?;

    let update = updater
        .check()
        .await
        .map_err(|e| format!("Failed to check for updates: {e}"))?
        .ok_or("No update available")?;

    // Download the update with progress tracking
    // on_chunk: FnMut(chunk_length: usize, content_length: Option<u64>)
    // on_download_finish: FnOnce()
    let mut downloaded = 0u64;
    update
        .download(
            |chunk_length, content_length| {
                downloaded += chunk_length as u64;
                if let Some(total) = content_length {
                    let percent = (downloaded as f64 / total as f64) * 100.0;
                    log::debug!(
                        "Update download progress: {:.2}% ({}/{} bytes)",
                        percent,
                        downloaded,
                        total
                    );
                } else {
                    log::debug!("Update download progress: {} bytes downloaded", downloaded);
                }
            },
            || {
                log::info!("Update download finished");
            },
        )
        .await
        .map_err(|e| format!("Failed to download update: {e}"))?;

    Ok(true)
}

#[tauri::command]
#[cfg(not(target_os = "macos"))]
async fn get_global_shortcut(_app: tauri::AppHandle, shortcut: String) -> Result<String, String> {
    let shortcut = shortcut.replace("CommandOrControl", "Ctrl");
    Ok(shortcut)
}

#[tauri::command]
#[cfg(target_os = "macos")]
async fn get_global_shortcut(_app: tauri::AppHandle, shortcut: String) -> Result<String, String> {
    let shortcut = shortcut.replace("CommandOrControl", "Cmd");
    Ok(shortcut)
}

#[tauri::command]
async fn install_app_update(app: tauri::AppHandle) -> Result<bool, String> {
    use tauri_plugin_updater::UpdaterExt;

    // Tauri updater 사용 (엔드포인트는 tauri.conf.json에서 설정, GitHub Releases latest.json)
    let updater = app
        .updater_builder()
        .build()
        .map_err(|e| format!("Failed to initialize updater: {e}"))?;

    let update = updater
        .check()
        .await
        .map_err(|e| format!("Failed to check for updates: {e}"))?
        .ok_or("No update available")?;

    // Download and install the update
    // on_chunk: FnMut(chunk_length: usize, content_length: Option<u64>)
    // on_download_finish: FnOnce()
    let mut downloaded = 0u64;
    update
        .download_and_install(
            |chunk_length, content_length| {
                downloaded += chunk_length as u64;
                if let Some(total) = content_length {
                    let percent = (downloaded as f64 / total as f64) * 100.0;
                    log::debug!(
                        "Update download progress: {:.2}% ({}/{} bytes)",
                        percent,
                        downloaded,
                        total
                    );
                } else {
                    log::debug!("Update download progress: {} bytes downloaded", downloaded);
                }
            },
            || {
                log::info!("Update download finished, installing...");
            },
        )
        .await
        .map_err(|e| format!("Failed to download and install update: {e}"))?;

    log::info!("Update installed successfully; restarting launcher");
    app.restart();
}

#[tauri::command]
fn get_app_version(app: tauri::AppHandle) -> Result<String, String> {
    Ok(app.package_info().version.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(GameState::default())
        .manage(DownloadState::default())
        .manage(PrepareState::default())
        .manage(discord_presence::DiscordPresenceState::default())
        .manage(downloader::DepotLoginState::default())
        .setup(|app| {
            // File logging (AppDataDir/logs/hq-launcher.log)
            logger::init(&app.handle()).map_err(|e| tauri::Error::Setup(e.into()))?;

            // Startup housekeeping (best-effort, won't block UI):
            // - Purge mods that remote manifest marks as enabled=false (and their configs)
            // - Ensure default config is downloaded if shared config dir is empty
            // - Warm the Thunderstore package cache for later update checks
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) =
                    installer::purge_remote_disabled_mods_on_startup(app_handle.clone()).await
                {
                    log::warn!("Failed to purge remote-disabled mods on startup: {e}");
                }
                if let Err(e) = installer::ensure_default_config(app_handle.clone()).await {
                    log::warn!("Failed to ensure default config on startup: {e}");
                }
                if let Err(e) = installer::ensure_pack_specific_configs_on_startup(&app_handle) {
                    log::warn!("Failed to ensure pack-specific configs on startup: {e}");
                }
                match thunderstore_cache_path(&app_handle) {
                    Ok(cache_path) => {
                        let client = reqwest::Client::new();
                        if let Err(e) =
                            thunderstore::fetch_community_packages(&client, &cache_path).await
                        {
                            log::warn!("Failed to warm Thunderstore cache on startup: {e}");
                        }
                    }
                    Err(e) => {
                        log::warn!("Failed to resolve Thunderstore cache path on startup: {e}");
                    }
                }
                #[cfg(target_os = "linux")]
                {
                    if let Err(e) = installer::install_proton_ge_impl(&app_handle).await {
                        log::warn!("Failed to install Proton-GE on startup: {e}");
                    }
                }
            });

            Ok(())
        })
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            download,
            cancel_download,
            prepare_preset,
            cancel_prepare,
            sync_latest_install_from_manifest,
            check_latest_install_manifest_update,
            check_mod_updates,
            apply_mod_updates,
            launch_game,
            launch_game_practice,
            launch_game_preset,
            get_game_status,
            list_running_games,
            stop_game,
            stop_game_instance,
            get_disabled_mods,
            apply_disabled_mods,
            set_mod_enabled,
            list_installed_mod_versions,
            get_manifest,
            get_practice_mod_list,
            list_installed_versions,
            list_config_files,
            get_config_link_state,
            link_config,
            unlink_config,
            get_config_link_state_for_version,
            link_config_for_version,
            unlink_config_for_version,
            list_config_files_for_mod_for_version,
            list_config_files_for_mod,
            read_config_file,
            open_config_file_for_version,
            read_bepinex_cfg,
            read_bepinex_cfg_for_version,
            set_bepinex_cfg_entry,
            set_bepinex_cfg_entry_for_version,
            write_config_file,
            get_steam_overlay_config,
            set_steam_overlay_config,
            pick_steam_overlay_path,
            downloader::depot_login,
            downloader::depot_login_start,
            downloader::depot_login_submit_code,
            downloader::depot_get_login_state,
            downloader::depot_logout,
            downloader::depot_download,
            downloader::depot_download_files,
            check_app_update,
            download_app_update,
            install_app_update,
            get_app_version,
            installer::install_proton_ge,
            installer::get_current_proton_dir,
            delete_installed_version,
            open_version_folder,
            open_downloader_folder,
            open_mod_folder,
            get_global_shortcut,
            discord_presence::set_discord_presence,
            discord_presence::clear_discord_presence
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

mod bepinex_cfg;
mod discord_presence;
mod downloader;
mod google_oauth;
mod installer;
mod lcstats_autosheet;
mod logger;
mod mod_config;
mod mods;
mod progress;
mod release_channel;
mod storage;
mod thunderstore;
mod variable;
mod zip_utils;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::ffi::OsString;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use std::sync::Mutex;
use tauri::{Emitter, Manager, State};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

#[cfg(target_os = "windows")]
use std::ffi::CString;
#[cfg(target_os = "windows")]
use std::os::windows::ffi::OsStringExt;
#[cfg(target_os = "windows")]
use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, HWND, LPARAM, RECT};
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
    CreateRemoteThread, OpenProcess, OpenThread, QueryFullProcessImageNameW, ResumeThread,
    WaitForSingleObject, INFINITE, PROCESS_CREATE_THREAD, PROCESS_QUERY_INFORMATION,
    PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_VM_OPERATION, PROCESS_VM_READ, PROCESS_VM_WRITE,
    THREAD_SUSPEND_RESUME,
};
#[cfg(target_os = "windows")]
use windows_sys::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetForegroundWindow, GetWindowLongPtrW, GetWindowRect, GetWindowTextLengthW,
    GetWindowTextW, GetWindowThreadProcessId, IsWindowVisible, SetWindowLongPtrW, SetWindowPos,
    GWL_EXSTYLE, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, WS_EX_APPWINDOW,
    WS_EX_TOOLWINDOW,
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
const DISABLEMOD_FILE_VERSION: u32 = 5;
const GAME_OVERLAY_WINDOW_LABEL: &str = "game-overlay";

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

fn is_eclipsed_hq_preset(preset: &str) -> bool {
    let p = preset.trim().to_lowercase();
    matches!(p.as_str(), "eclipsed" | "eclipsed_hq" | "eclipsed-hq")
}

fn is_eclipsed_hq_optional_mod(dev: &str, name: &str) -> bool {
    dev.eq_ignore_ascii_case("SlushyRH") && name.eq_ignore_ascii_case("FreeeeeeMoooooons")
}

fn allow_eclipsed_hq_optional_mods(preset: &str, forced_disabled_ids: &mut Vec<(String, String)>) {
    if !is_eclipsed_hq_preset(preset) {
        return;
    }

    forced_disabled_ids.retain(|(dev, name)| !is_eclipsed_hq_optional_mod(dev, name));
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
    Ok(storage::versions_dir(app)?.join(format!("v{version}")))
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

fn find_mod_icon_src(mod_dir: &std::path::Path) -> Option<String> {
    for file_name in ["icon.png", "icon.png.old"] {
        let path = mod_dir.join(file_name);
        if path.is_file() {
            let bytes = std::fs::read(path).ok()?;
            use base64::Engine as _;
            let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
            return Some(format!("data:image/png;base64,{encoded}"));
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
struct GameOverlayConfig {
    general: GameOverlayGeneralConfig,
    crosshair: CrosshairConfig,
    widgets: HashMap<String, OverlayWidgetPosition>,
    module_settings: HashMap<String, serde_json::Value>,
    end_summary: EndSummaryConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
struct GameOverlayGeneralConfig {
    enabled: bool,
    use_stream_overlays_api: bool,
    overlay_key: String,
    end_summary_duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
struct CrosshairConfig {
    enabled: bool,
    style: String,
    color: String,
    size: f64,
    thickness: f64,
    gap: f64,
    opacity: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
struct OverlayWidgetPosition {
    x: f64,
    y: f64,
    snap: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
struct EndSummaryConfig {
    position: String,
    duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GameOverlayEndSummaryPayload {
    title: String,
    lines: Vec<String>,
    duration_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
struct GameOverlayModuleDto {
    id: String,
    file_name: String,
    source: String,
}

const OVERLAY_MODULE_REFERENCE: &str =
    "/// <reference path=\"./hq-overlay-module.d.ts\" />\n// @ts-check\n\n";

const DEFAULT_OVERLAY_MODULES: &[(&str, &str)] = &[
    (
        "crosshair.js",
        r##"/// <reference path="./hq-overlay-module.d.ts" />
// @ts-check

setName("Crosshair");
setDescription("Locked center overlay. Size and color are controlled here, while layout dragging stays disabled.");
setLocked(true);
setDefaultPosition({ x: 50, y: 50 });
setCss(`
  .overlay-module-crosshair {
    transform: translate(-50%, -50%);
  }
`);
register("settings", [
  Setting.toggle("enabled", "Enabled", false),
  Setting.key("toggleKey", "Toggle Key", ""),
  Setting.selectMenu("style", "Style", [
    { label: "Plus", value: "plus" },
    { label: "Dot", value: "dot" },
    { label: "Circle", value: "circle" },
    { label: "X", value: "x" },
    { label: "Square", value: "square" }
  ], "plus"),
  Setting.color("color", "Color", "#ffffff"),
  Setting.range("size", "Size", 4, 96, 1, 24),
  Setting.range("thickness", "Thickness", 1, 12, 1, 2),
  Setting.range("gap", "Gap", 0, 32, 1, 5),
  Setting.range("opacity", "Opacity", 0.05, 1, 0.05, 0.9)
]);
let runtimeEnabled = null;
let lastSettingEnabled = null;
function crosshairEnabled(settings, api) {
  const settingEnabled = settings.enabled !== false;
  if (runtimeEnabled == null || lastSettingEnabled !== settingEnabled) {
    runtimeEnabled = settingEnabled;
    lastSettingEnabled = settingEnabled;
  }
  if (settings.toggleKey && api.input.consumePress(settings.toggleKey)) {
    runtimeEnabled = !runtimeEnabled;
  }
  return runtimeEnabled;
}
register("tick", ({ settings, api }) => {
  crosshairEnabled(settings, api);
});
register("visible", ({ settings, api }) => crosshairEnabled(settings, api));
register("renderOverlay", ({ settings }) => {
  const size = Number(settings.size ?? 24);
  const thickness = Number(settings.thickness ?? 2);
  const gap = Number(settings.gap ?? 5);
  const arm = Math.max(1, (size - gap) / 2);
  const color = settings.color ?? "#ffffff";
  const opacity = Number(settings.opacity ?? 0.9);
  const line = `position:absolute;background:${color};opacity:${opacity};box-shadow:0 0 8px rgba(0,0,0,.45);`;
  const center = size / 2 - thickness / 2;
  if (settings.style === "dot") {
    return `<div style="width:${size}px;height:${size}px;position:relative"><div style="${line}left:${center}px;top:${center}px;width:${thickness}px;height:${thickness}px;border-radius:999px"></div></div>`;
  }
  if (settings.style === "circle") {
    return `<div style="width:${size}px;height:${size}px;border:${thickness}px solid ${color};opacity:${opacity};border-radius:999px;box-shadow:0 0 8px rgba(0,0,0,.45)"></div>`;
  }
  if (settings.style === "x") {
    const xLine = `position:absolute;left:${gap / 2}px;top:${center}px;width:${Math.max(1, size - gap)}px;height:${thickness}px;background:${color};opacity:${opacity};box-shadow:0 0 8px rgba(0,0,0,.45);transform-origin:center;`;
    return `<div style="position:relative;width:${size}px;height:${size}px"><div style="${xLine}transform:rotate(45deg)"></div><div style="${xLine}transform:rotate(-45deg)"></div></div>`;
  }
  if (settings.style === "square") {
    return `<div style="width:${size}px;height:${size}px;border:${thickness}px solid ${color};opacity:${opacity};box-shadow:0 0 8px rgba(0,0,0,.45)"></div>`;
  }
  return `<div style="position:relative;width:${size}px;height:${size}px">
    <div style="${line}left:0;top:${size / 2 - thickness / 2}px;width:${arm}px;height:${thickness}px"></div>
    <div style="${line}right:0;top:${size / 2 - thickness / 2}px;width:${arm}px;height:${thickness}px"></div>
    <div style="${line}left:${size / 2 - thickness / 2}px;top:0;width:${thickness}px;height:${arm}px"></div>
    <div style="${line}left:${size / 2 - thickness / 2}px;bottom:0;width:${thickness}px;height:${arm}px"></div>
  </div>`;
});
"##,
    ),
    (
        "game_timer.js",
        r##"/// <reference path="./hq-overlay-module.d.ts" />
// @ts-check

setName("Game Timer");
setDescription("A simple always-on module. The styling lives in this JS file through css.");
setDefaultPosition({ x: 4, y: 6 });
setCss(`
  .overlay-module-game_timer .timer-box {
    min-width: 118px;
    padding: 20px;
  }
  .overlay-module-game_timer .timer-label {
    color: rgba(255,255,255,.52);
    font-size: 10px;
    font-weight: 700;
    text-transform: uppercase;
  }
  .overlay-module-game_timer .timer-value {
    color: white;
    font-size: 24px;
    font-weight: 760;
    line-height: 1.05;
    font-variant-numeric: tabular-nums;
  }
`);
register("settings", [Setting.toggle("enabled", "Enabled", false)]);
register("visible", ({ settings }) => settings.enabled !== false);
register("renderOverlay", ({ context, api }) =>
  `<div class="timer-box"><div class="timer-label">Game Timer</div><div class="timer-value">${api.formatSeconds(context.elapsedSeconds)}</div></div>`
);
"##,
    ),
    (
        "image.js",
        r##"/// <reference path="./hq-overlay-module.d.ts" />
// @ts-check

setName("Image");
setDescription("Displays an uploaded image on the overlay.");
setDefaultPosition({ x: 64, y: 16 });
register("settings", [
  Setting.toggle("enabled", "Enabled", false),
  Setting.image("image", "Image", ""),
  Setting.range("width", "Width", 48, 900, 1, 240),
  Setting.range("opacity", "Opacity", 0.05, 1, 0.05, 1),
  Setting.range("radius", "Corner Radius", 0, 48, 1, 0)
]);
register("visible", ({ context, settings }) => settings.enabled !== false && (context.editMode || settings.image));
register("renderOverlay", ({ settings, api }) => {
  const src = String(settings.image ?? "");
  if (!src) {
    return `<div style="border:1px dashed rgba(255,255,255,.25);background:rgba(0,0,0,.45);padding:12px 16px;border-radius:6px;color:rgba(255,255,255,.58);font-size:14px">Upload an image</div>`;
  }
  const width = Math.max(48, Math.min(900, Number(settings.width ?? 240) || 240));
  const opacity = Math.max(0.05, Math.min(1, Number(settings.opacity ?? 1) || 1));
  const radius = Math.max(0, Math.min(48, Number(settings.radius ?? 0) || 0));
  return `<img src="${api.html(src)}" alt="" style="display:block;width:${width}px;max-width:90vw;height:auto;opacity:${opacity};border-radius:${radius}px;filter:drop-shadow(0 12px 28px rgba(0,0,0,.45));" />`;
});
"##,
    ),
    (
        "real_bottom_line.js",
        r##"/// <reference path="./hq-overlay-module.d.ts" />
// @ts-check

setName("Real Bottom Line");
setDescription("Reads the full LCStatsTracker payload in JS and briefly shows the real bottom/top line after a run payload arrives.");
setDefaultPosition({ x: 4, y: 34 });
setCss(`
    .overlay-module-real_bottom_line .rl-card {
      min-width: 210px;
      border: 1px solid rgba(255,255,255,.14);
      border-radius: 6px;
      background: rgba(12,14,18,.86);
      box-shadow: 0 16px 42px rgba(0,0,0,.42);
      padding: 11px 12px;
      backdrop-filter: blur(10px);
    }
    .overlay-module-real_bottom_line .rl-head {
      margin-bottom: 7px;
      color: rgba(255,255,255,.58);
      font-size: 11px;
      font-weight: 700;
      letter-spacing: 0;
      text-transform: uppercase;
    }
    .overlay-module-real_bottom_line .rl-main {
      color: #f5f7ff;
      font-size: 22px;
      font-weight: 750;
      line-height: 1.05;
    }
    .overlay-module-real_bottom_line .rl-grid {
      display: grid;
      gap: 3px;
      margin-top: 8px;
      color: rgba(255,255,255,.72);
      font-size: 12px;
    }
`);
register("settings", [
  Setting.toggle("enabled", "Enabled", false)
]);
register("derive", ({ context, api }) => {
    const stats = context.lcstats;
    if (!stats) return null;
    const lostItems = Array.isArray(stats.MissedItems)
      ? stats.MissedItems.filter((item) => item && item.CollectedOnPreviousDay)
      : [];
    const lostScrap = lostItems.reduce((total, item) => total + api.intish(item.Value), 0);
    const real = api.intish(api.valueAtAny(stats, [
      "PerformanceInfo.TotalAvailableValue",
      "TotalAvailableValue",
      "BottomLineTrue"
    ]));
    return {
      moon: api.stripLcQuote(api.valueAt(stats, "MoonInfo.Name", "Unknown")),
      collected: api.intish(api.valueAtAny(stats, [
        "PerformanceInfo.CollectedTotal",
        "CollectedTotal"
      ])),
      real,
      topLine: real + lostScrap,
      lostScrap
    };
});
register("visible", ({ context, settings }) => {
    if (settings.enabled === false) return false;
    if (context.editMode) return true;
    return context.lcstatsAgeMs != null && context.lcstatsAgeMs <= Number(context.displayTimeMs ?? 10000);
});
register("renderOverlay", ({ data, api }) => {
    if (!data) {
      return `<div class="rl-card"><div class="rl-head">Real Bottom Line</div><div class="rl-grid">Waiting for LCStatsTracker...</div></div>`;
    }
    return `<div class="rl-card">
      <div class="rl-head">${api.html(data.moon)} - Real Bottom Line</div>
      <div class="rl-main">${api.number(data.real)}</div>
      <div class="rl-grid">
        <div>Collected: ${api.number(data.collected)}</div>
        <div>Top Line: ${api.number(data.topLine)}</div>
        <div>Lost Scrap: ${api.number(data.lostScrap)}</div>
      </div>
    </div>`;
});
"##,
    ),
    (
        "leaderboard.js",
        r##"/// <reference path="./hq-overlay-module.d.ts" />
// @ts-check

setName("Leaderboard");
setDescription("Shows your estimated HighQuotaHQ rank from the latest LCStatsTracker payload.");
setDefaultPosition({ x: 4, y: 18 });
setWrapperClass("rounded border border-white/15 bg-black/70 p-3 shadow-xl shadow-black/45");
register("settings", [
  Setting.toggle("enabled", "Enabled", false),
  Setting.selectMenu("track", "Track", [
    { label: "Vanilla", value: "vanilla" },
    { label: "Modded", value: "modded" }
  ], "vanilla"),
  Setting.toggle("includeCurrentVersion", "Current Version Only", false),
  Setting.selectMenu("displayMode", "Display Mode", [
    { label: "Detailed", value: "detailed" },
    { label: "Compact", value: "compact" }
  ], "detailed")
]);
register("visible", ({ context, settings }) => {
  if (settings.enabled === false) return false;
  if (context.editMode) return true;
  return context.lcstatsAgeMs != null && context.lcstatsAgeMs <= Number(context.displayTimeMs ?? 10000);
});
register("renderOverlay", ({ context, settings, api }) => {
  const leaderboard = context.leaderboard ?? { status: "idle" };
  if (leaderboard.status === "idle") {
    return `<div class="overlay-title">Leaderboard</div><div class="overlay-line">Waiting for LCStatsTracker...</div>`;
  }
  if (leaderboard.status === "loading") {
    return `<div class="overlay-title">Leaderboard</div><div class="overlay-line">Loading HighQuotaHQ ${api.html(leaderboard.boardType?.toUpperCase() ?? "")}...</div>`;
  }
  if (leaderboard.status === "error") {
    return `<div class="overlay-title">Leaderboard</div><div class="overlay-line">HighQuotaHQ lookup failed</div><div class="overlay-line">${api.html(leaderboard.error)}</div>`;
  }
  if (leaderboard.status === "waiting") {
    return `<div class="overlay-title">Leaderboard</div><div class="overlay-line">${api.html(leaderboard.reason)}</div>`;
  }
  const scoreLabel = leaderboard.metricLabel ?? "Score";
  const nextScore = leaderboard.nextScore != null ? api.number(leaderboard.nextScore) : "None";
  const versionLine = leaderboard.includeCurrentVersion ? `<div class="overlay-line">Version: ${api.html(leaderboard.version)}</div>` : "";
  if (settings.displayMode === "compact") {
    return `<div class="overlay-title">${api.html(leaderboard.boardType.toUpperCase())} #${api.number(leaderboard.rank)}</div>
      <div class="overlay-line">${api.html(scoreLabel)}: ${api.number(leaderboard.score)} / Top: ${leaderboard.top ? api.number(leaderboard.top.score) : "None"}</div>`;
  }
  return `<div class="overlay-title">Leaderboard - ${api.html(leaderboard.boardType.toUpperCase())}</div>
    <div class="overlay-line">${api.html(scoreLabel)}: ${api.number(leaderboard.score)}</div>
    <div class="overlay-line">Rank: #${api.number(leaderboard.rank)} / ${api.number(leaderboard.totalRecords + 1)}</div>
    ${versionLine}
    <div class="overlay-line">Top: ${leaderboard.top ? api.number(leaderboard.top.score) : "None"}</div>
    <div class="overlay-line">Next Below: ${nextScore}</div>`;
});
"##,
    ),
    (
        "end_summary.js",
        r##"/// <reference path="./hq-overlay-module.d.ts" />
// @ts-check

setName("End Summary");
setDescription("Shows a temporary end summary event or a preview while editing layout.");
setDefaultPosition({ x: 72, y: 8 });
setWrapperClass("rounded border border-white/15 bg-black/70 p-3 shadow-xl shadow-black/45");
register("visible", ({ context }) => context.editMode || !!context.endSummary);
register("renderOverlay", ({ context, api }) => {
  const summary = context.endSummary ?? {
    title: "Run Summary Preview",
    lines: ["Top: #1 12,430", "You: #8 9,180", "Next: #9 8,940"]
  };
  return `<div class="overlay-title">${api.html(summary.title ?? "Run Summary")}</div>${
    (summary.lines ?? []).map((line) => `<div class="overlay-line">${api.html(line)}</div>`).join("")
  }`;
});
"##,
    ),
];

const OVERLAY_MODULE_TYPES_FILE_NAME: &str = "hq-overlay-module.d.ts";
const OVERLAY_MODULE_TYPES: &str = r##"type OverlaySettingSchema =
  | { key: string; label: string; type: "boolean"; default?: boolean }
  | { key: string; label: string; type: "color"; default?: string }
  | { key: string; label: string; type: "number"; min?: number; max?: number; step?: number; default?: number }
  | { key: string; label: string; type: "range"; min: number; max: number; step?: number; default?: number }
  | { key: string; label: string; type: "text" | "textarea" | "key" | "image"; default?: string }
  | { key: string; label: string; type: "select"; options: Array<{ label: string; value: string }>; default?: string };

/** Shortcut strings captured by key buttons, for example "Insert", "Ctrl+Shift+K", or "Ctrl+Shift+*". */
type OverlayShortcutString = string;

type OverlayRegisterType = "metadata" | "settings" | "defaults" | "css" | "visible" | "derive" | "renderOverlay" | "tick" | "lcstats";

type LeaderboardBoardType = "hq" | "sdc" | "smhq";
type LeaderboardCollectionName =
  | "leaderboards_hq" | "leaderboards_sdc" | "leaderboards_smhq"
  | "modded_hq" | "modded_sdc" | "modded_smhq"
  | "lc_modded_brutal_hq" | "lc_modded_brutal_sdc" | "lc_modded_brutal_smhq"
  | "lc_modded_eclipsed_hq" | "lc_modded_eclipsed_smhq"
  | "lc_modded_wesleysmoons_hq" | "lc_modded_wesleysmoons_sdc" | "lc_modded_wesleysmoons_smhq"
  | "lc_modded_classicmoons_hq" | "lc_modded_classicmoons_sdc" | "lc_modded_classicmoons_smhq";

type LeaderboardRun = {
  id?: string;
  collectionName?: LeaderboardCollectionName | string;
  players?: string[];
  version?: string;
  verified?: boolean;
  quotaAmount?: number;
  quotaReached?: number;
  totalScrap?: number;
  moon?: string;
  scrapType?: string;
  videos?: any;
  date?: any;
  verifiedAt?: any;
  verifier?: string;
  [key: string]: any;
};

type LeaderboardState = {
  status: "idle" | "waiting" | "loading" | "ready" | "error";
  error?: string;
  reason?: string;
  track?: "vanilla" | "modded";
  boardType?: LeaderboardBoardType;
  collectionName?: LeaderboardCollectionName | string;
  metricKey?: "quotaAmount" | "totalScrap" | string;
  metricLabel?: string;
  score?: number;
  rank?: number;
  totalRecords?: number;
  top?: { rank: number; score: number; players: string[] } | null;
  next?: LeaderboardRun | null;
  nextScore?: number | null;
  includeCurrentVersion?: boolean;
  playerCount?: number;
  version?: string;
  moon?: string;
  collections: {
    vanilla: Record<LeaderboardBoardType, LeaderboardCollectionName>;
    modded: Record<LeaderboardBoardType, LeaderboardCollectionName>;
    legacyModded: Record<string, Partial<Record<LeaderboardBoardType, LeaderboardCollectionName>>>;
  };
  boardTypes: Record<LeaderboardBoardType, { id: LeaderboardBoardType; name: string; metricLabel: string; metricKey: "quotaAmount" | "totalScrap" }>;
  runFields: string[];
};

type OverlayEndSummary = {
  id?: number;
  title?: string;
  lines?: string[];
  payload?: any;
  expiresAt?: number;
};

type OverlayStreamOverlays = {
  type?: string;
  messageType?: string;
  showOverlay?: boolean;
  crewCount?: number;
  moonName?: string;
  weatherName?: string;
  quotaValue?: number;
  quotaIndex?: number;
  lootValue?: number;
  [key: string]: any;
};

type OverlayContext = {
  editMode: boolean;
  controlsOpen: boolean;
  elapsedSeconds: number;
  lcstats: any;
  lcstatsRaw: string | null;
  lcstatsPayload: { raw: string; stats: any } | null;
  lcstatsAgeMs: number | null;
  streamOverlays: OverlayStreamOverlays | null;
  streamOverlay: OverlayStreamOverlays | null;
  streamOverlaysAgeMs: number | null;
  displayTimeMs: number;
  leaderboard: LeaderboardState;
  /** @deprecated Use context.leaderboard. */
  recordChecker: LeaderboardState;
  endSummary: OverlayEndSummary | null;
  events: any[];
  inputSequence: number;
  formatSeconds(totalSeconds: number): string;
  escapeHtml(value: any): string;
  html(value: any): string;
  number(value: any): string;
  stripLcQuote(value: any): any;
  intish(value: any, fallback?: number): number;
  valueAt(root: any, path: string | string[], fallback?: any): any;
  valueAtAny(root: any, paths: Array<string | string[]>, fallback?: any): any;
};

type OverlayInputEvent = {
  id: string | number;
  type: "keydown" | "keyup";
  key: string;
  shortcut: OverlayShortcutString;
  ctrlKey: boolean;
  shiftKey: boolean;
  altKey: boolean;
  metaKey: boolean;
  source: "window" | "module-key" | "overlay-key" | "global-shortcut" | string;
  receivedAt: number;
};

type OverlayInputApi = {
  down(shortcut: OverlayShortcutString): boolean;
  held(shortcut: OverlayShortcutString): boolean;
  shortcut(shortcut: OverlayShortcutString): boolean;
  pressed(shortcut: OverlayShortcutString): boolean;
  released(shortcut: OverlayShortcutString): boolean;
  consumePress(shortcut: OverlayShortcutString): boolean;
  consumeRelease(shortcut: OverlayShortcutString): boolean;
  events(): OverlayInputEvent[];
  last(): OverlayInputEvent | null;
};

type OverlayHandlerArgs<TData = any> = {
  context: OverlayContext;
  data: TData;
  settings: Record<string, any>;
  config: any;
  api: OverlayModuleApi;
};

type OverlayModuleApi = {
  id: string;
  formatSeconds(totalSeconds: number): string;
  escapeHtml(value: any): string;
  html(value: any): string;
  number(value: any): string;
  stripLcQuote(value: any): any;
  intish(value: any, fallback?: number): number;
  valueAt(root: any, path: string | string[], fallback?: any): any;
  valueAtAny(root: any, paths: Array<string | string[]>, fallback?: any): any;
  className(name?: string): string;
  now(): number;
  input: OverlayInputApi;
  readonly context: OverlayContext | null;
  getLcStats(): any;
  getLcStatsRaw(): string | null;
  getStreamOverlay(): OverlayStreamOverlays | null;
};

declare const Setting: {
  toggle(key: string, label: string, defaultValue?: boolean): OverlaySettingSchema;
  color(key: string, label: string, defaultValue?: string): OverlaySettingSchema;
  number(key: string, label: string, defaultValue?: number, min?: number, max?: number, step?: number): OverlaySettingSchema;
  range(key: string, label: string, min: number, max: number, step?: number, defaultValue?: number): OverlaySettingSchema;
  text(key: string, label: string, defaultValue?: string): OverlaySettingSchema;
  textarea(key: string, label: string, defaultValue?: string): OverlaySettingSchema;
  image(key: string, label: string, defaultValue?: string): OverlaySettingSchema;
  key(key: string, label: string, defaultValue?: OverlayShortcutString): OverlaySettingSchema;
  hotkey(key: string, label: string, defaultValue?: OverlayShortcutString): OverlaySettingSchema;
  select(key: string, label: string, options: Array<{ label: string; value: string }>, defaultValue?: string): OverlaySettingSchema;
  selectMenu(key: string, label: string, options: Array<{ label: string; value: string }>, defaultValue?: string): OverlaySettingSchema;
};

declare function register(type: "settings", payload: OverlaySettingSchema[]): unknown;
declare function register(type: "defaults" | "metadata", payload: Record<string, any>): unknown;
declare function register(type: "css", payload: string): unknown;
declare function register(type: "visible", payload: (args: OverlayHandlerArgs) => boolean | void): unknown;
declare function register<TData = any>(type: "derive", payload: (args: OverlayHandlerArgs) => TData): unknown;
declare function register<TData = any>(type: "renderOverlay", payload: (args: OverlayHandlerArgs<TData>) => string | number | null | undefined): unknown;
declare function register<TData = any>(type: "tick" | "lcstats", payload: (args: OverlayHandlerArgs<TData>) => unknown): unknown;
declare function register(type: OverlayRegisterType, payload: any): unknown;

declare function setName(name: string): unknown;
declare function setDescription(description: string): unknown;
declare function setLocked(locked?: boolean): unknown;
declare function setDefaultPosition(position: { x: number; y: number }): unknown;
declare function setDefaultSettings(settings: Record<string, any>): unknown;
declare function setWrapperClass(wrapperClass: string): unknown;
declare function setCss(css: string): unknown;

declare const api: OverlayModuleApi;
declare const html: OverlayModuleApi["html"];
declare const formatSeconds: OverlayModuleApi["formatSeconds"];
declare const number: OverlayModuleApi["number"];
declare const valueAt: OverlayModuleApi["valueAt"];
declare const valueAtAny: OverlayModuleApi["valueAtAny"];
declare const intish: OverlayModuleApi["intish"];
"##;

impl Default for CrosshairConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            style: "plus".to_string(),
            color: "#ffffff".to_string(),
            size: 24.0,
            thickness: 2.0,
            gap: 5.0,
            opacity: 0.9,
        }
    }
}

impl Default for GameOverlayGeneralConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            use_stream_overlays_api: false,
            overlay_key: "Insert".to_string(),
            end_summary_duration_ms: 10_000,
        }
    }
}

impl Default for OverlayWidgetPosition {
    fn default() -> Self {
        Self {
            x: 50.0,
            y: 50.0,
            snap: true,
        }
    }
}

impl Default for EndSummaryConfig {
    fn default() -> Self {
        Self {
            position: "top-right".to_string(),
            duration_ms: 10_000,
        }
    }
}

impl Default for GameOverlayConfig {
    fn default() -> Self {
        Self {
            general: GameOverlayGeneralConfig::default(),
            crosshair: CrosshairConfig::default(),
            widgets: default_overlay_widget_positions(),
            module_settings: HashMap::new(),
            end_summary: EndSummaryConfig::default(),
        }
    }
}

fn default_overlay_widget_positions() -> HashMap<String, OverlayWidgetPosition> {
    HashMap::from([
        (
            "crosshair".to_string(),
            OverlayWidgetPosition {
                x: 50.0,
                y: 50.0,
                snap: false,
            },
        ),
        (
            "game_timer".to_string(),
            OverlayWidgetPosition {
                x: 4.0,
                y: 6.0,
                snap: true,
            },
        ),
        (
            "leaderboard".to_string(),
            OverlayWidgetPosition {
                x: 4.0,
                y: 18.0,
                snap: true,
            },
        ),
        (
            "image".to_string(),
            OverlayWidgetPosition {
                x: 64.0,
                y: 16.0,
                snap: true,
            },
        ),
        (
            "real_bottom_line".to_string(),
            OverlayWidgetPosition {
                x: 4.0,
                y: 34.0,
                snap: true,
            },
        ),
        (
            "end_summary".to_string(),
            OverlayWidgetPosition {
                x: 72.0,
                y: 8.0,
                snap: true,
            },
        ),
    ])
}

struct GameOverlayState {
    controls_open: AtomicBool,
    monitor_running: AtomicBool,
    preview_mode: AtomicBool,
    overlay_enabled: AtomicBool,
    stream_overlays_monitor_running: AtomicBool,
    window_visible: AtomicBool,
    last_foreground_pid: AtomicU32,
    last_match: AtomicBool,
    show_count: AtomicU64,
    input_count: AtomicU64,
    registered_overlay_shortcut: Mutex<Option<String>>,
    registered_overlay_input_shortcuts: Mutex<HashSet<String>>,
    last_message: Mutex<String>,
    last_error: Mutex<Option<String>>,
}

impl Default for GameOverlayState {
    fn default() -> Self {
        Self {
            controls_open: AtomicBool::new(false),
            monitor_running: AtomicBool::new(false),
            preview_mode: AtomicBool::new(false),
            overlay_enabled: AtomicBool::new(true),
            stream_overlays_monitor_running: AtomicBool::new(false),
            window_visible: AtomicBool::new(false),
            last_foreground_pid: AtomicU32::new(0),
            last_match: AtomicBool::new(false),
            show_count: AtomicU64::new(0),
            input_count: AtomicU64::new(0),
            registered_overlay_shortcut: Mutex::new(None),
            registered_overlay_input_shortcuts: Mutex::new(HashSet::new()),
            last_message: Mutex::new(String::new()),
            last_error: Mutex::new(None),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct GameOverlayDebugStatus {
    controls_open: bool,
    monitor_running: bool,
    last_foreground_pid: u32,
    last_match: bool,
    show_count: u64,
    last_message: String,
    last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct GameOverlayInputShortcutPayload {
    id: u64,
    shortcut: String,
    state: String,
    source: String,
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

fn legacy_game_overlay_config_path(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    Ok(app
        .path()
        .app_data_dir()
        .map_err(|e| format!("failed to resolve app data dir: {e}"))?
        .join("config")
        .join("game_overlay.json"))
}

fn game_overlay_config_dir(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    Ok(app
        .path()
        .app_data_dir()
        .map_err(|e| format!("failed to resolve app data dir: {e}"))?
        .join("config")
        .join("overlay"))
}

fn game_overlay_module_dir(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    Ok(app
        .path()
        .app_data_dir()
        .map_err(|e| format!("failed to resolve app data dir: {e}"))?
        .join("overlayModule"))
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
        normalize_mod_id("asta", "StreamChats"),
        normalize_mod_id("SlushyRH", "FreeeeeeMoooooons"),
        normalize_mod_id("stormytuna", "EclipseOnly"),
    ];
    default_mods.sort_by(|a, b| a.dev.cmp(&b.dev).then(a.name.cmp(&b.name)));
    if !path.exists() {
        // v4: include default disabled layer mods and StreamChats.
        let f = DisableModFile {
            version: DISABLEMOD_FILE_VERSION,
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
                version: DISABLEMOD_FILE_VERSION,
                mods: default_mods,
            };
            let _ = write_disablemod(app, &f);
            return Ok(f);
        }
    };

    // Migration: v1 -> v2
    if f.version == 1 {
        f.version = 2;
        f.mods
            .push(normalize_mod_id("SlushyRH", "FreeeeeeMoooooons"));
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
    if f.version == 4 {
        f.mods
            .retain(|m| m != &normalize_mod_id("MikuOreo", "LCStatsTracker"));
        f.version = DISABLEMOD_FILE_VERSION;
        let _ = write_disablemod(app, &f);
    }

    Ok(f)
}

async fn migrate_disablemod_v4_on_startup(app: &tauri::AppHandle) -> Result<(), String> {
    let path = disablemod_path(app)?;
    if !path.exists() {
        let _ = read_disablemod(app)?;
        return Ok(());
    }

    let text = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let current_version = serde_json::from_str::<serde_json::Value>(&text)
        .ok()
        .and_then(|v| v.get("version").and_then(|version| version.as_u64()))
        .unwrap_or(0) as u32;

    if current_version > 3 {
        return Ok(());
    }

    let patchable_count = installer::patchable_instance_count(app)? as u32;
    let steps_total = patchable_count + 2;

    progress::emit_progress(
        app,
        TaskProgressPayload {
            version: 0,
            steps_total,
            step: 1,
            step_name: "Security Migration".to_string(),
            step_progress: 0.0,
            overall_percent: overall_from_step(1, 0.0, steps_total),
            detail: Some("Preparing UnityApplicationPatcher".to_string()),
            downloaded_bytes: None,
            total_bytes: None,
            extracted_files: None,
            total_files: None,
        },
    );

    installer::ensure_app_patcher_installed_with_progress(app, steps_total, 1).await?;
    installer::patch_all_instances_with_progress(app, steps_total, 2).await?;

    let final_step = steps_total;
    progress::emit_progress(
        app,
        TaskProgressPayload {
            version: 0,
            steps_total,
            step: final_step,
            step_name: "Security Migration".to_string(),
            step_progress: 0.0,
            overall_percent: overall_from_step(final_step, 0.0, steps_total),
            detail: Some("Updating disablemod.json".to_string()),
            downloaded_bytes: None,
            total_bytes: None,
            extracted_files: None,
            total_files: None,
        },
    );

    let mut f = read_disablemod(app)?;
    f.mods.push(normalize_mod_id("asta", "StreamChats"));
    f.mods
        .sort_by(|a, b| a.dev.cmp(&b.dev).then(a.name.cmp(&b.name)));
    f.mods.dedup();
    f.version = 4;
    write_disablemod(app, &f)?;

    progress::emit_progress(
        app,
        TaskProgressPayload {
            version: 0,
            steps_total,
            step: final_step,
            step_name: "Security Migration".to_string(),
            step_progress: 1.0,
            overall_percent: overall_from_step(final_step, 1.0, steps_total),
            detail: Some("Security migration complete".to_string()),
            downloaded_bytes: None,
            total_bytes: None,
            extracted_files: None,
            total_files: None,
        },
    );
    progress::emit_finished(
        app,
        TaskFinishedPayload {
            version: 0,
            run_mode: None,
            path: path.to_string_lossy().to_string(),
        },
    );

    Ok(())
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

fn read_json_file_or_default<T>(path: &Path) -> T
where
    T: DeserializeOwned + Default,
{
    let Ok(text) = std::fs::read_to_string(path) else {
        return T::default();
    };
    serde_json::from_str::<T>(&text).unwrap_or_else(|e| {
        log::warn!("Failed to parse {}: {e}", path.display());
        T::default()
    })
}

fn write_json_file<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(value).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| e.to_string())
}

fn overlay_module_settings_file_name(module_id: &str) -> String {
    let stem: String = module_id
        .trim()
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
                ch
            } else {
                '_'
            }
        })
        .collect();
    let stem = stem.trim_matches('_');
    if stem.is_empty() {
        "module".to_string()
    } else {
        stem.to_string()
    }
}

fn read_game_overlay_config(app: &tauri::AppHandle) -> Result<GameOverlayConfig, String> {
    let dir = game_overlay_config_dir(app)?;
    if !dir.exists() {
        let legacy_path = legacy_game_overlay_config_path(app)?;
        if legacy_path.exists() {
            let text = std::fs::read_to_string(&legacy_path).map_err(|e| e.to_string())?;
            match serde_json::from_str::<GameOverlayConfig>(&text) {
                Ok(cfg) => {
                    let cfg = sanitize_game_overlay_config(cfg);
                    let _ = write_game_overlay_config(app, &cfg);
                    return Ok(cfg);
                }
                Err(e) => {
                    log::warn!("Failed to parse legacy game_overlay.json, resetting: {e}");
                }
            }
        }

        let cfg = GameOverlayConfig::default();
        let _ = write_game_overlay_config(app, &cfg);
        return Ok(cfg);
    }

    let mut cfg = GameOverlayConfig {
        general: read_json_file_or_default(&dir.join("general.json")),
        crosshair: read_json_file_or_default(&dir.join("crosshair.json")),
        widgets: read_json_file_or_default(&dir.join("widgets.json")),
        module_settings: HashMap::new(),
        end_summary: read_json_file_or_default(&dir.join("end_summary.json")),
    };

    let modules_dir = dir.join("modules");
    if modules_dir.exists() {
        for entry in std::fs::read_dir(&modules_dir).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path();
            if !path.is_file() || path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                continue;
            }
            let Some(module_id) = path.file_stem().and_then(|stem| stem.to_str()) else {
                continue;
            };
            let value: serde_json::Value = read_json_file_or_default(&path);
            if !value.is_null() {
                cfg.module_settings.insert(module_id.to_string(), value);
            }
        }
    }

    Ok(sanitize_game_overlay_config(cfg))
}

fn write_game_overlay_config(
    app: &tauri::AppHandle,
    cfg: &GameOverlayConfig,
) -> Result<(), String> {
    let dir = game_overlay_config_dir(app)?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    write_json_file(&dir.join("general.json"), &cfg.general)?;
    write_json_file(&dir.join("crosshair.json"), &cfg.crosshair)?;
    write_json_file(&dir.join("widgets.json"), &cfg.widgets)?;
    write_json_file(&dir.join("end_summary.json"), &cfg.end_summary)?;

    let modules_dir = dir.join("modules");
    std::fs::create_dir_all(&modules_dir).map_err(|e| e.to_string())?;
    for (module_id, settings) in &cfg.module_settings {
        let file_name = format!("{}.json", overlay_module_settings_file_name(module_id));
        write_json_file(&modules_dir.join(file_name), settings)?;
    }
    Ok(())
}

fn ensure_default_game_overlay_modules(app: &tauri::AppHandle) -> Result<(), String> {
    let dir = game_overlay_module_dir(app)?;
    if dir.exists() {
        let types_path = dir.join(OVERLAY_MODULE_TYPES_FILE_NAME);
        std::fs::write(&types_path, OVERLAY_MODULE_TYPES).map_err(|e| e.to_string())?;
        let legacy_example_path = dir.join("00_chattriggers_api_example.js");
        if legacy_example_path.exists() {
            std::fs::remove_file(&legacy_example_path).map_err(|e| e.to_string())?;
        }
        for (file_name, source) in DEFAULT_OVERLAY_MODULES {
            let path = dir.join(file_name);
            if !path.exists() {
                std::fs::write(&path, source).map_err(|e| e.to_string())?;
                continue;
            }
            let current = std::fs::read_to_string(&path).unwrap_or_default();
            if !current
                .trim_start()
                .starts_with("/// <reference path=\"./hq-overlay-module.d.ts\" />")
            {
                std::fs::write(&path, format!("{OVERLAY_MODULE_REFERENCE}{current}"))
                    .map_err(|e| e.to_string())?;
            }
        }
        for file_name in [
            "crosshair.js",
            "game_timer.js",
            "image.js",
            "leaderboard.js",
            "real_bottom_line.js",
            "end_summary.js",
        ] {
            let path = dir.join(file_name);
            if !path.exists() {
                continue;
            }
            let current = std::fs::read_to_string(&path).unwrap_or_default();
            let should_refresh = match file_name {
                "crosshair.js" => {
                    current.contains("setName(\"Crosshair\")")
                        && !current.contains("Setting.selectMenu(\"style\"")
                }
                "game_timer.js" => {
                    current.contains("setName(\"Game Timer\")")
                        && current.contains("backdrop-filter")
                }
                "real_bottom_line.js" => {
                    current.contains("setName(\"Real Bottom Line\")")
                        && current.contains("durationSeconds")
                }
                "leaderboard.js" => {
                    current.contains("setName(\"Leaderboard\")")
                        && (!current.contains("context.leaderboard")
                            || current.contains("register(\"renderOverlay\", ({ context, api })"))
                }
                "end_summary.js" => {
                    current.contains("setName(\"End Summary\")")
                        && current.contains("durationSeconds")
                }
                _ => false,
            };
            if should_refresh {
                if let Some((_, source)) = DEFAULT_OVERLAY_MODULES
                    .iter()
                    .find(|(default_file_name, _)| *default_file_name == file_name)
                {
                    std::fs::write(&path, source).map_err(|e| e.to_string())?;
                }
            }
        }
        let record_checker_path = dir.join("record_checker.js");
        if record_checker_path.exists() {
            let current = std::fs::read_to_string(&record_checker_path).unwrap_or_default();
            if current.contains("Preview placeholder for SDC/HQ rank checks")
                || current.contains("setName(\"Record Checker\")")
            {
                std::fs::remove_file(&record_checker_path).map_err(|e| e.to_string())?;
            }
        }
        return Ok(());
    }
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    std::fs::write(
        dir.join(OVERLAY_MODULE_TYPES_FILE_NAME),
        OVERLAY_MODULE_TYPES,
    )
    .map_err(|e| e.to_string())?;
    for (file_name, source) in DEFAULT_OVERLAY_MODULES {
        let path = dir.join(file_name);
        std::fs::write(&path, source).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn read_game_overlay_modules(app: &tauri::AppHandle) -> Result<Vec<GameOverlayModuleDto>, String> {
    ensure_default_game_overlay_modules(app)?;
    let dir = game_overlay_module_dir(app)?;
    let mut modules = Vec::new();
    for entry in std::fs::read_dir(&dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if !path.is_file() || path.extension().and_then(|ext| ext.to_str()) != Some("js") {
            continue;
        }
        let file_name = entry.file_name().to_string_lossy().to_string();
        let source = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
        let id = file_name
            .strip_suffix(".js")
            .unwrap_or(&file_name)
            .trim()
            .to_string();
        if id.is_empty() {
            continue;
        }
        modules.push(GameOverlayModuleDto {
            id,
            file_name,
            source,
        });
    }
    modules.sort_by(|left, right| left.file_name.cmp(&right.file_name));
    Ok(modules)
}

fn sanitize_game_overlay_config(mut cfg: GameOverlayConfig) -> GameOverlayConfig {
    let fallback = GameOverlayConfig::default();
    cfg.general.overlay_key = sanitize_overlay_key(&cfg.general.overlay_key);
    cfg.general.end_summary_duration_ms = cfg.general.end_summary_duration_ms.clamp(2_000, 30_000);
    let style = cfg.crosshair.style.trim().to_lowercase();
    cfg.crosshair.style = match style.as_str() {
        "plus" | "dot" | "circle" | "x" | "square" => style,
        _ => fallback.crosshair.style,
    };

    let color = cfg.crosshair.color.trim();
    cfg.crosshair.color = if color.len() == 7
        && color.starts_with('#')
        && color.chars().skip(1).all(|ch| ch.is_ascii_hexdigit())
    {
        color.to_string()
    } else {
        fallback.crosshair.color
    };

    cfg.crosshair.size = cfg.crosshair.size.clamp(4.0, 96.0);
    cfg.crosshair.thickness = cfg.crosshair.thickness.clamp(1.0, 12.0);
    cfg.crosshair.gap = cfg.crosshair.gap.clamp(0.0, 32.0);
    cfg.crosshair.opacity = cfg.crosshair.opacity.clamp(0.05, 1.0);
    for (id, default_position) in default_overlay_widget_positions() {
        cfg.widgets.entry(id).or_insert(default_position);
    }
    for position in cfg.widgets.values_mut() {
        sanitize_widget_position(position);
    }
    cfg.end_summary.position = sanitize_overlay_position(&cfg.end_summary.position);
    cfg.end_summary.duration_ms = cfg.end_summary.duration_ms.clamp(2_000, 30_000);
    cfg
}

fn sync_game_overlay_enabled_state(app: &tauri::AppHandle, enabled: bool) {
    app.state::<GameOverlayState>()
        .overlay_enabled
        .store(enabled, Ordering::Relaxed);
}

fn sanitize_widget_position(position: &mut OverlayWidgetPosition) {
    position.x = position.x.clamp(0.0, 100.0);
    position.y = position.y.clamp(0.0, 100.0);
}

fn sanitize_overlay_key(key: &str) -> String {
    let trimmed = key.trim();
    if trimmed.is_empty() || trimmed.len() > 64 || trimmed.chars().any(|ch| ch.is_control()) {
        "Insert".to_string()
    } else {
        trimmed.to_string()
    }
}

fn overlay_shortcut_keys() -> &'static [(&'static str, Code)] {
    &[
        ("A", Code::KeyA),
        ("B", Code::KeyB),
        ("C", Code::KeyC),
        ("D", Code::KeyD),
        ("E", Code::KeyE),
        ("F", Code::KeyF),
        ("G", Code::KeyG),
        ("H", Code::KeyH),
        ("I", Code::KeyI),
        ("J", Code::KeyJ),
        ("K", Code::KeyK),
        ("L", Code::KeyL),
        ("M", Code::KeyM),
        ("N", Code::KeyN),
        ("O", Code::KeyO),
        ("P", Code::KeyP),
        ("Q", Code::KeyQ),
        ("R", Code::KeyR),
        ("S", Code::KeyS),
        ("T", Code::KeyT),
        ("U", Code::KeyU),
        ("V", Code::KeyV),
        ("W", Code::KeyW),
        ("X", Code::KeyX),
        ("Y", Code::KeyY),
        ("Z", Code::KeyZ),
        ("0", Code::Digit0),
        ("1", Code::Digit1),
        ("2", Code::Digit2),
        ("3", Code::Digit3),
        ("4", Code::Digit4),
        ("5", Code::Digit5),
        ("6", Code::Digit6),
        ("7", Code::Digit7),
        ("8", Code::Digit8),
        ("9", Code::Digit9),
        (")", Code::Digit0),
        ("!", Code::Digit1),
        ("@", Code::Digit2),
        ("#", Code::Digit3),
        ("$", Code::Digit4),
        ("%", Code::Digit5),
        ("^", Code::Digit6),
        ("&", Code::Digit7),
        ("*", Code::Digit8),
        ("(", Code::Digit9),
        ("F1", Code::F1),
        ("F2", Code::F2),
        ("F3", Code::F3),
        ("F4", Code::F4),
        ("F5", Code::F5),
        ("F6", Code::F6),
        ("F7", Code::F7),
        ("F8", Code::F8),
        ("F9", Code::F9),
        ("F10", Code::F10),
        ("F11", Code::F11),
        ("F12", Code::F12),
        ("F13", Code::F13),
        ("F14", Code::F14),
        ("F15", Code::F15),
        ("F16", Code::F16),
        ("F17", Code::F17),
        ("F18", Code::F18),
        ("F19", Code::F19),
        ("F20", Code::F20),
        ("F21", Code::F21),
        ("F22", Code::F22),
        ("F23", Code::F23),
        ("F24", Code::F24),
        ("Insert", Code::Insert),
        ("Delete", Code::Delete),
        ("Home", Code::Home),
        ("End", Code::End),
        ("PageUp", Code::PageUp),
        ("PageDown", Code::PageDown),
        ("ArrowUp", Code::ArrowUp),
        ("ArrowDown", Code::ArrowDown),
        ("ArrowLeft", Code::ArrowLeft),
        ("ArrowRight", Code::ArrowRight),
        ("Escape", Code::Escape),
        ("Tab", Code::Tab),
        ("Enter", Code::Enter),
        ("Space", Code::Space),
        ("Backspace", Code::Backspace),
        ("`", Code::Backquote),
        ("-", Code::Minus),
        ("=", Code::Equal),
        ("[", Code::BracketLeft),
        ("]", Code::BracketRight),
        ("\\", Code::Backslash),
        (";", Code::Semicolon),
        ("'", Code::Quote),
        (",", Code::Comma),
        (".", Code::Period),
        ("/", Code::Slash),
        ("~", Code::Backquote),
        ("_", Code::Minus),
        ("+", Code::Equal),
        ("{", Code::BracketLeft),
        ("}", Code::BracketRight),
        ("|", Code::Backslash),
        (":", Code::Semicolon),
        ("\"", Code::Quote),
        ("<", Code::Comma),
        (">", Code::Period),
        ("?", Code::Slash),
        ("Numpad0", Code::Numpad0),
        ("Numpad1", Code::Numpad1),
        ("Numpad2", Code::Numpad2),
        ("Numpad3", Code::Numpad3),
        ("Numpad4", Code::Numpad4),
        ("Numpad5", Code::Numpad5),
        ("Numpad6", Code::Numpad6),
        ("Numpad7", Code::Numpad7),
        ("Numpad8", Code::Numpad8),
        ("Numpad9", Code::Numpad9),
        ("NumpadAdd", Code::NumpadAdd),
        ("+", Code::NumpadAdd),
        ("NumpadMultiply", Code::NumpadMultiply),
        ("*", Code::NumpadMultiply),
        ("NumpadSubtract", Code::NumpadSubtract),
        ("NumpadDecimal", Code::NumpadDecimal),
        ("NumpadDivide", Code::NumpadDivide),
        ("NumpadEnter", Code::NumpadEnter),
    ]
}

fn parse_overlay_shortcut_modifiers(prefix: &str) -> Option<Option<Modifiers>> {
    let prefix = prefix.trim();
    if prefix.is_empty() {
        return Some(None);
    }

    let mut modifiers = Modifiers::empty();
    for part in prefix.split('+') {
        match part.trim().to_ascii_lowercase().as_str() {
            "ctrl" | "control" => modifiers |= Modifiers::CONTROL,
            "shift" => modifiers |= Modifiers::SHIFT,
            "alt" => modifiers |= Modifiers::ALT,
            "meta" | "super" | "cmd" | "win" | "windows" => modifiers |= Modifiers::SUPER,
            _ => return None,
        }
    }
    Some(Some(modifiers))
}

fn overlay_shortcuts_for_key(key: &str) -> Vec<Shortcut> {
    let trimmed = key.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }
    let lower = trimmed.to_ascii_lowercase();
    let mut shortcuts = Vec::new();

    for &(shortcut_key, shortcut_code) in overlay_shortcut_keys() {
        let shortcut_lower = shortcut_key.to_ascii_lowercase();
        let modifier_prefix = if lower == shortcut_lower {
            Some("")
        } else if lower.ends_with(&shortcut_lower) {
            let prefix_end = trimmed.len().saturating_sub(shortcut_key.len());
            let prefix = &trimmed[..prefix_end];
            prefix
                .strip_suffix('+')
                .filter(|without_separator| !without_separator.is_empty())
        } else {
            None
        };

        let Some(modifier_prefix) = modifier_prefix else {
            continue;
        };
        let Some(modifiers) = parse_overlay_shortcut_modifiers(modifier_prefix) else {
            continue;
        };
        shortcuts.push(Shortcut::new(modifiers, shortcut_code));
    }

    shortcuts
}

fn sanitize_overlay_position(position: &str) -> String {
    match position.trim().to_lowercase().as_str() {
        "top-left" | "top-right" | "bottom-left" | "bottom-right" | "center" => {
            position.trim().to_lowercase()
        }
        _ => "top-right".to_string(),
    }
}

#[cfg(target_os = "windows")]
fn apply_game_overlay_window_style(window: &tauri::WebviewWindow) -> Result<(), String> {
    let hwnd = window.hwnd().map_err(|e| e.to_string())?.0 as HWND;
    let current = unsafe { GetWindowLongPtrW(hwnd, GWL_EXSTYLE) };
    let next = (current | WS_EX_TOOLWINDOW as isize) & !(WS_EX_APPWINDOW as isize);
    if next != current {
        unsafe {
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, next);
        }
    }
    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn apply_game_overlay_window_style(_window: &tauri::WebviewWindow) -> Result<(), String> {
    Ok(())
}

fn ensure_game_overlay_window(app: &tauri::AppHandle) -> Result<tauri::WebviewWindow, String> {
    if let Some(window) = app.get_webview_window(GAME_OVERLAY_WINDOW_LABEL) {
        apply_game_overlay_window_style(&window)?;
        return Ok(window);
    }

    let page_load_app = app.clone();
    let window = tauri::WebviewWindowBuilder::new(
        app,
        GAME_OVERLAY_WINDOW_LABEL,
        tauri::WebviewUrl::App("index.html#window=game-overlay".into()),
    )
    .on_page_load(move |_window, payload| {
        let event = match payload.event() {
            tauri::webview::PageLoadEvent::Started => "started",
            tauri::webview::PageLoadEvent::Finished => "finished",
        };
        set_game_overlay_debug(
            &page_load_app,
            format!("page load {event}: {}", payload.url()),
        );
    })
    .title("HQ Overlay")
    .decorations(false)
    .transparent(true)
    .shadow(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .fullscreen(true)
    .focused(false)
    .focusable(false)
    .visible(false)
    .build()
    .map_err(|e| format!("failed to create game overlay window: {e}"))?;

    apply_game_overlay_window_style(&window)?;
    apply_game_overlay_interaction(&window, false)?;
    set_game_overlay_debug(app, "window created");
    Ok(window)
}

fn apply_game_overlay_interaction(
    window: &tauri::WebviewWindow,
    controls_open: bool,
) -> Result<(), String> {
    window
        .set_ignore_cursor_events(!controls_open)
        .map_err(|e| e.to_string())?;
    window
        .set_focusable(controls_open)
        .map_err(|e| e.to_string())?;
    if controls_open {
        let _ = window.set_focus();
    }
    Ok(())
}

fn set_game_overlay_controls_open_inner(
    app: &tauri::AppHandle,
    state: &GameOverlayState,
    open: bool,
) -> Result<bool, String> {
    state.controls_open.store(open, Ordering::Relaxed);
    let window = ensure_game_overlay_window(app)?;
    window.show().map_err(|e| e.to_string())?;
    window.set_always_on_top(true).map_err(|e| e.to_string())?;
    force_game_overlay_topmost(&window)?;
    apply_game_overlay_interaction(&window, open)?;
    app.emit_to(
        GAME_OVERLAY_WINDOW_LABEL,
        "overlay://controls-open-changed",
        open,
    )
    .map_err(|e| e.to_string())?;
    let app_handle = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        let _ = app_handle.emit_to(
            GAME_OVERLAY_WINDOW_LABEL,
            "overlay://controls-open-changed",
            open,
        );
    });
    set_game_overlay_debug(app, format!("controls_open={open}"));
    Ok(open)
}

#[cfg(target_os = "windows")]
fn force_game_overlay_topmost(window: &tauri::WebviewWindow) -> Result<(), String> {
    let hwnd = window.hwnd().map_err(|e| e.to_string())?;
    let ok = unsafe {
        SetWindowPos(
            hwnd.0 as HWND,
            HWND_TOPMOST,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        )
    };
    if ok == 0 {
        return Err("SetWindowPos(HWND_TOPMOST) failed".to_string());
    }
    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn force_game_overlay_topmost(_window: &tauri::WebviewWindow) -> Result<(), String> {
    Ok(())
}

fn hide_game_overlay_window(app: &tauri::AppHandle, close_controls: bool) {
    let state = app.state::<GameOverlayState>();
    if !close_controls && !state.window_visible.load(Ordering::Relaxed) {
        return;
    }
    if let Some(window) = app.get_webview_window(GAME_OVERLAY_WINDOW_LABEL) {
        if close_controls {
            state.controls_open.store(false, Ordering::Relaxed);
            let _ = apply_game_overlay_interaction(&window, false);
            let _ = app.emit_to(
                GAME_OVERLAY_WINDOW_LABEL,
                "overlay://controls-open-changed",
                false,
            );
        }
        let _ = window.hide();
        if state.window_visible.swap(false, Ordering::Relaxed) {
            set_game_overlay_debug(
                app,
                format!("window hidden close_controls={close_controls}"),
            );
        }
    }
}

fn hide_game_overlay(app: &tauri::AppHandle) {
    if app
        .state::<GameOverlayState>()
        .controls_open
        .load(Ordering::Relaxed)
    {
        set_game_overlay_debug(app, "hide skipped because overlay controls are open");
        return;
    }
    #[cfg(target_os = "windows")]
    if find_lethal_company_window(&HashSet::new()).is_some() {
        set_game_overlay_debug(
            app,
            "hide skipped because Lethal Company window still exists",
        );
        return;
    }
    hide_game_overlay_window(app, true);
}

fn set_game_overlay_debug(app: &tauri::AppHandle, message: impl Into<String>) {
    let message = message.into();
    log::info!("game overlay: {message}");
    let state = app.state::<GameOverlayState>();
    {
        if let Ok(mut last_message) = state.last_message.lock() {
            *last_message = message;
        };
    }
}

fn set_game_overlay_error(app: &tauri::AppHandle, message: impl Into<String>) {
    let message = message.into();
    log::error!("game overlay: {message}");
    let state = app.state::<GameOverlayState>();
    {
        if let Ok(mut last_error) = state.last_error.lock() {
            *last_error = Some(message.clone());
        };
    }
    {
        if let Ok(mut last_message) = state.last_message.lock() {
            *last_message = message;
        };
    }
}

fn game_overlay_debug_status(state: &GameOverlayState) -> GameOverlayDebugStatus {
    GameOverlayDebugStatus {
        controls_open: state.controls_open.load(Ordering::Relaxed),
        monitor_running: state.monitor_running.load(Ordering::Relaxed),
        last_foreground_pid: state.last_foreground_pid.load(Ordering::Relaxed),
        last_match: state.last_match.load(Ordering::Relaxed),
        show_count: state.show_count.load(Ordering::Relaxed),
        last_message: state
            .last_message
            .lock()
            .map(|message| message.clone())
            .unwrap_or_default(),
        last_error: state
            .last_error
            .lock()
            .map(|error| error.clone())
            .unwrap_or_else(|_| Some("overlay debug lock failed".to_string())),
    }
}

fn start_stream_overlays_monitor(app: &tauri::AppHandle) {
    let state = app.state::<GameOverlayState>();
    if state
        .stream_overlays_monitor_running
        .swap(true, Ordering::Relaxed)
    {
        return;
    }

    let app_handle = app.clone();
    tauri::async_runtime::spawn(async move {
        loop {
            let enabled = read_game_overlay_config(&app_handle)
                .map(|cfg| cfg.general.use_stream_overlays_api && cfg.general.enabled)
                .unwrap_or(false);
            if !enabled {
                app_handle
                    .state::<GameOverlayState>()
                    .stream_overlays_monitor_running
                    .store(false, Ordering::Relaxed);
                break;
            }

            match discord_presence::receive_stream_overlays_data() {
                Some(payload) => {
                    set_game_overlay_debug(&app_handle, "stream overlays payload received");
                    let _ = app_handle.emit_to(
                        GAME_OVERLAY_WINDOW_LABEL,
                        "overlay://stream-overlays-updated",
                        &payload,
                    );
                }
                None => {
                    set_game_overlay_debug(&app_handle, "stream overlays payload unavailable");
                    let _ = app_handle.emit_to(
                        GAME_OVERLAY_WINDOW_LABEL,
                        "overlay://stream-overlays-log",
                        "payload unavailable",
                    );
                }
            }

            std::thread::sleep(std::time::Duration::from_millis(900));
        }
    });
}

#[cfg(target_os = "windows")]
fn foreground_window_pid_and_rect() -> Option<(u32, RECT)> {
    let hwnd = unsafe { GetForegroundWindow() };
    if hwnd.is_null() {
        return None;
    }

    let mut pid = 0u32;
    unsafe {
        GetWindowThreadProcessId(hwnd, &mut pid);
    }
    if pid == 0 {
        return None;
    }

    let mut rect = RECT {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    let ok = unsafe { GetWindowRect(hwnd, &mut rect) };
    if ok == 0 || rect.right <= rect.left || rect.bottom <= rect.top {
        return None;
    }

    Some((pid, rect))
}

#[cfg(target_os = "windows")]
fn windows_process_image_path(pid: u32) -> Option<std::path::PathBuf> {
    let handle = unsafe {
        OpenProcess(
            PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_QUERY_INFORMATION,
            0,
            pid,
        )
    };
    if handle.is_null() {
        return None;
    }

    let mut buffer = vec![0u16; 32_768];
    let mut size = buffer.len() as u32;
    let ok = unsafe { QueryFullProcessImageNameW(handle, 0, buffer.as_mut_ptr(), &mut size) };
    unsafe {
        CloseHandle(handle);
    }
    if ok == 0 || size == 0 {
        return None;
    }

    buffer.truncate(size as usize);
    Some(std::path::PathBuf::from(OsString::from_wide(&buffer)))
}

#[cfg(target_os = "windows")]
fn is_lethal_company_process(pid: u32) -> bool {
    windows_process_image_path(pid)
        .and_then(|path| {
            path.file_name()
                .map(|name| name.to_string_lossy().to_string())
        })
        .map(|name| name.eq_ignore_ascii_case("Lethal Company.exe"))
        .unwrap_or(false)
}

#[cfg(target_os = "windows")]
fn windows_window_title(hwnd: HWND) -> String {
    let len = unsafe { GetWindowTextLengthW(hwnd) };
    if len <= 0 {
        return String::new();
    }
    let mut buffer = vec![0u16; len as usize + 1];
    let written = unsafe { GetWindowTextW(hwnd, buffer.as_mut_ptr(), buffer.len() as i32) };
    if written <= 0 {
        return String::new();
    }
    buffer.truncate(written as usize);
    String::from_utf16_lossy(&buffer)
}

#[cfg(target_os = "windows")]
fn is_lethal_company_window_title(title: &str) -> bool {
    title.to_ascii_lowercase().contains("lethal company")
}

#[cfg(target_os = "windows")]
fn foreground_matches_game(foreground_pid: u32, active_pids: &HashSet<u32>) -> bool {
    active_pids.contains(&foreground_pid) || is_lethal_company_process(foreground_pid)
}

#[cfg(target_os = "windows")]
struct FindLethalCompanyWindowState {
    active_pids: HashSet<u32>,
    result: Option<(u32, RECT)>,
}

#[cfg(target_os = "windows")]
unsafe extern "system" fn enum_lethal_company_window(hwnd: HWND, lparam: LPARAM) -> i32 {
    if unsafe { IsWindowVisible(hwnd) } == 0 {
        return 1;
    }

    let mut pid = 0u32;
    unsafe {
        GetWindowThreadProcessId(hwnd, &mut pid);
    }
    if pid == 0 {
        return 1;
    }

    let title = windows_window_title(hwnd);
    let matches = state_matches_window_pid_or_title(pid, &title, unsafe {
        &*(lparam as *const FindLethalCompanyWindowState)
    });
    if !matches {
        return 1;
    }

    let mut rect = RECT {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    let ok = unsafe { GetWindowRect(hwnd, &mut rect) };
    if ok == 0 || rect.right <= rect.left || rect.bottom <= rect.top {
        return 1;
    }

    let state = unsafe { &mut *(lparam as *mut FindLethalCompanyWindowState) };
    state.result = Some((pid, rect));
    0
}

#[cfg(target_os = "windows")]
fn state_matches_window_pid_or_title(
    pid: u32,
    title: &str,
    state: &FindLethalCompanyWindowState,
) -> bool {
    state.active_pids.contains(&pid)
        || is_lethal_company_window_title(title)
        || is_lethal_company_process(pid)
}

#[cfg(target_os = "windows")]
fn find_lethal_company_window(active_pids: &HashSet<u32>) -> Option<(u32, RECT)> {
    let mut state = FindLethalCompanyWindowState {
        active_pids: active_pids.clone(),
        result: None,
    };
    unsafe {
        EnumWindows(
            Some(enum_lethal_company_window),
            &mut state as *mut FindLethalCompanyWindowState as LPARAM,
        );
    }
    state.result
}

#[cfg(target_os = "windows")]
fn overlay_target_window(active_pids: &HashSet<u32>) -> Option<(u32, RECT)> {
    if let Some((foreground_pid, rect)) = foreground_window_pid_and_rect() {
        if foreground_matches_game(foreground_pid, active_pids) {
            return Some((foreground_pid, rect));
        }
    }
    None
}

#[cfg(target_os = "windows")]
fn foreground_lethal_company_window() -> Option<(u32, RECT)> {
    let (foreground_pid, rect) = foreground_window_pid_and_rect()?;
    if is_lethal_company_process(foreground_pid) {
        Some((foreground_pid, rect))
    } else {
        None
    }
}

#[cfg(target_os = "windows")]
fn show_game_overlay_attached_to_rect(
    app: &tauri::AppHandle,
    overlay_state: &GameOverlayState,
    controls_open: bool,
    target_pid: u32,
    rect: RECT,
    source: &str,
) {
    let Ok(window) = ensure_game_overlay_window(app) else {
        set_game_overlay_error(app, "failed to create overlay window in monitor");
        return;
    };
    let width = (rect.right - rect.left).max(1) as u32;
    let height = (rect.bottom - rect.top).max(1) as u32;
    let was_visible = overlay_state.window_visible.load(Ordering::Relaxed);

    if let Err(e) = window.set_fullscreen(false) {
        set_game_overlay_error(app, format!("{source} set_fullscreen(false) failed: {e}"));
    }
    if let Err(e) = window.set_position(tauri::PhysicalPosition::new(rect.left, rect.top)) {
        set_game_overlay_error(app, format!("{source} set_position failed: {e}"));
    }
    if let Err(e) = window.set_size(tauri::PhysicalSize::new(width, height)) {
        set_game_overlay_error(app, format!("{source} set_size failed: {e}"));
    }

    if !was_visible || controls_open {
        if let Err(e) = window.set_always_on_top(true) {
            set_game_overlay_error(app, format!("{source} set_always_on_top failed: {e}"));
        }
        if let Err(e) = force_game_overlay_topmost(&window) {
            set_game_overlay_error(app, format!("{source} force topmost failed: {e}"));
        }
        if let Err(e) = apply_game_overlay_interaction(&window, controls_open) {
            set_game_overlay_error(app, format!("{source} apply interaction failed: {e}"));
        }
        if let Err(e) = window.show() {
            set_game_overlay_error(app, format!("{source} show failed: {e}"));
            return;
        }
    }

    overlay_state.window_visible.store(true, Ordering::Relaxed);
    overlay_state
        .last_foreground_pid
        .store(target_pid, Ordering::Relaxed);
    overlay_state.last_match.store(true, Ordering::Relaxed);
    if !was_visible || controls_open {
        let count = overlay_state.show_count.fetch_add(1, Ordering::Relaxed) + 1;
        if count <= 6 || controls_open {
            set_game_overlay_debug(
                app,
                format!(
                    "{source} window shown pid={target_pid} rect=({},{} {}x{}) controls_open={controls_open} count={count}",
                    rect.left, rect.top, width, height
                ),
            );
        }
    }
}

#[cfg(target_os = "windows")]
fn start_game_overlay_monitor(app: &tauri::AppHandle) {
    let overlay_state = app.state::<GameOverlayState>();
    if overlay_state.monitor_running.swap(true, Ordering::Relaxed) {
        return;
    }
    set_game_overlay_debug(app, "monitor started");

    let app_handle = app.clone();
    std::thread::spawn(move || loop {
        std::thread::sleep(std::time::Duration::from_millis(80));

        let overlay_state = app_handle.state::<GameOverlayState>();
        if !overlay_state.monitor_running.load(Ordering::Relaxed) {
            break;
        }
        if !overlay_state.overlay_enabled.load(Ordering::Relaxed) {
            hide_game_overlay_window(&app_handle, true);
            continue;
        }

        let active_pids: HashSet<u32> = app_handle
            .state::<GameState>()
            .active
            .lock()
            .map(|active_games| {
                active_games
                    .iter()
                    .map(|active| active.child.id())
                    .collect()
            })
            .unwrap_or_default();

        let controls_open = overlay_state.controls_open.load(Ordering::Relaxed);
        if active_pids.is_empty() {
            overlay_state.last_match.store(false, Ordering::Relaxed);
            if overlay_state.preview_mode.load(Ordering::Relaxed) {
                if controls_open {
                    continue;
                }
                if let Some((target_pid, rect)) = foreground_lethal_company_window() {
                    show_game_overlay_attached_to_rect(
                        &app_handle,
                        overlay_state.inner(),
                        controls_open,
                        target_pid,
                        rect,
                        "preview",
                    );
                    continue;
                }
                hide_game_overlay_window(&app_handle, false);
                continue;
            }
            hide_game_overlay_window(&app_handle, true);
            continue;
        }
        let target_window = overlay_target_window(&active_pids);
        let Some((foreground_pid, rect)) = target_window else {
            overlay_state.last_match.store(false, Ordering::Relaxed);
            if !controls_open {
                hide_game_overlay_window(&app_handle, false);
            }
            continue;
        };
        overlay_state
            .last_foreground_pid
            .store(foreground_pid, Ordering::Relaxed);

        if controls_open && foreground_pid == std::process::id() {
            continue;
        }

        overlay_state.last_match.store(true, Ordering::Relaxed);

        show_game_overlay_attached_to_rect(
            &app_handle,
            overlay_state.inner(),
            controls_open,
            foreground_pid,
            rect,
            "monitor",
        );
    });
}

#[cfg(not(target_os = "windows"))]
fn start_game_overlay_monitor(app: &tauri::AppHandle) {
    if !app
        .state::<GameOverlayState>()
        .overlay_enabled
        .load(Ordering::Relaxed)
    {
        hide_game_overlay_window(app, true);
        return;
    }
    match ensure_game_overlay_window(app) {
        Ok(window) => {
            let _ = window.set_fullscreen(true);
            let _ = window.set_always_on_top(true);
            let _ = window.show();
        }
        Err(e) => log::warn!("{e}"),
    }
}

fn show_game_overlay(app: &tauri::AppHandle) {
    if !app
        .state::<GameOverlayState>()
        .overlay_enabled
        .load(Ordering::Relaxed)
    {
        hide_game_overlay_window(app, true);
        return;
    }
    if let Err(e) = ensure_game_overlay_window(app) {
        log::warn!("{e}");
        return;
    }
    start_game_overlay_monitor(app);
}

fn should_emit_game_overlay_input(app: &tauri::AppHandle) -> bool {
    let state = app.state::<GameOverlayState>();
    if state.controls_open.load(Ordering::Relaxed) || state.preview_mode.load(Ordering::Relaxed) {
        return true;
    }

    #[cfg(target_os = "windows")]
    {
        let active_pids: HashSet<u32> = app
            .state::<GameState>()
            .active
            .lock()
            .map(|active_games| {
                active_games
                    .iter()
                    .map(|active| active.child.id())
                    .collect()
            })
            .unwrap_or_default();
        return overlay_target_window(&active_pids).is_some()
            || foreground_lethal_company_window().is_some();
    }

    #[cfg(not(target_os = "windows"))]
    {
        true
    }
}

fn emit_game_overlay_input_shortcut(
    app: &tauri::AppHandle,
    shortcut_label: &str,
    state: ShortcutState,
    source: &str,
) {
    if !should_emit_game_overlay_input(app) {
        return;
    }
    let event_state = match state {
        ShortcutState::Pressed => "Pressed",
        ShortcutState::Released => "Released",
    };
    let payload = GameOverlayInputShortcutPayload {
        id: app
            .state::<GameOverlayState>()
            .input_count
            .fetch_add(1, Ordering::Relaxed),
        shortcut: shortcut_label.to_string(),
        state: event_state.to_string(),
        source: source.to_string(),
    };
    let _ = app.emit_to(
        GAME_OVERLAY_WINDOW_LABEL,
        "overlay://input-shortcut",
        payload,
    );
}

fn handle_game_overlay_shortcut(app: &tauri::AppHandle, shortcut_label: &str) {
    if !app
        .state::<GameOverlayState>()
        .overlay_enabled
        .load(Ordering::Relaxed)
    {
        set_game_overlay_debug(
            app,
            "overlay shortcut ignored because HQLC overlay is disabled",
        );
        return;
    }
    let active_pids: HashSet<u32> = app
        .state::<GameState>()
        .active
        .lock()
        .map(|active_games| {
            active_games
                .iter()
                .map(|active| active.child.id())
                .collect()
        })
        .unwrap_or_default();
    if active_pids.is_empty() {
        let state = app.state::<GameOverlayState>();
        if !state.preview_mode.load(Ordering::Relaxed) {
            set_game_overlay_debug(
                app,
                "overlay shortcut ignored because no launched Lethal Company process is active",
            );
            return;
        }
        #[cfg(target_os = "windows")]
        {
            if !state.controls_open.load(Ordering::Relaxed)
                && foreground_lethal_company_window().is_none()
            {
                set_game_overlay_debug(
                    app,
                    "overlay shortcut ignored because Lethal Company is not focused",
                );
                return;
            }
        }
        let next_open = !state.controls_open.load(Ordering::Relaxed);
        if let Err(e) = set_game_overlay_controls_open_inner(app, state.inner(), next_open) {
            log::warn!("Failed to toggle game overlay controls: {e}");
        }
        return;
    }
    #[cfg(target_os = "windows")]
    {
        let Some((target_pid, _)) = overlay_target_window(&active_pids) else {
            let foreground = foreground_window_pid_and_rect()
                .map(|(pid, _)| {
                    let image = windows_process_image_path(pid)
                        .map(|path| path.to_string_lossy().to_string())
                        .unwrap_or_else(|| "unknown".to_string());
                    format!("foreground pid={pid}, image={image}")
                })
                .unwrap_or_else(|| "no foreground window".to_string());
            set_game_overlay_error(
                app,
                format!("{shortcut_label} pressed but no Lethal Company window was found ({foreground})"),
            );
            return;
        };
        set_game_overlay_debug(
            app,
            format!("{shortcut_label} accepted for Lethal Company pid={target_pid}"),
        );
    }
    let state = app.state::<GameOverlayState>();
    let next_open = !state.controls_open.load(Ordering::Relaxed);
    if let Err(e) = set_game_overlay_controls_open_inner(app, state.inner(), next_open) {
        log::warn!("Failed to toggle game overlay controls: {e}");
    }
}

fn register_game_overlay_shortcut(app: &tauri::AppHandle) {
    let shortcut_label = read_game_overlay_config(app)
        .map(|cfg| cfg.general.overlay_key)
        .unwrap_or_else(|_| "Insert".to_string());
    let shortcuts = overlay_shortcuts_for_key(&shortcut_label);
    let state = app.state::<GameOverlayState>();
    let mut registered = state.registered_overlay_shortcut.lock().unwrap();

    if registered
        .as_ref()
        .map(|current| current.eq_ignore_ascii_case(&shortcut_label))
        .unwrap_or(false)
    {
        return;
    }

    if let Some(previous_label) = registered.take() {
        let previous_shortcuts = overlay_shortcuts_for_key(&previous_label);
        if !previous_shortcuts.is_empty() {
            if let Err(e) = app
                .global_shortcut()
                .unregister_multiple(previous_shortcuts)
            {
                log::warn!("Failed to unregister previous overlay shortcut {previous_label}: {e}");
            }
        }
    }

    if shortcuts.is_empty() {
        set_game_overlay_error(
            app,
            format!("Unsupported overlay shortcut: {shortcut_label}"),
        );
        return;
    }

    let shortcut_label_for_handler = shortcut_label.clone();
    match app
        .global_shortcut()
        .on_shortcuts(shortcuts, move |app, _shortcut, event| {
            emit_game_overlay_input_shortcut(
                app,
                &shortcut_label_for_handler,
                event.state,
                "overlay-key",
            );
            if event.state != ShortcutState::Pressed {
                return;
            }
            handle_game_overlay_shortcut(app, &shortcut_label_for_handler);
        }) {
        Ok(()) => {
            set_game_overlay_debug(app, format!("registered overlay shortcut {shortcut_label}"));
            *registered = Some(shortcut_label);
        }
        Err(e) => {
            set_game_overlay_error(
                app,
                format!("Failed to register overlay shortcut {shortcut_label}: {e}"),
            );
        }
    }
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

fn add_disabled_mod(list: &mut DisableModFile, dev: &str, name: &str) -> bool {
    let id = normalize_mod_id(dev, name);
    if list.mods.iter().any(|m| m == &id) {
        return false;
    }
    list.mods.push(id);
    list.mods
        .sort_by(|a, b| a.dev.cmp(&b.dev).then(a.name.cmp(&b.name)));
    list.mods.dedup();
    true
}

fn manifest_disabled_base_mods_for_version(
    mods_cfg: &ModsConfig,
    version: u32,
) -> Vec<(String, String)> {
    mods_cfg
        .mods
        .iter()
        .filter(|m| !m.enabled && m.tags.is_empty() && m.is_install_compatible(version))
        .map(|m| (m.dev.clone(), m.name.clone()))
        .collect()
}

pub(crate) fn ensure_manifest_disabled_mods_disabled_for_version(
    app: &tauri::AppHandle,
    version: u32,
    mods_cfg: &ModsConfig,
) -> Result<(), String> {
    let disabled_mods = manifest_disabled_base_mods_for_version(mods_cfg, version);
    if !disabled_mods.is_empty() {
        let mut list = read_disablemod(app)?;
        let mut changed = false;
        for (dev, name) in &disabled_mods {
            let id = normalize_mod_id(dev, name);
            if list.mods.iter().any(|m| m == &id) {
                continue;
            }
            list.mods.push(id);
            changed = true;
        }
        if changed {
            list.mods
                .sort_by(|a, b| a.dev.cmp(&b.dev).then(a.name.cmp(&b.name)));
            list.mods.dedup();
            write_disablemod(app, &list)?;
        }
    }

    apply_effective_mod_states_for_version(app, version, &[], &[])
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

fn practice_mode_forced_enabled_ids() -> Vec<(String, String)> {
    vec![]
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
        let game_root = version_dir(&app, version)?;
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
    let dir = storage::versions_dir(&app)?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("failed to create versions dir: {e}"))?;
    open_folder_path(&dir)?;
    Ok(true)
}

#[tauri::command]
fn get_game_storage_settings(
    app: tauri::AppHandle,
) -> Result<storage::GameStorageSettings, String> {
    storage::game_storage_settings(&app)
}

fn same_storage_path(a: &std::path::Path, b: &std::path::Path) -> bool {
    if let (Ok(a), Ok(b)) = (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        return a == b;
    }
    a == b
}

fn ensure_storage_can_move(
    app: &tauri::AppHandle,
    game_state: &State<'_, GameState>,
    download_state: &State<'_, DownloadState>,
    prepare_state: &State<'_, PrepareState>,
) -> Result<(), String> {
    {
        let mut guard = game_state
            .active
            .lock()
            .map_err(|_| "game state lock poisoned".to_string())?;
        for active in guard.iter_mut() {
            if active
                .child
                .try_wait()
                .map_err(|e| e.to_string())?
                .is_none()
                || linux_lingering_game_pid(app, active.version).is_some()
            {
                return Err("Cannot change game storage while the game is running.".to_string());
            }
        }
        guard.clear();
    }

    {
        let guard = download_state
            .active
            .lock()
            .map_err(|_| "download state lock poisoned".to_string())?;
        if guard
            .as_ref()
            .is_some_and(|active| !active.cancel.load(Ordering::Relaxed))
        {
            return Err("Cannot change game storage while a download is running.".to_string());
        }
    }

    {
        let guard = prepare_state
            .active
            .lock()
            .map_err(|_| "prepare state lock poisoned".to_string())?;
        if guard
            .as_ref()
            .is_some_and(|active| !active.cancel.load(Ordering::Relaxed))
        {
            return Err("Cannot change game storage while a preset is being prepared.".to_string());
        }
    }

    Ok(())
}

#[tauri::command]
fn pick_game_storage_dir(initial_path: Option<String>) -> Result<Option<String>, String> {
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
fn set_game_storage_dir(
    app: tauri::AppHandle,
    custom_dir: Option<String>,
    game_state: State<'_, GameState>,
    download_state: State<'_, DownloadState>,
    prepare_state: State<'_, PrepareState>,
) -> Result<storage::GameStorageSettings, String> {
    ensure_storage_can_move(&app, &game_state, &download_state, &prepare_state)?;

    let normalized_custom = custom_dir
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .map(std::path::PathBuf::from);
    let linked_versions: Vec<u32> = list_installed_versions(app.clone())?
        .into_iter()
        .filter(|version| {
            installer::get_config_link_state_for_version(&app, *version)
                .map(|state| state.is_linked)
                .unwrap_or(false)
        })
        .collect();
    let old_versions_dir = storage::versions_dir(&app)?;
    let new_versions_dir = storage::versions_dir_for_custom(&app, normalized_custom.clone())?;

    if !same_storage_path(&old_versions_dir, &new_versions_dir) {
        storage::move_versions_dir(
            &old_versions_dir,
            &new_versions_dir,
            |done, total, detail| {
                let total = total.max(1);
                let step_progress = (done as f64 / total as f64).clamp(0.0, 1.0);
                progress::emit_progress(
                    &app,
                    TaskProgressPayload {
                        version: 0,
                        steps_total: 1,
                        step: 1,
                        step_name: "Move Storage".to_string(),
                        step_progress,
                        overall_percent: step_progress * 100.0,
                        detail,
                        downloaded_bytes: None,
                        total_bytes: None,
                        extracted_files: Some(done),
                        total_files: Some(total),
                    },
                );
            },
        )?;
    } else {
        std::fs::create_dir_all(&new_versions_dir).map_err(|e| e.to_string())?;
        progress::emit_progress(
            &app,
            TaskProgressPayload {
                version: 0,
                steps_total: 1,
                step: 1,
                step_name: "Move Storage".to_string(),
                step_progress: 1.0,
                overall_percent: 100.0,
                detail: Some("Storage folder is ready".to_string()),
                downloaded_bytes: None,
                total_bytes: None,
                extracted_files: Some(1),
                total_files: Some(1),
            },
        );
    }

    let settings = storage::set_game_storage_dir(&app, normalized_custom)?;
    for version in linked_versions {
        installer::link_config_for_version(&app, version)?;
    }
    let _ = app.emit("game-storage://changed", &settings);
    Ok(settings)
}

#[tauri::command]
async fn open_custom_layout_docs() -> Result<bool, String> {
    opener::open("https://github.com/P-Asta/hq-launcher/blob/main/docs/CUSTOM_LAYOUT.md")
        .map_err(|e| format!("failed to open Custom Layout docs: {e}"))?;
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
            "kate", "kwrite", "gedit", "mousepad", "xed", "code", "codium",
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
                && active
                    .child
                    .try_wait()
                    .map_err(|e| e.to_string())?
                    .is_none()
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

    let extract_dir = version_dir(&app, version)?;
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

        let game_root = version_dir(&app, version)?;
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

fn ensure_game_not_running(
    app: &tauri::AppHandle,
    state: &State<'_, GameState>,
) -> Result<(), String> {
    let mut guard = state
        .active
        .lock()
        .map_err(|_| "game state lock poisoned".to_string())?;
    let mut has_running = false;
    for active in guard.iter_mut() {
        if active
            .child
            .try_wait()
            .map_err(|e| e.to_string())?
            .is_none()
        {
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
fn non_verbatim_windows_path(path: &std::path::Path) -> std::path::PathBuf {
    let text = path.to_string_lossy();
    if let Some(stripped) = text.strip_prefix(r"\\?\UNC\") {
        return std::path::PathBuf::from(format!(r"\\{stripped}"));
    }
    if let Some(stripped) = text.strip_prefix(r"\\?\") {
        return std::path::PathBuf::from(stripped);
    }
    path.to_path_buf()
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

        let matches_version =
            bytes_contain(&cmdline, &game_root) || bytes_contain(&environ, &game_root);
        let matches_prefix = compat_prefix.as_ref().is_some_and(|prefix| {
            bytes_contain(&cmdline, prefix) || bytes_contain(&environ, prefix)
        }) || compat_env.as_ref().is_some_and(|env| {
            environ
                .windows(env.len())
                .any(|window| window == env.as_bytes())
        });

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

        let launch_exe_path = non_verbatim_windows_path(exe_path);
        let default_program = launch_exe_path.as_os_str().to_os_string();
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
    #[cfg(target_os = "windows")]
    let launch_current_dir = non_verbatim_windows_path(exe_dir);
    #[cfg(not(target_os = "windows"))]
    let launch_current_dir = exe_dir.to_path_buf();

    #[allow(unused_mut)]
    let mut child = command
        .current_dir(&launch_current_dir)
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
fn get_game_overlay_config(app: tauri::AppHandle) -> Result<GameOverlayConfig, String> {
    let cfg = read_game_overlay_config(&app)?;
    sync_game_overlay_enabled_state(&app, cfg.general.enabled);
    Ok(cfg)
}

#[tauri::command]
fn get_game_overlay_modules(app: tauri::AppHandle) -> Result<Vec<GameOverlayModuleDto>, String> {
    read_game_overlay_modules(&app)
}

#[tauri::command]
fn get_game_overlay_debug_status(
    state: State<'_, GameOverlayState>,
) -> Result<GameOverlayDebugStatus, String> {
    Ok(game_overlay_debug_status(&state))
}

#[tauri::command]
fn get_stream_overlays_data(
    app: tauri::AppHandle,
) -> Result<Option<discord_presence::StreamOverlaysEnvelope>, String> {
    if !read_game_overlay_config(&app)?
        .general
        .use_stream_overlays_api
    {
        return Ok(None);
    }
    Ok(discord_presence::receive_stream_overlays_data())
}

#[tauri::command]
fn get_stream_overlays_ws_url(app: tauri::AppHandle) -> Result<Option<String>, String> {
    if !read_game_overlay_config(&app)?
        .general
        .use_stream_overlays_api
    {
        return Ok(None);
    }
    Ok(Some(discord_presence::stream_overlays_ws_url()))
}

#[tauri::command]
fn report_game_overlay_frontend_ready(app: tauri::AppHandle) -> Result<(), String> {
    set_game_overlay_debug(&app, "frontend ready");
    Ok(())
}

#[tauri::command]
fn report_game_overlay_frontend_info(app: tauri::AppHandle, message: String) -> Result<(), String> {
    set_game_overlay_debug(&app, format!("frontend info: {message}"));
    Ok(())
}

#[tauri::command]
fn report_game_overlay_frontend_error(
    app: tauri::AppHandle,
    message: String,
) -> Result<(), String> {
    set_game_overlay_error(&app, format!("frontend error: {message}"));
    Ok(())
}

#[tauri::command]
fn set_game_overlay_config(
    app: tauri::AppHandle,
    config: GameOverlayConfig,
) -> Result<GameOverlayConfig, String> {
    let cfg = sanitize_game_overlay_config(config);
    write_game_overlay_config(&app, &cfg)?;
    sync_game_overlay_enabled_state(&app, cfg.general.enabled);
    if !cfg.general.enabled {
        app.state::<GameOverlayState>()
            .preview_mode
            .store(false, Ordering::Relaxed);
        hide_game_overlay_window(&app, true);
    } else {
        start_game_overlay_monitor(&app);
        if cfg.general.use_stream_overlays_api {
            start_stream_overlays_monitor(&app);
        }
    }
    register_game_overlay_shortcut(&app);
    let _ = app.emit_to(
        GAME_OVERLAY_WINDOW_LABEL,
        "overlay://config-changed",
        cfg.clone(),
    );
    Ok(cfg)
}

#[tauri::command]
fn toggle_game_overlay_only(
    app: tauri::AppHandle,
    state: State<'_, GameOverlayState>,
) -> Result<bool, String> {
    if !state.overlay_enabled.load(Ordering::Relaxed) {
        hide_game_overlay_window(&app, true);
        set_game_overlay_debug(
            &app,
            "overlay only ignored because HQLC overlay is disabled",
        );
        return Ok(false);
    }
    if state.preview_mode.load(Ordering::Relaxed) {
        state.preview_mode.store(false, Ordering::Relaxed);
        state.controls_open.store(false, Ordering::Relaxed);
        hide_game_overlay_window(&app, true);
        let _ = app.emit_to(
            GAME_OVERLAY_WINDOW_LABEL,
            "overlay://controls-open-changed",
            false,
        );
        set_game_overlay_debug(&app, "overlay only toggled off");
        return Ok(false);
    }

    state.preview_mode.store(true, Ordering::Relaxed);
    state.controls_open.store(false, Ordering::Relaxed);
    let _ = app.emit_to(
        GAME_OVERLAY_WINDOW_LABEL,
        "overlay://controls-open-changed",
        false,
    );
    set_game_overlay_debug(
        &app,
        "open overlay only requested; waiting for Lethal Company window",
    );
    start_game_overlay_monitor(&app);
    Ok(true)
}

#[tauri::command]
fn open_game_overlay_only(
    app: tauri::AppHandle,
    state: State<'_, GameOverlayState>,
) -> Result<bool, String> {
    state.preview_mode.store(false, Ordering::Relaxed);
    toggle_game_overlay_only(app, state)
}

#[tauri::command]
fn close_game_overlay_only(
    app: tauri::AppHandle,
    state: State<'_, GameOverlayState>,
) -> Result<bool, String> {
    state.preview_mode.store(false, Ordering::Relaxed);
    state.controls_open.store(false, Ordering::Relaxed);
    hide_game_overlay_window(&app, true);
    let _ = app.emit_to(
        GAME_OVERLAY_WINDOW_LABEL,
        "overlay://controls-open-changed",
        false,
    );
    set_game_overlay_debug(&app, "overlay only closed");
    Ok(true)
}

#[tauri::command]
fn open_game_overlay_modules_folder(app: tauri::AppHandle) -> Result<bool, String> {
    ensure_default_game_overlay_modules(&app)?;
    let dir = game_overlay_module_dir(&app)?;
    open_folder_path(&dir)?;
    Ok(true)
}

#[tauri::command]
fn set_game_overlay_controls_open(
    app: tauri::AppHandle,
    state: State<'_, GameOverlayState>,
    open: bool,
) -> Result<bool, String> {
    set_game_overlay_controls_open_inner(&app, &state, open)
}

#[tauri::command]
fn set_game_overlay_input_shortcuts(
    app: tauri::AppHandle,
    state: State<'_, GameOverlayState>,
    shortcuts: Vec<String>,
) -> Result<bool, String> {
    let overlay_key = read_game_overlay_config(&app)
        .map(|cfg| cfg.general.overlay_key)
        .unwrap_or_else(|_| "Insert".to_string());
    let next: HashSet<String> = shortcuts
        .into_iter()
        .map(|shortcut| sanitize_overlay_key(&shortcut))
        .filter(|shortcut| !shortcut.is_empty())
        .filter(|shortcut| !shortcut.eq_ignore_ascii_case(&overlay_key))
        .collect();

    let mut registered = state.registered_overlay_input_shortcuts.lock().unwrap();
    let previous = registered.clone();

    for shortcut in previous.difference(&next) {
        let parsed = overlay_shortcuts_for_key(shortcut);
        if !parsed.is_empty() {
            if let Err(e) = app.global_shortcut().unregister_multiple(parsed) {
                log::warn!("Failed to unregister overlay input shortcut {shortcut}: {e}");
            }
        }
    }

    for shortcut in next.difference(&previous) {
        let parsed = overlay_shortcuts_for_key(shortcut);
        if parsed.is_empty() {
            log::warn!("Unsupported overlay input shortcut: {shortcut}");
            continue;
        }
        let shortcut_for_handler = shortcut.clone();
        if let Err(e) = app
            .global_shortcut()
            .on_shortcuts(parsed, move |app, _shortcut, event| {
                emit_game_overlay_input_shortcut(
                    app,
                    &shortcut_for_handler,
                    event.state,
                    "module-key",
                );
            })
        {
            log::warn!("Failed to register overlay input shortcut {shortcut}: {e}");
        }
    }

    *registered = next;
    Ok(true)
}

#[tauri::command]
fn show_game_overlay_end_summary(
    app: tauri::AppHandle,
    mut payload: GameOverlayEndSummaryPayload,
) -> Result<bool, String> {
    if !app
        .state::<GameOverlayState>()
        .overlay_enabled
        .load(Ordering::Relaxed)
    {
        return Ok(false);
    }
    if payload.duration_ms.is_none() {
        payload.duration_ms = Some(
            read_game_overlay_config(&app)?
                .general
                .end_summary_duration_ms,
        );
    }
    let window = ensure_game_overlay_window(&app)?;
    window.show().map_err(|e| e.to_string())?;
    window.set_always_on_top(true).map_err(|e| e.to_string())?;
    force_game_overlay_topmost(&window)?;
    apply_game_overlay_interaction(&window, false)?;
    app.emit_to(
        GAME_OVERLAY_WINDOW_LABEL,
        "overlay://show-end-summary",
        payload,
    )
    .map_err(|e| e.to_string())?;
    Ok(true)
}

#[tauri::command]
async fn launch_game(
    app: tauri::AppHandle,
    version: u32,
    launch_options: Option<Vec<String>>,
    launch_command_template: Option<String>,
    allow_multiple: Option<bool>,
    lcstats_state: State<'_, lcstats_autosheet::LcStatsAutosheetState>,
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
    let lcstats_enabled = is_lcstats_enabled(&app)?;
    lcstats_autosheet::start_for_launch(app.clone(), lcstats_enabled, &lcstats_state);
    show_game_overlay(&app);
    Ok(pid)
}

#[tauri::command]
async fn launch_game_practice(
    app: tauri::AppHandle,
    version: u32,
    launch_options: Option<Vec<String>>,
    launch_command_template: Option<String>,
    allow_multiple: Option<bool>,
    lcstats_state: State<'_, lcstats_autosheet::LcStatsAutosheetState>,
    state: State<'_, GameState>,
    prepare_state: State<'_, PrepareState>,
) -> Result<u32, String> {
    wait_for_prepare_to_finish(&prepare_state, version, std::time::Duration::from_secs(30))?;
    let (dir, exe_path, exe_dir) = resolve_game_launch_paths(&app, version)?;

    // Practice run: install + enable practice mods (compatible with this game version).
    let practice_ids = prepare_practice_mods_for_version(&app, version, None).await?;
    let mut forced_disabled_ids = practice_mode_forced_disabled_ids();
    forced_disabled_ids.extend(run_mode_tagged_mod_ids(version, None).await?);
    let mut forced_enabled_ids = practice_ids.clone();
    forced_enabled_ids.extend(practice_mode_forced_enabled_ids());

    // Practice mode state wins over the saved disabled list on launch.
    let _ = apply_effective_mod_states_for_version(
        &app,
        version,
        &forced_disabled_ids,
        &forced_enabled_ids,
    );
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
    let lcstats_enabled = is_lcstats_enabled(&app)?;
    lcstats_autosheet::start_for_launch(app.clone(), lcstats_enabled, &lcstats_state);
    show_game_overlay(&app);
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
    lcstats_state: State<'_, lcstats_autosheet::LcStatsAutosheetState>,
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

    let mut tagged_disabled_ids = run_mode_tagged_mod_ids(version, None).await?;
    allow_eclipsed_hq_optional_mods(&preset, &mut tagged_disabled_ids);
    let mut forced_enabled_ids = preset_ids.clone();
    forced_enabled_ids.extend(practice_ids.clone());
    if practice {
        forced_enabled_ids.extend(practice_mode_forced_enabled_ids());
    }
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
    let lcstats_enabled = is_lcstats_enabled(&app)?;
    lcstats_autosheet::start_for_launch(app.clone(), lcstats_enabled, &lcstats_state);
    show_game_overlay(&app);
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

    let mut tagged_disabled_ids = run_mode_tagged_mod_ids(version, Some(&cancel)).await?;
    allow_eclipsed_hq_optional_mods(preset, &mut tagged_disabled_ids);
    let mut forced_enabled_ids = preset_ids.clone();
    forced_enabled_ids.extend(practice_ids.clone());
    if practice {
        forced_enabled_ids.extend(practice_mode_forced_enabled_ids());
    }
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
    lcstats_state: State<'_, lcstats_autosheet::LcStatsAutosheetState>,
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
        lcstats_autosheet::stop(&lcstats_state);
        hide_game_overlay(&app);
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
    lcstats_state: State<'_, lcstats_autosheet::LcStatsAutosheetState>,
) -> Result<Vec<RunningGameDto>, String> {
    let mut guard = state
        .active
        .lock()
        .map_err(|_| "game state lock poisoned".to_string())?;
    let any_finished = cleanup_active_games(&app, &mut guard)?;
    if any_finished {
        if guard.is_empty() {
            lcstats_autosheet::stop(&lcstats_state);
            hide_game_overlay(&app);
        }
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
    lcstats_state: State<'_, lcstats_autosheet::LcStatsAutosheetState>,
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
        lcstats_autosheet::stop(&lcstats_state);
        hide_game_overlay(&app);
        terminate_linux_game_processes_for_version(&app, active.version);
        if let Err(e) = restore_hqol_wesley_dont_store_backup_if_present(&app) {
            log::warn!(
                "Failed to restore HQoL Wesley dont-store backup after stopping instance: {e}"
            );
        }
    }

    Ok(true)
}

#[tauri::command]
fn stop_game(
    app: tauri::AppHandle,
    state: State<'_, GameState>,
    lcstats_state: State<'_, lcstats_autosheet::LcStatsAutosheetState>,
) -> Result<bool, String> {
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
        lcstats_autosheet::stop(&lcstats_state);
        hide_game_overlay(&app);
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
    ensure_lcstats_disabled_without_google_auth(&app)?;
    Ok(read_disablemod(&app)?.mods)
}

fn is_lcstats_enabled(app: &tauri::AppHandle) -> Result<bool, String> {
    let disabled_keys = disabled_mod_keys(&read_disablemod(app)?.mods);
    Ok(!disabled_keys.contains(&normalize_mod_key("MikuOreo", "LCStatsTracker")))
}

fn disable_lcstats_in_disablemod(app: &tauri::AppHandle) -> Result<(), String> {
    let mut list = read_disablemod(app)?;
    if add_disabled_mod(&mut list, "MikuOreo", "LCStatsTracker") {
        write_disablemod(app, &list)?;
    }
    Ok(())
}

fn ensure_lcstats_disabled_without_google_auth(app: &tauri::AppHandle) -> Result<(), String> {
    if google_oauth::get_settings(app.clone())?.allow_without_google {
        return Ok(());
    }
    if google_oauth::auth_status(app.clone())?.authenticated {
        return Ok(());
    }
    disable_lcstats_in_disablemod(app)
}

#[tauri::command]
fn google_lcstats_auth_status(
    app: tauri::AppHandle,
) -> Result<google_oauth::GoogleLcStatsAuthState, String> {
    google_oauth::auth_status(app)
}

#[tauri::command]
async fn google_lcstats_start_oauth(
    app: tauri::AppHandle,
) -> Result<google_oauth::GoogleLcStatsAuthState, String> {
    google_oauth::start_oauth(app).await
}

#[tauri::command]
async fn google_lcstats_access_token(app: tauri::AppHandle) -> Result<String, String> {
    google_oauth::access_token(app).await
}

#[tauri::command]
fn google_lcstats_picker_config(
    app: tauri::AppHandle,
) -> Result<google_oauth::GooglePickerConfig, String> {
    google_oauth::picker_config(app)
}

#[tauri::command]
async fn google_lcstats_pick_spreadsheet(
    app: tauri::AppHandle,
    spreadsheet_id: String,
) -> Result<Option<google_oauth::GoogleSpreadsheetFile>, String> {
    google_oauth::pick_spreadsheet(app, spreadsheet_id).await
}

#[tauri::command]
fn google_lcstats_logout(app: tauri::AppHandle) -> Result<bool, String> {
    google_oauth::logout(app.clone())?;
    if !google_oauth::get_settings(app.clone())?.allow_without_google {
        disable_lcstats_in_disablemod(&app)?;
    }
    Ok(true)
}

#[tauri::command]
fn get_lcstats_settings(app: tauri::AppHandle) -> Result<google_oauth::LcStatsSettings, String> {
    google_oauth::get_settings(app)
}

#[tauri::command]
fn get_lcstats_autosheet_tracking(
    lcstats_state: State<'_, lcstats_autosheet::LcStatsAutosheetState>,
) -> Result<bool, String> {
    Ok(lcstats_autosheet::is_running(&lcstats_state))
}

#[tauri::command]
fn get_lcstats_latest_payload(
    app: tauri::AppHandle,
    lcstats_state: State<'_, lcstats_autosheet::LcStatsAutosheetState>,
) -> Result<Option<lcstats_autosheet::LatestLcStatsPayload>, String> {
    if !google_oauth::get_settings(app)?.use_lcstats_api {
        return Ok(None);
    }
    lcstats_autosheet::latest_payload(&lcstats_state)
}

#[tauri::command]
fn set_lcstats_autosheet_tracking(
    app: tauri::AppHandle,
    enabled: bool,
    lcstats_state: State<'_, lcstats_autosheet::LcStatsAutosheetState>,
) -> Result<bool, String> {
    if enabled {
        if !google_oauth::get_settings(app.clone())?.use_lcstats_api {
            lcstats_autosheet::stop(&lcstats_state);
            return Err("LCStatsTracker API use is disabled in launcher settings.".to_string());
        }
        lcstats_autosheet::start_manual(app, &lcstats_state)
    } else {
        lcstats_autosheet::stop(&lcstats_state);
        Ok(false)
    }
}

#[tauri::command]
fn set_lcstats_settings(
    app: tauri::AppHandle,
    lcstats_state: State<'_, lcstats_autosheet::LcStatsAutosheetState>,
    settings: google_oauth::LcStatsSettings,
) -> Result<bool, String> {
    if !settings.use_lcstats_api {
        lcstats_autosheet::stop(&lcstats_state);
    }
    google_oauth::set_settings(app, settings)
}

#[tauri::command]
async fn list_lcstats_sheet_names(
    app: tauri::AppHandle,
    spreadsheet_id: String,
) -> Result<Vec<String>, String> {
    google_oauth::list_sheet_names(app, spreadsheet_id).await
}

#[tauri::command]
async fn list_lcstats_sheet_infos(
    app: tauri::AppHandle,
    spreadsheet_id: String,
) -> Result<Vec<google_oauth::GoogleSheetInfo>, String> {
    google_oauth::list_sheet_infos(app, spreadsheet_id).await
}

#[tauri::command]
async fn list_lcstats_spreadsheets(
    app: tauri::AppHandle,
) -> Result<Vec<google_oauth::GoogleSpreadsheetFile>, String> {
    google_oauth::list_spreadsheets(app).await
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
    allow_without_google: Option<bool>,
) -> Result<bool, String> {
    let is_lcstats =
        dev.eq_ignore_ascii_case("MikuOreo") && name.eq_ignore_ascii_case("LCStatsTracker");
    let settings_allow_without_google = if is_lcstats {
        google_oauth::get_settings(app.clone())?.allow_without_google
    } else {
        false
    };
    let allow_without_google =
        allow_without_google.unwrap_or(false) || settings_allow_without_google;

    if enabled && is_lcstats && !allow_without_google {
        let status = google_oauth::auth_status(app.clone())?;
        if !status.authenticated {
            return Err("Google login is required to enable MikuOreo-LCStatsTracker.".to_string());
        }
    }

    if enabled && is_lcstats && allow_without_google && !settings_allow_without_google {
        let mut settings = google_oauth::get_settings(app.clone())?;
        settings.allow_without_google = true;
        google_oauth::set_settings(app.clone(), settings)?;
    }

    let mut list = read_disablemod(&app)?;

    // Use normalized ids in the file.
    let id = normalize_mod_id(&dev, &name);
    list.mods.retain(|m| m != &id);
    if !enabled {
        add_disabled_mod(&mut list, &dev, &name);
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
                    icon_path: find_mod_icon_src(&path),
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
    let base = storage::versions_dir(&app)?;

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
// ðŸ”¹ AUTO-UPDATE COMMANDS
// =========================

#[derive(Debug, Clone, Serialize)]
struct UpdateInfo {
    available: bool,
    current_version: String,
    version: Option<String>,
    date: Option<String>,
    body: Option<String>,
    channel: release_channel::ReleaseChannel,
}

#[tauri::command]
async fn check_app_update(app: tauri::AppHandle) -> Result<UpdateInfo, String> {
    use tauri_plugin_updater::UpdaterExt;
    let current_version_str = app.package_info().version.to_string();
    let channel = release_channel::current();
    let endpoint = channel
        .updater_url()
        .parse()
        .map_err(|e| format!("Failed to parse updater endpoint: {e}"))?;

    let updater = app
        .updater_builder()
        .endpoints(vec![endpoint])
        .map_err(|e| format!("Failed to configure updater endpoint: {e}"))?
        .version_comparator(move |current_version, remote| {
            remote.version > current_version
                || (channel == release_channel::ReleaseChannel::Stable
                    && remote.version != current_version)
        })
        .build()
        .map_err(|e| format!("Failed to initialize updater: {e}"))?;

    let update = updater
        .check()
        .await
        .map_err(|e| format!("Failed to check for updates: {e}"))?;

    Ok(UpdateInfo {
        available: update.is_some(),
        current_version: current_version_str,
        version: update.as_ref().map(|u| u.version.clone()),
        date: update
            .as_ref()
            .and_then(|u| u.date.map(|date| date.to_string())),
        body: update.and_then(|u| u.body),
        channel,
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
    let channel = release_channel::current();
    let endpoint = channel
        .updater_url()
        .parse()
        .map_err(|e| format!("Failed to parse updater endpoint: {e}"))?;

    // Tauri updater ì‚¬ìš© (ì—”ë“œí¬ì¸íŠ¸ëŠ” tauri.conf.jsonì—ì„œ ì„¤ì •, GitHub Releases latest.json)
    let updater = app
        .updater_builder()
        .endpoints(vec![endpoint])
        .map_err(|e| format!("Failed to configure updater endpoint: {e}"))?
        .version_comparator(move |current_version, remote| {
            remote.version > current_version
                || (channel == release_channel::ReleaseChannel::Stable
                    && remote.version != current_version)
        })
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
    let channel = release_channel::current();
    let endpoint = channel
        .updater_url()
        .parse()
        .map_err(|e| format!("Failed to parse updater endpoint: {e}"))?;

    // Tauri updater ì‚¬ìš© (ì—”ë“œí¬ì¸íŠ¸ëŠ” tauri.conf.jsonì—ì„œ ì„¤ì •, GitHub Releases latest.json)
    let updater = app
        .updater_builder()
        .endpoints(vec![endpoint])
        .map_err(|e| format!("Failed to configure updater endpoint: {e}"))?
        .version_comparator(move |current_version, remote| {
            remote.version > current_version
                || (channel == release_channel::ReleaseChannel::Stable
                    && remote.version != current_version)
        })
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

#[tauri::command]
fn get_release_channel(
    app: tauri::AppHandle,
) -> Result<release_channel::ReleaseChannelDto, String> {
    let channel = release_channel::load(&app)?;
    Ok(channel.into_dto())
}

#[tauri::command]
fn set_release_channel(
    app: tauri::AppHandle,
    channel: release_channel::ReleaseChannel,
) -> Result<release_channel::ReleaseChannelDto, String> {
    release_channel::save(&app, channel)?;
    Ok(channel.into_dto())
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
        .manage(GameOverlayState::default())
        .manage(discord_presence::DiscordPresenceState::default())
        .manage(lcstats_autosheet::LcStatsAutosheetState::default())
        .manage(downloader::DepotLoginState::default())
        .setup(|app| {
            // File logging (AppDataDir/logs/hq-launcher.log)
            logger::init(&app.handle()).map_err(|e| tauri::Error::Setup(e.into()))?;
            release_channel::load(&app.handle()).map_err(|e| {
                let err: Box<dyn std::error::Error> =
                    Box::new(std::io::Error::new(std::io::ErrorKind::Other, e));
                tauri::Error::Setup(err.into())
            })?;

            if let Ok(cfg) = read_game_overlay_config(app.handle()) {
                sync_game_overlay_enabled_state(app.handle(), cfg.general.enabled);
                if cfg.general.enabled && cfg.general.use_stream_overlays_api {
                    start_stream_overlays_monitor(app.handle());
                }
            }
            register_game_overlay_shortcut(app.handle());
            start_game_overlay_monitor(app.handle());

            // Startup housekeeping (best-effort, won't block UI):
            // - Disable installed base mods that remote manifest marks as enabled=false
            // - Ensure default config is downloaded if shared config dir is empty
            // - Ensure default overlay modules exist for user editing
            // - Run disablemod/security migrations when needed
            // - Warm the Thunderstore package cache for later update checks
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = ensure_default_game_overlay_modules(&app_handle) {
                    log::warn!("Failed to ensure default overlay modules on startup: {e}");
                }
                if let Err(e) = migrate_disablemod_v4_on_startup(&app_handle).await {
                    log::warn!("Failed to migrate disablemod.json on startup: {e}");
                }
                if let Err(e) =
                    installer::purge_remote_disabled_mods_on_startup(app_handle.clone()).await
                {
                    log::warn!("Failed to disable remote-disabled mods on startup: {e}");
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
            google_lcstats_auth_status,
            google_lcstats_start_oauth,
            google_lcstats_access_token,
            google_lcstats_picker_config,
            google_lcstats_pick_spreadsheet,
            google_lcstats_logout,
            get_lcstats_settings,
            get_lcstats_autosheet_tracking,
            get_lcstats_latest_payload,
            set_lcstats_autosheet_tracking,
            set_lcstats_settings,
            list_lcstats_sheet_names,
            list_lcstats_sheet_infos,
            list_lcstats_spreadsheets,
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
            get_game_overlay_config,
            get_game_overlay_modules,
            get_game_overlay_debug_status,
            get_stream_overlays_data,
            get_stream_overlays_ws_url,
            report_game_overlay_frontend_ready,
            report_game_overlay_frontend_info,
            report_game_overlay_frontend_error,
            set_game_overlay_config,
            toggle_game_overlay_only,
            open_game_overlay_only,
            close_game_overlay_only,
            open_game_overlay_modules_folder,
            set_game_overlay_controls_open,
            set_game_overlay_input_shortcuts,
            show_game_overlay_end_summary,
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
            get_release_channel,
            set_release_channel,
            get_game_storage_settings,
            pick_game_storage_dir,
            set_game_storage_dir,
            installer::install_proton_ge,
            installer::get_current_proton_dir,
            delete_installed_version,
            open_custom_layout_docs,
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

//! App-owned Wine and Direct3D 11 runtime for macOS.
//!
//! Upstream release assets are pinned together with GitHub's published SHA-256
//! digests. No Homebrew, paid runtime, or system-wide Wine installation is used.
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;
use tauri::{Emitter, Manager};

const VERSION: &str = "wine-11.0_1-dxvk-1.10.3-gst-1.28.1-r1";
const WINE_ARCHIVE: &str = "https://github.com/Gcenx/macOS_Wine_builds/releases/download/11.0_1/wine-stable-11.0_1-osx64.tar.xz";
const WINE_SHA256: &str = "b50dc50ec7f41d58b115a6b685d4d1315ba3c797bd3aa0f49213f2703cb82388";
const WINE_SIZE: u64 = 185_303_032;
const DXVK_ARCHIVE: &str = "https://github.com/Gcenx/DXVK-macOS/releases/download/v1.10.3-20230507-repack/dxvk-macOS-async-v1.10.3-20230507-repack-builtin.tar.gz";
const DXVK_SHA256: &str = "810b1e5caf8ce975b784fae866a130ad23fa0ea233b0e5609cbc4a45f3ef6f00";
const DXVK_SIZE: u64 = 2_785_833;
const GST_ARCHIVE: &str =
    "https://gstreamer.freedesktop.org/data/pkg/osx/1.28.1/gstreamer-1.0-1.28.1-universal.pkg";
const GST_SHA256: &str = "02803f73435daabe8fb12b79c38c6775d0efb83af001474558ba25c4f874d305";
const GST_SIZE: u64 = 151_846_922;
const DXVK_DIR: &str = "dxvk-macOS-async-v1.10.3-20230507-repack-builtin";
const WINE_RELATIVE: &str = "Wine Stable.app/Contents/Resources/wine/bin/wine";
const WINE_LIB: &str = "Wine Stable.app/Contents/Resources/wine/lib/wine";
const MAX_EXTRACTED_BYTES: u64 = 4 * 1024 * 1024 * 1024;
static INSTALL_LOCK: Mutex<()> = Mutex::new(());

#[derive(Clone, Serialize)]
struct Progress<'a> {
    stage: &'a str,
    percent: Option<f64>,
    message: Option<&'a str>,
}

fn emit_progress(app: &tauri::AppHandle, stage: &str, percent: Option<f64>) {
    let _ = app.emit(
        "macos-runtime-progress",
        Progress {
            stage,
            percent,
            message: None,
        },
    );
}

fn runtime_root(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .map(|path| path.join("macos_env").join("runtime"))
        .map_err(|error| format!("macOS 실행 환경 저장 경로를 찾지 못했습니다: {error}"))
}

fn complete_wine_path(root: &Path) -> Option<PathBuf> {
    let wine = root.join(WINE_RELATIVE);
    let marker = fs::read_to_string(root.join(".hq-runtime-version")).ok()?;
    (marker.trim() == VERSION
        && wine.is_file()
        && root
            .join(WINE_LIB)
            .join("x86_64-windows/d3d11.dll")
            .is_file()
        && root
            .join(WINE_LIB)
            .join("x86_64-windows/d3d10core.dll")
            .is_file()
        && root
            .join("gstreamer/lib/libgstreamer-1.0.0.dylib")
            .is_file()
        && root
            .join("gstreamer/libexec/gstreamer-1.0/gst-plugin-scanner")
            .is_file())
    .then_some(wine)
}

/// Inspect the installed managed runtime without creating files or processes.
pub fn installed_wine(app: &tauri::AppHandle) -> Result<Option<PathBuf>, String> {
    Ok(complete_wine_path(&runtime_root(app)?.join(VERSION)))
}

/// The upstream media framework is relocatable when its libraries and plugins
/// are provided explicitly. Apply this only to game Wine processes.
pub fn configure_command(command: &mut std::process::Command, wine: &Path) {
    let Some(root) = wine.ancestors().nth(6) else {
        return;
    };
    if root.join(WINE_RELATIVE) != wine || !root.join("gstreamer").is_dir() {
        return;
    }
    let media = root.join("gstreamer");
    let wine_lib = wine.parent().and_then(Path::parent).unwrap().join("lib");
    // Do not override system libraries by basename: DYLD_LIBRARY_PATH also
    // replaces dependencies of macOS frameworks, breaking Wine's GUI dialogs.
    // Use fallback lookup only for dependencies absent from their install path.
    command.env_remove("DYLD_LIBRARY_PATH");
    if let Ok(paths) = std::env::join_paths([
        wine_lib,
        media.join("lib"),
        PathBuf::from("/usr/local/lib"),
        PathBuf::from("/usr/lib"),
    ]) {
        command.env("DYLD_FALLBACK_LIBRARY_PATH", paths);
    }
    command.env(
        "GST_PLUGIN_SYSTEM_PATH_1_0",
        media.join("lib/gstreamer-1.0"),
    );
    command.env(
        "GST_PLUGIN_SCANNER_1_0",
        media.join("libexec/gstreamer-1.0/gst-plugin-scanner"),
    );
    if let Some(environment_dir) = root.parent().and_then(Path::parent) {
        command.env(
            "GST_REGISTRY_1_0",
            environment_dir.join("gstreamer-registry-x86_64.bin"),
        );
    }
}

/// Concurrent startup and launch callers share one installation operation.
pub async fn ensure_installed(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let result = install_locked(&app);
        if let Err(error) = &result {
            let _ = app.emit(
                "macos-runtime-progress",
                Progress {
                    stage: "error",
                    percent: None,
                    message: Some(error),
                },
            );
        }
        result
    })
    .await
    .map_err(|error| format!("macOS 실행 환경 설치 작업에 실패했습니다: {error}"))?
}

fn check_rosetta() -> Result<(), String> {
    #[cfg(target_arch = "aarch64")]
    {
        // Unlike checking for oahd, this works when Rosetta is installed but idle.
        let available = std::process::Command::new("/usr/bin/arch")
            .args(["-x86_64", "/usr/bin/true"])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|status| status.success())
            .unwrap_or(false);
        if !available {
            // Apple's own UI presents the license and installation confirmation.
            // Do not silently accept the Rosetta license with softwareupdate.
            let _ = std::process::Command::new("/usr/bin/open")
                .args(["-a", "/System/Library/CoreServices/Rosetta 2 Updater.app"])
                .spawn();
            return Err("Apple Silicon에서 게임을 실행하려면 Rosetta 2가 필요합니다. 열린 Apple 설치 창에서 설치를 완료한 뒤 다시 실행해 주세요.".to_string());
        }
    }
    Ok(())
}

struct StagingDirectory(PathBuf);

impl Drop for StagingDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn install_locked(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let _lock = INSTALL_LOCK
        .lock()
        .map_err(|_| "macOS 실행 환경 설치 잠금을 얻지 못했습니다.".to_string())?;
    emit_progress(app, "checking", None);
    check_rosetta()?;
    let root = runtime_root(app)?;
    let destination = root.join(VERSION);
    if let Some(wine) = complete_wine_path(&destination) {
        emit_progress(app, "ready", Some(100.0));
        return Ok(wine);
    }
    fs::create_dir_all(&root).map_err(|error| error.to_string())?;
    let mut nonce = [0u8; 8];
    getrandom::fill(&mut nonce).map_err(|error| error.to_string())?;
    let staging =
        StagingDirectory(root.join(format!(".install-{:016x}", u64::from_le_bytes(nonce))));
    fs::create_dir(&staging.0).map_err(|error| error.to_string())?;
    let extracted = staging.0.join("extracted");
    fs::create_dir(&extracted).map_err(|error| error.to_string())?;
    let client = reqwest::blocking::Client::builder()
        .user_agent(concat!("hq-launcher/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(30))
        .timeout(Duration::from_secs(20 * 60))
        .build()
        .map_err(|error| error.to_string())?;
    let wine_archive = staging.0.join("wine.tar.xz");
    download(
        app,
        &client,
        WINE_ARCHIVE,
        WINE_SHA256,
        WINE_SIZE,
        &wine_archive,
        "downloading-wine",
    )?;
    let dxvk_archive = staging.0.join("dxvk.tar.gz");
    download(
        app,
        &client,
        DXVK_ARCHIVE,
        DXVK_SHA256,
        DXVK_SIZE,
        &dxvk_archive,
        "downloading-graphics",
    )?;
    let gst_archive = staging.0.join("gstreamer.pkg");
    download(
        app,
        &client,
        GST_ARCHIVE,
        GST_SHA256,
        GST_SIZE,
        &gst_archive,
        "downloading-media",
    )?;
    emit_progress(app, "extracting", None);
    let wine_reader =
        xz2::read::XzDecoder::new(File::open(&wine_archive).map_err(|error| error.to_string())?);
    unpack_safely(wine_reader, &extracted)?;
    let dxvk_reader =
        flate2::read::GzDecoder::new(File::open(&dxvk_archive).map_err(|error| error.to_string())?);
    unpack_safely(dxvk_reader, &extracted)?;
    extract_gstreamer(
        &gst_archive,
        &staging.0.join("media-package"),
        &extracted.join("gstreamer"),
    )?;

    install_dxvk(&extracted)?;
    let wine = extracted.join(WINE_RELATIVE);
    let mut version_command = std::process::Command::new(&wine);
    configure_command(&mut version_command, &wine);
    let version = version_command
        .arg("--version")
        .output()
        .map_err(|error| format!("다운로드한 Wine을 실행하지 못했습니다: {error}"))?;
    if !version.status.success() {
        return Err(format!(
            "다운로드한 Wine 실행 확인에 실패했습니다: {}",
            String::from_utf8_lossy(&version.stderr).trim()
        ));
    }
    fs::write(extracted.join(".hq-runtime-version"), VERSION).map_err(|error| error.to_string())?;
    fs::write(extracted.join("HQ-RUNTIME-SOURCES.txt"), format!(
        "Wine 11.0_1 (LGPL-2.1-or-later): https://github.com/Gcenx/macOS_Wine_builds\nWine source and license: https://gitlab.winehq.org/wine/wine/-/tree/wine-11.0\nPackaging/source: https://github.com/Gcenx/macports-wine\nArchive: {WINE_ARCHIVE}\nSHA256: {WINE_SHA256}\n\nDXVK macOS 1.10.3 (zlib): https://github.com/Gcenx/DXVK-macOS\nSource and license: https://github.com/Gcenx/DXVK-macOS/tree/v1.10.3-20230507-repack\nArchive: {DXVK_ARCHIVE}\nSHA256: {DXVK_SHA256}\n\nGStreamer 1.28.1 and dependencies: https://gstreamer.freedesktop.org/\nSource/build recipes: https://gitlab.freedesktop.org/gstreamer/cerbero/-/tree/1.28.1\nArchive: {GST_ARCHIVE}\nSHA256: {GST_SHA256}\nSee gstreamer/UPSTREAM-LICENSE.txt and gstreamer/share for component licenses.\n"
    )).map_err(|error| error.to_string())?;
    // Only a fully verified and runnable directory is published. The staging
    // directory is on the same filesystem, so rename is atomic.
    if destination.exists() {
        fs::remove_dir_all(&destination).map_err(|error| error.to_string())?;
    }
    fs::rename(&extracted, &destination).map_err(|error| error.to_string())?;
    emit_progress(app, "ready", Some(100.0));
    Ok(destination.join(WINE_RELATIVE))
}

fn install_dxvk(extracted: &Path) -> Result<(), String> {
    // The upstream "builtin" DXVK build explicitly supports replacing Wine's
    // builtin D3D10/11 DLLs. Retain Wine's dxgi.dll and bundled MoltenVK loader.
    for arch in ["x86_64-windows", "i386-windows"] {
        for dll in ["d3d10core.dll", "d3d11.dll"] {
            let source = extracted.join(DXVK_DIR).join(arch).join(dll);
            let target = extracted.join(WINE_LIB).join(arch).join(dll);
            if !source.is_file() || !target.is_file() {
                return Err(format!(
                    "macOS 실행 환경 압축 파일 구조가 올바르지 않습니다: {arch}/{dll}"
                ));
            }
            fs::copy(source, target).map_err(|error| error.to_string())?;
        }
    }
    Ok(())
}

fn extract_gstreamer(archive: &Path, expanded: &Path, destination: &Path) -> Result<(), String> {
    // pkgutil only expands the verified upstream package here. It never runs
    // installer scripts, requests administrator access, or installs into /Library.
    let result = std::process::Command::new("/usr/sbin/pkgutil")
        .arg("--expand-full")
        .arg(archive)
        .arg(expanded)
        .output()
        .map_err(|error| format!("미디어 실행 환경 압축 해제 실패: {error}"))?;
    if !result.status.success() {
        return Err(format!(
            "미디어 실행 환경 압축 해제 실패: {}",
            String::from_utf8_lossy(&result.stderr).trim()
        ));
    }
    fs::create_dir(destination).map_err(|error| error.to_string())?;
    // These are the runtime codecs/libraries used by Wine. GUI developer tools
    // and Python/GTK bindings are not part of the game runtime.
    for component in [
        "base-system-1.0",
        "base-crypto",
        "gstreamer-1.0-core",
        "gstreamer-1.0-system",
        "gstreamer-1.0-playback",
        "gstreamer-1.0-codecs",
        "gstreamer-1.0-codecs-restricted",
        "gstreamer-1.0-libav",
        "gstreamer-1.0-effects",
        "gstreamer-1.0-visualizers",
        "gstreamer-1.0-capture",
    ] {
        let package = expanded.join(format!("{component}-1.28.1-universal.pkg"));
        let info =
            fs::read_to_string(package.join("PackageInfo")).map_err(|error| error.to_string())?;
        if !info
            .contains("install-location=\"/Library/Frameworks/GStreamer.framework/Versions/1.0\"")
        {
            return Err(format!(
                "미디어 실행 환경의 패키지 구조가 예상과 다릅니다: {component}"
            ));
        }
        copy_payload(&package.join("Payload"), destination, Path::new(""))?;
    }
    fs::copy(
        expanded.join("Resources/license.txt"),
        destination.join("UPSTREAM-LICENSE.txt"),
    )
    .map_err(|error| error.to_string())?;
    Ok(())
}

fn copy_payload(
    source_root: &Path,
    destination_root: &Path,
    relative: &Path,
) -> Result<(), String> {
    for entry in fs::read_dir(source_root.join(relative)).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let rel = relative.join(entry.file_name());
        let source = source_root.join(&rel);
        let destination = destination_root.join(&rel);
        let kind = entry.file_type().map_err(|error| error.to_string())?;
        if let Ok(existing) = fs::symlink_metadata(&destination) {
            // Never descend through or copy over a symlink from another payload.
            if existing.file_type().is_symlink() {
                if kind.is_symlink()
                    && fs::read_link(&source).ok() == fs::read_link(&destination).ok()
                {
                    continue;
                }
                return Err(format!(
                    "미디어 패키지 경로가 중복됩니다: {}",
                    rel.display()
                ));
            }
        }
        if kind.is_dir() {
            fs::create_dir_all(&destination).map_err(|error| error.to_string())?;
            copy_payload(source_root, destination_root, &rel)?;
        } else if kind.is_file() {
            fs::copy(source, destination).map_err(|error| error.to_string())?;
        } else if kind.is_symlink() {
            let target = fs::read_link(&source).map_err(|error| error.to_string())?;
            if !safe_link_target(&rel, &target, true) {
                return Err(format!(
                    "미디어 패키지 링크가 설치 폴더를 벗어납니다: {}",
                    rel.display()
                ));
            }
            std::os::unix::fs::symlink(target, destination).map_err(|error| error.to_string())?;
        } else {
            return Err(format!(
                "지원하지 않는 미디어 패키지 항목: {}",
                rel.display()
            ));
        }
    }
    Ok(())
}

fn download(
    app: &tauri::AppHandle,
    client: &reqwest::blocking::Client,
    url: &str,
    expected_hash: &str,
    expected_size: u64,
    target: &Path,
    stage: &str,
) -> Result<(), String> {
    emit_progress(app, stage, Some(0.0));
    let mut response = client
        .get(url)
        .send()
        .and_then(|response| response.error_for_status())
        .map_err(|error| format!("macOS 실행 환경 다운로드 실패: {error}"))?;
    let mut file = File::create(target).map_err(|error| error.to_string())?;
    let mut digest = Sha256::new();
    let mut bytes = 0u64;
    let mut last_percent = 0u64;
    let mut buffer = [0u8; 128 * 1024];
    loop {
        let read = response
            .read(&mut buffer)
            .map_err(|error| error.to_string())?;
        if read == 0 {
            break;
        }
        bytes += read as u64;
        if bytes > expected_size {
            return Err("macOS 실행 환경 다운로드 크기가 예상값과 다릅니다.".to_string());
        }
        digest.update(&buffer[..read]);
        file.write_all(&buffer[..read])
            .map_err(|error| error.to_string())?;
        let percent = bytes.saturating_mul(100) / expected_size;
        if percent != last_percent {
            emit_progress(app, stage, Some(percent as f64));
            last_percent = percent;
        }
    }
    if bytes != expected_size || format!("{:x}", digest.finalize()) != expected_hash {
        return Err(
            "macOS 실행 환경 무결성(SHA-256) 확인에 실패했습니다. 다시 시도해 주세요.".to_string(),
        );
    }
    file.sync_all().map_err(|error| error.to_string())
}

fn safe_relative_path(path: &Path) -> bool {
    !path.as_os_str().is_empty()
        && path
            .components()
            .all(|part| matches!(part, Component::Normal(_) | Component::CurDir))
}

fn safe_link_target(entry_path: &Path, target: &Path, symbolic: bool) -> bool {
    let mut depth = if symbolic {
        entry_path.parent().map_or(0, |path| {
            path.components()
                .filter(|part| matches!(part, Component::Normal(_)))
                .count()
        })
    } else {
        0
    };
    for part in target.components() {
        match part {
            Component::Normal(_) => depth += 1,
            Component::CurDir => {}
            Component::ParentDir if depth > 0 => depth -= 1,
            _ => return false,
        }
    }
    !target.as_os_str().is_empty()
}

fn unpack_safely(reader: impl Read, destination: &Path) -> Result<(), String> {
    let mut archive = tar::Archive::new(reader);
    let mut total_bytes = 0u64;
    for entry in archive.entries().map_err(|error| error.to_string())? {
        let mut entry = entry.map_err(|error| error.to_string())?;
        let path = entry
            .path()
            .map_err(|error| error.to_string())?
            .into_owned();
        if !safe_relative_path(&path) {
            return Err(format!(
                "압축 파일에 안전하지 않은 경로가 있습니다: {}",
                path.display()
            ));
        }
        let kind = entry.header().entry_type();
        if !(kind.is_file() || kind.is_dir() || kind.is_symlink() || kind.is_hard_link()) {
            return Err(format!("지원하지 않는 압축 파일 항목: {}", path.display()));
        }
        if let Some(target) = entry.link_name().map_err(|error| error.to_string())? {
            if !safe_link_target(&path, &target, kind.is_symlink()) {
                return Err(format!(
                    "압축 파일의 링크가 설치 폴더를 벗어납니다: {}",
                    path.display()
                ));
            }
        }
        total_bytes = total_bytes
            .checked_add(entry.size())
            .ok_or("압축 파일 크기가 너무 큽니다.")?;
        if total_bytes > MAX_EXTRACTED_BYTES {
            return Err("압축 파일 크기가 너무 큽니다.".to_string());
        }
        // tar's containment checks also reject writes through symlink parents.
        if !entry
            .unpack_in(destination)
            .map_err(|error| error.to_string())?
        {
            return Err(format!(
                "압축 파일 경로가 설치 폴더를 벗어납니다: {}",
                path.display()
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn archive_paths_stay_within_destination() {
        assert!(safe_relative_path(Path::new(
            "./Wine Stable.app/Contents/Resources/wine/bin/wine"
        )));
        for path in ["/tmp/escape", "../escape", "bundle/../../escape", ""] {
            assert!(!safe_relative_path(Path::new(path)));
        }
    }

    #[test]
    fn relative_framework_links_are_allowed_but_escaping_links_are_not() {
        let entry = Path::new("Bundle/Versions/Current");
        assert!(safe_link_target(entry, Path::new("A"), true));
        assert!(safe_link_target(entry, Path::new("../A"), true));
        assert!(!safe_link_target(entry, Path::new("../../../escape"), true));
        assert!(!safe_link_target(entry, Path::new("/tmp/escape"), true));
        assert!(!safe_link_target(entry, Path::new("../escape"), false));
    }

    #[test]
    fn runtime_is_not_ready_without_commit_marker() {
        assert!(complete_wine_path(Path::new("/nonexistent/hq-runtime-test")).is_none());
    }

    /// Optional real-asset smoke test, requiring the three already downloaded
    /// pinned archives (wine.tar.xz, dxvk.tar.gz, gstreamer.pkg). No network,
    /// system installation, Steam login, or game process is involved.
    #[test]
    #[ignore = "set HQ_MACOS_RUNTIME_TEST_ASSETS to a directory of pinned upstream archives"]
    fn pinned_runtime_assets_extract_and_load() {
        let assets = PathBuf::from(
            std::env::var_os("HQ_MACOS_RUNTIME_TEST_ASSETS").expect("archive directory"),
        );
        for (name, expected) in [
            ("wine.tar.xz", WINE_SHA256),
            ("dxvk.tar.gz", DXVK_SHA256),
            ("gstreamer.pkg", GST_SHA256),
        ] {
            let mut reader = File::open(assets.join(name)).unwrap();
            let mut digest = Sha256::new();
            std::io::copy(&mut reader, &mut digest).unwrap();
            assert_eq!(format!("{:x}", digest.finalize()), expected, "{name}");
        }
        let mut nonce = [0u8; 8];
        getrandom::fill(&mut nonce).unwrap();
        let staging = StagingDirectory(std::env::temp_dir().join(format!(
            "hq-runtime-smoke-{:016x}",
            u64::from_le_bytes(nonce)
        )));
        fs::create_dir(&staging.0).unwrap();
        let extracted = staging.0.join("runtime");
        fs::create_dir(&extracted).unwrap();
        unpack_safely(
            xz2::read::XzDecoder::new(File::open(assets.join("wine.tar.xz")).unwrap()),
            &extracted,
        )
        .unwrap();
        unpack_safely(
            flate2::read::GzDecoder::new(File::open(assets.join("dxvk.tar.gz")).unwrap()),
            &extracted,
        )
        .unwrap();
        extract_gstreamer(
            &assets.join("gstreamer.pkg"),
            &staging.0.join("media-package"),
            &extracted.join("gstreamer"),
        )
        .unwrap();
        install_dxvk(&extracted).unwrap();
        let wine = extracted.join(WINE_RELATIVE);
        let mut wine_command = std::process::Command::new(&wine);
        configure_command(&mut wine_command, &wine);
        let output = wine_command.arg("--version").output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stdout).starts_with("wine-11.0"));
        let mut media_command =
            std::process::Command::new(extracted.join("gstreamer/bin/gst-inspect-1.0"));
        configure_command(&mut media_command, &wine);
        // Verify relocation/linking without scanning every optional codec. On
        // first use macOS can spend minutes checking all plugin signatures.
        media_command.env("GST_PLUGIN_SYSTEM_PATH_1_0", "");
        media_command.env("GST_PLUGIN_PATH_1_0", "");
        // Wine loads the x86_64 slice even on Apple Silicon. arch's -e option
        // restores these variables after macOS clears DYLD_* from system tools.
        let mut intel_media = std::process::Command::new("/usr/bin/arch");
        intel_media.arg("-x86_64");
        for (key, value) in media_command.get_envs() {
            if let Some(value) = value {
                let mut assignment = key.to_os_string();
                assignment.push("=");
                assignment.push(value);
                intel_media.arg("-e").arg(assignment);
            }
        }
        let output = intel_media
            .arg(media_command.get_program())
            .args(["--gst-disable-registry-update", "--version"])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains("1.28.1"));
        assert!(complete_wine_path(&extracted).is_none());
        fs::write(extracted.join(".hq-runtime-version"), VERSION).unwrap();
        assert_eq!(complete_wine_path(&extracted), Some(wine));
    }
}

const METAL_VERSION: &str = "wine-11.0_1-dxmt-soju-v1";
const METAL_ARCHIVE: &str = "https://github.com/BCD1210/soju/releases/download/steam-support-v1/soju-steam-support-v1.tar.gz";
const METAL_SHA256: &str = "5e3588b0c0a47eb01ef0ddfd7f15b0cbd2f4c02dc4e885eacb785b6a42199a84";
const METAL_SIZE: u64 = 25_027_503;

pub fn metal_supported() -> bool {
    static SUPPORTED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *SUPPORTED.get_or_init(|| {
        cfg!(target_arch = "aarch64")
            && std::process::Command::new("/usr/bin/sw_vers")
                .arg("-productVersion")
                .output()
                .ok()
                .filter(|o| o.status.success())
                .and_then(|o| String::from_utf8(o.stdout).ok())
                .and_then(|v| v.trim().split('.').next()?.parse::<u32>().ok())
                .is_some_and(|major| major >= 26)
    })
}

pub fn installed_metal_wine(app: &tauri::AppHandle) -> Result<Option<PathBuf>, String> {
    let root = runtime_root(app)?.join(METAL_VERSION);
    let valid = fs::read_to_string(root.join(".hq-metal-version"))
        .ok()
        .as_deref()
        == Some(METAL_VERSION)
        && root.join(WINE_RELATIVE).is_file()
        && [
            "x86_64-unix/winemetal.so",
            "x86_64-unix/winemac.so",
            "x86_64-windows/winemetal.dll",
            "x86_64-windows/d3d11.dll",
            "x86_64-windows/dxgi.dll",
        ]
        .iter()
        .all(|file| root.join(WINE_LIB).join(file).is_file())
        && root.join("gstreamer").is_dir();
    Ok(valid.then(|| root.join(WINE_RELATIVE)))
}

/// A separate game runtime keeps Steam and the known-working DXVK fallback intact.
pub async fn ensure_metal_wine(app: &tauri::AppHandle, prefix: &Path) -> Result<(), String> {
    let app = app.clone();
    let prefix = prefix.to_path_buf();
    tauri::async_runtime::spawn_blocking(move || {
        let result = install_metal_locked(&app, &prefix);
        if let Err(message) = &result {
            let _ = app.emit(
                "macos-runtime-progress",
                Progress {
                    stage: "error",
                    percent: None,
                    message: Some(message),
                },
            );
        }
        result
    })
    .await
    .map_err(|e| e.to_string())?
}

fn install_metal_locked(app: &tauri::AppHandle, prefix: &Path) -> Result<(), String> {
    let _lock = INSTALL_LOCK.lock().map_err(|e| e.to_string())?;
    if !metal_supported() {
        return Err("Metal renderer requires Apple Silicon and macOS 26 or later.".into());
    }
    if installed_metal_wine(app)?.is_none() {
        let base_wine = installed_wine(app)?.ok_or("Install the managed Wine runtime first.")?;
        let base = base_wine.ancestors().nth(6).ok_or("Invalid Wine path")?;
        let root = runtime_root(app)?;
        let mut nonce = [0; 8];
        getrandom::fill(&mut nonce).map_err(|e| e.to_string())?;
        let staging =
            StagingDirectory(root.join(format!(".metal-{:016x}", u64::from_le_bytes(nonce))));
        fs::create_dir(&staging.0).map_err(|e| e.to_string())?;
        let client = reqwest::blocking::Client::builder()
            .connect_timeout(Duration::from_secs(30))
            .timeout(Duration::from_secs(600))
            .build()
            .map_err(|e| e.to_string())?;
        let archive = staging.0.join("metal.tar.gz");
        download(
            app,
            &client,
            METAL_ARCHIVE,
            METAL_SHA256,
            METAL_SIZE,
            &archive,
            "downloading-metal",
        )?;
        let support = staging.0.join("support");
        fs::create_dir(&support).map_err(|e| e.to_string())?;
        unpack_safely(
            flate2::read::GzDecoder::new(File::open(&archive).map_err(|e| e.to_string())?),
            &support,
        )?;
        emit_progress(app, "preparing-metal", None);
        let prepared = staging.0.join("prepared");
        fs::create_dir(&prepared).map_err(|e| e.to_string())?;
        let copied = std::process::Command::new("/bin/cp")
            .arg("-cR")
            .arg(base.join("Wine Stable.app"))
            .arg(prepared.join("Wine Stable.app"))
            .status()
            .map_err(|e| e.to_string())?;
        if !copied.success() {
            let _ = fs::remove_dir_all(prepared.join("Wine Stable.app"));
            let fallback = std::process::Command::new("/bin/cp")
                .arg("-R")
                .arg(base.join("Wine Stable.app"))
                .arg(prepared.join("Wine Stable.app"))
                .status()
                .map_err(|e| e.to_string())?;
            if !fallback.success() {
                return Err("Failed to prepare the Metal game runtime.".into());
            }
        }
        let lib = prepared.join(WINE_LIB);
        for name in ["d3d11.dll", "dxgi.dll", "d3d10core.dll", "winemetal.dll"] {
            fs::copy(
                support.join("dxmt-x64").join(name),
                lib.join("x86_64-windows").join(name),
            )
            .map_err(|e| e.to_string())?;
        }
        fs::copy(
            support.join("dxmt-x64/winemetal.so"),
            lib.join("x86_64-unix/winemetal.so"),
        )
        .map_err(|e| e.to_string())?;
        fs::copy(
            support.join("winemac-patched.so"),
            lib.join("x86_64-unix/winemac.so"),
        )
        .map_err(|e| e.to_string())?;
        std::os::unix::fs::symlink(base.join("gstreamer"), prepared.join("gstreamer"))
            .map_err(|e| e.to_string())?;
        fs::rename(support.join("licenses"), prepared.join("licenses"))
            .map_err(|e| e.to_string())?;
        fs::copy(support.join("SOURCES.txt"), prepared.join("SOURCES.txt"))
            .map_err(|e| e.to_string())?;
        fs::write(prepared.join("SOURCE-DOWNLOAD.txt"), format!("{METAL_ARCHIVE}\nSHA256: {METAL_SHA256}\nCorresponding sources: https://github.com/BCD1210/soju/releases/download/steam-support-v1/soju-steam-support-v1-sources.tar.gz\n")).map_err(|e| e.to_string())?;
        fs::write(prepared.join(".hq-metal-version"), METAL_VERSION).map_err(|e| e.to_string())?;
        let destination = root.join(METAL_VERSION);
        if destination.exists() {
            return Err(
                "Incomplete Metal runtime exists; remove only its folder and retry.".into(),
            );
        }
        fs::rename(prepared, destination).map_err(|e| e.to_string())?;
    }
    // Wine needs the added builtin module to have a PE counterpart in this prefix.
    let root = runtime_root(app)?.join(METAL_VERSION);
    let target = prefix.join("drive_c/windows/system32/winemetal.dll");
    let bytes = fs::read(root.join(WINE_LIB).join("x86_64-windows/winemetal.dll"))
        .map_err(|e| e.to_string())?;
    if fs::read(&target).ok().as_deref() != Some(bytes.as_slice()) {
        let temporary = target.with_extension("hq-metal-part");
        fs::write(&temporary, bytes).map_err(|e| e.to_string())?;
        fs::rename(temporary, target).map_err(|e| e.to_string())?;
    }
    emit_progress(app, "ready", Some(100.0));
    Ok(())
}

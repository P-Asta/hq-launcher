use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

const BUILD_ENV_KEYS: [&str; 5] = [
    "DEV",
    "GOOGLE_LCSTATS_CLIENT_ID",
    "GOOGLE_LCSTATS_CLIENT_SECRET",
    "GOOGLE_LCSTATS_PICKER_API_KEY",
    "GOOGLE_LCSTATS_PICKER_APP_ID",
];

fn parse_env_file(path: &Path) -> HashMap<String, String> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return HashMap::new();
    };

    text.lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                return None;
            }
            let line = line.strip_prefix("export ").unwrap_or(line);
            let (key, value) = line.split_once('=')?;
            let key = key.trim().to_string();
            let value = value
                .trim()
                .trim_matches('"')
                .trim_matches('\'')
                .to_string();
            Some((key, value))
        })
        .collect()
}

fn env_paths() -> Vec<PathBuf> {
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let repo_dir = manifest_dir.parent().unwrap_or(&manifest_dir).to_path_buf();
    vec![repo_dir.join(".env"), manifest_dir.join(".env")]
}

fn inject_build_env() {
    let mut values = HashMap::new();
    for path in env_paths() {
        println!("cargo:rerun-if-changed={}", path.display());
        values.extend(parse_env_file(&path));
    }

    for key in BUILD_ENV_KEYS {
        if let Ok(value) = std::env::var(key) {
            println!("cargo:rustc-env={key}={value}");
        } else if let Some(value) = values.get(key) {
            println!("cargo:rustc-env={key}={value}");
        }
    }
}

/// Build the Wine-side inject helper (`hq-inject-helper.exe`) and stage it into
/// `resources/native-overlay/` so Tauri bundles it on every platform.
///
/// On a Windows host the helper is built with the default (native) target; on
/// Linux/macOS it is cross-compiled to `x86_64-pc-windows-gnu` so it runs under
/// Proton/Wine. The build is incremental: the helper is only rebuilt when its
/// source changes. If the cross toolchain is unavailable, a previously staged
/// copy is left in place (with a warning) rather than failing the whole build.
fn build_inject_helper() {
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let helper_crate = manifest_dir.join("inject-helper");
    let resources_dir = manifest_dir.join("resources").join("native-overlay");
    let staged_exe = resources_dir.join("hq-inject-helper.exe");

    // Rebuild when any helper source file changes. Walk the small crate rather
    // than tracking individual files, so additions are picked up automatically.
    if let Ok(entries) = std::fs::read_dir(helper_crate.join("src")) {
        for entry in entries.flatten() {
            println!("cargo:rerun-if-changed={}", entry.path().display());
        }
    }
    println!("cargo:rerun-if-changed={}", helper_crate.join("Cargo.toml").display());

    // Pick the target triple: native on Windows, MinGW cross on Unix hosts.
    let target = if cfg!(target_os = "windows") {
        None
    } else {
        Some("x86_64-pc-windows-gnu")
    };

    let mut cmd = Command::new("cargo");
    cmd.args(["build", "--release", "--manifest-path"])
        .arg(helper_crate.join("Cargo.toml"));
    if let Some(triple) = target {
        cmd.args(["--target", triple]);
    }

    let build_result = cmd.status();
    let build_succeeded = matches!(build_result, Ok(status) if status.success());
    if !build_succeeded {
        // The cross toolchain may be missing on a fresh dev machine. Keep any
        // previously staged copy instead of hard-failing the launcher build —
        // native overlay injection simply stays unavailable until the helper is
        // available (e.g. via the official release download).
        if staged_exe.is_file() {
            println!(
                "cargo:warning=injected helper build skipped/failed; keeping previously staged {}",
                staged_exe.display()
            );
        } else {
            println!(
                "cargo:warning=injected helper build failed and no staged copy exists; \
                 native overlay injection will be unavailable. \
                 Install the x86_64-pc-windows-gnu target and mingw-w64 linker to enable it."
            );
        }
        return;
    }

    // Resolve the freshly built binary and copy it into resources/.
    let output_dir = if let Some(triple) = target {
        helper_crate.join("target").join(triple).join("release")
    } else {
        helper_crate.join("target").join("release")
    };
    let built_exe = output_dir.join("hq-inject-helper.exe");
    if !built_exe.is_file() {
        println!(
            "cargo:warning=inject helper build reported success but {} was not found",
            built_exe.display()
        );
        return;
    }

    if let Err(error) = std::fs::create_dir_all(&resources_dir) {
        println!(
            "cargo:warning=failed to create native-overlay resources dir: {error}"
        );
        return;
    }
    // Only copy when the staged copy is missing or differs in size to avoid
    // touching the file (and thus Tauri's resource hashing) on every build.
    let needs_copy = match std::fs::metadata(&staged_exe) {
        Ok(meta) => {
            meta.len() != std::fs::metadata(&built_exe).map(|m| m.len()).unwrap_or(0)
        }
        Err(_) => true,
    };
    if needs_copy {
        if let Err(error) = std::fs::copy(&built_exe, &staged_exe) {
            println!("cargo:warning=failed to stage inject helper: {error}");
        }
    }
}

fn main() {
    inject_build_env();
    build_inject_helper();
    tauri_build::build()
}

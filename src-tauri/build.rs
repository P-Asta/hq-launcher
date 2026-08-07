use std::collections::HashMap;
use std::path::{Path, PathBuf};

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

fn main() {
    inject_build_env();
    tauri_build::build()
}

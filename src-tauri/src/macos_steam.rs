//! Keep the managed Wine prefix's Steam CEF workaround separate from game graphics.
use std::path::Path;

const WRAPPER: &[u8] = include_bytes!("../resources/macos-steam/steamwebhelper-wrapper.exe");
const MARKER: &[u8] = b"HQ_STEAM_CEF_WRAPPER_V1";

fn is_wrapper(bytes: &[u8]) -> bool {
    bytes.windows(MARKER.len()).any(|window| window == MARKER)
}

/// Preserve Valve's executable before atomically replacing the entry point.
/// Steam updates remain enabled: a new Valve binary refreshes the saved copy.
fn patch_helper(directory: &Path) -> Result<bool, String> {
    let target = directory.join("steamwebhelper.exe");
    if !target.is_file() {
        return Ok(false);
    }
    let bytes = std::fs::read(&target).map_err(|e| e.to_string())?;
    let original = directory.join("steamwebhelper_real.exe");
    if is_wrapper(&bytes) {
        let saved =
            std::fs::read(&original).map_err(|e| format!("Steam helper backup is missing: {e}"))?;
        if is_wrapper(&saved) || !saved.starts_with(b"MZ") {
            return Err(
                "Steam helper backup is invalid. Repair Windows Steam before retrying.".into(),
            );
        }
        if bytes == WRAPPER {
            return Ok(false);
        }
    } else {
        // Do not preserve an incomplete updater write or a foreign tiny shim.
        if bytes.len() < 500_000 || !bytes.starts_with(b"MZ") {
            return Err(
                "Steam web helper is incomplete; wait for Steam's update and retry.".into(),
            );
        }
        atomic_write(&original, &bytes)?;
    }
    atomic_write(&target, WRAPPER)?;
    Ok(true)
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let temporary = path.with_extension("hq-part");
    std::fs::write(&temporary, bytes).map_err(|e| e.to_string())?;
    std::fs::rename(&temporary, path).map_err(|e| e.to_string())
}

pub fn ensure_helpers(steam: &Path) -> Result<bool, String> {
    static PATCH_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _guard = PATCH_LOCK.lock().map_err(|e| e.to_string())?;
    let mut changed = false;
    for directory in ["cef.win64", "cef.win7x64"] {
        changed |= patch_helper(&steam.join("bin/cef").join(directory))?;
    }
    if changed {
        std::fs::write(
            steam.join("hq-steam-compat-LICENSE.txt"),
            include_bytes!("../resources/macos-steam/LICENSE"),
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(changed)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> std::path::PathBuf {
        let directory = std::env::temp_dir().join(format!(
            "hq-steam-wrapper-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        directory
    }
    fn valve_binary(value: u8) -> Vec<u8> {
        let mut data = vec![value; 500_001];
        data[..2].copy_from_slice(b"MZ");
        data
    }
    #[test]
    fn preserves_original_and_refreshes_after_steam_update() {
        let dir = fixture();
        let target = dir.join("steamwebhelper.exe");
        let backup = dir.join("steamwebhelper_real.exe");
        let old = valve_binary(1);
        std::fs::write(&target, &old).unwrap();
        assert!(patch_helper(&dir).unwrap());
        assert_eq!(std::fs::read(&backup).unwrap(), old);
        assert!(!patch_helper(&dir).unwrap());
        let updated = valve_binary(2);
        std::fs::write(&target, &updated).unwrap();
        assert!(patch_helper(&dir).unwrap());
        assert_eq!(std::fs::read(&backup).unwrap(), updated);
        assert_eq!(std::fs::read(&target).unwrap(), WRAPPER);
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn refuses_missing_backup_and_partial_update_without_destroying_files() {
        let dir = fixture();
        let target = dir.join("steamwebhelper.exe");
        std::fs::write(&target, WRAPPER).unwrap();
        assert!(patch_helper(&dir).is_err());
        std::fs::write(&target, b"MZpartial").unwrap();
        assert!(patch_helper(&dir).is_err());
        assert_eq!(std::fs::read(&target).unwrap(), b"MZpartial");
        assert!(!dir.join("steamwebhelper_real.exe").exists());
        std::fs::remove_dir_all(dir).unwrap();
    }
}

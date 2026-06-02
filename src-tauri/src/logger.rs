use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use log::{Level, LevelFilter, Log, Metadata, Record};
use log4rs::{
    append::rolling_file::{
        policy::compound::{
            roll::fixed_window::FixedWindowRoller, trigger::size::SizeTrigger, CompoundPolicy,
        },
        RollingFileAppender,
    },
    append::Append,
    config::{Appender, Config, Root},
    encode::pattern::PatternEncoder,
};
use serde::Serialize;
use tauri::{Emitter, Manager};

type AnyError = Box<dyn std::error::Error>;

const MAX_LOG_FILE_BYTES: u64 = 10 * 1024 * 1024;
const MAX_LOG_AGE: Duration = Duration::from_secs(14 * 24 * 60 * 60);
const MAX_LOG_FILES: usize = 8;

fn err(msg: impl Into<String>) -> AnyError {
    Box::new(std::io::Error::new(std::io::ErrorKind::Other, msg.into()))
}

fn log_paths(app: &tauri::AppHandle) -> Result<(PathBuf, PathBuf), AnyError> {
    let logs_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| err(format!("failed to resolve app data dir: {e}")))?
        .join("logs");
    let log_file = logs_dir.join("hq-launcher.log");
    Ok((logs_dir, log_file))
}

fn startup_housekeeping(logs_dir: &std::path::Path, log_file: &std::path::Path) {
    if let Ok(metadata) = std::fs::metadata(log_file) {
        if metadata.len() > MAX_LOG_FILE_BYTES {
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|duration| duration.as_secs())
                .unwrap_or_default();
            let rolled = logs_dir.join(format!("hq-launcher.startup-{timestamp}.log"));
            let _ = std::fs::rename(log_file, rolled);
        }
    }

    let now = SystemTime::now();
    let Ok(entries) = std::fs::read_dir(logs_dir) else {
        return;
    };
    let mut logs = entries
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            let name = path.file_name()?.to_string_lossy();
            if !name.starts_with("hq-launcher") || !name.ends_with(".log") {
                return None;
            }
            let modified = entry.metadata().ok()?.modified().ok()?;
            Some((path, modified))
        })
        .collect::<Vec<_>>();

    for (path, modified) in &logs {
        if path == log_file {
            continue;
        }
        if now
            .duration_since(*modified)
            .map(|age| age > MAX_LOG_AGE)
            .unwrap_or(false)
        {
            let _ = std::fs::remove_file(path);
        }
    }

    logs.retain(|(path, _)| path.exists());
    logs.sort_by_key(|(_, modified)| *modified);
    let removable_count = logs.len().saturating_sub(MAX_LOG_FILES);
    for (path, _) in logs.into_iter().take(removable_count) {
        if path != log_file {
            let _ = std::fs::remove_file(path);
        }
    }
}

#[derive(Debug)]
struct ErrorEventAppender {
    app: tauri::AppHandle,
}

#[derive(Clone, Serialize)]
struct ErrorLogEvent {
    message: String,
    target: String,
    module_path: Option<String>,
    file: Option<String>,
    line: Option<u32>,
    timestamp_ms: u128,
}

impl Log for ErrorEventAppender {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        metadata.level() == Level::Error
    }

    fn log(&self, record: &Record<'_>) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let message = record.args().to_string();
        if is_missing_mod_icon_asset_error(record, &message) {
            return;
        }
        let timestamp_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_millis())
            .unwrap_or_default();
        let event = ErrorLogEvent {
            message,
            target: record.target().to_string(),
            module_path: record.module_path().map(ToOwned::to_owned),
            file: record.file().map(ToOwned::to_owned),
            line: record.line(),
            timestamp_ms,
        };
        let _ = self.app.emit("app-error://created", event);
    }

    fn flush(&self) {}
}

fn is_missing_mod_icon_asset_error(record: &Record<'_>, message: &str) -> bool {
    if record.target() != "tauri::protocol::asset" {
        return false;
    }
    if !message.starts_with("File does not exist at path:") {
        return false;
    }
    let normalized = message.replace('\\', "/").to_ascii_lowercase();
    normalized.contains("/bepinex/plugins/")
        && (normalized.ends_with("/icon.png") || normalized.ends_with("/icon.png.old"))
}

pub fn init(app: &tauri::AppHandle) -> Result<(), AnyError> {
    // Avoid double-init (hot reload / multiple setup calls).
    if log::log_enabled!(log::Level::Error) && log::max_level() != LevelFilter::Off {
        // log crate doesn't expose "is initialized" reliably; log4rs will error if re-init.
        // We attempt init once and ignore subsequent errors below.
    }

    let (logs_dir, log_file) = log_paths(app)?;
    std::fs::create_dir_all(&logs_dir).map_err(|e| err(e.to_string()))?;
    startup_housekeeping(&logs_dir, &log_file);

    // 10MB per file, keep 5 rolled files.
    let roller = FixedWindowRoller::builder()
        .build(
            &logs_dir
                .join("hq-launcher.{}.log")
                .to_string_lossy()
                .to_string(),
            5,
        )
        .map_err(|e| err(e.to_string()))?;
    let policy = CompoundPolicy::new(
        Box::new(SizeTrigger::new(MAX_LOG_FILE_BYTES)),
        Box::new(roller),
    );

    let file_appender = RollingFileAppender::builder()
        .encoder(Box::new(PatternEncoder::new(
            "{d(%Y-%m-%d %H:%M:%S%.3f)} [{l}] {t} {M} - {m}{n}",
        )))
        .build(&log_file, Box::new(policy))
        .map_err(|e| err(e.to_string()))?;

    let error_events = ErrorEventAppender { app: app.clone() };

    let cfg_builder = {
        let cfg_builder = Config::builder()
            .appender(Appender::builder().build("file", Box::new(file_appender)))
            .appender(
                Appender::builder()
                    .build("error_events", Box::new(error_events) as Box<dyn Append>),
            );

        // In dev builds, also log to console for convenience.
        #[cfg(debug_assertions)]
        let cfg_builder = {
            use log4rs::append::console::ConsoleAppender;
            let stdout = ConsoleAppender::builder()
                .encoder(Box::new(PatternEncoder::new("[{l}] {m}{n}")))
                .build();
            cfg_builder.appender(Appender::builder().build("stdout", Box::new(stdout)))
        };

        cfg_builder
    };

    let root_builder = {
        let root_builder = Root::builder().appender("file").appender("error_events");
        #[cfg(debug_assertions)]
        let root_builder = root_builder.appender("stdout");
        root_builder
    };

    let cfg = cfg_builder
        .build(root_builder.build(LevelFilter::Info))
        .map_err(|e| err(e.to_string()))?;

    // Ignore error if already initialized.
    if log4rs::init_config(cfg).is_err() {
        return Ok(());
    }

    std::panic::set_hook(Box::new(|info| {
        log::error!("panic: {info}");
    }));

    log::info!("logger initialized");
    log::info!("log file: {}", log_file.to_string_lossy());
    Ok(())
}

use std::path::PathBuf;

use log::LevelFilter;
use log4rs::{
    append::{
        rolling_file::{
            policy::compound::{
                roll::fixed_window::FixedWindowRoller, trigger::size::SizeTrigger, CompoundPolicy,
            },
            RollingFileAppender,
        },
    },
    config::{Appender, Config, Root},
    encode::pattern::PatternEncoder,
};
use tauri::Manager;

type AnyError = Box<dyn std::error::Error>;

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

pub fn init(app: &tauri::AppHandle) -> Result<(), AnyError> {
    // Avoid double-init (hot reload / multiple setup calls).
    if log::log_enabled!(log::Level::Error) && log::max_level() != LevelFilter::Off {
        // log crate doesn't expose "is initialized" reliably; log4rs will error if re-init.
        // We attempt init once and ignore subsequent errors below.
    }

    let (logs_dir, log_file) = log_paths(app)?;
    std::fs::create_dir_all(&logs_dir).map_err(|e| err(e.to_string()))?;

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
    let policy = CompoundPolicy::new(Box::new(SizeTrigger::new(10 * 1024 * 1024)), Box::new(roller));

    let file_appender = RollingFileAppender::builder()
        .encoder(Box::new(PatternEncoder::new(
            "{d(%Y-%m-%d %H:%M:%S%.3f)} [{l}] {t} {M} - {m}{n}",
        )))
        .build(&log_file, Box::new(policy))
        .map_err(|e| err(e.to_string()))?;

    let cfg_builder = {
        let cfg_builder =
            Config::builder().appender(Appender::builder().build("file", Box::new(file_appender)));

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
        let root_builder = Root::builder().appender("file");
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


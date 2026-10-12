//! Supervise the external downloader without confusing CDN failures with Steam Guard.
use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tokio::process::Command;
use tokio::sync::mpsc;

#[derive(Debug, PartialEq)]
pub enum DownloadError {
    Cdn { host: String, result: String },
    Auth,
    Cancelled,
    Stalled,
    Process(String),
}

impl fmt::Display for DownloadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cdn { host, result } => write!(f, "Steam CDN authentication failed for {host} ({result}). Steam login is separate; please retry the download later or check your network/VPN."),
            Self::Auth => f.write_str("Steam Guard / login required. Please login and try again."),
            Self::Cancelled => f.write_str("Cancelled"),
            Self::Stalled => f.write_str("Download stalled (no output for 5 minutes). Check your network and retry."),
            Self::Process(message) => f.write_str(message),
        }
    }
}

pub fn cdn_failure(text: &str) -> Option<DownloadError> {
    let (_, response) = text.split_once("Got CDN auth token for ")?;
    let (host, response) = response.split_once(" result: ")?;
    let result = response.split_whitespace().next()?;
    if result == "OK" || host.is_empty() || host.chars().any(char::is_whitespace) {
        return None;
    }
    Some(DownloadError::Cdn { host: host.into(), result: result.into() })
}

pub fn login_confirmed(text: &str) -> bool {
    text.lines().any(|line| {
        line.trim().strip_prefix("Got ")
            .and_then(|line| line.strip_suffix(" licenses for account!"))
            .is_some_and(|count| count.parse::<u32>().is_ok())
    })
}

pub fn auth_prompt(text: &str) -> bool {
    let text = text.to_lowercase();
    if text.contains("pre-allocating") || text.contains('%') || text.contains("cdn auth") {
        return false;
    }
    text.contains("steam guard") || text.contains("two-factor") || text.contains("two factor")
        || text.contains("2fa") || (text.contains("enter") && (text.contains("code") || text.contains("password")))
        || text.contains("authentication code") || text.contains("emailed")
        || text.contains("use the steam mobile app to confirm")
        || text.contains("access token was rejected") || text.contains("unable to login to steam3:")
        || text.contains("failed to authenticate with steam:")
}

/// Keep enough PTY output to recognize confirmation even when a line is split across reads.
#[derive(Default)]
pub struct LoginOutput(String);
impl LoginOutput {
    pub fn push(&mut self, text: &str) -> bool {
        self.0.push_str(text);
        if login_confirmed(&self.0) { return true; }
        if self.0.len() > 16 * 1024 {
            let mut boundary = self.0.len() - 8 * 1024;
            while !self.0.is_char_boundary(boundary) { boundary += 1; }
            self.0.drain(..boundary);
        }
        false
    }
}

pub async fn run_attempt(
    mut command: Command,
    cancel: Option<&AtomicBool>,
    mut on_output: impl FnMut(bool, &str),
) -> Result<(), DownloadError> {
    if cancel.is_some_and(|flag| flag.load(Ordering::Relaxed)) { return Err(DownloadError::Cancelled); }
    let mut child = command
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn().map_err(|error| DownloadError::Process(format!("Failed to spawn DepotDownloader: {error}")))?;
    let (tx, mut rx) = mpsc::unbounded_channel();
    super::spawn_output_reader(child.stdout.take().unwrap(), false, tx.clone());
    super::spawn_output_reader(child.stderr.take().unwrap(), true, tx.clone());
    drop(tx);
    let mut last_output = Instant::now();
    let mut ticks = tokio::time::interval(Duration::from_millis(200));
    let mut status = None;
    let mut output_open = true;
    // Drain both pipes even if the process exits first, so its final CDN error is not lost.
    let failure = loop {
        if status.is_some() && !output_open { break None; }
        tokio::select! {
            result = child.wait(), if status.is_none() => {
                match result {
                    Ok(result) => status = Some(result),
                    Err(error) => break Some(DownloadError::Process(error.to_string())),
                }
            }
            msg = rx.recv(), if output_open => {
                let Some((is_stderr, line)) = msg else { output_open = false; continue; };
                last_output = Instant::now();
                let line = super::strip_ansi(&line);
                on_output(is_stderr, &line);
                if let Some(error) = cdn_failure(&line) { break Some(error); }
                if auth_prompt(&line) { break Some(DownloadError::Auth); }
            }
            _ = ticks.tick() => {
                if cancel.is_some_and(|flag| flag.load(Ordering::Relaxed)) { break Some(DownloadError::Cancelled); }
                if last_output.elapsed() > Duration::from_secs(300) { break Some(DownloadError::Stalled); }
            }
        }
    };
    if let Some(error) = failure {
        let _ = child.kill().await;
        let _ = child.wait().await;
        return Err(error);
    }
    let status = status.unwrap();
    if status.success() { Ok(()) } else {
        Err(DownloadError::Process(format!("DepotDownloader exited with {status}. Check the download log for details.")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cdn_failure_is_not_a_guard_prompt() {
        let line = "Got CDN auth token for cache1-seo1.steamcontent.com result: Fail (expires 01/01/1970 00:00:00)";
        assert!(matches!(cdn_failure(line), Some(DownloadError::Cdn { .. })));
        assert!(!auth_prompt(line));
        assert!(cdn_failure(&line.replace("result: Fail", "result: OK")).is_none());
        assert!(cdn_failure("Not a CDN response").is_none());
        assert!(auth_prompt("Access token was rejected (Expired)."));
        assert!(!auth_prompt("50.00% BepInEx/plugins/2FA/file.dll"));
    }
    #[test]
    fn login_requires_confirmed_licenses_even_with_split_pty_output() {
        let mut output = LoginOutput::default();
        assert!(!output.push("Logging in... Done!\nGot 266 lic"));
        assert!(output.push("enses for account!\r\nDownloading depot manifest"));
        assert!(!login_confirmed("Unable to get license list: Fail"));
        assert!(!login_confirmed("Got CDN auth token for cache result: Fail"));
    }
    #[cfg(unix)]
    fn shell(script: &str) -> Command {
        let mut command = Command::new("/bin/sh"); command.args(["-c", script]); command
    }
    #[cfg(unix)]
    #[tokio::test]
    async fn failed_cdn_does_not_wait_for_hung_child() {
        let result = tokio::time::timeout(Duration::from_secs(3), run_attempt(shell(
            "printf 'Got CDN auth token for cache1-seo1.steamcontent.com result: Fail (expires 01/01/1970)\n'; exec sleep 60"
        ), None, |_, _| {})).await.expect("must not hang");
        assert!(matches!(result, Err(DownloadError::Cdn { .. })));
    }
    #[cfg(unix)]
    #[tokio::test]
    async fn drains_final_output_and_preserves_non_auth_exit_errors() {
        let result = run_attempt(shell("printf 'Got CDN auth token for cache result: Fail\n' >&2; exit 1"), None, |_, _| {}).await;
        assert!(matches!(result, Err(DownloadError::Cdn { .. })));
        let result = run_attempt(shell("exit 7"), None, |_, _| {}).await;
        assert!(matches!(result, Err(DownloadError::Process(_))));
        assert!(run_attempt(shell("printf '100.00%% file.dll\n'"), None, |_, _| {}).await.is_ok());
    }
    #[cfg(unix)]
    #[tokio::test]
    async fn cancellation_stops_silent_download() {
        let cancel = AtomicBool::new(false);
        let download = run_attempt(shell("exec sleep 60"), Some(&cancel), |_, _| {});
        let trigger = async { tokio::time::sleep(Duration::from_millis(50)).await; cancel.store(true, Ordering::Relaxed); };
        let (result, _) = tokio::join!(download, trigger);
        assert_eq!(result, Err(DownloadError::Cancelled));
    }
}

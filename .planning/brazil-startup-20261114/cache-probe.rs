// Diagnostic: unchanged production cache; only its HTTP transport is simulated.
use std::sync::{mpsc, OnceLock};
use std::time::Duration;
mod release_channel {
    use super::OnceLock;
    pub static URL: OnceLock<String> = OnceLock::new();
    #[derive(Clone, Copy, PartialEq, Eq)]
    pub enum ReleaseChannel { Stable }
    pub fn current() -> ReleaseChannel { ReleaseChannel::Stable }
    impl ReleaseChannel { pub fn manifest_url(self) -> &'static str { URL.get().unwrap() } }
}
mod transport {
    use super::*;
    #[derive(Clone)] pub struct Client { pub started: mpsc::Sender<()> }
    pub struct Request { started: mpsc::Sender<()>, timeout: Duration }
    pub struct Response;
    impl Client {
        pub fn get(&self, _url: &str) -> Request {
            Request { started: self.started.clone(), timeout: Duration::from_secs(12) }
        }
    }
    impl Request {
        pub fn timeout(mut self, value: Duration) -> Self { self.timeout = value; self }
        pub async fn send(self) -> Result<Response, String> {
            self.started.send(()).unwrap();
            tokio::time::timeout(self.timeout, tokio::time::sleep(Duration::from_millis(500)))
                .await.map_err(|error| error.to_string())?;
            Ok(Response)
        }
    }
    impl Response {
        pub fn error_for_status(self) -> Result<Self, String> { Ok(self) }
        pub async fn json<T: serde::de::DeserializeOwned>(self) -> Result<T, String> {
            serde_json::from_str(r#"{"version":1,"chain_config":[],"mods":[]}"#)
                .map_err(|error| error.to_string())
        }
    }
}
mod production {
    use crate::transport as reqwest;
    include!("C:/Users/pAsta/hq-launcher/src-tauri/src/mod_config.rs");
    pub async fn probe(client: reqwest::Client) -> bool {
        ModsConfig::fetch_remote_manifest_cached(&client, None).await.is_ok()
    }
}
fn main() {
    let expect_stall = std::env::args().any(|arg| arg == "--expect-stall");
    release_channel::URL.set("http://simulated.local/manifest.json".into()).unwrap();
    let (started_tx, started_rx) = mpsc::channel();
    let client = transport::Client { started: started_tx };
    let runtime = tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap();
    let (done_tx, done_rx) = mpsc::channel();
    let first_client = client.clone();
    let first_tx = done_tx.clone();
    runtime.spawn(async move { first_tx.send(production::probe(first_client).await).unwrap(); });
    started_rx.recv_timeout(Duration::from_secs(3)).unwrap();
    for _ in 0..4 {
        let waiter_client = client.clone();
        let waiter_tx = done_tx.clone();
        runtime.spawn(async move { waiter_tx.send(production::probe(waiter_client).await).unwrap(); });
    }
    let mut completed = 0;
    let mut successful = 0;
    let wait = Duration::from_secs(if expect_stall { 13 } else { 3 });
    while completed < 5 {
        match done_rx.recv_timeout(wait) {
            Ok(success) => { completed += 1; successful += usize::from(success); }
            Err(_) => break,
        }
    }
    println!("TOKIO_WORKER_THREADS={} transport=simulated completed={completed}/5 successful={successful}/5 expect_stall={expect_stall}", std::env::var("TOKIO_WORKER_THREADS").unwrap_or_default());
    runtime.shutdown_timeout(Duration::from_millis(100));
    if expect_stall { assert_eq!(completed, 0, "expected production cache to stall"); }
    else { assert_eq!(successful, 5, "expected all requests to finish with extra workers"); }
}
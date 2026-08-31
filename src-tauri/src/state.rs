use crate::db::Pool;
use reqwest::Client;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::watch;
use tokio::sync::Mutex as AsyncMutex;
use tokio_cron_scheduler::JobScheduler;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

pub const USER_AGENT: &str = "DeadLinkSentinel/0.1 (+docs-link-checker)";

#[derive(Clone)]
pub struct AppState {
    pub db: Pool,
    pub http: Client,
    pub status_bind: Arc<std::sync::Mutex<String>>,
    pub status_shutdown: Arc<std::sync::Mutex<Option<watch::Sender<bool>>>>,
    pub scheduler: Arc<AsyncMutex<Option<JobScheduler>>>,
    pub job_ids: Arc<std::sync::Mutex<HashMap<i64, Uuid>>>,
    pub scan_tokens: Arc<std::sync::Mutex<HashMap<i64, CancellationToken>>>,
}

impl AppState {
    pub fn new(db: Pool) -> Self {
        let http = Client::builder()
            .user_agent(USER_AGENT)
            .redirect(reqwest::redirect::Policy::limited(10))
            .timeout(Duration::from_secs(30))
            .build()
            .expect("http client");
        Self {
            db,
            http,
            status_bind: Arc::new(std::sync::Mutex::new(String::new())),
            status_shutdown: Arc::new(std::sync::Mutex::new(None)),
            scheduler: Arc::new(AsyncMutex::new(None)),
            job_ids: Arc::new(std::sync::Mutex::new(HashMap::new())),
            scan_tokens: Arc::new(std::sync::Mutex::new(HashMap::new())),
        }
    }
}

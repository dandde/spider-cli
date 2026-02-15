use crate::state::StateManager;
use anyhow::Result;
use askama::Template;
use axum::{
    Router,
    extract::{Form, State},
    response::IntoResponse,
    routing::{get, post},
};
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

pub struct DashboardServer {
    state_manager: Arc<StateManager>,
}

struct AppState {
    state_manager: Arc<StateManager>,
    sites: RwLock<Vec<SiteDisplay>>,
    tokens: RwLock<HashMap<i64, tokio_util::sync::CancellationToken>>,
}

#[derive(Clone, Default)]
struct SiteDisplay {
    id: i64,
    url: String,
    entries: Vec<LogEntry>,
    finished: bool,
}

#[derive(Clone)]
struct LogEntry {
    status: String,
    url: String,
}

#[derive(Template)]
#[template(path = "index.html")]
struct IndexTemplate {}

#[derive(Template)]
#[template(path = "stats.html")]
struct StatsTemplate {
    sites: Vec<SiteDisplay>,
}

#[derive(Deserialize)]
struct StartParams {
    url: Option<String>,
    config: Option<String>,
}

#[derive(Deserialize)]
struct StopParams {
    id: i64,
}

impl DashboardServer {
    pub fn new(state_manager: Arc<StateManager>) -> Self {
        Self { state_manager }
    }

    pub async fn run(self, port: u16) -> Result<()> {
        let state = Arc::new(AppState {
            state_manager: self.state_manager,
            sites: RwLock::new(vec![]),
            tokens: RwLock::new(HashMap::new()),
        });

        let app = Router::new()
            .route("/", get(index))
            .route("/stats", get(stats))
            .route("/control/start", post(start_crawl))
            .route("/control/stop", post(stop_crawl))
            .with_state(state);

        let addr = format!("0.0.0.0:{}", port);
        let listener = tokio::net::TcpListener::bind(&addr).await?;
        tracing::info!("Dashboard running on http://{}", addr);
        axum::serve(listener, app).await?;
        Ok(())
    }
}

async fn index() -> impl IntoResponse {
    match (IndexTemplate {}).render() {
        Ok(html) => axum::response::Html(html).into_response(),
        Err(e) => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            format!("Template error: {}", e),
        )
            .into_response(),
    }
}

async fn stats(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let sites = state.sites.read().unwrap().clone();
    let template = StatsTemplate { sites };
    match template.render() {
        Ok(html) => axum::response::Html(html).into_response(),
        Err(e) => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            format!("Template error: {}", e),
        )
            .into_response(),
    }
}

async fn start_crawl(
    State(state): State<Arc<AppState>>,
    Form(params): Form<StartParams>,
) -> impl IntoResponse {
    let mut final_config =
        if let Some(config_path) = params.config.as_ref().filter(|s| !s.is_empty()) {
            match crate::config::ConfigLoader::load(config_path) {
                Ok(c) => c,
                Err(e) => {
                    return (
                        axum::http::StatusCode::BAD_REQUEST,
                        format!("Config Error: {}", e),
                    )
                        .into_response();
                }
            }
        } else {
            crate::config::SpiderConfig {
                name: "adhoc".to_string(),
                start_urls: params
                    .url
                    .as_ref()
                    .map(|u| vec![u.clone()])
                    .unwrap_or_default(),
                ..crate::config::SpiderConfig::default()
            }
        };

    // Override with URL if provided explicitly
    if let Some(u) = params.url.as_ref().filter(|s| !s.is_empty()) {
        final_config.start_urls = vec![u.clone()];
    }

    if final_config.start_urls.is_empty() {
        return (axum::http::StatusCode::BAD_REQUEST, "No start URL provided").into_response();
    }

    let url = final_config.start_urls[0].clone();

    // Create record in DB
    let crawl_id = state
        .state_manager
        .create_crawl(&format!("UI Crawl: {}", url))
        .await
        .unwrap_or(0);

    // Add to UI state
    {
        let mut sites = state.sites.write().unwrap();
        sites.push(SiteDisplay {
            id: crawl_id,
            url: url.clone(),
            entries: vec![],
            finished: false,
        });
    }

    // Register cancellation token
    let cancel_token = tokio_util::sync::CancellationToken::new();
    {
        let mut tokens = state.tokens.write().unwrap();
        tokens.insert(crawl_id, cancel_token.clone());
    }

    // Spawn Crawler Task
    let app_state = state.clone();
    let state_manager = state.state_manager.clone();
    tokio::spawn(async move {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let crawler = crate::crawler::Crawler::new(state_manager.clone(), crawl_id, vec![]);

        let selectors: HashMap<String, String> = if final_config.selectors.is_empty() {
            let mut s = HashMap::new();
            s.insert("title".to_string(), "title".to_string());
            s
        } else {
            final_config
                .selectors
                .into_iter()
                .map(|(k, v)| (k, v.to_query_string()))
                .collect()
        };

        let respect_robots = final_config.respect_robots;
        let delay = Some(final_config.delay_ms);
        let concurrency = final_config.concurrency;

        let crawler_cancel = cancel_token.clone();
        tokio::spawn(async move {
            if let Err(e) = crawler
                .run(
                    &url,
                    selectors,
                    true,
                    respect_robots,
                    delay,
                    concurrency,
                    final_config.blacklist,
                    final_config.whitelist,
                    final_config.max_depth,
                    Some(tx),
                    crawler_cancel,
                )
                .await
            {
                tracing::error!("Crawler background error: {}", e);
            }
        });

        // Listen to Crawler's status updates for UI logs
        while let Some(page_url) = rx.recv().await {
            {
                let mut sites = app_state.sites.write().unwrap();
                if let Some(site) = sites.iter_mut().find(|s| s.id == crawl_id) {
                    if site.entries.len() > 10 {
                        site.entries.remove(0);
                    }
                    site.entries.push(LogEntry {
                        status: "DONE".to_string(),
                        url: page_url,
                    });
                }
            }
        }

        // Mark as finished after the Crawler finishes (rx closes)
        {
            let mut sites = app_state.sites.write().unwrap();
            let mut tokens = app_state.tokens.write().unwrap();

            if let Some(site) = sites.iter_mut().find(|s| s.id == crawl_id) {
                site.finished = true;
            }
            tokens.remove(&crawl_id);
        }
    });

    "Crawl started".into_response()
}

async fn stop_crawl(
    State(state): State<Arc<AppState>>,
    Form(params): Form<StopParams>,
) -> impl IntoResponse {
    let tokens = state.tokens.read().unwrap();
    if let Some(token) = tokens.get(&params.id) {
        token.cancel();
        "Crawl stopping...".into_response()
    } else {
        (
            axum::http::StatusCode::NOT_FOUND,
            "Crawl not found or already stopped",
        )
            .into_response()
    }
}

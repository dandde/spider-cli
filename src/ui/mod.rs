use crate::state::StateManager;
use anyhow::Result;
use askama::Template;
use axum::{
    Router,
    extract::{Form, Path, State},
    response::IntoResponse,
    routing::{get, post},
};
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use tower_http::services::ServeDir;

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

#[derive(Template)]
#[template(path = "help.html")]
struct HelpTemplate {}

#[derive(Template)]
#[template(path = "hierarchy.html")]
struct HierarchyTemplate {
    crawl_id: i64,
    stats: String,
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
        let existing_crawls = self
            .state_manager
            .get_all_crawls()
            .await
            .unwrap_or_default();
        let mut initial_sites = vec![];
        for c in existing_crawls {
            let url = c
                .name
                .strip_prefix("Crawl: ")
                .or_else(|| c.name.strip_prefix("UI Crawl: "))
                .unwrap_or(&c.name)
                .to_string();
            initial_sites.push(SiteDisplay {
                id: c.id,
                url,
                entries: vec![],
                finished: true,
            });
        }

        let state = Arc::new(AppState {
            state_manager: self.state_manager,
            sites: RwLock::new(initial_sites),
            tokens: RwLock::new(HashMap::new()),
        });

        let app = Router::new()
            .route("/", get(index))
            .route("/help", get(help))
            .route("/stats", get(stats))
            .route("/hierarchy/:id", get(hierarchy))
            .route("/hierarchy/:id/json", get(hierarchy_json))
            .route("/control/start", post(start_crawl))
            .route("/control/stop", post(stop_crawl))
            .nest_service("/assets", ServeDir::new("assets"))
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

async fn stats(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> impl IntoResponse {
    // If not an HTMX request, redirect to home
    if !headers.contains_key("hx-request") {
        return axum::response::Redirect::to("/").into_response();
    }

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

async fn help() -> impl IntoResponse {
    match (HelpTemplate {}).render() {
        Ok(html) => axum::response::Html(html).into_response(),
        Err(e) => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            format!("Template error: {}", e),
        )
            .into_response(),
    }
}

async fn hierarchy(State(state): State<Arc<AppState>>, Path(id): Path<i64>) -> impl IntoResponse {
    match state.state_manager.get_results_urls(id).await {
        Ok(urls) => {
            let mut collection = crate::url_parser::UrlCollection::new();
            for url_str in &urls {
                if let Ok(url_ref) = crate::url_parser::UrlRef::from_str(url_str) {
                    let _ = collection.add(url_ref);
                }
            }

            // Redirect stdout to capture tree display or implement a string version
            // For now, let's use a simple string-based tree builder or similar
            // Actually, I can implement a method that returns the tree as a string in mod.rs

            // Re-use display logic but into a String
            // I'll add a helper to UrlCollection for this

            let template = HierarchyTemplate {
                crawl_id: id,
                stats: collection.stats(),
            };

            match template.render() {
                Ok(html) => axum::response::Html(html).into_response(),
                Err(e) => (
                    axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Template error: {}", e),
                )
                    .into_response(),
            }
        }
        Err(e) => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            format!("Database error: {}", e),
        )
            .into_response(),
    }
}

async fn hierarchy_json(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    match state.state_manager.get_results_urls(id).await {
        Ok(urls) => {
            let mut collection = crate::url_parser::UrlCollection::new();
            for url_str in &urls {
                if let Ok(url_ref) = crate::url_parser::UrlRef::from_str(url_str) {
                    let _ = collection.add(url_ref);
                }
            }
            axum::Json(collection).into_response()
        }
        Err(e) => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            format!("Database error: {}", e),
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

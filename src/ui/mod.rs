use axum::{
    routing::{get, post},
    Router,
    response::IntoResponse,
    extract::{State, Form},
};
use askama::Template;
use std::sync::{Arc, RwLock};
use crate::state::StateManager;
use anyhow::Result;
use serde::Deserialize;
use spider::website::Website;

pub struct DashboardServer {
    state_manager: Arc<StateManager>,
}

struct AppState {
    state_manager: Arc<StateManager>,
    sites: RwLock<Vec<SiteDisplay>>,
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
    url: String,
}

impl DashboardServer {
    pub fn new(state_manager: Arc<StateManager>) -> Self {
        Self { state_manager }
    }

    pub async fn run(self, port: u16) -> Result<()> {
        let state = Arc::new(AppState {
            state_manager: self.state_manager,
            sites: RwLock::new(vec![]),
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
        Err(e) => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, format!("Template error: {}", e)).into_response(),
    }
}

async fn stats(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let sites = state.sites.read().unwrap().clone();
    let template = StatsTemplate { sites };
    match template.render() {
        Ok(html) => axum::response::Html(html).into_response(),
        Err(e) => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, format!("Template error: {}", e)).into_response(),
    }
}

async fn start_crawl(
    State(state): State<Arc<AppState>>, 
    Form(params): Form<StartParams>
) -> impl IntoResponse {
    let url = params.url.clone();
    
    // Create record in DB
    let crawl_id = state.state_manager.create_crawl(&format!("UI Crawl: {}", url)).await.unwrap_or(0);
    
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

    // Spawn Crawler Task
    let app_state = state.clone();
    let state_manager = state.state_manager.clone();
    tokio::spawn(async move {
        let mut website = Website::new(&url);
        let mut rx = website.subscribe(16).unwrap();
        
        // Use Crawler's CacheManager and logic if needed, but here we simplify for the UI display
        tokio::spawn(async move {
            website.crawl().await;
        });

        while let Ok(res) = rx.recv().await {
            let page_url = res.get_url().to_string();
            {
                let mut sites = app_state.sites.write().unwrap();
                if let Some(site) = sites.iter_mut().find(|s| s.id == crawl_id) {
                    // Limit logs to keep it clean
                    if site.entries.len() > 10 {
                        site.entries.remove(0);
                    }
                    site.entries.push(LogEntry {
                        status: "DONE".to_string(),
                        url: page_url.clone(),
                    });
                }
            }
            // Also persist to DB via StateManager
            let _ = state_manager.save_result(crawl_id, &page_url, &serde_json::json!({"status": "DONE"})).await;
        }

        // Mark as finished after the channel closes
        {
            let mut sites = app_state.sites.write().unwrap();
            if let Some(site) = sites.iter_mut().find(|s| s.id == crawl_id) {
                site.finished = true;
            }
        }
    });

    "" // HTMX will handle refresh
}

async fn stop_crawl() -> impl IntoResponse {
    "Crawl stopped"
}

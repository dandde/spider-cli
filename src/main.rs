mod models;
mod state;
mod crawler;
mod ui;
mod features;

use anyhow::Result;
use clap::{Parser, Subcommand};
use tracing_subscriber::{EnvFilter, fmt};

use std::sync::Arc;
use std::collections::HashMap;

#[derive(Parser)]
#[command(name = "spider-cli")]
#[command(about = "High-performance, durable web crawler", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start a new crawl or resume an existing one
    Crawl {
        /// Target URL to crawl
        url: String,

        /// Respect robots.txt
        #[arg(short, long)]
        respect_robots: bool,

        /// Polite delay in milliseconds
        #[arg(short, long)]
        delay: Option<u64>,

        /// Number of concurrent requests
        #[arg(short, long, default_value_t = 1)]
        concurrency: usize,

        /// Run the web dashboard during the crawl
        #[arg(long)]
        dashboard: bool,
    },
    /// Just launch the monitoring dashboard
    Serve {
        /// Port to run the dashboard on
        #[arg(short, long, default_value_t = 3030)]
        port: u16,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive(tracing::Level::INFO.into()))
        .init();

    let cli = Cli::parse();

    // 1. Initialize State Manager
    let state_manager = Arc::new(state::StateManager::new("crawl_state.db").await?);

    match cli.command {
        Commands::Crawl { url, respect_robots, delay, concurrency, dashboard } => {
            tracing::info!("Starting spider-cli headless crawl: {}", url);

            let crawl_id = if let Some(id) = state_manager.get_active_crawl().await? {
                tracing::info!("Found active crawl, resuming ID: {}", id);
                id
            } else {
                let id = state_manager.create_crawl(&format!("Crawl: {}", url)).await?;
                tracing::info!("Created new crawl, ID: {}", id);
                id
            };

            if dashboard {
                let ds = ui::DashboardServer::new(state_manager.clone());
                tokio::spawn(async move {
                    if let Err(e) = ds.run(3030).await {
                        tracing::error!("Dashboard server error: {}", e);
                    }
                });
                tracing::info!("Dashboard active at http://localhost:3030");
            }

            let crawler = crawler::Crawler::new(state_manager.clone(), crawl_id, vec![]);
            let mut selectors = HashMap::new();
            selectors.insert("title".to_string(), "title".to_string());

            tokio::select! {
                res = crawler.run(&url, selectors, true, respect_robots, delay, concurrency) => {
                    if let Err(e) = res {
                        tracing::error!("Crawler error: {}", e);
                    }
                    if dashboard {
                        tracing::info!("Crawl finished. Dashboard remains active at http://localhost:3030. Press Ctrl+C to stop.");
                        tokio::signal::ctrl_c().await?;
                    }
                }
                _ = tokio::signal::ctrl_c() => {
                    tracing::info!("Shutting down gracefully...");
                }
            }
        }
        Commands::Serve { port } => {
            tracing::info!("Starting spider-cli Flawless Dashboard...");
            let ds = ui::DashboardServer::new(state_manager.clone());
            ds.run(port).await?;
        }
    }

    Ok(())
}

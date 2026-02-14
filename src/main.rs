mod models;
mod state;
mod crawler;
mod ui;
mod features;
mod config;

use anyhow::Result;
use clap::{Parser, Subcommand};
use tracing_subscriber::{EnvFilter, fmt};

use std::sync::Arc;
use std::collections::HashMap;
use std::path::PathBuf;

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
        url: Option<String>,

        /// Path to a configuration file
        #[arg(short, long)]
        config: Option<PathBuf>,

        /// Respect robots.txt
        #[arg(short, long)]
        respect_robots: bool,

        /// Polite delay in milliseconds
        #[arg(short, long)]
        delay: Option<u64>,

        /// Number of concurrent requests
        #[arg(short = 'j', long, default_value_t = 1)]
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
        Commands::Crawl { url, config, respect_robots, delay, concurrency, dashboard } => {
            let mut final_config = if let Some(config_path) = config {
                config::ConfigLoader::load(config_path)?
            } else {
                config::SpiderConfig {
                    name: "default".to_string(),
                    start_urls: vec![],
                    selectors: HashMap::new(),
                    concurrency,
                    delay_ms: delay.unwrap_or(0),
                    respect_robots,
                    extends: None,
                }
            };

            // Override with CLI flags if provided
            if let Some(u) = &url {
                final_config.start_urls = vec![u.clone()];
            }
            if respect_robots {
                final_config.respect_robots = true;
            }
            if let Some(d) = delay {
                final_config.delay_ms = d;
            }
            if concurrency != 1 {
                final_config.concurrency = concurrency;
            }

            if final_config.start_urls.is_empty() {
                anyhow::bail!("No start URL provided. Please provide a URL or a config file with start_urls.");
            }

            let first_url = final_config.start_urls[0].clone();
            tracing::info!("Starting spider-cli crawl for: {}", first_url);

            let crawl_id = if let Some(id) = state_manager.get_active_crawl().await? {
                tracing::info!("Found active crawl, resuming ID: {}", id);
                id
            } else {
                let id = state_manager.create_crawl(&format!("Crawl: {}", first_url)).await?;
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
            
            let selectors = if final_config.selectors.is_empty() {
                let mut s = HashMap::new();
                s.insert("title".to_string(), "title".to_string());
                s
            } else {
                final_config.selectors.into_iter().map(|(k, v)| (k, v.to_query_string())).collect()
            };

            tokio::select! {
                res = crawler.run(&first_url, selectors, true, final_config.respect_robots, Some(final_config.delay_ms), final_config.concurrency, None) => {
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

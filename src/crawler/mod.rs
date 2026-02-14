use crate::features::cache::CacheManager;
use crate::features::proxy::ProxyManager;
use crate::state::StateManager;
use chadselect::ChadSelect;
use spider::website::Website;
use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;

pub struct Crawler {
    state_manager: Arc<StateManager>,
    proxy_manager: Option<Arc<ProxyManager>>,
    cache_manager: Arc<CacheManager>,
    crawl_id: i64,
}

use tokio::sync::mpsc::UnboundedSender;

impl Crawler {
    pub fn new(state_manager: Arc<StateManager>, crawl_id: i64, proxies: Vec<String>) -> Self {
        let proxy_manager = if proxies.is_empty() {
            None
        } else {
            Some(Arc::new(ProxyManager::new(proxies)))
        };

        Self {
            state_manager,
            proxy_manager,
            cache_manager: Arc::new(CacheManager::new()),
            crawl_id,
        }
    }

    pub async fn run(
        &self, 
        start_url: &str, 
        selectors: HashMap<String, String>, 
        resume: bool,
        respect_robots: bool,
        delay: Option<u64>,
        _concurrency: usize,
        status_tx: Option<UnboundedSender<String>>,
    ) -> Result<()> {
        let mut website: Website = Website::new(start_url);
        
        // Configuration
        website.configuration.respect_robots_txt = respect_robots;
        // website.configuration.concurrency = concurrency; // Field not found in 2.0 Configuration
        if let Some(d) = delay {
            website.configuration.delay = d;
        }
        if let Some(proxy_manager) = &self.proxy_manager {
            if let Some(_proxy) = proxy_manager.get_next() {
                // In spider 2.0, proxies might be a Vec or a different field.
                // Estimating 'proxies' based on common plural patterns in recent spider versions.
                // website.configuration.proxies = Some(vec![proxy.clone()]);
            }
        }

        if resume {
            tracing::info!("Resuming crawl from database...");
            let pending = self.state_manager.get_pending_frontier(self.crawl_id, 1000).await?;
            for (_, _url, _) in pending {
                // Seed spider's internal frontier if possible, or just scrape
                // For now, we'll use start_urls if we can't deep-seed
            }
        } else {
            // Initial seed
            self.state_manager.add_to_frontier(self.crawl_id, vec![(start_url.to_string(), 0)]).await?;
        }

        let mut rx2 = website.subscribe(8).unwrap();
        
        // Start crawling
        tokio::spawn(async move {
            website.crawl().await;
        });

        // Process discovered pages
        while let Ok(res) = rx2.recv().await {
            let url = res.get_url().to_string();
            
            if self.cache_manager.is_cached(&url) {
                continue;
            }
            
            let html = res.get_html();
            
            // 1. Mark as processing in DB (Simplified: we track results and links)
            
            let extracted_data = {
                let mut cs = ChadSelect::new();
                cs.add_html(html);

                let mut data = serde_json::Map::new();
                for (name, selector) in &selectors {
                    let val = cs.select(0, selector);
                    if !val.is_empty() {
                        data.insert(name.clone(), serde_json::json!(val));
                    }
                }
                data
            };

            self.state_manager.save_result(self.crawl_id, &url, &serde_json::Value::Object(extracted_data)).await?;
            self.cache_manager.cache(url.clone());

            if let Some(tx) = &status_tx {
                let _ = tx.send(url.clone());
            }

            tracing::info!("Processed and persisted: {}", url);
        }

        Ok(())
    }
}

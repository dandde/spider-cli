use crate::features::cache::CacheManager;
use crate::features::proxy::ProxyManager;
use crate::state::StateManager;
use anyhow::Result;
use chadselect::ChadSelect;
use spider::website::Website;
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
        blacklist: Vec<String>,
        whitelist: Vec<String>,
        max_depth: Option<usize>,
        status_tx: Option<UnboundedSender<String>>,
        cancel_token: tokio_util::sync::CancellationToken,
    ) -> Result<()> {
        let mut website: Website = Website::new(start_url);

        tracing::info!(
            "Crawler::run config - depth: {:?}, whitelist: {:?}, blacklist: {:?}",
            max_depth,
            whitelist,
            blacklist
        );

        // Configuration
        website.configuration.respect_robots_txt = respect_robots;
        if let Some(depth) = max_depth {
            website.configuration.depth = depth;
        }

        if !blacklist.is_empty() {
            website.with_blacklist_url(Some(blacklist.iter().map(|s| s.clone().into()).collect()));
        }
        if !whitelist.is_empty() {
            website.with_whitelist_url(Some(whitelist.iter().map(|s| s.clone().into()).collect()));
        }
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

        if let Ok(visited) = self.state_manager.get_visited_urls(self.crawl_id).await {
            self.cache_manager.extend(visited);
        }

        if resume {
            tracing::info!("Resuming crawl from database...");
            if let Ok(pending) = self
                .state_manager
                .get_pending_frontier(self.crawl_id, 1000)
                .await
            {
                // In a mature implementation, we'd seed these directly into the underlying crawler.
                // For now, we rely on the database uniqueness to avoid duplicates.
                tracing::info!("Found {} pending URLs to resume.", pending.len());
            }
        } else {
            // Initial seed
            self.state_manager
                .add_to_frontier(self.crawl_id, vec![(start_url.to_string(), 0)])
                .await?;
        }

        let mut rx2 = website.subscribe(8).unwrap();

        // Start crawling
        let website_handle = tokio::spawn(async move {
            website.crawl().await;
        });

        // Process discovered pages
        loop {
            tokio::select! {
                _ = cancel_token.cancelled() => {
                    tracing::info!("Crawl cancelled by token.");
                    website_handle.abort();
                    break;
                }
                res = rx2.recv() => {
                    match res {
                        Ok(res) => {
                            let raw_url = res.get_url().to_string();
                            let url = crate::url_parser::normalize_url(&raw_url);

                            if self.cache_manager.is_cached(&url) {
                                continue;
                            }

                            let html = res.get_html();

                            let extracted_data = {
                                let mut cs = ChadSelect::new();
                                cs.add_html(html);

                                let mut data = serde_json::Map::new();
                                for (name, selector) in &selectors {
                                    let final_selector = if selector.starts_with("css:")
                                        || selector.starts_with("xpath:")
                                        || selector.starts_with("regex:")
                                    {
                                        selector.clone()
                                    } else {
                                        format!("css:{}", selector)
                                    };

                                    let val = cs.select(0, &final_selector);
                                    // Note: chadselect warns if select(0, ...) is called on 0 results.
                                    // In a production environment, we should use a safer API if available.
                                    if !val.is_empty() {
                                        data.insert(name.clone(), serde_json::json!(val));
                                    }
                                }
                                data
                            };

                            self.state_manager
                                .save_result(
                                    self.crawl_id,
                                    &url,
                                    &serde_json::Value::Object(extracted_data),
                                )
                                .await?;
                            self.cache_manager.cache(url.clone());

                            if let Some(tx) = &status_tx {
                                let _ = tx.send(url.clone());
                            }

                            tracing::info!("Processed and persisted: {}", url);
                        }
                        Err(_) => break, // Channel closed/end of crawl
                    }
                }
            }
        }

        Ok(())
    }
}

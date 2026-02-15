use std::collections::HashSet;
use std::sync::RwLock;

pub struct CacheManager {
    visited_results: RwLock<HashSet<String>>,
}

impl CacheManager {
    pub fn new() -> Self {
        Self {
            visited_results: RwLock::new(HashSet::new()),
        }
    }

    pub fn is_cached(&self, url: &str) -> bool {
        let cache = self.visited_results.read().unwrap();
        cache.contains(url)
    }

    pub fn cache(&self, url: String) {
        let mut cache = self.visited_results.write().unwrap();
        cache.insert(url);
    }

    pub fn extend(&self, urls: Vec<String>) {
        let mut cache = self.visited_results.write().unwrap();
        cache.extend(urls);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_functionality() {
        let manager = CacheManager::new();
        let url = "http://example.com".to_string();

        assert!(!manager.is_cached(&url));
        manager.cache(url.clone());
        assert!(manager.is_cached(&url));
    }
}

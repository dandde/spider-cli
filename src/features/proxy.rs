use std::sync::atomic::{AtomicUsize, Ordering};

pub struct ProxyManager {
    proxies: Vec<String>,
    current: AtomicUsize,
}

impl ProxyManager {
    pub fn new(proxies: Vec<String>) -> Self {
        Self {
            proxies,
            current: AtomicUsize::new(0),
        }
    }

    pub fn get_next(&self) -> Option<&String> {
        if self.proxies.is_empty() {
            return None;
        }
        let idx = self.current.fetch_add(1, Ordering::SeqCst) % self.proxies.len();
        Some(&self.proxies[idx])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_proxy_rotation() {
        let proxies = vec![
            "http://proxy1:8080".to_string(),
            "http://proxy2:8080".to_string(),
        ];
        let manager = ProxyManager::new(proxies);

        assert_eq!(manager.get_next().unwrap(), "http://proxy1:8080");
        assert_eq!(manager.get_next().unwrap(), "http://proxy2:8080");
        assert_eq!(manager.get_next().unwrap(), "http://proxy1:8080");
    }

    #[test]
    fn test_empty_proxy_manager() {
        let manager = ProxyManager::new(vec![]);
        assert!(manager.get_next().is_none());
    }
}

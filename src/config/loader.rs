use crate::config::schema::SpiderConfig;
use anyhow::{Context, Result};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use validator::Validate;

pub struct ConfigLoader;

impl ConfigLoader {
    pub fn load<P: AsRef<Path>>(path: P) -> Result<SpiderConfig> {
        let path = path.as_ref();
        let mut visited = HashSet::new();
        Self::load_with_inheritance(path, &mut visited, false)
    }

    fn load_with_inheritance(
        path: &Path,
        visited: &mut HashSet<PathBuf>,
        is_parent_load: bool,
    ) -> Result<SpiderConfig> {
        let path = fs::canonicalize(path)
            .with_context(|| format!("Failed to canonicalize path: {}", path.display()))?;

        if visited.contains(&path) {
            anyhow::bail!("Circular inheritance detected involving {}", path.display());
        }
        visited.insert(path.clone());

        let config = Self::load_file(&path)?;

        let final_config = if let Some(parent_path_str) = &config.extends {
            let parent_path = path
                .parent()
                .context("Cannot determine parent directory")?
                .join(parent_path_str);

            let parent_config = Self::load_with_inheritance(&parent_path, visited, true)?;
            Self::merge_configs(parent_config, config)
        } else {
            config
        };

        if !is_parent_load {
            final_config.validate()?;
        }

        Ok(final_config)
    }

    fn load_file(path: &Path) -> Result<SpiderConfig> {
        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read config file: {}", path.display()))?;

        match path.extension().and_then(|ext| ext.to_str()) {
            Some("json") => {
                let config: SpiderConfig = serde_json::from_str(&content)?;
                Ok(config)
            }
            Some("yaml") | Some("yml") => {
                let config: SpiderConfig = serde_yaml::from_str(&content)?;
                Ok(config)
            }
            Some("toml") => {
                let config: SpiderConfig = toml::from_str(&content)?;
                Ok(config)
            }
            _ => anyhow::bail!("Unsupported config file extension: {}", path.display()),
        }
    }

    fn merge_configs(mut parent: SpiderConfig, child: SpiderConfig) -> SpiderConfig {
        if !child.name.is_empty() {
            parent.name = child.name;
        }
        if !child.start_urls.is_empty() {
            parent.start_urls = child.start_urls;
        }
        if child.concurrency != 1 {
            parent.concurrency = child.concurrency;
        }
        if child.delay_ms != 0 {
            parent.delay_ms = child.delay_ms;
        }
        if child.respect_robots {
            parent.respect_robots = child.respect_robots;
        }

        if !child.blacklist.is_empty() {
            parent.blacklist = child.blacklist;
        }
        if !child.whitelist.is_empty() {
            parent.whitelist = child.whitelist;
        }
        if child.max_depth.is_some() {
            parent.max_depth = child.max_depth;
        }

        for (key, val) in child.selectors {
            parent.selectors.insert(key, val);
        }

        parent.extends = None;
        parent
    }
}

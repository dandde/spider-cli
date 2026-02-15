use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use validator::Validate;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SelectorConfig {
    Simple(String),
    Advanced {
        selector: String,
        #[serde(default)]
        attr: Option<String>,
    },
}

impl Default for SelectorConfig {
    fn default() -> Self {
        SelectorConfig::Simple(String::new())
    }
}

impl SelectorConfig {
    pub fn to_query_string(&self) -> String {
        match self {
            SelectorConfig::Simple(s) => s.clone(),
            SelectorConfig::Advanced { selector, .. } => selector.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate, Default)]
pub struct SpiderConfig {
    #[serde(default)]
    #[validate(length(min = 1))]
    pub name: String,

    #[serde(default)]
    #[validate(length(min = 1))]
    pub start_urls: Vec<String>,

    #[serde(default)]
    pub selectors: HashMap<String, SelectorConfig>,

    #[serde(default = "default_concurrency")]
    pub concurrency: usize,

    #[serde(default = "default_delay")]
    pub delay_ms: u64,

    #[serde(default)]
    pub respect_robots: bool,

    #[serde(default)]
    pub blacklist: Vec<String>,

    #[serde(default)]
    pub whitelist: Vec<String>,

    #[serde(default)]
    pub max_depth: Option<usize>,

    /// Optional path to a parent configuration file to inherit from
    #[serde(default)]
    pub extends: Option<String>,
}

fn default_concurrency() -> usize {
    1
}

fn default_delay() -> u64 {
    0
}

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use url::Url;

/// Node type for hierarchical structure
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeType {
    /// Domain level (root or sub-root)
    Domain,
    /// Folder/Path segment
    Folder,
    /// Final leaf/File
    File,
}

/// Zero-Copy URL Components Using Lifetimes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct UrlRef<'a> {
    /// Full original URL
    pub full_url: &'a str,
    /// Protocol portion (e.g., "https")
    pub protocol: &'a str,
    /// Subdomain portion (e.g., "blog")
    pub subdomain: &'a str,
    /// Domain portion (e.g., "example.com")
    pub domain: &'a str,
    /// Hostname portion (e.g., "blog.example.com")
    pub hostname: &'a str,
    /// Path portion (e.g., "/folder/page.html")
    pub path: &'a str,
    /// Query portion (e.g., "?item=1")
    pub query: &'a str,
    /// Fragment portion (e.g., "#section2")
    pub fragment: &'a str,
    /// Path depth
    pub depth: usize,
}

impl<'a> UrlRef<'a> {
    pub fn from_str(full_url: &'a str) -> Result<Self> {
        // Use url crate for validation and initial component identification
        let parsed = Url::parse(full_url)?;

        // Extract references from original string to ensure zero-copy and 'a lifetime
        let protocol_str = parsed.scheme();
        let protocol = if !protocol_str.is_empty() {
            let start = full_url
                .find(protocol_str)
                .ok_or_else(|| anyhow::anyhow!("Protocol not found in URL"))?;
            &full_url[start..start + protocol_str.len()]
        } else {
            ""
        };

        let hostname_str = parsed.host_str().unwrap_or("");
        let hostname = if !hostname_str.is_empty() {
            let start = full_url
                .find(hostname_str)
                .ok_or_else(|| anyhow::anyhow!("Hostname not found in URL"))?;
            &full_url[start..start + hostname_str.len()]
        } else {
            ""
        };

        // Parse domain and subdomain (zero-copy)
        let (subdomain, domain) = Self::parse_domain(hostname);

        let path_start = if !hostname.is_empty() {
            full_url.find(hostname).unwrap() + hostname.len()
        } else {
            full_url.find(':').unwrap_or(0) + 1
        };

        let query_start = full_url.find('?').unwrap_or(full_url.len());
        let fragment_start = full_url.find('#').unwrap_or(full_url.len());

        let end_of_path = query_start.min(fragment_start);
        let path = &full_url[path_start..end_of_path];

        let query = if query_start < fragment_start {
            &full_url[query_start..fragment_start]
        } else {
            ""
        };

        let fragment = if fragment_start < full_url.len() {
            &full_url[fragment_start..]
        } else {
            ""
        };

        let depth = path.split('/').filter(|s| !s.is_empty()).count();

        Ok(UrlRef {
            full_url,
            protocol,
            subdomain,
            domain,
            hostname,
            path,
            query,
            fragment,
            depth,
        })
    }

    /// Split path into segments, filtering out empty ones
    pub fn path_segments(&self) -> Vec<&'a str> {
        self.path.split('/').filter(|s| !s.is_empty()).collect()
    }

    /// Parse domain into subdomain and domain (zero-copy)
    fn parse_domain(hostname: &'a str) -> (&'a str, &'a str) {
        let parts: Vec<&str> = hostname.split('.').collect();

        match parts.len() {
            0 => ("", ""),
            1 => ("", hostname),
            2 => ("", hostname),
            _ => {
                // Heuristic: last two parts are the domain (e.g., example.com)
                // In production, one might use a Public Suffix List
                if let Some(pos) = hostname.rfind('.') {
                    if let Some(prev_pos) = hostname[..pos].rfind('.') {
                        let domain_start = prev_pos + 1;
                        let subdomain = &hostname[..prev_pos];
                        let domain = &hostname[domain_start..];
                        return (subdomain, domain);
                    }
                }
                ("", hostname)
            }
        }
    }

    /// Normalize URL for deduplication
    pub fn normalize(&self) -> String {
        let mut path = self.path;
        if path.is_empty() {
            path = "/";
        }

        // Trim trailing slash for non-root paths
        let trimmed_path = if path.len() > 1 && path.ends_with('/') {
            &path[..path.len() - 1]
        } else {
            path
        };

        // Normalize query: sort parameters
        let mut query_part = String::new();
        if !self.query.is_empty() && self.query.len() > 1 {
            let mut params: Vec<(&str, &str)> = Vec::new();
            let query_str = &self.query[1..]; // skip '?'
            for pair in query_str.split('&') {
                if let Some(pos) = pair.find('=') {
                    params.push((&pair[..pos], &pair[pos + 1..]));
                } else {
                    params.push((pair, ""));
                }
            }
            params.sort_by(|a, b| a.0.cmp(b.0));

            if !params.is_empty() {
                query_part.push('?');
                for (i, (k, v)) in params.iter().enumerate() {
                    if i > 0 {
                        query_part.push('&');
                    }
                    if v.is_empty() {
                        query_part.push_str(k);
                    } else {
                        query_part.push_str(&format!("{}={}", k, v));
                    }
                }
            }
        }

        format!(
            "{}://{}{}{}",
            self.protocol.to_lowercase(),
            self.hostname.to_lowercase(),
            trimmed_path,
            query_part
        )
    }
}

pub fn normalize_url(url: &str) -> String {
    match UrlRef::from_str(url) {
        Ok(u) => u.normalize(),
        Err(_) => url.to_string(),
    }
}

/// Hierarchical Tree Node Using Zero-Copy Design
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreeNode<'a> {
    pub name: &'a str,
    pub node_type: NodeType,
    #[serde(borrow)]
    pub children: HashMap<&'a str, Box<TreeNode<'a>>>,
    #[serde(borrow)]
    pub urls: Vec<UrlRef<'a>>,
}

impl<'a> TreeNode<'a> {
    pub fn new(name: &'a str, node_type: NodeType) -> Self {
        TreeNode {
            name,
            node_type,
            children: HashMap::new(),
            urls: Vec::new(),
        }
    }

    /// Insert URL into tree (zero-copy structure)
    pub fn insert(&mut self, url: UrlRef<'a>) {
        let segments = url.path_segments();
        let mut current = self;

        for (i, segment) in segments.iter().enumerate() {
            let node_type = if i == segments.len() - 1 {
                NodeType::File
            } else {
                NodeType::Folder
            };

            current
                .children
                .entry(segment)
                .or_insert_with(|| Box::new(TreeNode::new(segment, node_type)));

            current = current.children.get_mut(segment).unwrap();
        }

        current.urls.push(url);
    }

    /// Display tree (zero-copy: uses references)
    pub fn display(&self, prefix: &str, is_last: bool, is_root: bool) {
        print!("{}", self.render_to_string(prefix, is_last, is_root));
    }

    /// Render tree to string (zero-copy: involves recursive prefix tracking)
    pub fn render_to_string(&self, prefix: &str, is_last: bool, is_root: bool) -> String {
        let mut output = String::new();

        // Node marker and name
        if !is_root {
            let marker = if is_last { "└── " } else { "├── " };
            output.push_str(&format!("{}{}", prefix, marker));
        }

        let icon = match self.node_type {
            NodeType::Domain => "",
            NodeType::Folder => "",
            NodeType::File => "",
        };

        let url_count = if is_root {
            "".to_string()
        } else {
            format!(" ({})", self.urls.len())
        };

        // Note: Using a space after the icon for better alignment
        output.push_str(&format!("{} {}{}\n", icon, self.name, url_count));

        // Prepare prefix for children
        let new_prefix = if is_root {
            "".to_string()
        } else {
            format!("{}{}", prefix, if is_last { "    " } else { "│   " })
        };

        let mut children_vec: Vec<_> = self.children.values().collect();
        // Sort children by name for deterministic output
        children_vec.sort_by(|a, b| a.name.cmp(b.name));

        for (i, child) in children_vec.iter().enumerate() {
            let last_child = i == children_vec.len() - 1;
            output.push_str(&child.render_to_string(&new_prefix, last_child, false));
        }

        output
    }
}

/// Collection of unique URLs and their hierarchies
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct UrlCollection<'a> {
    #[serde(borrow)]
    pub unique_urls: HashMap<String, UrlRef<'a>>,
    #[serde(borrow)]
    pub hierarchies: HashMap<&'a str, TreeNode<'a>>,
}

impl<'a> UrlCollection<'a> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, url: UrlRef<'a>) -> Result<()> {
        let normalized = url.normalize();
        if !self.unique_urls.contains_key(&normalized) {
            self.unique_urls.insert(normalized, url);

            // Add to hierarchy using hostname as root
            let host = url.hostname;
            let root = self
                .hierarchies
                .entry(host)
                .or_insert_with(|| TreeNode::new(host, NodeType::Domain));
            root.insert(url);
        }
        Ok(())
    }

    pub fn unique_count(&self) -> usize {
        self.unique_urls.len()
    }

    pub fn stats(&self) -> String {
        let mut stats = format!("  Total Unique URLs:    {}\n", self.unique_count());
        stats.push_str(&format!(
            "  Total Domains:        {}\n",
            self.hierarchies.len()
        ));

        let mut total_depth = 0;
        for url in self.unique_urls.values() {
            total_depth += url.depth;
        }

        if !self.unique_urls.is_empty() {
            stats.push_str(&format!(
                "  Average Depth:        {:.2}\n",
                total_depth as f64 / self.unique_urls.len() as f64
            ));
        }

        stats
    }

    pub fn display_trees(&self) {
        let mut hierarchies_vec: Vec<_> = self.hierarchies.values().collect();
        hierarchies_vec.sort_by(|a, b| a.name.cmp(b.name));

        for (i, root) in hierarchies_vec.iter().enumerate() {
            let last = i == hierarchies_vec.len() - 1;
            root.display("", last, true);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalization() {
        let url1 = "https://quotes.toscrape.com/tag/age/page/1/";
        let url2 = "https://quotes.toscrape.com/tag/age/page/1";
        let url3 = "https://quotes.toscrape.com/tag/age/page/1?b=2&a=1";

        assert_eq!(normalize_url(url1), normalize_url(url2));
        assert_eq!(
            normalize_url(url3),
            "https://quotes.toscrape.com/tag/age/page/1?a=1&b=2"
        );
        println!("Normalized URL: {}", normalize_url(url3));
    }

    #[test]
    fn test_url_components() {
        let url = "https://blog.example.com/path/to/page.html?q=1#section";
        let u = UrlRef::from_str(url).unwrap();

        println!(
            "URL components:\nProtocol: {}\nHostname: {}\nSubdomain: {}\nDomain: {}\nPath: {}\nQuery: {}\nFragment: {}\nDepth: {}",
            u.protocol, u.hostname, u.subdomain, u.domain, u.path, u.query, u.fragment, u.depth
        );
        println!(
            "JSON representation:\n{}",
            serde_json::to_string_pretty(&u).unwrap()
        );

        assert_eq!(u.protocol, "https");
        assert_eq!(u.hostname, "blog.example.com");
        assert_eq!(u.subdomain, "blog");
        assert_eq!(u.domain, "example.com");
        assert_eq!(u.path, "/path/to/page.html");
        assert_eq!(u.query, "?q=1");
        assert_eq!(u.fragment, "#section");
        assert_eq!(u.depth, 3);

        let root = "https://example.com/";
        let u_root = UrlRef::from_str(root).unwrap();
        assert_eq!(u_root.depth, 0);
        assert_eq!(u_root.subdomain, "");
        assert_eq!(u_root.domain, "example.com");
    }

    #[test]
    fn test_hierarchical_tree() {
        let mut collection = UrlCollection::new();

        let urls = vec![
            "https://example.com/blog/posts/rust.html",
            "https://example.com/blog/posts/python.html",
            "https://example.com/about",
            "https://api.example.com/v1/users",
        ];

        for url_str in &urls {
            let url_ref = UrlRef::from_str(url_str).unwrap();
            collection.add(url_ref).unwrap();
        }

        assert_eq!(collection.unique_count(), 4);
        assert_eq!(collection.hierarchies.len(), 2);

        println!("📊 Collection Stats:\n{}", collection.stats());
        println!("🌳 Tree Structure:");
        collection.display_trees();

        /* println!(
            "JSON representation:\n{}",
            serde_json::to_string_pretty(&collection).unwrap()
        ); */

        let root = &collection.hierarchies["example.com"];
        assert_eq!(root.name, "example.com");
        assert_eq!(root.children.len(), 2);
    }
}

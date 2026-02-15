# 🕷️ Spider CLI

A powerful, high-performance web crawler and scraper CLI built on top of the `spider` engine. Feature-rich, zero-copy architecture, and persistent state management.

## ✨ Features

- **🚀 High Performance**: Built with `spider` 2.0 and a zero-copy URL parser for maximum efficiency.
- **📂 Persistent State**: Integrated SQLite backend for resuming crawls and deduplicating results.
- **🛠️ Flexible Configuration**: Supports JSON, YAML, and TOML with inheritance (`extends`).
- **📊 Interactive Dashboard**: Built-in web server with a real-time crawl dashboard.
- **🛑 Graceful Control**: Real-time stop functionality and depth-limited crawling.
- **🔍 Advanced Extraction**: CSS and XPath selector support with automatic prefixing.

## 🚀 Getting Started

### Installation

```bash
cargo build --release
```

## 🤖 Automation Workflows

We provide three primary automation scripts for the pipeline:

### 1. `run_test_build.sh`
Executes unit tests and starts a standard crawl.

### 2. `verify_dashboard.sh`
A comprehensive verification tool that checks syntax, tests logic, builds release, and performs a live connectivity check on the dashboard.

### 3. `start_dashboard.sh`
Launches the monitoring server independently. Automatically kills port 3030 if in use.

### Usage:
```bash
# Full verification
cd spider-cli && ./verify_dashboard.sh

# Start the dashboard
cd spider-cli && ./start_dashboard.sh
```

## 🏃 Manual Execution

```bash
# To crawl a specific site headlessly
cargo run -- crawl https://example.com --delay 500

# To start the Flawless Dashboard
cargo run -- serve --port 3030
```
Then open [http://localhost:3030](http://localhost:3030) in your browser.

### Basic Usage

**Ad-hoc Crawl:**
```bash
./target/release/spider-cli crawl --url https://example.com --selector title="css:title"
```

**Config-based Crawl:**
```bash
./target/release/spider-cli crawl --config configs/hacker_news.json
```

**Dashboard Mode:**
```bash
./target/release/spider-cli serve --port 3030
```

## ⚙️ Configuration

Spider CLI supports hierarchical configuration via JSON, YAML, or TOML.

## ⚙️ Configuration Examples

Spider CLI supports hierarchical configuration. Below are examples of the same complex job in different formats.

````carousel
```json
{
  "name": "hacker-news-complex",
  "start_urls": ["https://news.ycombinator.com"],
  "selectors": {
    "headline": "css:.titleline > a",
    "score": "css:.score",
    "user": "css:.hnuser",
    "link": {
      "selector": "xpath://span[@class='titleline']/a",
      "attr": "href"
    }
  },
  "concurrency": 5,
  "max_depth": 2,
  "respect_robots": true
}
```
<!-- slide -->
```yaml
name: hacker-news-yaml
start_urls:
  - https://news.ycombinator.com
selectors:
  headline: css:.titleline > a
  link:
    selector: xpath://span[@class='titleline']/a
    attr: href
  score: css:.score
concurrency: 5
max_depth: 3
```
<!-- slide -->
```toml
name = "hacker-news-toml"
concurrency = 5
max_depth = 2
start_urls = ["https://news.ycombinator.com"]

[selectors]
headline = "css:.titleline > a"
score = "css:.score"

[selectors.link]
selector = "xpath://span[@class='titleline']/a"
attr = "href"
```
````

For a detailed guide on the configuration system, see [CONFIG_GUIDE.md](./CONFIG_GUIDE.md).

## 🗃️ Output & Persistence

Crawl results are persisted in `crawl_state.db` (SQLite). The crawler automatically handles:
- **Deduplication**: Never crawl the same URL twice across sessions.
- **Normalization**: Zero-copy URL normalization ensures consistent mapping.
- **Normalization Strategy**: Fragments are stripped, query params are sorted, and trailing slashes are unified.

## 🛠️ Development

- **Language**: Rust (Edition 2024)
- **Engine**: [Spider](https://github.com/spider-rs/spider)
- **UI**: Axum + Askama + HTMX

---
Built with ❤️ by the Spider CLI team.

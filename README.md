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

Example `spider.json`:
```json
{
  "name": "my-crawl",
  "start_urls": ["https://quotes.toscrape.com"],
  "selectors": {
    "quote": "css:span.text",
    "author": "css:small.author"
  },
  "max_depth": 3,
  "respect_robots": true
}
```

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

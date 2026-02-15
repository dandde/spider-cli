# ⚙️ Spider CLI Configuration Guide

`spider-cli` supports a flexible configuration system using **JSON**, **YAML**, and **TOML**. It features hierarchical inheritance, allowing you to share common settings across multiple crawl jobs.

---

## 🏗️ Configuration Schema

| Field | Type | Default | Description |
|:--- |:--- |:--- |:--- |
| `name` | String | `""` | Unique identifier for the crawl session. |
| `start_urls` | Array | `[]` | List of entry points for the crawler. |
| `selectors` | Map | `{}` | Map of field names to selector configurations. |
| `concurrency` | Integer | `1` | Number of concurrent workers. |
| `delay_ms` | Integer | `0` | Delay between requests in milliseconds. |
| `respect_robots`| Boolean | `false`| Whether to obey `robots.txt`. |
| `blacklist` | Array | `[]` | URL patterns to exclude (glob format). |
| `whitelist` | Array | `[]` | URL patterns to exclusively follow (glob format). |
| `max_depth` | Integer | `None` | Maximum depth from the `start_urls`. |
| `extends` | Path | `None` | Path to a parent config file to inherit from. |

---

## 🎯 Selector System

The `selectors` map allows you to define what data to extract from each page.

### 1. Simple Selectors
A simple string that defaults to CSS but can be prefixed.
- **CSS**: `"css:div.title"` or just `"div.title"`
- **XPath**: `"xpath://h1/text()"`

### 2. Advanced Selectors
For capturing specific attributes.
```json
{
  "selectors": {
    "image_src": {
      "selector": "css:img.hero",
      "attr": "src"
    }
  }
}
```

---

## 🔄 Inheritance (`extends`)

You can create a `base.json` with common settings and extend it in specialized configs.

**base.json**
```json
{
  "concurrency": 2,
  "respect_robots": true,
  "selectors": {
    "title": "css:title"
  }
}
```

**hacker_news.json**
```json
{
  "extends": "base.json",
  "name": "hn-crawl",
  "start_urls": ["https://news.ycombinator.com"],
  "selectors": {
    "headline": "css:.titleline > a"
  }
}
```
*The `hacker_news.json` will inherit the `concurrency`, `respect_robots`, and the `title` selector from `base.json`.*

---

## 📝 Format Examples

### TOML (`configs/site.toml`)
> [!IMPORTANT]
> In TOML, global fields must be defined **before** the `[selectors]` table.

```toml
name = "my-site"
concurrency = 5
start_urls = ["https://example.com"]

[selectors]
title = "css:h1"
body = "css:p"
```

### YAML (`configs/site.yaml`)
```yaml
name: my-site
start_urls:
  - https://example.com
selectors:
  title: css:h1
  body:
    selector: css:p
    attr: null
```

---

## 🔍 URL Normalization & Deduplication

`spider-cli` uses a **Zero-Copy URL Parser** to ensure high performance and reliable deduplication.
- **Normalization**: Fragments are stripped, query parameters are sorted, and trailing slashes are unified.
- **Persistence**: Visited URLs are stored in `crawl_state.db`. If you restart a crawl with the same `name` (or resume a session), duplicates will be skipped automatically.

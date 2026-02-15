# ⚙️ Spider CLI Configuration Guide

`spider-cli` supports a flexible, multi-format configuration system. You can define your scraping jobs in **JSON**, **YAML**, or **TOML**.

---

## 🏗️ Configuration Structure

A spider configuration file consists of the following top-level fields:

| Field | Type | Default | Description |
|:--- |:--- |:--- |:--- |
| `name` | String | `""` | Unique identifier for the crawl session. |
| `start_urls` | Array | `[]` | List of entry points for the crawler. |
| `selectors` | Map | `{}` | Map of field names to selector configurations. |
| `concurrency` | Integer | `1` | Number of concurrent requests. |
| `delay_ms` | Integer | `0` | Delay between requests in milliseconds. |
| `respect_robots`| Boolean | `false`| Whether to obey `robots.txt` rules. |
| `blacklist` | Array | `[]` | URL patterns to exclude (glob format). |
| `whitelist` | Array | `[]` | URL patterns to exclusively follow (glob format). |
| `max_depth` | Integer | `None` | Maximum depth from the `start_urls`. |
| `extends` | Path | `None` | Path to a parent config file for inheritance. |

---

## 🎯 Selector System (Two Variants)

The crawler supports two ways to define selectors. Choose the one that fits your complexity level.

### 1. Simple String Selectors (Recommended)
You can use standard selector strings with an optional engine prefix. If no prefix is provided, `css:` is assumed.

- **CSS Selectors**: `css:.quote.text` or simply `.quote.text`
- **XPath Selectors**: `xpath://div[@class='quote']`

### 2. Advanced Structured Selectors
For complex logic, you can use structured objects. This is useful for capturing specific attributes or building nested structures.

| Standard Format | Standard Example | Crawler String Format | Advanced Object (JSON Example) |
|:--- |:--- |:--- |:--- |
| **CSS Tag** | `div` | `"css:div"` | `{"selector": "div"}` |
| **CSS Class** | `.quote` | `"css:.quote"` | `{"selector": ".quote"}` |
| **CSS ID** | `#main` | `"css:#main"` | `{"selector": "#main"}` |
| **Attribute** | `img[src]` | `"css:img"` | `{"selector": "img", "attr": "src"}` |
| **XPath Text** | `//a/text()` | `"xpath://a/text()"` | `{"selector": "//a/text()"}` |
| **XPath Attr** | `//a/@href` | `"xpath://a/@href"` | `{"selector": "//a", "attr": "href"}` |

---

## 🚀 Full Examples

````carousel
```json
{
  "name": "hacker-news-complex",
  "start_urls": ["https://news.ycombinator.com"],
  "selectors": {
    "headline": "css:.titleline > a",
    "score": "css:.score",
    "user": "css:.hnuser"
  },
  "concurrency": 2,
  "max_depth": 2
}
```
<!-- slide -->
```yaml
name: hacker-news-yaml
start_urls:
  - https://news.ycombinator.com
selectors:
  headline:
    selector: css:.titleline > a
    attr: null
  score: css:.score
  user: css:.hnuser
concurrency: 5
```
<!-- slide -->
```toml
name = "hacker-news-toml"
concurrency = 5
start_urls = ["https://news.ycombinator.com"]

[selectors]
headline = "css:.titleline > a"
score = "css:.score"
user = "css:.hnuser"
```
````

---

## 🏗️ Configuration Inheritance (`extends`)

Reuse shared logic using the `extends` field.

**base.json**
```json
{
  "concurrency": 5,
  "delay_ms": 1000,
  "selectors": {
    "site_title": "css:title"
  }
}
```

**specialized.json**
```json
{
  "extends": "base.json",
  "name": "my-spider",
  "start_urls": ["https://example.com"]
}
```

The `specialized.json` will inherit the `concurrency`, `delay_ms`, and the `site_title` selector from `base.json`.

---

## 🔍 URL Normalization & Deduplication

`spider-cli` uses a **Zero-Copy URL Parser** for high efficiency.
- **Normalization**: Fragments are stripped, query params are sorted, and trailing slashes are unified.
- **Persistence**: Results are stored in `crawl_state.db`. Resuming a crawl with the same `name` will skip already visited URLs.

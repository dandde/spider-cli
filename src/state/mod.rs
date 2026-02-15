use anyhow::{Context, Result};
use sqlx::{ConnectOptions, Pool, Sqlite, sqlite::SqliteConnectOptions, sqlite::SqlitePoolOptions};
use std::path::Path;
use std::str::FromStr;

pub struct StateManager {
    pool: Pool<Sqlite>,
}

impl StateManager {
    pub async fn new<P: AsRef<Path>>(db_path: P) -> Result<Self> {
        let db_url = format!("sqlite:{}", db_path.as_ref().to_string_lossy());

        let connection_options = SqliteConnectOptions::from_str(&db_url)?
            .create_if_missing(true)
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
            .busy_timeout(std::time::Duration::from_millis(5000))
            .disable_statement_logging();

        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(connection_options)
            .await
            .context("Failed to connect to SQLite")?;

        let manager = Self { pool };
        manager.initialize_schema().await?;

        Ok(manager)
    }

    async fn initialize_schema(&self) -> Result<()> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS crawls (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                status TEXT NOT NULL,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
            );",
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS frontier (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                crawl_id INTEGER NOT NULL,
                url TEXT NOT NULL,
                depth INTEGER DEFAULT 0,
                status TEXT DEFAULT 'pending', -- pending, processing, completed, failed
                added_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY(crawl_id) REFERENCES crawls(id),
                UNIQUE(crawl_id, url)
            );",
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS results (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                crawl_id INTEGER NOT NULL,
                url TEXT NOT NULL,
                data TEXT NOT NULL, -- JSON
                found_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY(crawl_id) REFERENCES crawls(id),
                UNIQUE(crawl_id, url)
            );",
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_visited_urls(&self, crawl_id: i64) -> Result<Vec<String>> {
        let urls = sqlx::query_scalar::<_, String>("SELECT url FROM results WHERE crawl_id = ?")
            .bind(crawl_id)
            .fetch_all(&self.pool)
            .await?;
        Ok(urls)
    }

    pub async fn get_results_urls(&self, crawl_id: i64) -> Result<Vec<String>> {
        // Alias for get_visited_urls but specifically for results table discovery
        self.get_visited_urls(crawl_id).await
    }

    pub async fn add_to_frontier(&self, crawl_id: i64, urls: Vec<(String, usize)>) -> Result<()> {
        for (url, depth) in urls {
            sqlx::query(
                "INSERT OR IGNORE INTO frontier (crawl_id, url, depth, status) 
                 VALUES (?, ?, ?, 'pending')",
            )
            .bind(crawl_id)
            .bind(url)
            .bind(depth as i32)
            .execute(&self.pool)
            .await?;
        }
        Ok(())
    }

    pub async fn get_pending_frontier(
        &self,
        crawl_id: i64,
        limit: i32,
    ) -> Result<Vec<(i64, String, usize)>> {
        let rows = sqlx::query_as::<_, (i64, String, i32)>(
            "SELECT id, url, depth FROM frontier 
             WHERE crawl_id = ? AND status = 'pending' 
             LIMIT ?",
        )
        .bind(crawl_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|(id, url, depth)| (id, url, depth as usize))
            .collect())
    }

    pub async fn save_result(
        &self,
        crawl_id: i64,
        url: &str,
        data: &serde_json::Value,
    ) -> Result<()> {
        let data_str = serde_json::to_string(data)?;
        sqlx::query("INSERT OR IGNORE INTO results (crawl_id, url, data) VALUES (?, ?, ?)")
            .bind(crawl_id)
            .bind(url)
            .bind(data_str)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn create_crawl(&self, name: &str) -> Result<i64> {
        let row =
            sqlx::query("INSERT INTO crawls (name, status) VALUES (?, 'active') RETURNING id")
                .bind(name)
                .fetch_one(&self.pool)
                .await?;

        use sqlx::Row;
        Ok(row.get(0))
    }

    pub async fn get_active_crawl(&self) -> Result<Option<i64>> {
        let row = sqlx::query_scalar::<_, i64>(
            "SELECT id FROM crawls WHERE status = 'active' ORDER BY updated_at DESC LIMIT 1",
        )
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn test_state_manager_init() -> Result<()> {
        let tmp_file = NamedTempFile::new()?;
        let db_path = tmp_file.path();

        let manager = StateManager::new(db_path).await?;
        let crawl_id = manager.create_crawl("test").await?;
        assert_eq!(crawl_id, 1);

        Ok(())
    }

    #[tokio::test]
    async fn test_frontier_management() -> Result<()> {
        let tmp_file = NamedTempFile::new()?;
        let manager = StateManager::new(tmp_file.path()).await?;
        let crawl_id = manager.create_crawl("test").await?;

        manager
            .add_to_frontier(crawl_id, vec![("http://example.com".to_string(), 0)])
            .await?;

        let pending = manager.get_pending_frontier(crawl_id, 10).await?;
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].1, "http://example.com");

        let pending_after = manager.get_pending_frontier(crawl_id, 10).await?;
        assert_eq!(pending_after.len(), 1);

        Ok(())
    }
}

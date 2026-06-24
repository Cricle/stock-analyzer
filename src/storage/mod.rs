//! PostgreSQL-backed implementations of `sa_models` storage traits.
//!
//! Provides [`PgStore`] which implements `AnalysisStore`, `CheckpointStore`,
//! and `GuidanceStore` using a `sqlx::PgPool`.

pub mod analysis;
pub mod checkpoint;
pub mod guidance;

use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;

/// PostgreSQL storage handle. Clone-friendly (wraps `PgPool` which is
/// reference-counted internally).
#[derive(Clone)]
pub struct PgStore {
    pool: PgPool,
}

impl PgStore {
    /// Create a new `PgStore` backed by the given PostgreSQL connection pool.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Connect to PostgreSQL and create a new `PgStore`.
    pub async fn connect(database_url: &str) -> anyhow::Result<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(24)
            .connect(database_url)
            .await?;
        Ok(Self { pool })
    }

    /// Borrow the underlying [`PgPool`].
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Create the required tables if they don't exist.
    pub async fn init_schema(&self) -> anyhow::Result<()> {
        sqlx::raw_sql(SCHEMA_SQL).execute(&self.pool).await?;
        Ok(())
    }
}

const SCHEMA_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS analysis_tasks (
    task_id TEXT PRIMARY KEY,
    owner_username TEXT NOT NULL DEFAULT '',
    symbol TEXT NOT NULL,
    stock_name TEXT NOT NULL DEFAULT '',
    market_type TEXT NOT NULL,
    analysis_date TEXT NOT NULL,
    research_depth TEXT NOT NULL DEFAULT 'deep',
    status TEXT NOT NULL DEFAULT 'pending',
    progress INTEGER NOT NULL DEFAULT 0,
    request_json TEXT NOT NULL DEFAULT '{}',
    current_step_name TEXT NOT NULL DEFAULT '',
    current_step_description TEXT NOT NULL DEFAULT '',
    message TEXT NOT NULL DEFAULT '',
    error_message TEXT,
    llm_total_requests BIGINT NOT NULL DEFAULT 0,
    llm_prompt_tokens BIGINT NOT NULL DEFAULT 0,
    llm_completion_tokens BIGINT NOT NULL DEFAULT 0,
    llm_total_tokens BIGINT NOT NULL DEFAULT 0,
    llm_usage_json TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS analysis_results (
    task_id TEXT PRIMARY KEY REFERENCES analysis_tasks(task_id),
    result_json TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS analysis_checkpoints (
    task_id TEXT NOT NULL,
    step_name TEXT NOT NULL,
    stage TEXT NOT NULL DEFAULT '',
    node TEXT NOT NULL DEFAULT '',
    step BIGINT NOT NULL DEFAULT 0,
    data_json TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL,
    PRIMARY KEY (task_id, step_name)
);

CREATE TABLE IF NOT EXISTS guidance_rules (
    id TEXT PRIMARY KEY,
    market_type TEXT NOT NULL,
    rule_type TEXT NOT NULL,
    content TEXT NOT NULL,
    priority INTEGER NOT NULL DEFAULT 0,
    enabled INTEGER NOT NULL DEFAULT 1
);
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_sql_contains_all_tables() {
        assert!(SCHEMA_SQL.contains("analysis_tasks"));
        assert!(SCHEMA_SQL.contains("analysis_results"));
        assert!(SCHEMA_SQL.contains("analysis_checkpoints"));
        assert!(SCHEMA_SQL.contains("guidance_rules"));
    }

    #[test]
    fn test_schema_sql_analysis_tasks_columns() {
        assert!(SCHEMA_SQL.contains("task_id TEXT PRIMARY KEY"));
        assert!(SCHEMA_SQL.contains("symbol TEXT NOT NULL"));
        assert!(SCHEMA_SQL.contains("market_type TEXT NOT NULL"));
        assert!(SCHEMA_SQL.contains("status TEXT NOT NULL"));
        assert!(SCHEMA_SQL.contains("llm_total_requests BIGINT"));
        assert!(SCHEMA_SQL.contains("created_at TEXT NOT NULL"));
    }

    #[test]
    fn test_schema_sql_checkpoints_primary_key() {
        assert!(SCHEMA_SQL.contains("PRIMARY KEY (task_id, step_name)"));
    }

    #[test]
    fn test_schema_sql_guidance_rules_columns() {
        assert!(SCHEMA_SQL.contains("rule_type TEXT NOT NULL"));
        assert!(SCHEMA_SQL.contains("priority INTEGER NOT NULL DEFAULT 0"));
        assert!(SCHEMA_SQL.contains("enabled INTEGER NOT NULL DEFAULT 1"));
    }

    #[test]
    fn test_pg_store_new() {
        // Can't actually connect to DB, but verify the struct compiles
        let pool = sqlx::PgPool::connect_lazy("postgres://localhost/test");
        match pool {
            Ok(p) => {
                let store = PgStore::new(p);
                let _ = store.pool();
            }
            Err(_) => {} // Expected in test env without DB
        }
    }
}

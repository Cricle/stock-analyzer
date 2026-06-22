//! PostgreSQL-backed implementations of `sa_models` storage traits.
//!
//! Provides [`PgStore`] which implements `AnalysisStore`, `CheckpointStore`,
//! and `GuidanceStore` using a `sqlx::PgPool`.

pub mod analysis;
pub mod checkpoint;
pub mod guidance;

use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;

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

//! `AnalysisStore` implementation for `PgStore`.

use async_trait::async_trait;
use sa_models::store::{AnalysisStore, StoredAnalysisSummary};
use sa_models::{AnalysisResult, PersistedTask, SingleAnalysisRequest};
use sqlx::Row;

use crate::PgStore;

#[async_trait]
impl AnalysisStore for PgStore {
    async fn insert_task(&self, task: &PersistedTask) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            INSERT INTO analysis_tasks (
                task_id, owner_username, symbol, stock_name, market_type, analysis_date,
                research_depth, status, progress, request_json,
                current_step_name, current_step_description,
                message, error_message, llm_total_requests, llm_prompt_tokens,
                llm_completion_tokens, llm_total_tokens, llm_usage_json,
                created_at, updated_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21)
            "#,
        )
        .bind(&task.task_id)
        .bind(&task.owner_username)
        .bind(&task.symbol)
        .bind(&task.stock_name)
        .bind(&task.market_type)
        .bind(&task.analysis_date)
        .bind(&task.research_depth)
        .bind(task.status_string())
        .bind(task.progress)
        .bind(serde_json::to_string(&task.request)?)
        .bind(&task.current_step_name)
        .bind(&task.current_step_description)
        .bind(&task.message)
        .bind(&task.error_message)
        .bind(task.llm_token_usage.total_requests)
        .bind(task.llm_token_usage.prompt_tokens)
        .bind(task.llm_token_usage.completion_tokens)
        .bind(task.llm_token_usage.total_tokens)
        .bind(serde_json::to_string(&task.llm_token_usage)?)
        .bind(task.created_at.to_rfc3339())
        .bind(task.updated_at.to_rfc3339())
        .execute(self.pool())
        .await?;
        Ok(())
    }

    async fn update_task(&self, task: &PersistedTask) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            UPDATE analysis_tasks
            SET status = $1, progress = $2, current_step_name = $3, current_step_description = $4,
                message = $5, error_message = $6, llm_total_requests = $7, llm_prompt_tokens = $8,
                llm_completion_tokens = $9, llm_total_tokens = $10, llm_usage_json = $11,
                updated_at = $12
            WHERE task_id = $13
            "#,
        )
        .bind(task.status_string())
        .bind(task.progress)
        .bind(&task.current_step_name)
        .bind(&task.current_step_description)
        .bind(&task.message)
        .bind(&task.error_message)
        .bind(task.llm_token_usage.total_requests)
        .bind(task.llm_token_usage.prompt_tokens)
        .bind(task.llm_token_usage.completion_tokens)
        .bind(task.llm_token_usage.total_tokens)
        .bind(serde_json::to_string(&task.llm_token_usage)?)
        .bind(task.updated_at.to_rfc3339())
        .bind(&task.task_id)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    async fn get_task(&self, task_id: &str) -> anyhow::Result<Option<PersistedTask>> {
        let row = sqlx::query(
            r#"
            SELECT task_id, symbol, market_type, analysis_date, status, progress,
                   owner_username, stock_name, research_depth, request_json,
                   current_step_name, current_step_description, message, error_message,
                   llm_total_requests, llm_prompt_tokens, llm_completion_tokens,
                   llm_total_tokens, llm_usage_json, created_at, updated_at
            FROM analysis_tasks
            WHERE task_id = $1
            "#,
        )
        .bind(task_id)
        .fetch_optional(self.pool())
        .await?;

        row.map(row_to_task).transpose()
    }

    async fn list_tasks(&self, limit: i64, offset: i64) -> anyhow::Result<Vec<PersistedTask>> {
        let rows = sqlx::query(
            r#"
            SELECT task_id, symbol, market_type, analysis_date, status, progress,
                   owner_username, stock_name, research_depth, request_json,
                   current_step_name, current_step_description, message, error_message,
                   llm_total_requests, llm_prompt_tokens, llm_completion_tokens,
                   llm_total_tokens, llm_usage_json, created_at, updated_at
            FROM analysis_tasks
            ORDER BY created_at DESC
            LIMIT $1 OFFSET $2
            "#,
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(self.pool())
        .await?;

        rows.into_iter().map(row_to_task).collect()
    }

    async fn list_tasks_for_user(
        &self,
        owner_username: &str,
        limit: i64,
        offset: i64,
    ) -> anyhow::Result<Vec<PersistedTask>> {
        let rows = sqlx::query(
            r#"
            SELECT task_id, symbol, market_type, analysis_date, status, progress,
                   owner_username, stock_name, research_depth, request_json,
                   current_step_name, current_step_description, message, error_message,
                   llm_total_requests, llm_prompt_tokens, llm_completion_tokens,
                   llm_total_tokens, llm_usage_json, created_at, updated_at
            FROM analysis_tasks
            WHERE owner_username = $1
            ORDER BY created_at DESC
            LIMIT $2 OFFSET $3
            "#,
        )
        .bind(owner_username)
        .bind(limit)
        .bind(offset)
        .fetch_all(self.pool())
        .await?;

        rows.into_iter().map(row_to_task).collect()
    }

    async fn find_cached_task(
        &self,
        symbol: &str,
        analysis_date: &str,
    ) -> anyhow::Result<Option<String>> {
        let task_id = sqlx::query_scalar::<_, String>(
            "SELECT task_id FROM analysis_tasks WHERE symbol = $1 AND analysis_date = $2 AND status = 'completed' ORDER BY updated_at DESC LIMIT 1",
        )
        .bind(symbol)
        .bind(analysis_date)
        .fetch_optional(self.pool())
        .await?;
        Ok(task_id)
    }

    async fn save_result(&self, task_id: &str, result: &AnalysisResult) -> anyhow::Result<()> {
        let json = serde_json::to_string(result)?;
        let usage_json = serde_json::to_string(&result.artifacts.llm_token_usage)?;
        let existing = sqlx::query_scalar::<_, String>(
            "SELECT result_json FROM analysis_results WHERE task_id = $1",
        )
        .bind(task_id)
        .fetch_optional(self.pool())
        .await?;
        if existing.as_deref() == Some(json.as_str()) {
            return Ok(());
        }
        if existing.is_none() {
            sqlx::query("INSERT INTO analysis_results(task_id, result_json) VALUES ($1, $2)")
                .bind(task_id)
                .bind(&json)
                .execute(self.pool())
                .await?;
        } else {
            sqlx::query("UPDATE analysis_results SET result_json = $1 WHERE task_id = $2")
                .bind(&json)
                .bind(task_id)
                .execute(self.pool())
                .await?;
        }
        sqlx::query(
            r#"
            UPDATE analysis_tasks
            SET llm_total_requests = $1,
                llm_prompt_tokens = $2,
                llm_completion_tokens = $3,
                llm_total_tokens = $4,
                llm_usage_json = $5
            WHERE task_id = $6
            "#,
        )
        .bind(result.artifacts.llm_token_usage.total_requests)
        .bind(result.artifacts.llm_token_usage.prompt_tokens)
        .bind(result.artifacts.llm_token_usage.completion_tokens)
        .bind(result.artifacts.llm_token_usage.total_tokens)
        .bind(usage_json)
        .bind(task_id)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    async fn load_result(&self, task_id: &str) -> anyhow::Result<Option<AnalysisResult>> {
        let row = sqlx::query("SELECT result_json FROM analysis_results WHERE task_id = $1")
            .bind(task_id)
            .fetch_optional(self.pool())
            .await?;

        match row {
            Some(row) => {
                let json: String = row.try_get("result_json")?;
                Ok(Some(serde_json::from_str(&json)?))
            }
            None => Ok(None),
        }
    }

    async fn list_analyses(
        &self,
        _symbol: Option<&str>,
        limit: usize,
    ) -> anyhow::Result<Vec<StoredAnalysisSummary>> {
        let tasks = self.list_tasks(limit as i64, 0).await?;
        Ok(tasks
            .into_iter()
            .map(|t| StoredAnalysisSummary {
                task_id: t.task_id,
                symbol: t.symbol,
                stock_name: t.stock_name,
                market_type: t.market_type,
                status: t.status.as_str().to_string(),
                created_at: t.created_at.to_rfc3339(),
                updated_at: t.updated_at.to_rfc3339(),
            })
            .collect())
    }

    async fn delete_analysis(&self, task_id: &str) -> anyhow::Result<()> {
        sqlx::query("DELETE FROM analysis_results WHERE task_id = $1")
            .bind(task_id)
            .execute(self.pool())
            .await?;
        sqlx::query("DELETE FROM analysis_tasks WHERE task_id = $1")
            .bind(task_id)
            .execute(self.pool())
            .await?;
        Ok(())
    }

    async fn save_request(
        &self,
        _task_id: &str,
        _request: &SingleAnalysisRequest,
    ) -> anyhow::Result<()> {
        // Request is already stored in the task's request_json field.
        Ok(())
    }

    async fn load_request(&self, task_id: &str) -> anyhow::Result<Option<SingleAnalysisRequest>> {
        let task = self.get_task(task_id).await?;
        Ok(task.map(|t| t.request))
    }
}

fn row_to_task(row: sqlx::postgres::PgRow) -> anyhow::Result<PersistedTask> {
    use chrono::{DateTime, Utc};
    use std::str::FromStr;

    let created_at: String = row.try_get("created_at")?;
    let updated_at: String = row.try_get("updated_at")?;
    let status_str: String = row.try_get("status")?;
    let request_json: String = row
        .try_get::<String, _>("request_json")
        .unwrap_or_else(|_| "{}".to_string());
    let usage_json: String = row
        .try_get::<String, _>("llm_usage_json")
        .unwrap_or_else(|_| "{}".to_string());

    let created_at = DateTime::parse_from_rfc3339(&created_at)
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now());
    let updated_at = DateTime::parse_from_rfc3339(&updated_at)
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now());

    let default_usage = sa_models::LlmTokenUsageSummary {
        total_requests: row
            .try_get::<i64, _>("llm_total_requests")
            .unwrap_or_default(),
        prompt_tokens: row
            .try_get::<i64, _>("llm_prompt_tokens")
            .unwrap_or_default(),
        completion_tokens: row
            .try_get::<i64, _>("llm_completion_tokens")
            .unwrap_or_default(),
        total_tokens: row
            .try_get::<i64, _>("llm_total_tokens")
            .unwrap_or_default(),
        by_model: Vec::new(),
    };

    Ok(PersistedTask {
        task_id: row.try_get("task_id")?,
        owner_username: row
            .try_get::<String, _>("owner_username")
            .unwrap_or_default(),
        symbol: row.try_get("symbol")?,
        stock_name: row.try_get::<String, _>("stock_name").unwrap_or_default(),
        market_type: row.try_get("market_type")?,
        analysis_date: row.try_get("analysis_date")?,
        research_depth: row
            .try_get::<String, _>("research_depth")
            .unwrap_or_else(|_| "deep".to_string()),
        request: serde_json::from_str(&request_json)?,
        status: sa_models::TaskStatus::from_str(&status_str)
            .unwrap_or(sa_models::TaskStatus::Pending),
        progress: row.try_get("progress")?,
        current_step_name: row.try_get("current_step_name")?,
        current_step_description: row.try_get("current_step_description")?,
        message: row.try_get("message")?,
        error_message: row.try_get("error_message")?,
        llm_token_usage: serde_json::from_str(&usage_json).unwrap_or(default_usage),
        created_at,
        updated_at,
    })
}

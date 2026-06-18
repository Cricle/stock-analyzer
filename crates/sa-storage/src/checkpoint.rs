//! `CheckpointStore` implementation for `PgStore`.

use async_trait::async_trait;
use sa_models::store::{CheckpointInfo, CheckpointStore, StoredCheckpoint};
use sqlx::Row;

use crate::PgStore;

#[async_trait]
impl CheckpointStore for PgStore {
    async fn save_checkpoint(
        &self,
        task_id: &str,
        step_name: &str,
        checkpoint: &StoredCheckpoint,
    ) -> anyhow::Result<()> {
        let data_json = serde_json::to_string(&checkpoint.data)?;
        sqlx::query(
            r#"INSERT INTO analysis_checkpoints (task_id, step_name, stage, node, step, data_json, created_at)
               VALUES ($1, $2, $3, $4, $5, $6, $7)
               ON CONFLICT (task_id, step_name) DO UPDATE SET
                 stage = EXCLUDED.stage,
                 node = EXCLUDED.node,
                 step = EXCLUDED.step,
                 data_json = EXCLUDED.data_json,
                 created_at = EXCLUDED.created_at"#,
        )
        .bind(task_id)
        .bind(step_name)
        .bind(&checkpoint.stage)
        .bind(&checkpoint.node)
        .bind(checkpoint.step)
        .bind(&data_json)
        .bind(&checkpoint.created_at)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    async fn load_checkpoint(&self, task_id: &str) -> anyhow::Result<Option<StoredCheckpoint>> {
        let row = sqlx::query(
            r#"SELECT task_id, step_name, stage, node, step, data_json, created_at
               FROM analysis_checkpoints
               WHERE task_id = $1
               ORDER BY created_at DESC
               LIMIT 1"#,
        )
        .bind(task_id)
        .fetch_optional(self.pool())
        .await?;
        match row {
            Some(row) => {
                let data_json: String = row.try_get("data_json")?;
                Ok(Some(StoredCheckpoint {
                    task_id: row.try_get("task_id")?,
                    step_name: row.try_get("step_name")?,
                    stage: row.try_get("stage")?,
                    node: row.try_get("node")?,
                    step: row.try_get::<i64, _>("step")?,
                    data: serde_json::from_str(&data_json).unwrap_or_default(),
                    created_at: row.try_get("created_at")?,
                }))
            }
            None => Ok(None),
        }
    }

    async fn list_checkpoints(&self, task_id: &str) -> anyhow::Result<Vec<CheckpointInfo>> {
        let rows = sqlx::query(
            r#"SELECT task_id, step_name, created_at
               FROM analysis_checkpoints
               WHERE task_id = $1
               ORDER BY created_at ASC"#,
        )
        .bind(task_id)
        .fetch_all(self.pool())
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| CheckpointInfo {
                task_id: row.try_get("task_id").unwrap_or_default(),
                checkpoint_id: row.try_get::<String, _>("step_name").unwrap_or_default(),
                created_at: row.try_get("created_at").unwrap_or_default(),
                step_name: row.try_get("step_name").unwrap_or_default(),
            })
            .collect())
    }

    async fn delete_checkpoints(&self, task_id: &str) -> anyhow::Result<()> {
        sqlx::query("DELETE FROM analysis_checkpoints WHERE task_id = $1")
            .bind(task_id)
            .execute(self.pool())
            .await?;
        Ok(())
    }
}

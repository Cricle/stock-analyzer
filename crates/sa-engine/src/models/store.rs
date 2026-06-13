use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::models::analysis::{AnalysisResult, SingleAnalysisRequest};
use crate::models::task::PersistedTask;

/// Cache entry metadata.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CacheEntry {
    pub key: String,
    pub created_at: String,
    pub expires_at: Option<String>,
    pub size_bytes: u64,
}

/// Checkpoint metadata.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CheckpointInfo {
    pub task_id: String,
    pub checkpoint_id: String,
    pub created_at: String,
    pub step_name: String,
}

/// Stored checkpoint data for resumable analysis.
///
/// This is the storage-level checkpoint with full task context,
/// distinct from the display-level `AnalysisCheckpoint` used in reports.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StoredCheckpoint {
    pub task_id: String,
    pub step_name: String,
    pub stage: String,
    pub node: String,
    pub step: i64,
    pub data: serde_json::Value,
    pub created_at: String,
}

/// Persistent storage for analysis results, task state, and checkpoints.
#[async_trait]
pub trait AnalysisStore: Send + Sync {
    // --- Task management ---

    /// Insert a new task.
    async fn insert_task(&self, task: &PersistedTask) -> anyhow::Result<()>;

    /// Update an existing task.
    async fn update_task(&self, task: &PersistedTask) -> anyhow::Result<()>;

    /// Get a task by ID.
    async fn get_task(&self, task_id: &str) -> anyhow::Result<Option<PersistedTask>>;

    /// List tasks with pagination.
    async fn list_tasks(&self, limit: i64, offset: i64) -> anyhow::Result<Vec<PersistedTask>>;

    /// List tasks for a specific user.
    async fn list_tasks_for_user(
        &self,
        owner_username: &str,
        limit: i64,
        offset: i64,
    ) -> anyhow::Result<Vec<PersistedTask>>;

    /// Find a cached completed task for the same symbol and date.
    async fn find_cached_task(&self, symbol: &str, analysis_date: &str) -> anyhow::Result<Option<String>>;

    // --- Result management ---

    /// Save a completed analysis result.
    async fn save_result(&self, task_id: &str, result: &AnalysisResult) -> anyhow::Result<()>;

    /// Load a previously saved analysis result.
    async fn load_result(&self, task_id: &str) -> anyhow::Result<Option<AnalysisResult>>;

    /// Get an analysis result (alias for load_result).
    async fn get_result(&self, task_id: &str) -> anyhow::Result<Option<AnalysisResult>> {
        self.load_result(task_id).await
    }

    /// Delete an analysis by task ID.
    async fn delete_analysis(&self, task_id: &str) -> anyhow::Result<()>;

    // --- Request management ---

    /// Save an analysis request for later replay.
    async fn save_request(&self, task_id: &str, request: &SingleAnalysisRequest) -> anyhow::Result<()>;

    /// Load a saved analysis request.
    async fn load_request(&self, task_id: &str) -> anyhow::Result<Option<SingleAnalysisRequest>>;
}

/// Key-value cache for intermediate data (quotes, news, fundamentals).
#[async_trait]
pub trait CacheStore: Send + Sync {
    /// Get a cached value by key.
    async fn get(&self, key: &str) -> anyhow::Result<Option<Vec<u8>>>;

    /// Set a cached value with optional TTL in seconds.
    async fn set(&self, key: &str, value: &[u8], ttl_seconds: Option<u64>) -> anyhow::Result<()>;

    /// Delete a cached value.
    async fn delete(&self, key: &str) -> anyhow::Result<()>;

    /// Check if a key exists.
    async fn exists(&self, key: &str) -> anyhow::Result<bool>;

    /// List cache entries matching a prefix.
    async fn list_entries(&self, prefix: &str) -> anyhow::Result<Vec<CacheEntry>>;
}

/// Checkpoint storage for resumable analysis workflows.
#[async_trait]
pub trait CheckpointStore: Send + Sync {
    /// Save a checkpoint for a running analysis.
    async fn save_checkpoint(
        &self,
        task_id: &str,
        step_name: &str,
        checkpoint: &StoredCheckpoint,
    ) -> anyhow::Result<()>;

    /// Load the latest checkpoint for a task.
    async fn load_checkpoint(&self, task_id: &str) -> anyhow::Result<Option<StoredCheckpoint>>;

    /// List all checkpoints for a task.
    async fn list_checkpoints(&self, task_id: &str) -> anyhow::Result<Vec<CheckpointInfo>>;

    /// Delete all checkpoints for a task.
    async fn delete_checkpoints(&self, task_id: &str) -> anyhow::Result<()>;
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_entry_serialization() {
        let e = CacheEntry {
            key: "quote:AAPL:2025-01-15".into(),
            created_at: "2025-01-15T10:00:00Z".into(),
            expires_at: Some("2025-01-15T14:00:00Z".into()),
            size_bytes: 1024,
        };
        let json = serde_json::to_string(&e).unwrap();
        let e2: CacheEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(e.key, e2.key);
        assert_eq!(e.size_bytes, e2.size_bytes);
    }

    #[test]
    fn checkpoint_info_serialization() {
        let c = CheckpointInfo {
            task_id: "t1".into(),
            checkpoint_id: "cp-1".into(),
            created_at: "2025-01-15".into(),
            step_name: "market_analysis".into(),
        };
        let json = serde_json::to_string(&c).unwrap();
        let c2: CheckpointInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(c.step_name, c2.step_name);
    }

    #[test]
    fn stored_checkpoint_serialization() {
        let c = StoredCheckpoint {
            task_id: "t1".into(),
            step_name: "analysis".into(),
            stage: "runtime".into(),
            node: "market".into(),
            step: 3,
            data: serde_json::json!({"key": "value"}),
            created_at: "2025-01-15".into(),
        };
        let json = serde_json::to_string(&c).unwrap();
        let c2: StoredCheckpoint = serde_json::from_str(&json).unwrap();
        assert_eq!(c.step, c2.step);
        assert_eq!(c.data, c2.data);
    }
}

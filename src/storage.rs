//! In-memory storage implementation for AnalysisStore, CheckpointStore, and CacheStore.
//!
//! This replaces the PgStore stub with a working in-memory implementation.

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::analysis::{AnalysisResult, SingleAnalysisRequest};
use crate::store::{
    AnalysisStore, CacheEntry, CacheStore, CheckpointInfo, CheckpointStore, StoredAnalysisSummary,
    StoredCheckpoint,
};
use crate::task::PersistedTask;

/// In-memory store that implements all required storage traits.
#[derive(Clone)]
pub struct PgStore {
    tasks: Arc<RwLock<HashMap<String, PersistedTask>>>,
    results: Arc<RwLock<HashMap<String, AnalysisResult>>>,
    requests: Arc<RwLock<HashMap<String, SingleAnalysisRequest>>>,
    checkpoints: Arc<RwLock<HashMap<String, StoredCheckpoint>>>,
    cache: Arc<RwLock<HashMap<String, Vec<u8>>>>,
}

impl PgStore {
    pub async fn connect(_database_url: &str) -> anyhow::Result<Self> {
        tracing::info!("PgStore: using in-memory storage");
        Ok(Self {
            tasks: Arc::new(RwLock::new(HashMap::new())),
            results: Arc::new(RwLock::new(HashMap::new())),
            requests: Arc::new(RwLock::new(HashMap::new())),
            checkpoints: Arc::new(RwLock::new(HashMap::new())),
            cache: Arc::new(RwLock::new(HashMap::new())),
        })
    }
}

#[async_trait]
impl AnalysisStore for PgStore {
    async fn insert_task(&self, task: &PersistedTask) -> anyhow::Result<()> {
        self.tasks
            .write()
            .await
            .insert(task.task_id.clone(), task.clone());
        Ok(())
    }

    async fn update_task(&self, task: &PersistedTask) -> anyhow::Result<()> {
        self.tasks
            .write()
            .await
            .insert(task.task_id.clone(), task.clone());
        Ok(())
    }

    async fn get_task(&self, task_id: &str) -> anyhow::Result<Option<PersistedTask>> {
        Ok(self.tasks.read().await.get(task_id).cloned())
    }

    async fn list_tasks(&self, limit: i64, offset: i64) -> anyhow::Result<Vec<PersistedTask>> {
        let tasks = self.tasks.read().await;
        let mut sorted: Vec<_> = tasks.values().cloned().collect();
        sorted.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(sorted
            .into_iter()
            .skip(offset as usize)
            .take(limit as usize)
            .collect())
    }

    async fn list_tasks_for_user(
        &self,
        owner_username: &str,
        limit: i64,
        offset: i64,
    ) -> anyhow::Result<Vec<PersistedTask>> {
        let tasks = self.tasks.read().await;
        let mut sorted: Vec<_> = tasks
            .values()
            .filter(|t| t.owner_username == owner_username)
            .cloned()
            .collect();
        sorted.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(sorted
            .into_iter()
            .skip(offset as usize)
            .take(limit as usize)
            .collect())
    }

    async fn find_cached_task(
        &self,
        symbol: &str,
        analysis_date: &str,
    ) -> anyhow::Result<Option<String>> {
        let tasks = self.tasks.read().await;
        for task in tasks.values() {
            if task.symbol == symbol
                && task.analysis_date == analysis_date
                && task.status == crate::TaskStatus::Completed
            {
                return Ok(Some(task.task_id.clone()));
            }
        }
        Ok(None)
    }

    async fn save_result(&self, task_id: &str, result: &AnalysisResult) -> anyhow::Result<()> {
        self.results
            .write()
            .await
            .insert(task_id.to_string(), result.clone());
        Ok(())
    }

    async fn load_result(&self, task_id: &str) -> anyhow::Result<Option<AnalysisResult>> {
        Ok(self.results.read().await.get(task_id).cloned())
    }

    async fn list_analyses(
        &self,
        symbol: Option<&str>,
        limit: usize,
    ) -> anyhow::Result<Vec<StoredAnalysisSummary>> {
        let tasks = self.tasks.read().await;
        let mut summaries: Vec<StoredAnalysisSummary> = Vec::new();

        for task in tasks.values() {
            if let Some(sym) = symbol {
                if task.symbol != sym {
                    continue;
                }
            }
            summaries.push(StoredAnalysisSummary {
                task_id: task.task_id.clone(),
                symbol: task.symbol.clone(),
                stock_name: task.stock_name.clone(),
                market_type: task.market_type.clone(),
                status: task.status.as_str().to_string(),
                created_at: task.created_at.to_rfc3339(),
                updated_at: task.updated_at.to_rfc3339(),
            });
        }

        summaries.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        summaries.truncate(limit);
        Ok(summaries)
    }

    async fn delete_analysis(&self, task_id: &str) -> anyhow::Result<()> {
        self.tasks.write().await.remove(task_id);
        self.results.write().await.remove(task_id);
        self.requests.write().await.remove(task_id);
        Ok(())
    }

    async fn save_request(
        &self,
        task_id: &str,
        request: &SingleAnalysisRequest,
    ) -> anyhow::Result<()> {
        self.requests
            .write()
            .await
            .insert(task_id.to_string(), request.clone());
        Ok(())
    }

    async fn load_request(&self, task_id: &str) -> anyhow::Result<Option<SingleAnalysisRequest>> {
        Ok(self.requests.read().await.get(task_id).cloned())
    }
}

#[async_trait]
impl CheckpointStore for PgStore {
    async fn save_checkpoint(
        &self,
        task_id: &str,
        _step_name: &str,
        checkpoint: &StoredCheckpoint,
    ) -> anyhow::Result<()> {
        self.checkpoints
            .write()
            .await
            .insert(task_id.to_string(), checkpoint.clone());
        Ok(())
    }

    async fn load_checkpoint(&self, task_id: &str) -> anyhow::Result<Option<StoredCheckpoint>> {
        Ok(self.checkpoints.read().await.get(task_id).cloned())
    }

    async fn list_checkpoints(&self, _task_id: &str) -> anyhow::Result<Vec<CheckpointInfo>> {
        Ok(vec![])
    }

    async fn delete_checkpoints(&self, task_id: &str) -> anyhow::Result<()> {
        self.checkpoints.write().await.remove(task_id);
        Ok(())
    }
}

#[async_trait]
impl CacheStore for PgStore {
    async fn get(&self, key: &str) -> anyhow::Result<Option<Vec<u8>>> {
        Ok(self.cache.read().await.get(key).cloned())
    }

    async fn set(&self, key: &str, value: &[u8], _ttl_seconds: Option<u64>) -> anyhow::Result<()> {
        self.cache
            .write()
            .await
            .insert(key.to_string(), value.to_vec());
        Ok(())
    }

    async fn delete(&self, key: &str) -> anyhow::Result<()> {
        self.cache.write().await.remove(key);
        Ok(())
    }

    async fn exists(&self, key: &str) -> anyhow::Result<bool> {
        Ok(self.cache.read().await.contains_key(key))
    }

    async fn list_entries(&self, prefix: &str) -> anyhow::Result<Vec<CacheEntry>> {
        let cache = self.cache.read().await;
        let entries: Vec<CacheEntry> = cache
            .iter()
            .filter(|(k, _)| k.starts_with(prefix))
            .map(|(k, v)| CacheEntry {
                key: k.clone(),
                size_bytes: v.len() as u64,
                created_at: chrono::Utc::now().to_rfc3339(),
                expires_at: None,
            })
            .collect();
        Ok(entries)
    }
}

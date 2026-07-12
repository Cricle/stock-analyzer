//! Default in-memory implementations of storage traits.
//!
//! These implementations are suitable for development, testing, and single-process
//! deployments. For production use with persistence, implement the traits yourself.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::RwLock;

use super::{CacheEntry, CheckpointInfo, GuidanceRule, StoredAnalysisSummary, StoredCheckpoint};
use crate::{
    AnalysisResult, AnalysisStore, CacheStore, CheckpointStore, GuidanceStore, PersistedTask,
    SingleAnalysisRequest, TaskStatus,
};

// ---------------------------------------------------------------------------
// InMemoryAnalysisStore
// ---------------------------------------------------------------------------

/// In-memory implementation of [`AnalysisStore`] for testing and single-process use.
#[derive(Clone)]
pub struct InMemoryAnalysisStore {
    tasks: Arc<RwLock<HashMap<String, PersistedTask>>>,
    results: Arc<RwLock<HashMap<String, AnalysisResult>>>,
    requests: Arc<RwLock<HashMap<String, SingleAnalysisRequest>>>,
}

impl InMemoryAnalysisStore {
    /// Create a new empty store.
    pub fn new() -> Self {
        Self {
            tasks: Arc::new(RwLock::new(HashMap::new())),
            results: Arc::new(RwLock::new(HashMap::new())),
            requests: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl Default for InMemoryAnalysisStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AnalysisStore for InMemoryAnalysisStore {
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
        let mut all: Vec<_> = tasks.values().cloned().collect();
        all.sort_by_key(|b| std::cmp::Reverse(b.created_at));
        Ok(all
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
        let mut all: Vec<_> = tasks
            .values()
            .filter(|t| t.owner_username == owner_username)
            .cloned()
            .collect();
        all.sort_by_key(|b| std::cmp::Reverse(b.created_at));
        Ok(all
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
        let found = tasks.values().find(|t| {
            t.symbol == symbol
                && t.analysis_date == analysis_date
                && t.status == TaskStatus::Completed
        });
        Ok(found.map(|t| t.task_id.clone()))
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
        let mut summaries: Vec<StoredAnalysisSummary> = tasks
            .values()
            .filter(|t| symbol.is_none_or(|s| t.symbol == s))
            .map(|t| StoredAnalysisSummary {
                task_id: t.task_id.clone(),
                symbol: t.symbol.clone(),
                stock_name: t.stock_name.clone(),
                market_type: t.market_type.clone(),
                status: t.status.as_str().to_string(),
                created_at: t.created_at.to_rfc3339(),
                updated_at: t.updated_at.to_rfc3339(),
            })
            .collect();
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

// ---------------------------------------------------------------------------
// InMemoryCacheStore
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct CacheValue {
    data: Vec<u8>,
    expires_at: Option<std::time::Instant>,
}

/// In-memory implementation of [`CacheStore`] with TTL support.
#[derive(Clone)]
pub struct InMemoryCacheStore {
    entries: Arc<RwLock<HashMap<String, CacheValue>>>,
}

impl InMemoryCacheStore {
    /// Create a new empty cache store.
    pub fn new() -> Self {
        Self {
            entries: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl Default for InMemoryCacheStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl CacheStore for InMemoryCacheStore {
    async fn get(&self, key: &str) -> anyhow::Result<Option<Vec<u8>>> {
        let entries = self.entries.read().await;
        match entries.get(key) {
            Some(entry) => {
                if let Some(exp) = entry.expires_at
                    && std::time::Instant::now() > exp
                {
                    return Ok(None);
                }
                Ok(Some(entry.data.clone()))
            }
            None => Ok(None),
        }
    }

    async fn set(&self, key: &str, value: &[u8], ttl_seconds: Option<u64>) -> anyhow::Result<()> {
        let expires_at =
            ttl_seconds.map(|ttl| std::time::Instant::now() + std::time::Duration::from_secs(ttl));
        self.entries.write().await.insert(
            key.to_string(),
            CacheValue {
                data: value.to_vec(),
                expires_at,
            },
        );
        Ok(())
    }

    async fn delete(&self, key: &str) -> anyhow::Result<()> {
        self.entries.write().await.remove(key);
        Ok(())
    }

    async fn exists(&self, key: &str) -> anyhow::Result<bool> {
        let entries = self.entries.read().await;
        match entries.get(key) {
            Some(entry) => {
                if let Some(exp) = entry.expires_at
                    && std::time::Instant::now() > exp
                {
                    return Ok(false);
                }
                Ok(true)
            }
            None => Ok(false),
        }
    }

    async fn list_entries(&self, prefix: &str) -> anyhow::Result<Vec<CacheEntry>> {
        let entries = self.entries.read().await;
        Ok(entries
            .iter()
            .filter(|(k, _)| k.starts_with(prefix))
            .map(|(k, v)| CacheEntry {
                key: k.clone(),
                created_at: String::new(),
                expires_at: v.expires_at.map(|exp| format!("{:?}", exp)),
                size_bytes: v.data.len() as u64,
            })
            .collect())
    }
}

// ---------------------------------------------------------------------------
// InMemoryCheckpointStore
// ---------------------------------------------------------------------------

/// In-memory implementation of [`CheckpointStore`].
#[derive(Clone)]
pub struct InMemoryCheckpointStore {
    checkpoints: Arc<RwLock<HashMap<String, StoredCheckpoint>>>,
}

impl InMemoryCheckpointStore {
    /// Create a new empty checkpoint store.
    pub fn new() -> Self {
        Self {
            checkpoints: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl Default for InMemoryCheckpointStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl CheckpointStore for InMemoryCheckpointStore {
    async fn save_checkpoint(
        &self,
        task_id: &str,
        step_name: &str,
        checkpoint: &StoredCheckpoint,
    ) -> anyhow::Result<()> {
        let key = format!("{}:{}", task_id, step_name);
        self.checkpoints
            .write()
            .await
            .insert(key, checkpoint.clone());
        Ok(())
    }

    async fn load_checkpoint(&self, task_id: &str) -> anyhow::Result<Option<StoredCheckpoint>> {
        let checkpoints = self.checkpoints.read().await;
        Ok(checkpoints
            .iter()
            .filter(|(k, _)| k.starts_with(&format!("{}:", task_id)))
            .max_by_key(|(_, v)| v.step)
            .map(|(_, v)| v.clone()))
    }

    async fn list_checkpoints(&self, task_id: &str) -> anyhow::Result<Vec<CheckpointInfo>> {
        let checkpoints = self.checkpoints.read().await;
        Ok(checkpoints
            .iter()
            .filter(|(k, _)| k.starts_with(&format!("{}:", task_id)))
            .map(|(_, v)| CheckpointInfo {
                task_id: v.task_id.clone(),
                checkpoint_id: format!("{}:{}", v.task_id, v.step_name),
                created_at: v.created_at.clone(),
                step_name: v.step_name.clone(),
            })
            .collect())
    }

    async fn delete_checkpoints(&self, task_id: &str) -> anyhow::Result<()> {
        let prefix = format!("{}:", task_id);
        let mut checkpoints = self.checkpoints.write().await;
        checkpoints.retain(|k, _| !k.starts_with(&prefix));
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// InMemoryGuidanceStore
// ---------------------------------------------------------------------------

/// In-memory implementation of [`GuidanceStore`].
#[derive(Clone)]
pub struct InMemoryGuidanceStore {
    rules: Arc<RwLock<HashMap<String, GuidanceRule>>>,
}

impl InMemoryGuidanceStore {
    /// Create a new empty guidance store.
    pub fn new() -> Self {
        Self {
            rules: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl Default for InMemoryGuidanceStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl GuidanceStore for InMemoryGuidanceStore {
    async fn list_rules(&self, market_type: &str) -> anyhow::Result<Vec<GuidanceRule>> {
        let rules = self.rules.read().await;
        Ok(rules
            .values()
            .filter(|r| r.market_type == market_type)
            .cloned()
            .collect())
    }

    async fn get_rule(&self, rule_id: &str) -> anyhow::Result<Option<GuidanceRule>> {
        Ok(self.rules.read().await.get(rule_id).cloned())
    }

    async fn upsert_rule(&self, rule: &GuidanceRule) -> anyhow::Result<()> {
        self.rules
            .write()
            .await
            .insert(rule.id.clone(), rule.clone());
        Ok(())
    }

    async fn delete_rule(&self, rule_id: &str) -> anyhow::Result<()> {
        self.rules.write().await.remove(rule_id);
        Ok(())
    }
}

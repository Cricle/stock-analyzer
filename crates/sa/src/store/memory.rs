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
    AnalysisResult, AnalysisStore, CacheStore, CheckpointStore, GuidanceStore,
    PersistedTask, SingleAnalysisRequest, TaskStatus,
};

// ---------------------------------------------------------------------------
// InMemoryAnalysisStore
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct InMemoryAnalysisStore {
    tasks: Arc<RwLock<HashMap<String, PersistedTask>>>,
    results: Arc<RwLock<HashMap<String, AnalysisResult>>>,
    requests: Arc<RwLock<HashMap<String, SingleAnalysisRequest>>>,
}

impl InMemoryAnalysisStore {
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
        all.sort_by(|a, b| b.created_at.cmp(&a.created_at));
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
        all.sort_by(|a, b| b.created_at.cmp(&a.created_at));
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
            .filter(|t| symbol.map_or(true, |s| t.symbol == s))
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

#[derive(Clone)]
pub struct InMemoryCacheStore {
    entries: Arc<RwLock<HashMap<String, CacheValue>>>,
}

impl InMemoryCacheStore {
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
                if let Some(exp) = entry.expires_at {
                    if std::time::Instant::now() > exp {
                        return Ok(None);
                    }
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
                if let Some(exp) = entry.expires_at {
                    if std::time::Instant::now() > exp {
                        return Ok(false);
                    }
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

#[derive(Clone)]
pub struct InMemoryCheckpointStore {
    checkpoints: Arc<RwLock<HashMap<String, StoredCheckpoint>>>,
}

impl InMemoryCheckpointStore {
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

#[derive(Clone)]
pub struct InMemoryGuidanceStore {
    rules: Arc<RwLock<HashMap<String, GuidanceRule>>>,
}

impl InMemoryGuidanceStore {
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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn make_task(id: &str, symbol: &str, status: TaskStatus) -> PersistedTask {
        PersistedTask {
            task_id: id.to_string(),
            owner_username: "user1".to_string(),
            symbol: symbol.to_string(),
            stock_name: "Test".to_string(),
            market_type: "US".to_string(),
            analysis_date: "2026-01-01".to_string(),
            research_depth: "deep".to_string(),
            request: SingleAnalysisRequest {
                symbol: Some(symbol.to_string()),
                stock_code: None,
                stock_name: None,
                parameters: None,
                force_refresh: false,
            },
            status,
            progress: 0,
            current_step_name: String::new(),
            current_step_description: String::new(),
            message: String::new(),
            error_message: None,
            llm_token_usage: Default::default(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[tokio::test]
    async fn analysis_store_insert_and_get() {
        let store = InMemoryAnalysisStore::new();
        let task = make_task("t1", "AAPL", TaskStatus::Pending);
        store.insert_task(&task).await.unwrap();
        let got = store.get_task("t1").await.unwrap();
        assert!(got.is_some());
        assert_eq!(got.unwrap().task_id, "t1");
    }

    #[tokio::test]
    async fn analysis_store_get_missing() {
        let store = InMemoryAnalysisStore::new();
        let got = store.get_task("nonexistent").await.unwrap();
        assert!(got.is_none());
    }

    #[tokio::test]
    async fn analysis_store_update_task() {
        let store = InMemoryAnalysisStore::new();
        let mut task = make_task("t1", "AAPL", TaskStatus::Pending);
        store.insert_task(&task).await.unwrap();
        task.status = TaskStatus::Running;
        task.progress = 50;
        store.update_task(&task).await.unwrap();
        let got = store.get_task("t1").await.unwrap().unwrap();
        assert_eq!(got.status, TaskStatus::Running);
        assert_eq!(got.progress, 50);
    }

    #[tokio::test]
    async fn analysis_store_list_tasks() {
        let store = InMemoryAnalysisStore::new();
        store.insert_task(&make_task("t1", "AAPL", TaskStatus::Pending)).await.unwrap();
        store.insert_task(&make_task("t2", "GOOGL", TaskStatus::Running)).await.unwrap();
        let tasks = store.list_tasks(10, 0).await.unwrap();
        assert_eq!(tasks.len(), 2);
    }

    #[tokio::test]
    async fn analysis_store_list_tasks_with_limit() {
        let store = InMemoryAnalysisStore::new();
        store.insert_task(&make_task("t1", "AAPL", TaskStatus::Pending)).await.unwrap();
        store.insert_task(&make_task("t2", "GOOGL", TaskStatus::Running)).await.unwrap();
        let tasks = store.list_tasks(1, 0).await.unwrap();
        assert_eq!(tasks.len(), 1);
    }

    #[tokio::test]
    async fn analysis_store_list_tasks_for_user() {
        let store = InMemoryAnalysisStore::new();
        let mut task1 = make_task("t1", "AAPL", TaskStatus::Pending);
        task1.owner_username = "alice".to_string();
        let mut task2 = make_task("t2", "GOOGL", TaskStatus::Pending);
        task2.owner_username = "bob".to_string();
        store.insert_task(&task1).await.unwrap();
        store.insert_task(&task2).await.unwrap();
        let tasks = store.list_tasks_for_user("alice", 10, 0).await.unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].task_id, "t1");
    }

    #[tokio::test]
    async fn analysis_store_find_cached_task() {
        let store = InMemoryAnalysisStore::new();
        store.insert_task(&make_task("t1", "AAPL", TaskStatus::Completed)).await.unwrap();
        store.insert_task(&make_task("t2", "AAPL", TaskStatus::Running)).await.unwrap();
        let found = store.find_cached_task("AAPL", "2026-01-01").await.unwrap();
        assert_eq!(found, Some("t1".to_string()));
    }

    #[tokio::test]
    async fn analysis_store_find_cached_task_not_found() {
        let store = InMemoryAnalysisStore::new();
        store.insert_task(&make_task("t1", "AAPL", TaskStatus::Running)).await.unwrap();
        let found = store.find_cached_task("AAPL", "2026-01-01").await.unwrap();
        assert!(found.is_none());
    }

    #[tokio::test]
    async fn analysis_store_save_and_load_result() {
        use crate::analysis::{AnalysisGraph, AgentStateSnapshot, StructuredReport};
        let store = InMemoryAnalysisStore::new();
        let result = AnalysisResult {
            task_id: "t1".to_string(),
            report_id: "r1".to_string(),
            symbol: "AAPL".to_string(),
            stock_name: "Apple".to_string(),
            analysis_date: "2026-01-01".to_string(),
            market_type: "US".to_string(),
            graph: AnalysisGraph::default(),
            agent_state: AgentStateSnapshot::default(),
            artifacts: Default::default(),
            report: StructuredReport::default(),
            ic_report: StructuredReport::default(),
            created_at: "2026-01-01".to_string(),
        };
        store.save_result("t1", &result).await.unwrap();
        let loaded = store.load_result("t1").await.unwrap();
        assert!(loaded.is_some());
    }

    #[tokio::test]
    async fn analysis_store_load_result_missing() {
        let store = InMemoryAnalysisStore::new();
        let loaded = store.load_result("nonexistent").await.unwrap();
        assert!(loaded.is_none());
    }

    #[tokio::test]
    async fn analysis_store_list_analyses() {
        let store = InMemoryAnalysisStore::new();
        store.insert_task(&make_task("t1", "AAPL", TaskStatus::Completed)).await.unwrap();
        store.insert_task(&make_task("t2", "GOOGL", TaskStatus::Completed)).await.unwrap();
        let analyses = store.list_analyses(None, 10).await.unwrap();
        assert_eq!(analyses.len(), 2);
    }

    #[tokio::test]
    async fn analysis_store_list_analyses_filtered() {
        let store = InMemoryAnalysisStore::new();
        store.insert_task(&make_task("t1", "AAPL", TaskStatus::Completed)).await.unwrap();
        store.insert_task(&make_task("t2", "GOOGL", TaskStatus::Completed)).await.unwrap();
        let analyses = store.list_analyses(Some("AAPL"), 10).await.unwrap();
        assert_eq!(analyses.len(), 1);
    }

    #[tokio::test]
    async fn analysis_store_delete_analysis() {
        let store = InMemoryAnalysisStore::new();
        store.insert_task(&make_task("t1", "AAPL", TaskStatus::Pending)).await.unwrap();
        store.delete_analysis("t1").await.unwrap();
        let got = store.get_task("t1").await.unwrap();
        assert!(got.is_none());
    }

    #[tokio::test]
    async fn analysis_store_save_and_load_request() {
        let store = InMemoryAnalysisStore::new();
        let request = SingleAnalysisRequest {
            symbol: Some("AAPL".to_string()),
            stock_code: None,
            stock_name: None,
            parameters: None,
            force_refresh: false,
        };
        store.save_request("t1", &request).await.unwrap();
        let loaded = store.load_request("t1").await.unwrap();
        assert!(loaded.is_some());
        assert_eq!(loaded.unwrap().symbol, Some("AAPL".to_string()));
    }

    #[tokio::test]
    async fn cache_store_set_and_get() {
        let store = InMemoryCacheStore::new();
        store.set("key1", b"value1", None).await.unwrap();
        let got = store.get("key1").await.unwrap();
        assert_eq!(got, Some(b"value1".to_vec()));
    }

    #[tokio::test]
    async fn cache_store_get_missing() {
        let store = InMemoryCacheStore::new();
        let got = store.get("nonexistent").await.unwrap();
        assert!(got.is_none());
    }

    #[tokio::test]
    async fn cache_store_delete() {
        let store = InMemoryCacheStore::new();
        store.set("key1", b"value1", None).await.unwrap();
        store.delete("key1").await.unwrap();
        let got = store.get("key1").await.unwrap();
        assert!(got.is_none());
    }

    #[tokio::test]
    async fn cache_store_exists() {
        let store = InMemoryCacheStore::new();
        assert!(!store.exists("key1").await.unwrap());
        store.set("key1", b"value1", None).await.unwrap();
        assert!(store.exists("key1").await.unwrap());
    }

    #[tokio::test]
    async fn cache_store_list_entries() {
        let store = InMemoryCacheStore::new();
        store.set("prefix:a", b"1", None).await.unwrap();
        store.set("prefix:b", b"2", None).await.unwrap();
        store.set("other:c", b"3", None).await.unwrap();
        let entries = store.list_entries("prefix:").await.unwrap();
        assert_eq!(entries.len(), 2);
    }

    #[tokio::test]
    async fn checkpoint_store_save_and_load() {
        let store = InMemoryCheckpointStore::new();
        let cp = StoredCheckpoint {
            task_id: "t1".to_string(),
            step_name: "step1".to_string(),
            stage: "init".to_string(),
            node: "node1".to_string(),
            step: 1,
            data: serde_json::json!({"key": "value"}),
            created_at: "2026-01-01".to_string(),
        };
        store.save_checkpoint("t1", "step1", &cp).await.unwrap();
        let loaded = store.load_checkpoint("t1").await.unwrap();
        assert!(loaded.is_some());
        assert_eq!(loaded.unwrap().step_name, "step1");
    }

    #[tokio::test]
    async fn checkpoint_store_load_missing() {
        let store = InMemoryCheckpointStore::new();
        let loaded = store.load_checkpoint("nonexistent").await.unwrap();
        assert!(loaded.is_none());
    }

    #[tokio::test]
    async fn checkpoint_store_list_checkpoints() {
        let store = InMemoryCheckpointStore::new();
        let cp1 = StoredCheckpoint {
            task_id: "t1".to_string(),
            step_name: "step1".to_string(),
            stage: String::new(),
            node: String::new(),
            step: 1,
            data: serde_json::json!({}),
            created_at: "2026-01-01".to_string(),
        };
        let cp2 = StoredCheckpoint {
            task_id: "t1".to_string(),
            step_name: "step2".to_string(),
            stage: String::new(),
            node: String::new(),
            step: 2,
            data: serde_json::json!({}),
            created_at: "2026-01-02".to_string(),
        };
        store.save_checkpoint("t1", "step1", &cp1).await.unwrap();
        store.save_checkpoint("t1", "step2", &cp2).await.unwrap();
        let list = store.list_checkpoints("t1").await.unwrap();
        assert_eq!(list.len(), 2);
    }

    #[tokio::test]
    async fn checkpoint_store_delete_checkpoints() {
        let store = InMemoryCheckpointStore::new();
        let cp = StoredCheckpoint {
            task_id: "t1".to_string(),
            step_name: "step1".to_string(),
            stage: String::new(),
            node: String::new(),
            step: 1,
            data: serde_json::json!({}),
            created_at: "2026-01-01".to_string(),
        };
        store.save_checkpoint("t1", "step1", &cp).await.unwrap();
        store.delete_checkpoints("t1").await.unwrap();
        let loaded = store.load_checkpoint("t1").await.unwrap();
        assert!(loaded.is_none());
    }

    #[tokio::test]
    async fn guidance_store_upsert_and_get() {
        let store = InMemoryGuidanceStore::new();
        let rule = GuidanceRule {
            id: "r1".to_string(),
            market_type: "US".to_string(),
            rule_type: "risk".to_string(),
            content: "Be careful".to_string(),
            priority: 1,
            enabled: true,
        };
        store.upsert_rule(&rule).await.unwrap();
        let got = store.get_rule("r1").await.unwrap();
        assert!(got.is_some());
        assert_eq!(got.unwrap().content, "Be careful");
    }

    #[tokio::test]
    async fn guidance_store_get_missing() {
        let store = InMemoryGuidanceStore::new();
        let got = store.get_rule("nonexistent").await.unwrap();
        assert!(got.is_none());
    }

    #[tokio::test]
    async fn guidance_store_list_rules() {
        let store = InMemoryGuidanceStore::new();
        store.upsert_rule(&GuidanceRule {
            id: "r1".to_string(),
            market_type: "US".to_string(),
            rule_type: "risk".to_string(),
            content: "Rule 1".to_string(),
            priority: 1,
            enabled: true,
        }).await.unwrap();
        store.upsert_rule(&GuidanceRule {
            id: "r2".to_string(),
            market_type: "US".to_string(),
            rule_type: "risk".to_string(),
            content: "Rule 2".to_string(),
            priority: 2,
            enabled: true,
        }).await.unwrap();
        store.upsert_rule(&GuidanceRule {
            id: "r3".to_string(),
            market_type: "CN".to_string(),
            rule_type: "risk".to_string(),
            content: "Rule 3".to_string(),
            priority: 1,
            enabled: true,
        }).await.unwrap();
        let rules = store.list_rules("US").await.unwrap();
        assert_eq!(rules.len(), 2);
    }

    #[tokio::test]
    async fn guidance_store_delete_rule() {
        let store = InMemoryGuidanceStore::new();
        store.upsert_rule(&GuidanceRule {
            id: "r1".to_string(),
            market_type: "US".to_string(),
            rule_type: "risk".to_string(),
            content: "Rule 1".to_string(),
            priority: 1,
            enabled: true,
        }).await.unwrap();
        store.delete_rule("r1").await.unwrap();
        let got = store.get_rule("r1").await.unwrap();
        assert!(got.is_none());
    }
}

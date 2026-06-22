//! Task checkpoint store for resumable analysis.
//!
//! Migrated from `backend/src/engine/checkpoint/`.
//!
//! Uses `sa_models::CheckpointStore` trait instead of direct Redis access.

use std::sync::Arc;

use adk_graph::{
    checkpoint::Checkpointer, error::Result as GraphResult, state::Checkpoint as GraphCheckpoint,
};
use anyhow::Context;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use sa_models::AnalysisResult;

#[derive(Clone)]
pub struct TaskCheckpointStore {
    inner: Arc<dyn sa_models::CheckpointStore>,
    // TODO: The graph checkpointer currently requires a Redis-backed implementation.
    // For now we store an optional in-memory fallback; production should wire
    // a real Checkpointer backed by the same CheckpointStore.
    graph_checkpoints: Arc<tokio::sync::RwLock<std::collections::HashMap<String, GraphCheckpoint>>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TaskCheckpoint {
    pub task_id: String,
    pub symbol: String,
    pub analysis_date: String,
    pub stage: String,
    pub node: String,
    pub result: AnalysisResult,
    pub step: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CheckpointWrite {
    pub stage: String,
    pub node: String,
    pub step: i64,
    pub created_at: String,
}

impl TaskCheckpointStore {
    pub fn new(inner: Arc<dyn sa_models::CheckpointStore>) -> Self {
        Self {
            inner,
            graph_checkpoints: Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
        }
    }

    pub async fn save(&self, checkpoint: &TaskCheckpoint) -> anyhow::Result<()> {
        let step_name = format!("{}:{}", checkpoint.stage, checkpoint.node);
        // Convert TaskCheckpoint to AnalysisCheckpoint for the trait
        let analysis_checkpoint = sa_models::StoredCheckpoint {
            task_id: checkpoint.task_id.clone(),
            step_name: step_name.clone(),
            stage: checkpoint.stage.clone(),
            node: checkpoint.node.clone(),
            step: checkpoint.step,
            data: serde_json::to_value(checkpoint)?,
            created_at: chrono::Utc::now().to_rfc3339(),
        };
        self.inner
            .save_checkpoint(&checkpoint.task_id, &step_name, &analysis_checkpoint)
            .await
    }

    pub async fn load(
        &self,
        task_id: &str,
        symbol: &str,
        analysis_date: &str,
    ) -> anyhow::Result<Option<TaskCheckpoint>> {
        let _ = (symbol, analysis_date); // Thread ID used for redis keying; trait handles this
        let checkpoint = self.inner.load_checkpoint(task_id).await?;
        match checkpoint {
            Some(cp) => {
                let task_checkpoint: TaskCheckpoint = serde_json::from_value(cp.data)
                    .context("failed to deserialize TaskCheckpoint")?;
                Ok(Some(task_checkpoint))
            }
            None => Ok(None),
        }
    }

    pub async fn clear(
        &self,
        task_id: &str,
        _symbol: &str,
        _analysis_date: &str,
    ) -> anyhow::Result<()> {
        self.inner.delete_checkpoints(task_id).await
    }

    pub async fn checkpoint_step(
        &self,
        task_id: &str,
        symbol: &str,
        analysis_date: &str,
    ) -> anyhow::Result<Option<i64>> {
        Ok(self
            .load(task_id, symbol, analysis_date)
            .await?
            .map(|item| item.step))
    }

    pub async fn clear_graph_runtime(
        &self,
        task_id: &str,
        _symbol: &str,
        _analysis_date: &str,
    ) -> anyhow::Result<()> {
        let thread_id = Self::thread_id(task_id, "", "");
        let mut checkpoints = self.graph_checkpoints.write().await;
        // Remove all checkpoints matching this thread_id prefix
        checkpoints.retain(|key, _| !key.starts_with(&thread_id));
        Ok(())
    }

    pub async fn load_writes(
        &self,
        task_id: &str,
        _symbol: &str,
        _analysis_date: &str,
    ) -> anyhow::Result<Vec<CheckpointWrite>> {
        // The trait-based CheckpointStore doesn't have a separate writes log.
        // Return checkpoint info converted to CheckpointWrite format.
        let infos = self.inner.list_checkpoints(task_id).await?;
        Ok(infos
            .into_iter()
            .map(|info| CheckpointWrite {
                stage: info.step_name.clone(),
                node: info.step_name,
                step: 0,
                created_at: info.created_at,
            })
            .collect())
    }

    pub fn graph_checkpointer(&self, _symbol: &str) -> anyhow::Result<Arc<dyn Checkpointer>> {
        Ok(Arc::new(InMemoryGraphCheckpointer {
            checkpoints: self.graph_checkpoints.clone(),
        }))
    }

    pub fn thread_id(task_id: &str, symbol: &str, analysis_date: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(format!(
            "{}:{}:{}",
            task_id,
            symbol.to_uppercase(),
            analysis_date
        ));
        let digest = hasher.finalize();
        hex_16(&digest)
    }
}

/// In-memory graph checkpointer fallback.
///
/// TODO: Replace with a proper implementation backed by `CheckpointStore` trait
/// for production persistence.
struct InMemoryGraphCheckpointer {
    checkpoints: Arc<tokio::sync::RwLock<std::collections::HashMap<String, GraphCheckpoint>>>,
}

#[async_trait]
impl Checkpointer for InMemoryGraphCheckpointer {
    async fn save(&self, checkpoint: &GraphCheckpoint) -> GraphResult<String> {
        let checkpoint_id = checkpoint.checkpoint_id.clone();
        let mut map = self.checkpoints.write().await;
        map.insert(checkpoint_id.clone(), checkpoint.clone());
        Ok(checkpoint_id)
    }

    async fn load(&self, thread_id: &str) -> GraphResult<Option<GraphCheckpoint>> {
        let map = self.checkpoints.read().await;
        // Find the latest checkpoint for this thread_id
        Ok(map
            .values()
            .filter(|cp| cp.thread_id == thread_id)
            .max_by_key(|cp| cp.created_at)
            .cloned())
    }

    async fn load_by_id(&self, checkpoint_id: &str) -> GraphResult<Option<GraphCheckpoint>> {
        let map = self.checkpoints.read().await;
        Ok(map.get(checkpoint_id).cloned())
    }

    async fn list(&self, thread_id: &str) -> GraphResult<Vec<GraphCheckpoint>> {
        let map = self.checkpoints.read().await;
        let mut cps: Vec<_> = map
            .values()
            .filter(|cp| cp.thread_id == thread_id)
            .cloned()
            .collect();
        cps.sort_by_key(|cp| cp.created_at);
        Ok(cps)
    }

    async fn delete(&self, thread_id: &str) -> GraphResult<()> {
        let mut map = self.checkpoints.write().await;
        map.retain(|_, cp| cp.thread_id != thread_id);
        Ok(())
    }
}

fn hex_16(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(16);
    for byte in bytes.iter().take(8) {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

//! Filesystem-backed implementations of AnalysisStore, CacheStore, CheckpointStore.

use std::path::PathBuf;

use async_trait::async_trait;
use chrono::Utc;
use sha2::{Digest, Sha256};

use crate::models::{
    AnalysisResult, CacheEntry, CheckpointInfo, PersistedTask, SingleAnalysisRequest,
    StoredCheckpoint,
};

// ---------------------------------------------------------------------------
// FilesystemAnalysisStore
// ---------------------------------------------------------------------------

/// Filesystem-backed implementation of [`crate::models::AnalysisStore`].
///
/// Layout:
/// ```text
/// {base_dir}/tasks/{task_id}.json
/// {base_dir}/tasks/{task_id}/result.json
/// {base_dir}/tasks/{task_id}/request.json
/// ```
pub struct FilesystemAnalysisStore {
    base_dir: PathBuf,
}

impl FilesystemAnalysisStore {
    pub fn new(base_dir: impl Into<PathBuf>) -> Self {
        Self {
            base_dir: base_dir.into(),
        }
    }

    fn tasks_dir(&self) -> PathBuf {
        self.base_dir.join("tasks")
    }

    fn task_path(&self, task_id: &str) -> PathBuf {
        self.tasks_dir().join(format!("{task_id}.json"))
    }

    fn task_dir(&self, task_id: &str) -> PathBuf {
        self.tasks_dir().join(task_id)
    }
}

#[async_trait]
impl crate::models::AnalysisStore for FilesystemAnalysisStore {
    async fn insert_task(&self, task: &PersistedTask) -> anyhow::Result<()> {
        let dir = self.tasks_dir();
        tokio::fs::create_dir_all(&dir).await?;
        let path = self.task_path(&task.task_id);
        let data = serde_json::to_vec_pretty(task)?;
        tokio::fs::write(&path, data).await?;
        Ok(())
    }

    async fn update_task(&self, task: &PersistedTask) -> anyhow::Result<()> {
        self.insert_task(task).await
    }

    async fn get_task(&self, task_id: &str) -> anyhow::Result<Option<PersistedTask>> {
        let path = self.task_path(task_id);
        if !path.exists() {
            return Ok(None);
        }
        let data = tokio::fs::read(&path).await?;
        let task: PersistedTask = serde_json::from_slice(&data)?;
        Ok(Some(task))
    }

    async fn list_tasks(&self, limit: i64, offset: i64) -> anyhow::Result<Vec<PersistedTask>> {
        let dir = self.tasks_dir();
        if !dir.exists() {
            return Ok(Vec::new());
        }
        let mut tasks = Vec::new();
        let mut entries = tokio::fs::read_dir(&dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "json")
                && let Ok(data) = tokio::fs::read(&path).await
                && let Ok(task) = serde_json::from_slice::<PersistedTask>(&data)
            {
                tasks.push(task);
            }
        }
        tasks.sort_by_key(|t| std::cmp::Reverse(t.created_at));
        let start = offset.max(0) as usize;
        let end = start + limit.max(0) as usize;
        Ok(tasks.into_iter().skip(start).take(end - start).collect())
    }

    async fn list_tasks_for_user(
        &self,
        owner_username: &str,
        limit: i64,
        offset: i64,
    ) -> anyhow::Result<Vec<PersistedTask>> {
        let all = self.list_tasks(limit + offset, 0).await?;
        let filtered: Vec<_> = all
            .into_iter()
            .filter(|t| t.owner_username.eq_ignore_ascii_case(owner_username))
            .collect();
        let start = offset.max(0) as usize;
        let end = start + limit.max(0) as usize;
        Ok(filtered.into_iter().skip(start).take(end - start).collect())
    }

    async fn find_cached_task(
        &self,
        symbol: &str,
        analysis_date: &str,
    ) -> anyhow::Result<Option<String>> {
        let dir = self.tasks_dir();
        if !dir.exists() {
            return Ok(None);
        }
        let mut entries = tokio::fs::read_dir(&dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "json")
                && let Ok(data) = tokio::fs::read(&path).await
                && let Ok(task) = serde_json::from_slice::<PersistedTask>(&data)
                && task.symbol.eq_ignore_ascii_case(symbol)
                && task.analysis_date == analysis_date
                && task.status == crate::models::TaskStatus::Completed
            {
                return Ok(Some(task.task_id));
            }
        }
        Ok(None)
    }

    async fn save_result(
        &self,
        task_id: &str,
        result: &AnalysisResult,
    ) -> anyhow::Result<()> {
        let dir = self.task_dir(task_id);
        tokio::fs::create_dir_all(&dir).await?;
        let path = dir.join("result.json");
        let data = serde_json::to_vec_pretty(result)?;
        tokio::fs::write(&path, data).await?;
        Ok(())
    }

    async fn load_result(&self, task_id: &str) -> anyhow::Result<Option<AnalysisResult>> {
        let path = self.task_dir(task_id).join("result.json");
        if !path.exists() {
            return Ok(None);
        }
        let data = tokio::fs::read(&path).await?;
        let result: AnalysisResult = serde_json::from_slice(&data)?;
        Ok(Some(result))
    }

    async fn delete_analysis(&self, task_id: &str) -> anyhow::Result<()> {
        let task_path = self.task_path(task_id);
        if task_path.exists() {
            tokio::fs::remove_file(&task_path).await?;
        }
        let dir = self.task_dir(task_id);
        if dir.exists() {
            tokio::fs::remove_dir_all(&dir).await?;
        }
        Ok(())
    }

    async fn save_request(
        &self,
        task_id: &str,
        request: &SingleAnalysisRequest,
    ) -> anyhow::Result<()> {
        let dir = self.task_dir(task_id);
        tokio::fs::create_dir_all(&dir).await?;
        let path = dir.join("request.json");
        let data = serde_json::to_vec_pretty(request)?;
        tokio::fs::write(&path, data).await?;
        Ok(())
    }

    async fn load_request(
        &self,
        task_id: &str,
    ) -> anyhow::Result<Option<SingleAnalysisRequest>> {
        let path = self.task_dir(task_id).join("request.json");
        if !path.exists() {
            return Ok(None);
        }
        let data = tokio::fs::read(&path).await?;
        let request: SingleAnalysisRequest = serde_json::from_slice(&data)?;
        Ok(Some(request))
    }
}

// ---------------------------------------------------------------------------
// FilesystemCacheStore
// ---------------------------------------------------------------------------

/// Filesystem-backed implementation of [`crate::models::CacheStore`].
///
/// Layout:
/// ```text
/// {base_dir}/cache/{sha256(key)}.json
/// ```
///
/// Each file contains a JSON object with `expires_at`, `created_at`, and `value` (base64).
pub struct FilesystemCacheStore {
    base_dir: PathBuf,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct CacheFile {
    created_at: String,
    expires_at: Option<String>,
    value: Vec<u8>,
}

impl FilesystemCacheStore {
    pub fn new(base_dir: impl Into<PathBuf>) -> Self {
        Self {
            base_dir: base_dir.into(),
        }
    }

    fn cache_dir(&self) -> PathBuf {
        self.base_dir.join("cache")
    }

    fn cache_path(&self, key: &str) -> PathBuf {
        let hash = hex_hash(key);
        self.cache_dir().join(format!("{hash}.json"))
    }
}

fn hex_hash(key: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(key.as_bytes());
    let digest = hasher.finalize();
    hex::encode(digest)
}

#[async_trait]
impl crate::models::CacheStore for FilesystemCacheStore {
    async fn get(&self, key: &str) -> anyhow::Result<Option<Vec<u8>>> {
        let path = self.cache_path(key);
        if !path.exists() {
            return Ok(None);
        }
        let data = tokio::fs::read(&path).await?;
        let file: CacheFile = serde_json::from_slice(&data)?;

        // Check TTL expiration
        if let Some(ref expires_at) = file.expires_at
            && let Ok(expires) = chrono::DateTime::parse_from_rfc3339(expires_at)
            && Utc::now() > expires.with_timezone(&Utc)
        {
            // Expired — clean up
            let _ = tokio::fs::remove_file(&path).await;
            return Ok(None);
        }

        Ok(Some(file.value))
    }

    async fn set(
        &self,
        key: &str,
        value: &[u8],
        ttl_seconds: Option<u64>,
    ) -> anyhow::Result<()> {
        let dir = self.cache_dir();
        tokio::fs::create_dir_all(&dir).await?;
        let now = Utc::now();
        let expires_at = ttl_seconds.map(|ttl| {
            (now + chrono::Duration::seconds(ttl as i64)).to_rfc3339()
        });
        let file = CacheFile {
            created_at: now.to_rfc3339(),
            expires_at,
            value: value.to_vec(),
        };
        let path = self.cache_path(key);
        let data = serde_json::to_vec(&file)?;
        tokio::fs::write(&path, data).await?;
        Ok(())
    }

    async fn delete(&self, key: &str) -> anyhow::Result<()> {
        let path = self.cache_path(key);
        if path.exists() {
            tokio::fs::remove_file(&path).await?;
        }
        Ok(())
    }

    async fn exists(&self, key: &str) -> anyhow::Result<bool> {
        let path = self.cache_path(key);
        if !path.exists() {
            return Ok(false);
        }
        // Also check expiration
        Ok(self.get(key).await?.is_some())
    }

    async fn list_entries(&self, prefix: &str) -> anyhow::Result<Vec<CacheEntry>> {
        let dir = self.cache_dir();
        if !dir.exists() {
            return Ok(Vec::new());
        }
        let mut entries = Vec::new();
        let mut dir_entries = tokio::fs::read_dir(&dir).await?;
        while let Some(entry) = dir_entries.next_entry().await? {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "json")
                && let Ok(data) = tokio::fs::read(&path).await
                && let Ok(file) = serde_json::from_slice::<CacheFile>(&data)
            {
                let _ = prefix;
                let size_bytes = file.value.len() as u64;
                entries.push(CacheEntry {
                    key: path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or_default()
                        .to_string(),
                    created_at: file.created_at,
                    expires_at: file.expires_at,
                    size_bytes,
                });
            }
        }
        Ok(entries)
    }
}

// ---------------------------------------------------------------------------
// FilesystemCheckpointStore
// ---------------------------------------------------------------------------

/// Filesystem-backed implementation of [`crate::models::CheckpointStore`].
///
/// Layout:
/// ```text
/// {base_dir}/checkpoints/{task_id}/{checkpoint_id}.json
/// ```
pub struct FilesystemCheckpointStore {
    base_dir: PathBuf,
}

impl FilesystemCheckpointStore {
    pub fn new(base_dir: impl Into<PathBuf>) -> Self {
        Self {
            base_dir: base_dir.into(),
        }
    }

    fn checkpoints_dir(&self) -> PathBuf {
        self.base_dir.join("checkpoints")
    }

    fn task_checkpoints_dir(&self, task_id: &str) -> PathBuf {
        self.checkpoints_dir().join(task_id)
    }
}

#[async_trait]
impl crate::models::CheckpointStore for FilesystemCheckpointStore {
    async fn save_checkpoint(
        &self,
        task_id: &str,
        _step_name: &str,
        checkpoint: &StoredCheckpoint,
    ) -> anyhow::Result<()> {
        let dir = self.task_checkpoints_dir(task_id);
        tokio::fs::create_dir_all(&dir).await?;
        let checkpoint_id = format!(
            "{}_{}",
            checkpoint.step,
            Utc::now().timestamp_millis()
        );
        let path = dir.join(format!("{checkpoint_id}.json"));
        let data = serde_json::to_vec_pretty(checkpoint)?;
        tokio::fs::write(&path, data).await?;
        Ok(())
    }

    async fn load_checkpoint(
        &self,
        task_id: &str,
    ) -> anyhow::Result<Option<StoredCheckpoint>> {
        let dir = self.task_checkpoints_dir(task_id);
        if !dir.exists() {
            return Ok(None);
        }
        // Find the latest checkpoint by modification time
        let mut latest: Option<(std::time::SystemTime, StoredCheckpoint)> = None;
        let mut entries = tokio::fs::read_dir(&dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "json")
                && let Ok(data) = tokio::fs::read(&path).await
                && let Ok(cp) = serde_json::from_slice::<StoredCheckpoint>(&data)
            {
                let mtime = entry
                    .metadata()
                    .await
                    .ok()
                    .and_then(|m| m.modified().ok())
                    .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                if latest.is_none() || mtime > latest.as_ref().unwrap().0 {
                    latest = Some((mtime, cp));
                }
            }
        }
        Ok(latest.map(|(_, cp)| cp))
    }

    async fn list_checkpoints(
        &self,
        task_id: &str,
    ) -> anyhow::Result<Vec<CheckpointInfo>> {
        let dir = self.task_checkpoints_dir(task_id);
        if !dir.exists() {
            return Ok(Vec::new());
        }
        let mut infos = Vec::new();
        let mut entries = tokio::fs::read_dir(&dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "json")
                && let Ok(data) = tokio::fs::read(&path).await
                && let Ok(cp) = serde_json::from_slice::<StoredCheckpoint>(&data)
            {
                infos.push(CheckpointInfo {
                    task_id: cp.task_id.clone(),
                    checkpoint_id: path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or_default()
                        .to_string(),
                    created_at: cp.created_at.clone(),
                    step_name: cp.step_name.clone(),
                });
            }
        }
        infos.sort_by_key(|i| i.created_at.clone());
        Ok(infos)
    }

    async fn delete_checkpoints(&self, task_id: &str) -> anyhow::Result<()> {
        let dir = self.task_checkpoints_dir(task_id);
        if dir.exists() {
            tokio::fs::remove_dir_all(&dir).await?;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// FilesystemRecommendationStore
// ---------------------------------------------------------------------------

/// Filesystem-backed implementation of [`crate::models::RecommendationStore`].
///
/// Layout:
/// ```text
/// {base_dir}/recommendations/{market}/{symbol}/{timestamp}.json
/// {base_dir}/recommendations/{market}/_latest.json   (rolling summary)
/// ```
pub struct FilesystemRecommendationStore {
    base_dir: PathBuf,
}

impl FilesystemRecommendationStore {
    pub fn new(base_dir: impl Into<PathBuf>) -> Self {
        Self {
            base_dir: base_dir.into(),
        }
    }

    fn rec_dir(&self, market: &str, symbol: &str) -> PathBuf {
        self.base_dir
            .join("recommendations")
            .join(market)
            .join(symbol)
    }

    fn latest_path(&self, market: &str) -> PathBuf {
        self.base_dir
            .join("recommendations")
            .join(market)
            .join("_latest.json")
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
struct LatestSummary {
    market: String,
    analysis_date: String,
    saved_at: String,
    picks: Vec<crate::models::PersistedRecommendation>,
}

#[async_trait]
impl crate::models::RecommendationStore for FilesystemRecommendationStore {
    async fn save_recommendation(&self, rec: &crate::models::PersistedRecommendation) -> anyhow::Result<()> {
        let dir = self.rec_dir(&rec.market, &rec.symbol);
        tokio::fs::create_dir_all(&dir).await?;
        let ts = chrono::Utc::now().format("%Y%m%dT%H%M%S%.3f");
        let path = dir.join(format!("{ts}.json"));
        let data = serde_json::to_vec_pretty(rec)?;
        tokio::fs::write(&path, data).await?;
        Ok(())
    }

    async fn get_recommendations(&self, symbol: &str) -> anyhow::Result<Vec<crate::models::PersistedRecommendation>> {
        // Scan all market subdirs for this symbol
        let base = self.base_dir.join("recommendations");
        if !base.exists() {
            return Ok(Vec::new());
        }
        let mut results = Vec::new();
        let mut markets = tokio::fs::read_dir(&base).await?;
        while let Some(market_entry) = markets.next_entry().await? {
            if !market_entry.file_type().await?.is_dir() {
                continue;
            }
            let sym_dir = market_entry.path().join(symbol);
            if !sym_dir.exists() {
                continue;
            }
            let mut files = tokio::fs::read_dir(&sym_dir).await?;
            while let Some(file_entry) = files.next_entry().await? {
                let path = file_entry.path();
                if path.extension().is_some_and(|ext| ext == "json")
                    && let Ok(data) = tokio::fs::read(&path).await
                    && let Ok(rec) = serde_json::from_slice::<crate::models::PersistedRecommendation>(&data)
                {
                    results.push(rec);
                }
            }
        }
        results.sort_by(|a, b| b.scored_at.cmp(&a.scored_at));
        Ok(results)
    }

    async fn get_latest(&self, limit: usize) -> anyhow::Result<Vec<crate::models::PersistedRecommendation>> {
        let base = self.base_dir.join("recommendations");
        if !base.exists() {
            return Ok(Vec::new());
        }
        let mut all = Vec::new();
        let mut markets = tokio::fs::read_dir(&base).await?;
        while let Some(market_entry) = markets.next_entry().await? {
            if !market_entry.file_type().await?.is_dir() {
                continue;
            }
            let mut symbols = tokio::fs::read_dir(market_entry.path()).await?;
            while let Some(sym_entry) = symbols.next_entry().await? {
                if !sym_entry.file_type().await?.is_dir() {
                    continue;
                }
                let mut files = tokio::fs::read_dir(sym_entry.path()).await?;
                while let Some(file_entry) = files.next_entry().await? {
                    let path = file_entry.path();
                    if path.extension().is_some_and(|ext| ext == "json")
                        && let Ok(data) = tokio::fs::read(&path).await
                        && let Ok(rec) = serde_json::from_slice::<crate::models::PersistedRecommendation>(&data)
                    {
                        all.push(rec);
                    }
                }
            }
        }
        all.sort_by(|a, b| b.scored_at.cmp(&a.scored_at));
        all.truncate(limit);
        Ok(all)
    }

    async fn get_latest_stock_pick_summary(&self, market: &str) -> anyhow::Result<Option<serde_json::Value>> {
        let path = self.latest_path(market);
        if !path.exists() {
            return Ok(None);
        }
        let data = tokio::fs::read(&path).await?;
        let summary: LatestSummary = serde_json::from_slice(&data)?;
        Ok(Some(serde_json::to_value(summary)?))
    }

    async fn delete_recommendations(&self, symbol: &str) -> anyhow::Result<()> {
        let base = self.base_dir.join("recommendations");
        if !base.exists() {
            return Ok(());
        }
        let mut markets = tokio::fs::read_dir(&base).await?;
        while let Some(market_entry) = markets.next_entry().await? {
            if !market_entry.file_type().await?.is_dir() {
                continue;
            }
            let sym_dir = market_entry.path().join(symbol);
            if sym_dir.exists() {
                tokio::fs::remove_dir_all(&sym_dir).await?;
            }
        }
        Ok(())
    }
}

/// Save a rolling summary of the latest picks for a market.
pub async fn save_latest_pick_summary(
    store: &FilesystemRecommendationStore,
    market: &str,
    analysis_date: &str,
    picks: &[crate::models::PersistedRecommendation],
) -> anyhow::Result<()> {
    let dir = store.base_dir.join("recommendations").join(market);
    tokio::fs::create_dir_all(&dir).await?;
    let summary = LatestSummary {
        market: market.to_string(),
        analysis_date: analysis_date.to_string(),
        saved_at: chrono::Utc::now().to_rfc3339(),
        picks: picks.to_vec(),
    };
    let path = dir.join("_latest.json");
    let data = serde_json::to_vec_pretty(&summary)?;
    tokio::fs::write(&path, data).await?;
    Ok(())
}

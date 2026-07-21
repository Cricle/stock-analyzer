use std::{collections::HashMap, sync::Arc};

use chrono::{Datelike, Duration, Local, LocalResult, TimeZone, Utc};
use tokio::sync::{RwLock, broadcast};
use tokio::task::AbortHandle;

use crate::checkpoint::TaskCheckpointStore;
use crate::memory::TradingMemoryLog;
use crate::memory::cross_collection::CrossCollectionSearcher;
use crate::task::PersistedTask;
use crate::telemetry::SharedTelemetry;

/// Ordered analysis pipeline steps with progress percentages.
pub const TASK_STEPS: [(&str, &str, i32); 5] = [
    (
        "\u{5e02}\u{573a}\u{5206}\u{6790}",
        "\u{6293}\u{53d6}\u{884c}\u{60c5}\u{4e0e}\u{6280}\u{672f}\u{6307}\u{6807}",
        15,
    ),
    (
        "\u{57fa}\u{672c}\u{9762}\u{5206}\u{6790}",
        "\u{6c47}\u{603b}\u{8d22}\u{52a1}\u{4e0e}\u{4f30}\u{503c}\u{4fe1}\u{606f}",
        35,
    ),
    (
        "\u{65b0}\u{95fb}\u{5206}\u{6790}",
        "\u{63d0}\u{53d6}\u{8fd1}\u{671f}\u{65b0}\u{95fb}\u{548c}\u{4e8b}\u{4ef6}\u{98ce}\u{9669}",
        55,
    ),
    (
        "\u{7814}\u{7a76}\u{51b3}\u{7b56}",
        "\u{751f}\u{6210}\u{7814}\u{7a76}\u{8ba1}\u{5212}\u{548c}\u{4ea4}\u{6613}\u{5efa}\u{8bae}",
        75,
    ),
    (
        "\u{7ec4}\u{5408}\u{51b3}\u{7b56}",
        "\u{5f62}\u{6210}\u{6700}\u{7ec8}\u{6295}\u{8d44}\u{7ed3}\u{8bba}\u{548c}\u{62a5}\u{544a}",
        90,
    ),
];

fn task_wait_is_terminal(status: &crate::TaskStatus) -> bool {
    status.is_terminal()
}

/// Parameters for running an analysis task.
#[derive(Clone)]
pub struct TaskRunParams {
    pub market_type: String,
    pub analysis_date: String,
    pub scenario: crate::AnalysisScenarioContext,
    pub selected_analysts: Vec<String>,
    pub past_context: String,
    pub memory_context: crate::MemoryContextSnapshot,
    pub llm_base_url: Option<String>,
    pub llm_api_key: Option<String>,
    pub quick_analysis_model: Option<String>,
    pub deep_analysis_model: Option<String>,
    pub language: String,
    pub user_context: crate::AnalysisUserContext,
    pub user_context_prompt: String,
    pub sector_context: String,
}

impl TaskRunParams {
    /// Create minimal params for a reflection-only run (no LLM, no market data).
    pub fn for_reflection(analysis_date: String, language: &str) -> Self {
        Self {
            market_type: "unknown".to_string(),
            analysis_date,
            scenario: crate::AnalysisScenarioContext::from_market_type("unknown"),
            selected_analysts: Vec::new(),
            past_context: String::new(),
            memory_context: crate::MemoryContextSnapshot::default(),
            llm_base_url: None,
            llm_api_key: None,
            quick_analysis_model: None,
            deep_analysis_model: None,
            language: language.to_string(),
            user_context: crate::AnalysisUserContext::default(),
            user_context_prompt: String::new(),
            sector_context: String::new(),
        }
    }

    /// Create reflection params that inherit LLM configuration from another run.
    pub fn for_reflection_with_llm(
        analysis_date: String,
        language: &str,
        llm_params: &TaskRunParams,
    ) -> Self {
        Self {
            llm_base_url: llm_params.llm_base_url.clone(),
            llm_api_key: llm_params.llm_api_key.clone(),
            quick_analysis_model: llm_params.quick_analysis_model.clone(),
            deep_analysis_model: llm_params.deep_analysis_model.clone(),
            user_context: llm_params.user_context.clone(),
            user_context_prompt: llm_params.user_context_prompt.clone(),
            ..Self::for_reflection(analysis_date, language)
        }
    }
}

/// Construct a `MemoryContextSnapshot` from a `MemoryContextBundleWithTags`.
pub fn memory_snapshot_from_bundle(
    bundle: &crate::memory::MemoryContextBundleWithTags,
) -> crate::MemoryContextSnapshot {
    crate::MemoryContextSnapshot {
        source: bundle.source.clone(),
        retrieval_mode: bundle.retrieval_mode.clone(),
        embedding_provider: bundle.embedding_provider.clone(),
        embedding_failure_reason: bundle.embedding_failure_reason.clone(),
        same_ticker_count: bundle.same_ticker_count,
        cross_ticker_count: bundle.cross_ticker_count,
        vector_hit_count: bundle.vector_hit_count,
        effective_top_k: bundle.effective_top_k,
        market_sample_count: 0,
        used_market_profile: false,
        setup_tags: bundle.setup_tags.clone(),
        resolved_setup_tags: Vec::new(),
        used_setup_filtered_retrieval: bundle.used_setup_filtered_retrieval,
        used_setup_fallback_calibration: bundle.used_setup_fallback_calibration,
        setup_calibration_sample_count: bundle.setup_calibration_sample_count,
        setup_match_count: bundle.setup_match_count,
        setup_pending_match_count: bundle.setup_pending_match_count,
        setup_resolved_match_count: bundle.setup_resolved_match_count,
        setup_match_hit_rate: bundle.setup_match_hit_rate,
        setup_match_avg_alpha_return: bundle.setup_match_avg_alpha_return,
        setup_long_match_count: bundle.setup_long_match_count,
        setup_short_match_count: bundle.setup_short_match_count,
        setup_neutral_match_count: bundle.setup_neutral_match_count,
        historical_same_ticker_highlights: bundle.same_ticker_highlights.clone(),
        historical_cross_ticker_highlights: bundle.cross_ticker_highlights.clone(),
        context_excerpt: bundle.context_text.chars().take(1200).collect(),
    }
}

/// Central orchestrator for analysis tasks.
///
/// Manages task lifecycle, LLM resolution, memory, checkpoints,
/// and real-time event broadcasting.
#[derive(Clone)]
pub struct TaskManager {
    pub analysis_store: Arc<dyn crate::AnalysisStore>,
    pub cache_store: Arc<dyn crate::CacheStore>,
    pub llm_client: Option<crate::llm::LlmClient>,
    pub llm_template: Option<crate::llm::LlmClient>,
    pub market_data: crate::data::MarketDataClient,
    pub toolbox: crate::types::TradingToolbox,
    pub data_dir: String,
    pub memory_log: TradingMemoryLog,
    pub checkpoint_store: TaskCheckpointStore,
    pub max_debate_rounds: usize,
    pub max_risk_discuss_rounds: usize,
    pub telemetry: SharedTelemetry,
    pub cross_collection: CrossCollectionSearcher,
    pub broadcasters: Arc<RwLock<HashMap<String, broadcast::Sender<crate::TaskEvent>>>>,
    pub running_tasks: Arc<RwLock<HashMap<String, AbortHandle>>>,
}

#[cfg(test)]
mod tests {
    use super::{TASK_STEPS, TaskManager, task_wait_is_terminal};
    use crate::TaskStatus;

    #[test]
    fn wait_loops_stop_for_blocked_task_statuses() {
        assert!(task_wait_is_terminal(&TaskStatus::BlockedData));
        assert!(task_wait_is_terminal(&TaskStatus::BlockedLlm));
    }

    #[test]
    fn blocked_data_uses_a_pipeline_step() {
        assert_eq!(TaskManager::blocked_data_step_name(), TASK_STEPS[0].0);
    }
}

impl TaskManager {
    pub(crate) fn blocked_data_step_name() -> &'static str {
        TASK_STEPS[0].0
    }

    /// Create a new `TaskManager` with the provided configuration.
    pub async fn new(
        analysis_store: Arc<dyn crate::AnalysisStore>,
        cache_store: Arc<dyn crate::CacheStore>,
        llm_client: Option<crate::llm::LlmClient>,
        llm_template: Option<crate::llm::LlmClient>,
        market_data: crate::data::MarketDataClient,
        data_dir: String,
        memory_log: TradingMemoryLog,
        checkpoint_store: TaskCheckpointStore,
        max_debate_rounds: usize,
        max_risk_discuss_rounds: usize,
        telemetry: SharedTelemetry,
    ) -> anyhow::Result<Self> {
        let cross_collection = CrossCollectionSearcher::new(memory_log.clone());

        Ok(Self {
            analysis_store,
            cache_store,
            llm_client,
            llm_template,
            toolbox: crate::types::TradingToolbox::new(market_data.clone()),
            market_data,
            data_dir,
            memory_log,
            checkpoint_store,
            max_debate_rounds,
            max_risk_discuss_rounds,
            telemetry,
            cross_collection,
            broadcasters: Arc::new(RwLock::new(HashMap::new())),
            running_tasks: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// Borrow the analysis store.
    pub fn analysis_store(&self) -> &dyn crate::AnalysisStore {
        self.analysis_store.as_ref()
    }

    /// Borrow the cache store.
    pub fn cache_store(&self) -> &dyn crate::CacheStore {
        self.cache_store.as_ref()
    }

    /// Borrow the market data client.
    pub fn market_data(&self) -> &crate::data::MarketDataClient {
        &self.market_data
    }

    /// Resolve an LLM client for the given task parameters.
    pub async fn resolve_llm_client(
        &self,
        params: &TaskRunParams,
    ) -> anyhow::Result<crate::llm::LlmClient> {
        let has_request_override = params
            .llm_base_url
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty())
            || params
                .llm_api_key
                .as_deref()
                .is_some_and(|value| !value.trim().is_empty());

        let (source, client) = if has_request_override {
            let client = self
                .llm_client
                .clone()
                .or_else(|| self.llm_template.clone())
                .ok_or_else(|| anyhow::anyhow!("llm is disabled or not configured"))?;
            ("request_override", client)
        } else if let Some(client) = self.llm_client.clone() {
            ("settings", client)
        } else {
            (
                "template_fallback",
                self.llm_template
                    .clone()
                    .ok_or_else(|| anyhow::anyhow!("llm is disabled or not configured"))?,
            )
        };
        let client = client
            .with_base_url(params.llm_base_url.as_deref())
            .with_api_key(params.llm_api_key.as_deref());
        tracing::info!(
            source,
            base_url = %client.openai_base_url,
            model = %client.model,
            has_api_key = !client.openai_api_key.trim().is_empty(),
            "resolved llm client"
        );
        if client.openai_api_key.trim().is_empty() {
            anyhow::bail!("llm api key is not configured");
        }
        Ok(client)
    }

    /// Get or create a broadcaster for a given task_id.
    pub async fn broadcaster(&self, task_id: &str) -> broadcast::Sender<crate::TaskEvent> {
        let mut guard = self.broadcasters.write().await;
        guard
            .entry(task_id.to_string())
            .or_insert_with(|| broadcast::channel(64).0)
            .clone()
    }

    /// Subscribe to task events.
    pub async fn subscribe(&self, task_id: &str) -> broadcast::Receiver<crate::TaskEvent> {
        self.broadcaster(task_id).await.subscribe()
    }

    /// Publish task event for cross-instance delivery.
    ///
    /// TODO: Implement cross-instance event publishing via trait.
    pub async fn publish_task_event(&self, _task_id: &str, _event: &crate::TaskEvent) {
        // TODO: Implement cross-instance event publishing
    }

    /// Cache a completed analysis result via CacheStore trait.
    pub async fn cache_completed_analysis(
        &self,
        task: &crate::PersistedTask,
        result: &crate::AnalysisResult,
    ) -> anyhow::Result<()> {
        let ttl = seconds_until_local_midnight();
        if ttl <= 0 {
            return Ok(());
        }
        let now = Utc::now();
        let expires_at = now + Duration::seconds(ttl);
        let payload = serde_json::json!({
            "source_task_id": task.task_id,
            "owner_username": task.owner_username,
            "symbol": task.symbol,
            "stock_name": task.stock_name,
            "market_type": task.market_type,
            "analysis_date": task.analysis_date,
            "cached_at": now.to_rfc3339(),
            "expires_at": expires_at.to_rfc3339(),
            "summary": result.report.summary,
            "recommendation": result.report.recommendation,
        });
        let key =
            Self::analysis_reuse_cache_key(&task.owner_username, &task.symbol, &task.market_type);
        self.cache_store
            .set(
                &key,
                serde_json::to_string(&payload)?.as_bytes(),
                Some(ttl as u64),
            )
            .await?;
        Ok(())
    }

    /// Build the cache key for analysis reuse checks.
    pub fn analysis_reuse_cache_key(
        owner_username: &str,
        symbol: &str,
        market_type: &str,
    ) -> String {
        format!(
            "analysis:reuse:{}:{}:{}",
            owner_username.trim().to_ascii_lowercase(),
            symbol.trim().to_ascii_uppercase(),
            market_type.trim()
        )
    }

    /// Wait for a task to complete or fail.
    ///
    /// Returns the final `PersistedTask`. If timeout elapses, returns the current state.
    pub async fn wait_for_task(
        &self,
        task_id: &str,
        timeout: std::time::Duration,
    ) -> anyhow::Result<crate::PersistedTask> {
        let deadline = std::time::Instant::now() + timeout;
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(3));
        interval.tick().await;

        loop {
            let task = self
                .analysis_store()
                .get_task(task_id)
                .await?
                .ok_or_else(|| anyhow::anyhow!("task {task_id} not found"))?;

            if task_wait_is_terminal(&task.status) {
                return Ok(task);
            }

            if std::time::Instant::now() > deadline {
                return Ok(task);
            }
            interval.tick().await;
        }
    }

    /// Wait for a task with a progress callback.
    pub async fn wait_for_task_with_progress<F>(
        &self,
        task_id: &str,
        timeout: std::time::Duration,
        mut on_progress: F,
    ) -> anyhow::Result<crate::PersistedTask>
    where
        F: FnMut(&crate::PersistedTask),
    {
        let deadline = std::time::Instant::now() + timeout;
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(3));
        interval.tick().await;

        loop {
            let task = self
                .analysis_store()
                .get_task(task_id)
                .await?
                .ok_or_else(|| anyhow::anyhow!("task {task_id} not found"))?;

            if task_wait_is_terminal(&task.status) {
                return Ok(task);
            }
            on_progress(&task);

            if std::time::Instant::now() > deadline {
                return Ok(task);
            }
            interval.tick().await;
        }
    }

    /// Fetch sector context for analysis from guidance store.
    pub async fn fetch_sector_context_for_analysis(&self, market_type: &str) -> String {
        let store = crate::guide::GuidanceStore::from_env();
        let query_text = format!("market {} sector highlights sentiment", market_type);
        let embedding = self.memory_log.embed_text(&query_text);
        if embedding.is_empty() {
            return String::new();
        }
        let mut context_parts = Vec::new();
        match store
            .search_sector_context(&embedding, market_type, 3)
            .await
        {
            Ok(results) if !results.is_empty() => {
                let mut lines = vec!["=== Sector Highlights ===".to_string()];
                for r in &results {
                    let payload = r.get("payload").unwrap_or(r);
                    let sector = payload
                        .get("sector_name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    let direction = payload
                        .get("direction")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    let driver = payload
                        .get("key_driver")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    let date = payload.get("date").and_then(|v| v.as_str()).unwrap_or("");
                    lines.push(format!(
                        "- [{}] {} {} -- {}",
                        date, sector, direction, driver
                    ));
                }
                context_parts.push(lines.join("\n"));
            }
            _ => {}
        }
        match store
            .search_sentiment_context(&embedding, market_type, 2)
            .await
        {
            Ok(results) if !results.is_empty() => {
                let mut lines = vec!["=== Market Sentiment ===".to_string()];
                for r in &results {
                    let payload = r.get("payload").unwrap_or(r);
                    let label = payload
                        .get("sentiment_label")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    let score = payload
                        .get("sentiment_score")
                        .and_then(|v| v.as_f64())
                        .unwrap_or(0.0);
                    let date = payload.get("date").and_then(|v| v.as_str()).unwrap_or("");
                    let drivers = payload
                        .get("drivers")
                        .and_then(|v| v.as_array())
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|v| v.as_str())
                                .collect::<Vec<_>>()
                                .join(", ")
                        })
                        .unwrap_or_default();
                    lines.push(format!(
                        "- [{}] {} (score {:.1}) -- {}",
                        date, label, score, drivers
                    ));
                }
                context_parts.push(lines.join("\n"));
            }
            _ => {}
        }
        context_parts.join("\n\n")
    }

    /// Mark a task as failed with an error message.
    pub async fn mark_task_failed(&self, task_id: &str, error: String) -> anyhow::Result<()> {
        if let Some(mut task) = self.analysis_store.get_task(task_id).await? {
            task.status = crate::TaskStatus::Failed;
            task.error_message = Some(error);
            task.updated_at = Utc::now();
            self.analysis_store.update_task(&task).await?;
        }
        Ok(())
    }

    /// Cancel a running task.
    pub async fn cancel_task(&self, task_id: &str) -> anyhow::Result<bool> {
        let mut guard = self.running_tasks.write().await;
        if let Some(handle) = guard.remove(task_id) {
            handle.abort();
            if let Some(mut task) = self.analysis_store.get_task(task_id).await? {
                task.status = crate::TaskStatus::Cancelled;
                task.updated_at = Utc::now();
                self.analysis_store.update_task(&task).await?;
            }
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Check if a task is still running (has an active tokio task).
    pub async fn is_task_running(&self, task_id: &str) -> bool {
        self.running_tasks.read().await.contains_key(task_id)
    }

    /// Get a persisted task by ID.
    pub async fn get_task(&self, task_id: &str) -> anyhow::Result<Option<PersistedTask>> {
        self.analysis_store.get_task(task_id).await
    }

    /// Create a task for a user and run it (non-blocking, returns task_id).
    pub async fn create_task_for_user(
        &self,
        owner_username: &str,
        request: crate::SingleAnalysisRequest,
    ) -> anyhow::Result<String> {
        self.create_task_and_run_blocking(owner_username, request, None)
            .await
    }

    /// Resume a pending/failed/cancelled task.
    pub async fn resume_task(&self, task_id: &str) -> anyhow::Result<bool> {
        if let Some(mut task) = self.analysis_store.get_task(task_id).await? {
            if task.status == crate::TaskStatus::Failed
                || task.status == crate::TaskStatus::Cancelled
                || task.status == crate::TaskStatus::Pending
            {
                task.status = crate::TaskStatus::Running;
                task.updated_at = Utc::now();
                self.analysis_store.update_task(&task).await?;
                let params = self
                    .task_run_params_from_request(&task, &task.request)
                    .await;
                let this = self.clone();
                let tid = task_id.to_string();
                tokio::spawn(async move {
                    if let Err(e) = this.execute_existing_task(tid.clone(), params).await {
                        tracing::error!("resume task {} failed: {:?}", tid, e);
                    }
                });
                return Ok(true);
            }
            Ok(false)
        } else {
            Ok(false)
        }
    }

    /// Clear checkpoints for a task.
    pub async fn clear_task_checkpoint(&self, task_id: &str) -> anyhow::Result<bool> {
        self.checkpoint_store.clear(task_id, "", "").await?;
        Ok(true)
    }

    /// Get task result as report JSON.
    pub async fn task_result_report_json(
        &self,
        task_id: &str,
    ) -> anyhow::Result<Option<serde_json::Value>> {
        if let Some(result) = self.analysis_store.load_result(task_id).await? {
            Ok(Some(serde_json::to_value(&result.report)?))
        } else {
            Ok(None)
        }
    }

    /// Get task result as chart JSON.
    pub async fn task_result_chart_json(
        &self,
        task_id: &str,
    ) -> anyhow::Result<Option<serde_json::Value>> {
        if let Some(result) = self.analysis_store.load_result(task_id).await? {
            Ok(Some(serde_json::to_value(&result.report.market_chart)?))
        } else {
            Ok(None)
        }
    }

    /// Get task result as artifacts JSON.
    pub async fn task_result_artifacts_json(
        &self,
        task_id: &str,
    ) -> anyhow::Result<Option<serde_json::Value>> {
        if let Some(result) = self.analysis_store.load_result(task_id).await? {
            Ok(Some(serde_json::json!({
                "artifacts": result.artifacts,
            })))
        } else {
            Ok(None)
        }
    }

    /// Get task result as IC report JSON.
    pub async fn task_result_ic_report_json(
        &self,
        task_id: &str,
    ) -> anyhow::Result<Option<serde_json::Value>> {
        if let Some(result) = self.analysis_store.load_result(task_id).await? {
            Ok(Some(serde_json::json!({
                "ic_report": result.report.ic_navigator,
            })))
        } else {
            Ok(None)
        }
    }
}

/// Seconds remaining until the next local midnight (minimum 1).
pub fn seconds_until_local_midnight() -> i64 {
    let now = Local::now();
    let next_day = now.date_naive() + Duration::days(1);
    let next_midnight =
        match Local.with_ymd_and_hms(next_day.year(), next_day.month(), next_day.day(), 0, 0, 0) {
            LocalResult::Single(value) => value,
            LocalResult::Ambiguous(first, _) => first,
            LocalResult::None => now + Duration::hours(8),
        };
    (next_midnight - now).num_seconds().max(1)
}

use std::{collections::HashMap, sync::Arc};

use chrono::{Datelike, Duration, Local, LocalResult, TimeZone, Utc};
use tokio::sync::{RwLock, broadcast};
use tokio::task::AbortHandle;

use crate::engine::memory::TradingMemoryLog;
use crate::engine::checkpoint::TaskCheckpointStore;

/// Ordered analysis pipeline steps with progress percentages.
pub const TASK_STEPS: [(&str, &str, i32); 5] = [
    ("\u{5e02}\u{573a}\u{5206}\u{6790}", "\u{6293}\u{53d6}\u{884c}\u{60c5}\u{4e0e}\u{6280}\u{672f}\u{6307}\u{6807}", 15),
    ("\u{57fa}\u{672c}\u{9762}\u{5206}\u{6790}", "\u{6c47}\u{603b}\u{8d22}\u{52a1}\u{4e0e}\u{4f30}\u{503c}\u{4fe1}\u{606f}", 35),
    ("\u{65b0}\u{95fb}\u{5206}\u{6790}", "\u{63d0}\u{53d6}\u{8fd1}\u{671f}\u{65b0}\u{95fb}\u{548c}\u{4e8b}\u{4ef6}\u{98ce}\u{9669}", 55),
    ("\u{7814}\u{7a76}\u{51b3}\u{7b56}", "\u{751f}\u{6210}\u{7814}\u{7a76}\u{8ba1}\u{5212}\u{548c}\u{4ea4}\u{6613}\u{5efa}\u{8bae}", 75),
    ("\u{7ec4}\u{5408}\u{51b3}\u{7b56}", "\u{5f62}\u{6210}\u{6700}\u{7ec8}\u{6295}\u{8d44}\u{7ed3}\u{8bba}\u{548c}\u{62a5}\u{544a}", 90),
];

/// Parameters for running an analysis task.
#[derive(Clone)]
pub struct TaskRunParams {
    pub market_type: String,
    pub analysis_date: String,
    pub scenario: crate::models::AnalysisScenarioContext,
    pub selected_analysts: Vec<String>,
    pub past_context: String,
    pub memory_context: crate::models::MemoryContextSnapshot,
    pub llm_base_url: Option<String>,
    pub llm_api_key: Option<String>,
    pub quick_analysis_model: Option<String>,
    pub deep_analysis_model: Option<String>,
    pub language: String,
    pub user_context: crate::models::AnalysisUserContext,
    pub user_context_prompt: String,
    pub sector_context: String,
}

impl TaskRunParams {
    pub fn for_reflection(analysis_date: String, language: &str) -> Self {
        Self {
            market_type: "unknown".to_string(),
            analysis_date,
            scenario: crate::models::AnalysisScenarioContext::from_market_type("unknown"),
            selected_analysts: Vec::new(),
            past_context: String::new(),
            memory_context: crate::models::MemoryContextSnapshot::default(),
            llm_base_url: None,
            llm_api_key: None,
            quick_analysis_model: None,
            deep_analysis_model: None,
            language: language.to_string(),
            user_context: crate::models::AnalysisUserContext::default(),
            user_context_prompt: String::new(),
            sector_context: String::new(),
        }
    }

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
    bundle: &crate::engine::memory::MemoryContextBundleWithTags,
) -> crate::models::MemoryContextSnapshot {
    crate::models::MemoryContextSnapshot {
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


/// Event bus for cross-instance task event delivery.
///
/// Implementations can use filesystem, message queues, or any pub/sub mechanism.
#[async_trait::async_trait]
pub trait EventBus: Send + Sync {
    /// Publish a task event.
    async fn publish(&self, task_id: &str, event: &crate::models::TaskEvent) -> anyhow::Result<()>;
}

/// Filesystem-backed event bus.
///
/// Writes events as JSON lines to `{base_dir}/events/{task_id}.jsonl`.
pub struct FilesystemEventBus {
    base_dir: std::path::PathBuf,
}

impl FilesystemEventBus {
    pub fn new(base_dir: impl Into<std::path::PathBuf>) -> Self {
        Self { base_dir: base_dir.into() }
    }
}

#[async_trait::async_trait]
impl EventBus for FilesystemEventBus {
    async fn publish(&self, task_id: &str, event: &crate::models::TaskEvent) -> anyhow::Result<()> {
        let dir = self.base_dir.join("events");
        tokio::fs::create_dir_all(&dir).await?;
        let path = dir.join(format!("{task_id}.jsonl"));
        let mut line = serde_json::to_string(event)?;
        line.push('\n');
        use tokio::io::AsyncWriteExt;
        let mut file = tokio::fs::OpenOptions::new()
            .create(true).append(true).open(&path).await?;
        file.write_all(line.as_bytes()).await?;
        Ok(())
    }
}

/// Central orchestrator for analysis tasks.
///
/// Manages task lifecycle, LLM resolution, memory, checkpoints,
/// and real-time event broadcasting.
#[derive(Clone)]
pub struct TaskManager {
    pub analysis_store: Arc<dyn crate::models::AnalysisStore>,
    pub cache_store: Arc<dyn crate::models::CacheStore>,
    pub llm_client: Option<crate::engine::llm::LlmClient>,
    pub llm_template: Option<crate::engine::llm::LlmClient>,
    pub market_data: crate::data::MarketDataClient,
    pub toolbox: crate::engine::tools::TradingToolbox,
    pub storage: Arc<dyn crate::engine::storage::StorageBackend>,
    pub memory_log: TradingMemoryLog,
    pub checkpoint_store: TaskCheckpointStore,
    pub max_debate_rounds: usize,
    pub max_risk_discuss_rounds: usize,
    pub broadcasters:
        Arc<RwLock<HashMap<String, broadcast::Sender<crate::models::TaskEvent>>>>,
    pub running_tasks: Arc<RwLock<HashMap<String, AbortHandle>>>,
    pub event_bus: Option<Arc<dyn EventBus>>,
}

impl TaskManager {
    /// Create a new `TaskManager` with the provided configuration.
    #[allow(clippy::too_many_arguments)]
    pub async fn new(
        analysis_store: Arc<dyn crate::models::AnalysisStore>,
        cache_store: Arc<dyn crate::models::CacheStore>,
        llm_client: Option<crate::engine::llm::LlmClient>,
        llm_template: Option<crate::engine::llm::LlmClient>,
        market_data: crate::data::MarketDataClient,
        storage: Arc<dyn crate::engine::storage::StorageBackend>,
        memory_log: TradingMemoryLog,
        checkpoint_store: TaskCheckpointStore,
        max_debate_rounds: usize,
        max_risk_discuss_rounds: usize,
        event_bus: Option<Arc<dyn EventBus>>,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            analysis_store,
            cache_store,
            llm_client,
            llm_template,
            toolbox: crate::engine::tools::TradingToolbox::new(market_data.clone()),
            market_data,
            storage,
            memory_log,
            checkpoint_store,
            max_debate_rounds,
            max_risk_discuss_rounds,
            broadcasters: Arc::new(RwLock::new(HashMap::new())),
            running_tasks: Arc::new(RwLock::new(HashMap::new())),
            event_bus,
        })
    }

    /// Borrow the analysis store.
    pub fn analysis_store(&self) -> &dyn crate::models::AnalysisStore {
        self.analysis_store.as_ref()
    }

    /// Borrow the cache store.
    pub fn cache_store(&self) -> &dyn crate::models::CacheStore {
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
    ) -> anyhow::Result<crate::engine::llm::LlmClient> {
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
    pub async fn broadcaster(&self, task_id: &str) -> broadcast::Sender<crate::models::TaskEvent> {
        let mut guard = self.broadcasters.write().await;
        guard
            .entry(task_id.to_string())
            .or_insert_with(|| broadcast::channel(64).0)
            .clone()
    }

    /// Subscribe to task events.
    pub async fn subscribe(&self, task_id: &str) -> broadcast::Receiver<crate::models::TaskEvent> {
        self.broadcaster(task_id).await.subscribe()
    }

    /// Publish task event for cross-instance delivery.
    pub async fn publish_task_event(
        &self,
        task_id: &str,
        event: &crate::models::TaskEvent,
    ) {
        if let Some(bus) = &self.event_bus
            && let Err(e) = bus.publish(task_id, event).await
        {
            tracing::warn!(task_id = %task_id, error = %e, "failed to publish task event");
        }
    }

    /// Cache a completed analysis result.
    ///
    /// Uses the CacheStore trait instead of direct Redis access.
    pub async fn cache_completed_analysis(
        &self,
        task: &crate::models::PersistedTask,
        result: &crate::models::AnalysisResult,
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
            .set(&key, serde_json::to_string(&payload)?.as_bytes(), Some(ttl as u64))
            .await?;
        Ok(())
    }

    fn analysis_reuse_cache_key(owner_username: &str, symbol: &str, market_type: &str) -> String {
        format!(
            "analysis:reuse:{}:{}:{}",
            owner_username.trim().to_ascii_lowercase(),
            symbol.trim().to_ascii_uppercase(),
            market_type.trim()
        )
    }

    /// Fetch sector context for analysis from guidance store.
    pub async fn fetch_sector_context_for_analysis(&self, _market_type: &str) -> String {
        // Vector-based sector/sentiment search removed with RAG system.
        String::new()
    }
}

fn seconds_until_local_midnight() -> i64 {
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

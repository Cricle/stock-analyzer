use anyhow::Context;
use chrono::Utc;
use uuid::Uuid;

use crate::TaskManager;
use crate::{PersistedTask, SingleAnalysisRequest, TaskStatus};

impl TaskManager {
    pub async fn create_task_and_run_blocking(
        &self,
        owner_username: &str,
        req: SingleAnalysisRequest,
        requested_task_id: Option<String>,
    ) -> anyhow::Result<String> {
        let original_request = req.clone();
        let task_id = self
            .create_task_with_id(owner_username, req, requested_task_id, false)
            .await?;
        let task = self
            .analysis_store
            .get_task(&task_id)
            .await?
            .context("task not found after creation")?;
        let params = self
            .task_run_params_from_request(&task, &original_request)
            .await;
        self.execute_existing_task(task_id.clone(), params).await?;
        Ok(task_id)
    }

    pub async fn create_task_with_id(
        &self,
        owner_username: &str,
        req: SingleAnalysisRequest,
        requested_task_id: Option<String>,
        execute: bool,
    ) -> anyhow::Result<String> {
        let (task_id, _) = self
            .create_task_with_idempotency(owner_username, req, requested_task_id, execute)
            .await?;
        Ok(task_id)
    }

    /// Create a task with an optional caller-provided idempotency key.
    ///
    /// The task primary key is the concurrency boundary. A duplicate key is only reused when
    /// it belongs to the same owner; no checkpoints or lifecycle state are changed in that case.
    pub async fn create_task_with_idempotency(
        &self,
        owner_username: &str,
        req: SingleAnalysisRequest,
        requested_task_id: Option<String>,
        execute: bool,
    ) -> anyhow::Result<(String, bool)> {
        let has_requested_task_id = requested_task_id.is_some();
        // Daily cache check — return existing completed task for same symbol+date
        if !req.force_refresh && requested_task_id.is_none() {
            let symbol = req
                .symbol
                .as_deref()
                .or(req.stock_code.as_deref())
                .unwrap_or("");
            let analysis_date = req
                .parameters
                .as_ref()
                .and_then(|p| p.analysis_date.as_deref())
                .unwrap_or("");
            if !symbol.is_empty()
                && !analysis_date.is_empty()
                && let Some(cached_id) = self
                    .analysis_store
                    .find_cached_task_for_owner(owner_username, symbol, analysis_date)
                    .await?
            {
                tracing::info!(symbol, analysis_date, cached_id, "returning cached report");
                return Ok((cached_id, false));
            }
        }

        let original_request = req.clone();
        let symbol = req
            .symbol
            .or(req.stock_code)
            .filter(|s| !s.trim().is_empty())
            .context("symbol is required")?;
        let params = req.parameters.unwrap_or_default();
        let now = Utc::now();
        let task_id = requested_task_id.unwrap_or_else(|| Uuid::new_v4().to_string());
        let stock_name = original_request
            .stock_name
            .clone()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| symbol.clone());
        let inferred_market_type = match self.market_data.detect_market(&symbol) {
            crate::data::MarketKind::AShare => "A-share",
            crate::data::MarketKind::HongKong => "HK",
            crate::data::MarketKind::UsEquity => "US",
        }
        .to_string();
        let task = PersistedTask {
            task_id: task_id.clone(),
            owner_username: owner_username.trim().to_string(),
            symbol: symbol.clone(),
            stock_name,
            market_type: params.market_type.unwrap_or(inferred_market_type),
            analysis_date: params
                .analysis_date
                .unwrap_or_else(|| now.format("%Y-%m-%d").to_string()),
            research_depth: "deep".to_string(),
            request: original_request,
            status: TaskStatus::Pending,
            progress: 0,
            current_step_name: "Task created".to_string(),
            current_step_description: "Waiting for analysis engine".to_string(),
            message: "Task submitted".to_string(),
            error_message: None,
            llm_token_usage: crate::LlmTokenUsageSummary::default(),
            quality_gate_json: None,
            charge_state: "uncharged".to_string(),
            charge_ledger_id: None,
            refund_ledger_id: None,
            retry_of_task_id: None,
            logical_request_id: None,
            created_at: now,
            updated_at: now,
        };
        let inserted = match self.analysis_store.insert_task(&task).await {
            Ok(()) => true,
            Err(error) if has_requested_task_id => {
                let existing = self.analysis_store.get_task(&task_id).await?;
                if existing.is_some_and(|existing| existing.owner_username == task.owner_username) {
                    return Ok((task_id, false));
                }
                return Err(error);
            }
            Err(error) => return Err(error),
        };
        if inserted {
            let _ = self
                .checkpoint_store
                .clear(&task.task_id, &task.symbol, &task.analysis_date)
                .await;
            let _ = self
                .checkpoint_store
                .clear_graph_runtime(&task.task_id, &task.symbol, &task.analysis_date)
                .await;
        }
        if !execute {
            return Ok((task_id, true));
        }
        let tx = self.broadcaster(&task_id).await;
        let spawned_task_id = task_id.clone();
        let task_params = self
            .task_run_params_from_request(&task, &task.request)
            .await;
        let this = self.clone();
        let handle = tokio::spawn(async move {
            if let Err(error) = this.run_task(spawned_task_id.clone(), task_params).await {
                tracing::error!("task {} failed: {:?}", spawned_task_id, error);
                let _ = this
                    .publish_failure(&spawned_task_id, format!("Analysis task failed: {error:#}"))
                    .await;
            }
            drop(tx);
        });
        self.running_tasks
            .write()
            .await
            .insert(task_id.clone(), handle.abort_handle());
        Ok((task_id, true))
    }
}

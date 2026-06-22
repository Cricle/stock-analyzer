use anyhow::Context;
use chrono::Utc;
use uuid::Uuid;

use crate::TaskManager;
use sa_models::{PersistedTask, SingleAnalysisRequest, TaskStatus};

impl TaskManager {
    pub async fn create_task(&self, req: SingleAnalysisRequest) -> anyhow::Result<String> {
        self.create_task_for_user("", req).await
    }

    pub async fn create_task_record(&self, req: SingleAnalysisRequest) -> anyhow::Result<String> {
        self.create_task_record_for_user("", req).await
    }

    pub async fn create_task_for_user(
        &self,
        owner_username: &str,
        req: SingleAnalysisRequest,
    ) -> anyhow::Result<String> {
        self.create_task_with_id(owner_username, req, None, true)
            .await
    }

    pub async fn create_task_record_for_user(
        &self,
        owner_username: &str,
        req: SingleAnalysisRequest,
    ) -> anyhow::Result<String> {
        self.create_task_with_id(owner_username, req, None, false)
            .await
    }

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
        // Daily cache check — return existing completed task for same symbol+date
        if !req.force_refresh {
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
            if !symbol.is_empty() && !analysis_date.is_empty() {
                if let Some(cached_id) = self
                    .analysis_store
                    .find_cached_task(symbol, analysis_date)
                    .await?
                {
                    tracing::info!(symbol, analysis_date, cached_id, "returning cached report");
                    return Ok(cached_id);
                }
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
            sa_data::MarketKind::AShare => "A-share",
            sa_data::MarketKind::HongKong => "HK",
            sa_data::MarketKind::UsEquity => "US",
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
            llm_token_usage: sa_models::LlmTokenUsageSummary::default(),
            created_at: now,
            updated_at: now,
        };
        let _ = self
            .checkpoint_store
            .clear(&task.task_id, &task.symbol, &task.analysis_date)
            .await;
        let _ = self
            .checkpoint_store
            .clear_graph_runtime(&task.task_id, &task.symbol, &task.analysis_date)
            .await;
        self.analysis_store.insert_task(&task).await?;
        if !execute {
            return Ok(task_id);
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
        Ok(task_id)
    }
}

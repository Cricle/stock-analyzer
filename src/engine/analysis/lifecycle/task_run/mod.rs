mod execute;
mod formatters;
mod params;
mod run;

use crate::TaskManager;
use crate::models::TaskStatus;
use chrono::Utc;

// Re-export lifecycle helpers for child modules (params.rs)
#[allow(unused_imports)]
use super::{build_user_context, build_user_context_prompt};

impl TaskManager {
    pub async fn resume_task(&self, task_id: &str) -> anyhow::Result<bool> {
        let Some(task) = self.analysis_store.get_task(task_id).await? else {
            return Ok(false);
        };
        let checkpoint_step = self
            .checkpoint_store
            .checkpoint_step(&task.task_id, &task.symbol, &task.analysis_date)
            .await?;
        let can_resume_completed = checkpoint_step.is_some_and(|step| step < 100);
        if matches!(task.status, TaskStatus::Completed) && !can_resume_completed {
            return Ok(true);
        }
        if matches!(task.status, TaskStatus::Cancelled) {
            return Ok(false);
        }
        let params = self
            .task_run_params_from_request(&task, &task.request)
            .await;
        let this = self.clone();
        let task_id = task_id.to_string();
        let running_task_id = task_id.clone();
        let handle = tokio::spawn(async move {
            if let Err(error) = this.run_task(task_id.clone(), params).await {
                tracing::error!("resume task {} failed: {:?}", task_id, error);
                let _ = this
                    .publish_failure(
                        &task_id,
                        format!("Failed to resume analysis task: {error:#}"),
                    )
                    .await;
            }
        });
        self.running_tasks
            .write()
            .await
            .insert(running_task_id, handle.abort_handle());
        Ok(true)
    }

    pub async fn clear_task_checkpoint(&self, task_id: &str) -> anyhow::Result<bool> {
        let Some(task) = self.analysis_store.get_task(task_id).await? else {
            return Ok(false);
        };
        self.checkpoint_store
            .clear(&task.task_id, &task.symbol, &task.analysis_date)
            .await?;
        self.checkpoint_store
            .clear_graph_runtime(&task.task_id, &task.symbol, &task.analysis_date)
            .await?;
        Ok(true)
    }

    pub async fn cancel_task(&self, task_id: &str) -> anyhow::Result<bool> {
        let Some(mut task) = self.analysis_store.get_task(task_id).await? else {
            return Ok(false);
        };
        if matches!(
            task.status,
            TaskStatus::Completed | TaskStatus::Cancelled | TaskStatus::Failed
        ) {
            return Ok(false);
        }

        if let Some(handle) = self.running_tasks.write().await.remove(task_id) {
            handle.abort();
        }

        task.status = TaskStatus::Cancelled;
        task.current_step_name = "Task cancelled".to_string();
        task.current_step_description = "Cancelled by user".to_string();
        task.message = "Task cancelled".to_string();
        task.error_message = Some("cancelled_by_user".to_string());
        task.updated_at = Utc::now();
        self.update_task(
            task_id,
            TaskStatus::Cancelled,
            task.progress,
            &task.current_step_name,
            &task.current_step_description,
            &task.message,
            task.error_message.clone(),
        )
        .await?;

        let params = self
            .task_run_params_from_request(&task, &task.request)
            .await;
        let mut result = match self.analysis_store.get_result(task_id).await? {
            Some(result) => result,
            None => {
                crate::engine::analysis::runtime::TradingAgentsGraph::prepare_result(
                    self, &task, &params,
                )
                .await?
            }
        };
        result.agent_state.company_of_interest = result.symbol.clone();
        result.agent_state.trade_date = result.analysis_date.clone();
        result.agent_state.sender = "System".to_string();
        result.agent_state.past_context = params.past_context.clone();
        result.artifacts.user_context = params.user_context.clone();
        self.refresh_structured_report_snapshot(&mut result).await?;
        result.apply_calibrated_markdown();
        self.analysis_store.save_result(task_id, &result).await?;

        // Store analysis summary in Qdrant for cross-module retrieval
        if let Err(e) = store_analysis_in_qdrant(&result).await {
            tracing::warn!(symbol = %result.symbol, error = %e, "failed to store analysis in qdrant");
        }

        Ok(true)
    }
}

/// Store analysis result summary in the stock_pick Qdrant collection
/// so stock picks and guidance can retrieve it.
async fn store_analysis_in_qdrant(result: &crate::models::AnalysisResult) -> anyhow::Result<()> {
    let qdrant_url = std::env::var("QDRANT_URL")
        .or_else(|_| std::env::var("RAG_QDRANT_URL"))
        .unwrap_or_else(|_| "http://127.0.0.1:6333".to_string());
    let collection = "tradingagents_stock_pick";

    let report = &result.report;
    let rating = report.recommendation.clone();
    let confidence = report.confidence_score;
    let entry_price = report.decision_view.current_price.clone();
    let target_price = report.decision_view.target_reference.clone();
    let stop_loss = report.decision_view.invalidation_level.clone();
    let reader_summary = report.decision_view.reader_summary.clone();

    // Extract key news headlines
    let news_headlines: Vec<String> = report
        .news_insights
        .iter()
        .take(5)
        .map(|n| n.title.clone())
        .collect();

    // Extract key technical signals
    let technical_summary: Vec<String> = report
        .technical_indicators
        .categories
        .iter()
        .flat_map(|c| c.indicators.iter())
        .filter(|i| !i.signal_code.is_empty())
        .take(5)
        .map(|i| format!("{}: {:?}", i.key, i.value.unwrap_or(0.0)))
        .collect();

    let summary_text = format!(
        "{} {} market {} analysis {} conclusion {} rating {}",
        result.symbol,
        result.stock_name,
        result.market_type,
        result.analysis_date,
        reader_summary,
        rating
    );

    use sha2::{Digest, Sha256};
    use uuid::Uuid;

    let point_id = {
        let digest = Sha256::digest(format!(
            "analysis:{}:{}:{}",
            result.symbol, result.analysis_date, result.task_id
        ));
        let mut bytes = [0u8; 16];
        bytes.copy_from_slice(&digest[..16]);
        bytes[6] = (bytes[6] & 0x0f) | 0x50;
        bytes[8] = (bytes[8] & 0x3f) | 0x80;
        Uuid::from_bytes(bytes).to_string()
    };

    let embedding = crate::engine::guidance::semantic_embed(&summary_text);

    let client = crate::engine::shared::shared_http_client();
    let resp = client
        .put(format!(
            "{}/collections/{}/points?wait=true",
            qdrant_url.trim().trim_end_matches('/'),
            collection
        ))
        .json(&serde_json::json!({
            "points": [{
                "id": point_id,
                "vector": embedding,
                "payload": {
                    "entry_kind": "analysis_result",
                    "symbol": result.symbol,
                    "stock_name": result.stock_name,
                    "market": result.market_type,
                    "analysis_date": result.analysis_date,
                    "task_id": result.task_id,
                    "rating": rating.clone(),
                    "confidence": confidence as i64,
                    "entry_price": entry_price,
                    "target_price": target_price,
                    "stop_loss": stop_loss,
                    "reader_summary": reader_summary,
                    "news_headlines": news_headlines,
                    "technical_summary": technical_summary,
                    "created_at": result.created_at
                }
            }]
        }))
        .send()
        .await?;

    if !resp.status().is_success() {
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!("qdrant analysis store failed: {body}");
    }

    tracing::info!(
        symbol = %result.symbol,
        analysis_date = %result.analysis_date,
        "analysis result stored in qdrant"
    );
    Ok(())
}

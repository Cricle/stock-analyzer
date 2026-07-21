use anyhow::Context;
use chrono::Utc;
use std::time::Instant;

use super::fetch::analysis_news_start;
use super::format::build_technical_summary;
use super::{build_user_context, build_user_context_prompt};
use crate::{AnalysisResult, PersistedTask, SingleAnalysisRequest, TaskEvent, TaskStatus};
use crate::{TaskManager, TaskRunParams};

// ---------------------------------------------------------------------------
// TaskManager impl — task lifecycle methods
// ---------------------------------------------------------------------------

impl TaskManager {
    pub async fn task_run_params_from_request(
        &self,
        task: &PersistedTask,
        request: &SingleAnalysisRequest,
    ) -> TaskRunParams {
        let params = request.parameters.clone().unwrap_or_default();
        let user_context = build_user_context(&params);
        let memory_context = self
            .initial_memory_context(&task.symbol, &task.market_type, None, None, &[])
            .await;
        let scenario = crate::AnalysisScenarioContext::from_market_type(&task.market_type);
        let sector_context = self
            .fetch_sector_context_for_analysis(&task.market_type)
            .await;
        TaskRunParams {
            market_type: task.market_type.clone(),
            analysis_date: task.analysis_date.clone(),
            scenario,
            selected_analysts: params.selected_analysts.unwrap_or_default(),
            past_context: memory_context.context_text.clone(),
            memory_context: crate::task_manager::memory_snapshot_from_bundle(&memory_context),
            llm_base_url: params.llm_base_url,
            llm_api_key: params.llm_api_key,
            quick_analysis_model: params.quick_analysis_model,
            deep_analysis_model: params.deep_analysis_model,
            language: user_context.language.clone(),
            user_context_prompt: build_user_context_prompt(&user_context),
            user_context,
            sector_context,
        }
    }

    pub async fn execute_existing_task(
        &self,
        task_id: String,
        params: TaskRunParams,
    ) -> anyhow::Result<()> {
        let this = self.clone();
        let running_task_id = task_id.clone();
        let handle = tokio::spawn(async move {
            if let Err(error) = this.run_task(task_id.clone(), params).await {
                tracing::error!("worker task {} failed: {:?}", task_id, error);
                let _ = this
                    .publish_failure(&task_id, format!("Analysis task failed: {error:#}"))
                    .await;
            }
        });
        self.running_tasks
            .write()
            .await
            .insert(running_task_id, handle.abort_handle());
        Ok(())
    }

    pub(super) async fn run_task(
        &self,
        task_id: String,
        params: TaskRunParams,
    ) -> anyhow::Result<()> {
        let task_started_at = Instant::now();
        self.update_task(
            &task_id,
            TaskStatus::Running,
            5,
            "Preparing analysis",
            "Initializing task and loading context",
            "Preparing analysis",
            None,
        )
        .await?;

        let task = self
            .analysis_store
            .get_task(&task_id)
            .await?
            .context("task missing after execution")?;
        crate::telemetry::mark_span_task(&task.task_id, &task.symbol, &task.market_type);
        tracing::info!(
            task_id = %task.task_id,
            symbol = %task.symbol,
            market_type = %task.market_type,
            analysis_date = %task.analysis_date,
            owner_username = %task.owner_username,
            "analysis task started"
        );
        // Prepare result early so we can check for checkpoint resume data
        tracing::info!(
            task_id = %task.task_id,
            symbol = %task.symbol,
            "run_task: prepare_result start (early)"
        );
        let mut result =
            crate::report::runtime::TradingAgentsGraph::prepare_result(self, &task, &params)
                .await?;
        tracing::info!(
            task_id = %task.task_id,
            symbol = %task.symbol,
            resumed_from_step = result.artifacts.resumed_from_step,
            has_cached_quote = result.artifacts.scenario_data.quote.is_some(),
            has_cached_news = !result.artifacts.scenario_data.company_news.is_empty(),
            "run_task: prepare_result done (early)"
        );

        let is_resume_with_data = result.artifacts.resumed_from_step > 0
            && result.artifacts.scenario_data.quote.is_some();

        let (quote, fundamentals, news_items, market_chart, news_start, quality_gate);

        let _hydration_span = tracing::info_span!(
            "analysis.market_data_hydration",
            task_id = %task.task_id,
            symbol = %task.symbol,
            market_type = %task.market_type,
            is_resume = is_resume_with_data,
        );

        if is_resume_with_data {
            tracing::info!(
                task_id = %task.task_id,
                symbol = %task.symbol,
                "run_task: resuming from checkpoint, reusing cached market data"
            );
            quote = result.artifacts.scenario_data.quote.clone();
            fundamentals = result.artifacts.scenario_data.fundamentals.clone();
            news_items = result.artifacts.scenario_data.company_news.clone();
            news_start = result
                .artifacts
                .scenario_data
                .company_news_start_date
                .clone();
            market_chart = result.artifacts.market_chart.clone();
            quality_gate = super::ReportQualityGate::from_acquired_data(
                &quote,
                &market_chart.candles,
                &fundamentals,
                &news_items,
                Default::default(),
                Utc::now(),
            );
        } else {
            // Fresh run — fetch market data via helpers
            let news_start_val = analysis_news_start(&task.analysis_date);
            news_start = news_start_val.clone();

            let core_data = self.fetch_core_market_data(&task, news_start_val).await;
            self.fetch_enrichment_and_store(&task, &mut result).await;

            result.artifacts.scenario_data.fetch_diagnosis = core_data.fetch_diagnosis;
            result.artifacts.scenario_data.technical_summary =
                build_technical_summary(&core_data.market_chart);

            quote = core_data.quote;
            fundamentals = core_data.fundamentals;
            news_items = core_data.news_items;
            market_chart = core_data.market_chart;
            quality_gate = core_data.quality_gate;
        }

        if !quality_gate.passed {
            let summary = quality_gate.summary();
            self.analysis_store.save_result(&task_id, &result).await?;
            self.update_task(
                &task_id,
                TaskStatus::BlockedData,
                100,
                "Data quality blocked",
                "Critical report evidence could not be acquired",
                "Analysis blocked before LLM execution",
                Some(summary),
            )
            .await?;
            self.running_tasks.write().await.remove(&task_id);
            return Ok(());
        }

        // Calculate data quality scores
        let validator = crate::data::validator::DataValidator;
        let fundamentals_score = fundamentals
            .as_ref()
            .map(|f| validator.validate_fundamentals(f).score)
            .unwrap_or(0.0);

        let quote_score = if quote.is_some() { 100.0 } else { 0.0 };
        let news_score = if news_items.is_empty() { 0.0 } else { 80.0 };
        let candles_score = if market_chart.candles.is_empty() {
            0.0
        } else {
            90.0
        };

        let overall_quality =
            validator.overall_score(quote_score, fundamentals_score, news_score, candles_score);

        tracing::info!(
            task_id = %task.task_id,
            symbol = %task.symbol,
            data_quality = overall_quality,
            fundamentals_score = fundamentals_score,
            "data quality assessment"
        );

        if let Err(error) = self.resolve_pending_entries(&task.symbol, &params).await {
            tracing::warn!(
                "failed to resolve pending memory entries for {} before analysis: {:?}",
                task.symbol,
                error
            );
        }

        let refined_memory_context = self
            .initial_memory_context(
                &task.symbol,
                &task.market_type,
                quote.as_ref(),
                fundamentals.as_ref(),
                &news_items,
            )
            .await;
        let mut params = params;
        params.past_context = refined_memory_context.context_text.clone();
        params.memory_context =
            crate::task_manager::memory_snapshot_from_bundle(&refined_memory_context);

        result.artifacts.scenario_context = params.scenario.clone();
        if !is_resume_with_data {
            Self::hydrate_scenario_data(
                &mut result,
                market_chart,
                &quote,
                &fundamentals,
                &news_items,
                &news_start,
                &task,
            );
        }

        tracing::info!(
            task_id = %task.task_id,
            symbol = %task.symbol,
            quote_present = result.artifacts.scenario_data.quote.is_some(),
            fundamentals_present = result.artifacts.scenario_data.fundamentals.is_some(),
            company_news_count = result.artifacts.scenario_data.company_news.len(),
            candles_count = result.artifacts.scenario_data.candles.len(),
            chart_candles_count = result.artifacts.market_chart.candles.len(),
            "run_task: scenario hydration done"
        );
        tracing::info!(
            task_id = %task.task_id,
            symbol = %task.symbol,
            "run_task: refresh_structured_report_snapshot start"
        );
        let _ = self.refresh_structured_report_snapshot(&mut result).await;
        tracing::info!(
            task_id = %task.task_id,
            symbol = %task.symbol,
            "run_task: refresh_structured_report_snapshot done"
        );
        tracing::info!(
            task_id = %task.task_id,
            symbol = %task.symbol,
            "run_task: initial save_result start"
        );
        self.analysis_store.save_result(&task_id, &result).await?;
        tracing::info!(
            task_id = %task.task_id,
            symbol = %task.symbol,
            "run_task: initial save_result done"
        );

        self.update_task(
            &task_id,
            TaskStatus::Running,
            10,
            "Analyst phase",
            "Running analyst agents",
            "Analyst phase running",
            None,
        )
        .await?;

        drop(_hydration_span);

        let _graph_span = tracing::info_span!(
            "analysis.graph_execution",
            task_id = %task.task_id,
            symbol = %task.symbol,
        );

        if let Err(error) = self
            .run_agent_graph(
                &mut result,
                &params,
                quote.as_ref(),
                fundamentals.as_ref(),
                &news_items,
            )
            .await
        {
            let _ = self
                .finalize_partial_result_on_failure(&mut result, &params)
                .await;
            let _ = self.analysis_store.save_result(&task_id, &result).await;
            return Err(error);
        }

        // Run consistency validator after all LLM stages complete, before saving.
        let consistency_issues =
            crate::report::diagnosis::ConsistencyValidator::validate_and_fix(&mut result);
        if !consistency_issues.is_empty() {
            tracing::info!(
                task_id = %task_id,
                issues_count = consistency_issues.len(),
                "consistency validator applied auto-fixes"
            );
            result.artifacts.diagnosis_summary =
                Some(crate::DiagnosisSummary::from_issues(&consistency_issues));
        }

        self.analysis_store
            .save_result(&task_id, &result)
            .await
            .with_context(|| format!("failed to save final result for task {task_id}"))?;

        drop(_graph_span);
        let _finalization_span = tracing::info_span!(
            "analysis.result_finalization",
            task_id = %task.task_id,
            symbol = %task.symbol,
        );

        self.save_checkpoint(
            &task_id,
            &task.symbol,
            &task.analysis_date,
            "complete",
            "complete",
            &result,
        )
        .await
        .with_context(|| format!("failed to save final checkpoint for task {task_id}"))?;
        let _ = self
            .memory_log
            .store_decision_async(
                &task.symbol,
                &task.analysis_date,
                &result.agent_state.final_trade_decision,
                result.report.recommendation.as_str(),
                result.report.trader_plan.action.as_str(),
                &result.market_type,
                result.report.direction_score,
                result.report.confidence_score,
                result.report.action_score,
                Some(&crate::memory::ResearchMemoryRecord {
                    setup_tags: crate::derive_setup_tags(
                        &result.report.confidence_breakdown,
                        &result.report.direction_breakdown,
                        &result.report.execution_readiness,
                        &result.report.research_plan,
                        &result.report.trader_plan,
                        &result.report.portfolio_decision,
                    ),
                    stock_name: result.stock_name.clone(),
                    summary: result.report.summary.key.clone(),
                    risk_assessment: result.report.risk_assessment.key.clone(),
                    rationale: result.report.rationale.key.clone(),
                    structured_risk: crate::StructuredRiskAssessment::from_text(
                        result.report.risk_assessment.as_str(),
                    ),
                    structured_reflection: result.report.reflection.clone(),
                    trigger_checklist: result
                        .report
                        .portfolio_decision
                        .trigger_checklist
                        .iter()
                        .chain(result.report.trader_plan.execution_trigger_checklist.iter())
                        .cloned()
                        .collect(),
                    blocking_gaps: result
                        .report
                        .portfolio_decision
                        .missing_evidence_ladder
                        .blocking_gaps
                        .iter()
                        .chain(result.report.trader_plan.blocking_gaps.iter())
                        .cloned()
                        .collect(),
                    execution_boundary_complete: result
                        .report
                        .execution_readiness
                        .execution_boundary_complete,
                    structured_snapshot: serde_json::json!({
                        "market_chart": result.report.market_chart,
                        "price_context": result.report.price_context,
                        "probability_view": result.report.probability_view,
                        "profit_risk": result.report.profit_risk,
                        "ic_navigator": result.report.ic_navigator,
                        "technical_indicators": result.report.technical_indicators,
                        "evidence_cards": result.report.evidence_cards,
                        "news_insights": result.report.news_insights,
                        "risk_controls": result.report.risk_controls,
                    }),
                }),
                &task.owner_username,
            )
            .await;
        self.checkpoint_store
            .clear(&task.task_id, &task.symbol, &task.analysis_date)
            .await
            .with_context(|| format!("failed to clear checkpoint for task {task_id}"))?;
        self.checkpoint_store
            .clear_graph_runtime(&task.task_id, &task.symbol, &task.analysis_date)
            .await
            .with_context(|| {
                format!("failed to clear graph runtime checkpoint for task {task_id}")
            })?;

        self.update_task(
            &task_id,
            TaskStatus::Completed,
            100,
            "Analysis completed",
            "Report and decision generated",
            "Analysis completed",
            None,
        )
        .await
        .with_context(|| format!("failed to publish completed status for task {task_id}"))?;
        crate::telemetry::record_analysis_task_duration(
            &self.telemetry,
            "completed",
            &task.market_type,
            task_started_at.elapsed().as_secs_f64() * 1000.0,
            None,
        );
        tracing::info!(
            task_id = %task.task_id,
            symbol = %task.symbol,
            market_type = %task.market_type,
            elapsed_ms = task_started_at.elapsed().as_secs_f64() * 1000.0,
            "analysis task completed"
        );
        let _ = self.cache_completed_analysis(&task, &result).await;
        self.running_tasks.write().await.remove(&task_id);
        Ok(())
    }
}

impl TaskManager {
    pub(super) async fn publish_failure(
        &self,
        task_id: &str,
        error_message: String,
    ) -> anyhow::Result<()> {
        let task = self.analysis_store.get_task(task_id).await?;
        let market_type = task
            .as_ref()
            .map(|task| task.market_type.clone())
            .unwrap_or_else(|| "unknown".to_string());
        let reason = if error_message.to_ascii_lowercase().contains("timeout") {
            "timeout"
        } else if error_message.contains("worker") {
            "worker_failure"
        } else if error_message.contains("LLM")
            || error_message.to_ascii_lowercase().contains("model")
        {
            "llm_failure"
        } else if error_message.to_ascii_lowercase().contains("database") {
            "database_failure"
        } else {
            "internal_error"
        };
        self.update_task(
            task_id,
            TaskStatus::Failed,
            100,
            "Analysis failed",
            "An error occurred during execution",
            &error_message,
            Some(error_message.clone()),
        )
        .await?;
        crate::telemetry::record_analysis_task_duration(
            &self.telemetry,
            "failed",
            &market_type,
            0.0,
            Some(reason),
        );
        tracing::error!(
            task_id = %task_id,
            market_type = %market_type,
            analysis_date = task.as_ref().map(|task| task.analysis_date.as_str()).unwrap_or("unknown"),
            symbol = task.as_ref().map(|task| task.symbol.as_str()).unwrap_or("unknown"),
            owner_username = task.as_ref().map(|task| task.owner_username.as_str()).unwrap_or(""),
            error_reason = reason,
            error_message = %error_message,
            "analysis task failed"
        );
        self.running_tasks.write().await.remove(task_id);
        Ok(())
    }

    pub(crate) async fn update_task(
        &self,
        task_id: &str,
        status: TaskStatus,
        progress: i32,
        step_name: &str,
        step_description: &str,
        message: &str,
        error_message: Option<String>,
    ) -> anyhow::Result<()> {
        tracing::info!(task_id, progress, step_name, "update_task: load task start");
        let mut task = self
            .analysis_store
            .get_task(task_id)
            .await?
            .context("task not found")?;
        tracing::info!(task_id, progress, step_name, "update_task: load task done");
        task.status = status.clone();
        task.progress = progress;
        task.current_step_name = step_name.to_string();
        task.current_step_description = step_description.to_string();
        task.message = message.to_string();
        task.error_message = error_message.clone();
        task.updated_at = Utc::now();
        tracing::info!(
            task_id,
            progress,
            step_name,
            "update_task: store update start"
        );
        self.analysis_store.update_task(&task).await?;
        tracing::info!(
            task_id,
            progress,
            step_name,
            "update_task: store update done"
        );
        // Avoid deserializing the full result snapshot during hot progress updates.
        // This path only feeds websocket metadata and was causing stack overflows
        // under large in-flight analysis payloads.
        let result_stage = None;

        tracing::info!(
            task_id,
            progress,
            step_name,
            "update_task: broadcaster start"
        );
        let sender = self.broadcaster(task_id).await;
        tracing::info!(
            task_id,
            progress,
            step_name,
            "update_task: broadcaster done"
        );
        let event = TaskEvent {
            event_type: "progress_update".to_string(),
            task_id: task_id.to_string(),
            status,
            progress,
            message: message.to_string(),
            current_step_name: step_name.to_string(),
            current_step_description: step_description.to_string(),
            emitted_at: Utc::now().to_rfc3339(),
            result_stage,
            llm_token_usage: task.llm_token_usage,
        };
        let _ = sender.send(event.clone());
        // Publish event for cross-instance delivery
        self.publish_task_event(task_id, &event).await;
        tracing::info!(task_id, progress, step_name, "update_task: send done");
        Ok(())
    }

    pub(crate) async fn save_checkpoint(
        &self,
        task_id: &str,
        symbol: &str,
        analysis_date: &str,
        stage: &str,
        node: &str,
        result: &AnalysisResult,
    ) -> anyhow::Result<()> {
        self.checkpoint_store
            .save(&crate::checkpoint::TaskCheckpoint {
                task_id: task_id.to_string(),
                symbol: symbol.to_string(),
                analysis_date: analysis_date.to_string(),
                stage: stage.to_string(),
                node: node.to_string(),
                result: result.clone(),
                step: crate::TaskManager::stage_step(stage),
            })
            .await
    }
}

pub(crate) mod artifacts;
mod facts;
mod stages;

use chrono::Utc;

use crate::engine::task_manager::TaskRunParams;
use crate::engine::analysis::lifecycle::task_run::TaskUpdate;
use crate::data::{FundamentalsSnapshot, NewsItem, QuoteSnapshot};
use crate::models::{
    AgentStateSnapshot, AnalysisArtifacts, AnalysisGraph, AnalysisResult, PersistedTask,
    RuntimeNodeTrace, TaskStatus,
};

impl crate::TaskManager {
    async fn update_graph_stage(
        &self,
        task_id: &str,
        progress: i32,
        step_name: &str,
        step_description: &str,
        message: &str,
    ) -> anyhow::Result<()> {
        self.update_task(TaskUpdate {
            task_id,
            status: TaskStatus::Running,
            progress,
            step_name,
            step_description,
            message,
            error_message: None,
        })
        .await
    }

    pub(crate) fn normalized_selected_analysts(params: &TaskRunParams) -> Vec<String> {
        let items = params
            .selected_analysts
            .iter()
            .map(|item| item.trim().to_lowercase())
            .filter(|item| !item.is_empty())
            .map(|item| {
                if item == "social" {
                    "sentiment".to_string()
                } else {
                    item
                }
            })
            .collect::<Vec<_>>();

        if items.is_empty() {
            return vec![
                "market".to_string(),
                "sentiment".to_string(),
                "news".to_string(),
                "fundamentals".to_string(),
            ];
        }

        let mut ordered = Vec::new();
        for item in items {
            if !ordered.iter().any(|existing| existing == &item) {
                ordered.push(item);
            }
        }
        ordered
    }

    pub(crate) async fn refresh_structured_report_snapshot(
        &self,
        result: &mut AnalysisResult,
    ) -> anyhow::Result<()> {
        result.sync_derived_fields();
        let history_entries = self.memory_log.load_entries().await.unwrap_or_default();
        let resolved_entries = history_entries
            .into_iter()
            .filter(|entry| {
                !entry.pending && entry.raw_return.is_some() && entry.alpha_return.is_some()
            })
            .collect::<Vec<_>>();
        let market_entries = resolved_entries
            .iter()
            .filter(|entry| {
                entry
                    .market
                    .trim()
                    .eq_ignore_ascii_case(result.market_type.trim())
            })
            .cloned()
            .collect::<Vec<_>>();
        let use_market_profile = market_entries.len() >= 12;
        let calibration_profile = if use_market_profile {
            crate::engine::memory::derive_calibration_profile(&market_entries)
        } else {
            crate::engine::memory::derive_calibration_profile(&resolved_entries)
        };
        result.artifacts.memory_context.market_sample_count = market_entries.len();
        result.artifacts.memory_context.used_market_profile = use_market_profile;
        result.rebuild_report(&calibration_profile);
        result.artifacts.memory_context.resolved_setup_tags = crate::models::derive_setup_tags(
            &result.report.confidence_breakdown,
            &result.report.direction_breakdown,
            &result.report.execution_readiness,
            &result.report.research_plan,
            &result.report.trader_plan,
            &result.report.portfolio_decision,
        );
        Ok(())
    }

    pub(crate) async fn persist_runtime_stage(
        &self,
        result: &AnalysisResult,
        stage: &str,
        node: &str,
    ) -> anyhow::Result<()> {
        tracing::info!(
            task_id = %result.task_id,
            symbol = %result.symbol,
            stage,
            node,
            "persist runtime stage: clone start"
        );
        let mut snapshot = result.clone();
        tracing::info!(
            task_id = %result.task_id,
            symbol = %result.symbol,
            stage,
            node,
            "persist runtime stage: clone done"
        );
        self.refresh_structured_report_snapshot(&mut snapshot).await?;
        tracing::info!(
            task_id = %snapshot.task_id,
            symbol = %snapshot.symbol,
            stage,
            node,
            "persist runtime stage: refresh structured report done"
        );
        self.analysis_store.save_result(&snapshot.task_id, &snapshot).await?;
        tracing::info!(
            task_id = %snapshot.task_id,
            symbol = %snapshot.symbol,
            stage,
            node,
            "persist runtime stage: save_result done"
        );
        self.checkpoint_store
            .save(&crate::engine::checkpoint::TaskCheckpoint {
                task_id: snapshot.task_id.clone(),
                symbol: snapshot.symbol.clone(),
                analysis_date: snapshot.analysis_date.clone(),
                stage: stage.to_string(),
                node: node.to_string(),
                result: snapshot,
                step: Self::stage_step(stage),
            })
            .await?;
        tracing::info!(
            task_id = %result.task_id,
            symbol = %result.symbol,
            stage,
            node,
            "persist runtime stage: checkpoint save done"
        );
        Ok(())
    }

    pub(crate) fn build_initial_result(
        &self,
        task: &PersistedTask,
        params: &TaskRunParams,
    ) -> AnalysisResult {
        let mut result = AnalysisResult {
            task_id: task.task_id.clone(),
            report_id: format!("report-{}", task.task_id),
            symbol: task.symbol.clone(),
            stock_name: if task.stock_name.trim().is_empty() {
                task.symbol.clone()
            } else {
                task.stock_name.clone()
            },
            analysis_date: task.analysis_date.clone(),
            market_type: task.market_type.clone(),
            graph: AnalysisGraph::default(),
            agent_state: AgentStateSnapshot {
                company_of_interest: task.symbol.clone(),
                trade_date: task.analysis_date.clone(),
                sender: "System".to_string(),
                past_context: params.past_context.clone(),
                ..Default::default()
            },
            artifacts: AnalysisArtifacts::default(),
            report: Default::default(),
            ic_report: Default::default(),
            created_at: Utc::now().to_rfc3339(),
        };
        result.artifacts.memory_context = params.memory_context.clone();
        result.artifacts.user_context = params.user_context.clone();
        result.artifacts.scenario_context = params.scenario.clone();
        result.sync_derived_fields();
        let overview_summary = result.derived_summary();
        crate::engine::analysis::graph::push_checkpoint(
            &mut result,
            "overview",
            "Initial Overview",
            "completed",
            overview_summary,
        );
        result
    }

    pub(crate) async fn run_agent_graph(
        &self,
        result: &mut AnalysisResult,
        params: &TaskRunParams,
        quote: Option<&QuoteSnapshot>,
        fundamentals: Option<&FundamentalsSnapshot>,
        news_items: &[NewsItem],
    ) -> anyhow::Result<()> {
        crate::engine::analysis::runtime::TradingAgentsGraph::new(
            self,
            params,
            quote,
            fundamentals,
            news_items,
        )
        .await?
        .execute(result)
        .await
    }

    pub(crate) async fn finalize_result(
        &self,
        result: &mut AnalysisResult,
        params: &TaskRunParams,
    ) -> anyhow::Result<()> {
        result.agent_state.company_of_interest = result.symbol.clone();
        result.agent_state.trade_date = result.analysis_date.clone();
        result.agent_state.sender = "Portfolio Manager".to_string();
        result.agent_state.past_context = params.past_context.clone();
        result.artifacts.user_context = params.user_context.clone();
        if !result.artifacts.runtime_nodes.iter().any(|item| {
            item.stage == "complete" && item.node == "Portfolio Manager" && item.step == 100
        }) {
            result.artifacts.runtime_nodes.push(RuntimeNodeTrace {
                stage: "complete".to_string(),
                node: "Portfolio Manager".to_string(),
                step: 100,
                timestamp: Utc::now().to_rfc3339(),
            });
        }
        self.refresh_structured_report_snapshot(result).await?;
        result.apply_calibrated_markdown();
        result.artifacts.full_state_log_path = self.write_full_state_log(result).await?;
        result.sync_derived_fields();
        Ok(())
    }

    pub(crate) async fn finalize_partial_result_on_failure(
        &self,
        result: &mut AnalysisResult,
        params: &TaskRunParams,
    ) -> anyhow::Result<()> {
        result.agent_state.company_of_interest = result.symbol.clone();
        result.agent_state.trade_date = result.analysis_date.clone();
        result.agent_state.sender = "System".to_string();
        result.agent_state.past_context = params.past_context.clone();
        result.artifacts.user_context = params.user_context.clone();
        self.refresh_structured_report_snapshot(result).await?;
        result.apply_calibrated_markdown();
        Ok(())
    }

    pub(crate) fn stage_step(stage: &str) -> i64 {
        let stage = stage.trim();
        if stage == "overview" {
            0
        } else if stage.starts_with("analyst:") {
            10
        } else if stage == "analysts" {
            20
        } else if stage.starts_with("debate:") {
            30
        } else if stage == "debate" {
            40
        } else if stage == "research" {
            50
        } else if stage == "trader" {
            60
        } else if stage.starts_with("risk:") {
            70
        } else if stage == "risk" {
            80
        } else if stage == "portfolio" {
            90
        } else if stage == "complete" {
            100
        } else {
            0
        }
    }
}

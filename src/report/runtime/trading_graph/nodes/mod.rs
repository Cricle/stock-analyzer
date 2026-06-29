use crate::AnalysisResult;
use crate::types::PendingToolCall;
use adk_graph::{
    error::{GraphError, Result as GraphResult},
    node::{NodeContext, NodeOutput},
};
use chrono::Utc;
use serde_json::json;

use crate::TaskManager;
use crate::llm::LlmClient;

use crate::task_manager::TaskRunParams;

use super::summarize::{format_fundamental_metrics, format_volume_profile};
use super::{
    analyst_already_completed, analyst_node_name, clear_node_name, graph_error, load_result,
    result_output, tool_history_text, tool_node_name,
};

pub(super) fn analyst_planner_node(
    manager: TaskManager,
    params: TaskRunParams,
    llm: LlmClient,
    analyst_key: &'static str,
) -> impl Fn(NodeContext) -> futures::future::BoxFuture<'static, GraphResult<NodeOutput>>
+ Send
+ Sync
+ 'static {
    move |ctx| {
        let manager = manager.clone();
        let params = params.clone();
        let llm = llm.clone();
        Box::pin(async move {
            let mut result = load_result(&ctx)?;
            tracing::info!(
                task_id = %result.task_id,
                symbol = %result.symbol,
                analyst = analyst_key,
                "enter analyst planner node"
            );
            if analyst_already_completed(&result, analyst_key) {
                tracing::info!(
                    task_id = %result.task_id,
                    symbol = %result.symbol,
                    analyst = analyst_key,
                    "skip analyst planner node because report already exists"
                );
                return result_output(result);
            }
            let (progress, step_name, step_description, message) =
                analyst_step_metadata(analyst_key);
            manager
                .update_task(
                    &result.task_id,
                    crate::TaskStatus::Running,
                    progress,
                    step_name,
                    step_description,
                    message,
                    None,
                )
                .await
                .map_err(graph_error)?;
            let runtime = result
                .analyst_runtime_state(analyst_key)
                .cloned()
                .unwrap_or_default();
            let decision = {
                let llm = llm.clone();
                let symbol = result.symbol.clone();
                let market_type = params.market_type.clone();
                let analysis_date = params.analysis_date.clone();
                let key = analyst_key.to_string();
                let title = analyst_title(analyst_key).to_string();
                let agent = analyst_agent(analyst_key).to_string();
                let mission = analyst_mission(analyst_key).to_string();
                let tools: Vec<&str> = analyst_tools(analyst_key).to_vec();
                let tool_history = tool_history_text(&runtime.tool_history);
                let context_owned = analyst_context(&result, &params, analyst_key);

                crate::llm::retry::retry_with_diagnosis(
                    &format!("analyst:{analyst_key}"),
                    2,
                    |retry_hint: Option<&str>| {
                        let retry_owned = retry_hint.map(|s| s.to_string());
                        let llm = llm.clone();
                        let symbol = symbol.clone();
                        let market_type = market_type.clone();
                        let analysis_date = analysis_date.clone();
                        let key = key.clone();
                        let title = title.clone();
                        let agent = agent.clone();
                        let mission = mission.clone();
                        let tools = tools.clone();
                        let tool_history = tool_history.clone();
                        let context_owned = context_owned.clone();
                        async move {
                            let context: Vec<(&str, &str)> = context_owned
                                .iter()
                                .map(|(k, v)| (k.as_str(), v.as_str()))
                                .collect();
                            llm.generate_analyst_decision(crate::llm::AnalystDecisionParams {
                                symbol: &symbol,
                                market_type: &market_type,
                                analysis_date: &analysis_date,
                                role_key: &key,
                                role_title: &title,
                                role_agent: &agent,
                                role_brief: &mission,
                                available_tools: &tools,
                                tool_history: &tool_history,
                                extra_context: &context,
                                retry_hint: retry_owned.as_deref(),
                            })
                            .await
                            .map(|d| {
                                let raw = serde_json::to_string(&d).unwrap_or_default();
                                (d, raw)
                            })
                        }
                    },
                    |pair| crate::llm::parse::validate_analyst_decision(&pair.0, &pair.1),
                    crate::llm::retry::default_retry_hint_builder,
                )
                .await
                .map(|(d, _raw)| d)
                .map_err(graph_error)?
            };
            tracing::info!(
                task_id = %result.task_id,
                symbol = %result.symbol,
                analyst = analyst_key,
                action = %decision.action,
                tool_name = decision.tool_name.as_deref().unwrap_or(""),
                "analyst planner produced decision"
            );
            if decision.action.eq_ignore_ascii_case("tool") {
                let task_id = result.task_id.clone();
                let symbol = result.symbol.clone();
                let runtime = result.analyst_runtime_state_mut(analyst_key);
                // Support batch tool_calls or single tool_name fallback
                let tools: Vec<PendingToolCall> = if !decision.tool_calls.is_empty() {
                    decision.tool_calls.into_iter().map(|tc| PendingToolCall {
                        tool_name: tc.tool_name,
                        arguments: tc.tool_arguments,
                        reason: decision.reasoning.clone(),
                    }).collect()
                } else if let Some(name) = decision.tool_name {
                    vec![PendingToolCall {
                        tool_name: name,
                        arguments: decision.tool_arguments.unwrap_or_else(|| json!({})),
                        reason: decision.reasoning.clone(),
                    }]
                } else {
                    vec![]
                };
                runtime.pending_tools = tools;
                tracing::info!(
                    task_id = %task_id,
                    symbol = %symbol,
                    analyst = analyst_key,
                    pending_tools = ?runtime.pending_tools,
                    "stored pending analyst tool calls"
                );
                result.artifacts.llm_token_usage = llm.usage_summary().await;
                manager
                    .persist_runtime_stage(
                        &result,
                        &format!("analyst:{analyst_key}:tool_request"),
                        analyst_node_name(analyst_key),
                    )
                    .await
                    .map_err(graph_error)?;
                return result_output(result);
            }

            let final_report =
                decision
                    .final_report
                    .ok_or_else(|| GraphError::NodeExecutionFailed {
                        node: analyst_node_name(analyst_key).to_string(),
                        message: "analyst decided to finalize without final_report".to_string(),
                    })?;
            apply_analyst_report(&manager, &mut result, &params, analyst_key, final_report)
                .await
                .map_err(graph_error)?;
            result.artifacts.llm_token_usage = llm.usage_summary().await;
            let runtime = result.analyst_runtime_state_mut(analyst_key);
            runtime.final_messages.push(decision.reasoning);
            result_output(result)
        })
    }
}

fn analyst_step_metadata(analyst_key: &str) -> (i32, &'static str, &'static str, &'static str) {
    match analyst_key {
        "market" => (
            87,
            "\u{5e02}\u{573a}\u{6280}\u{672f}\u{5206}\u{6790}",
            "\u{751f}\u{6210}\u{5e02}\u{573a}\u{6280}\u{672f}\u{5206}\u{6790}\u{5e08}\u{62a5}\u{544a}",
            "\u{5e02}\u{573a}\u{6280}\u{672f}\u{5206}\u{6790}\u{4e2d}",
        ),
        "sentiment" => (
            88,
            "\u{8d44}\u{91d1}\u{60c5}\u{7eea}\u{5206}\u{6790}",
            "\u{751f}\u{6210}\u{8d44}\u{91d1}\u{60c5}\u{7eea}\u{5206}\u{6790}\u{5e08}\u{62a5}\u{544a}",
            "\u{8d44}\u{91d1}\u{60c5}\u{7eea}\u{5206}\u{6790}\u{4e2d}",
        ),
        "news" => (
            89,
            "\u{65b0}\u{95fb}\u{4e8b}\u{4ef6}\u{5206}\u{6790}",
            "\u{751f}\u{6210}\u{65b0}\u{95fb}\u{4e8b}\u{4ef6}\u{5206}\u{6790}\u{5e08}\u{62a5}\u{544a}",
            "\u{65b0}\u{95fb}\u{4e8b}\u{4ef6}\u{5206}\u{6790}\u{4e2d}",
        ),
        "fundamentals" => (
            90,
            "\u{57fa}\u{672c}\u{9762}\u{5206}\u{6790}",
            "\u{751f}\u{6210}\u{57fa}\u{672c}\u{9762}\u{5206}\u{6790}\u{5e08}\u{62a5}\u{544a}",
            "\u{57fa}\u{672c}\u{9762}\u{5206}\u{6790}\u{4e2d}",
        ),
        _ => (
            86,
            "\u{5206}\u{6790}\u{5e08}\u{9636}\u{6bb5}",
            "\u{751f}\u{6210}\u{5206}\u{6790}\u{5e08}\u{62a5}\u{544a}",
            "\u{5206}\u{6790}\u{5e08}\u{9636}\u{6bb5}\u{8fdb}\u{884c}\u{4e2d}",
        ),
    }
}

pub(super) fn tool_node(
    manager: TaskManager,
    analyst_key: &'static str,
) -> impl Fn(NodeContext) -> futures::future::BoxFuture<'static, GraphResult<NodeOutput>>
+ Send
+ Sync
+ 'static {
    move |ctx| {
        let manager = manager.clone();
        Box::pin(async move {
            let mut result = load_result(&ctx)?;
            let pending_runtime = result.analyst_runtime_state_mut(analyst_key).clone();
            tracing::info!(
                task_id = %result.task_id,
                symbol = %result.symbol,
                analyst = analyst_key,
                runtime = ?pending_runtime,
                "loaded analyst runtime state before tool execution"
            );
            let pending = pending_runtime.pending_tool.clone().ok_or_else(|| {
                GraphError::NodeExecutionFailed {
                    node: tool_node_name(analyst_key).to_string(),
                    message: "pending tool call missing".to_string(),
                }
            })?;
            tracing::info!(
                task_id = %result.task_id,
                symbol = %result.symbol,
                analyst = analyst_key,
                tool = %pending.tool_name,
                "execute analyst tool"
            );
            let scenario = result.artifacts.scenario_data.to_scenario_data();
            let observation = manager
                .toolbox
                .execute(
                    &result.symbol,
                    &result.market_type,
                    Some(&scenario),
                    &pending,
                )
                .await;
            let runtime = result.analyst_runtime_state_mut(analyst_key);
            runtime.tool_history.push(observation);
            runtime.pending_tools.clear();
            manager
                .persist_runtime_stage(
                    &result,
                    &format!("analyst:{analyst_key}:tool_result"),
                    tool_node_name(analyst_key),
                )
                .await
                .map_err(graph_error)?;
            result_output(result)
        })
    }
}

pub(super) fn clear_node(
    manager: TaskManager,
    analyst_key: &'static str,
) -> impl Fn(NodeContext) -> futures::future::BoxFuture<'static, GraphResult<NodeOutput>>
+ Send
+ Sync
+ 'static {
    move |ctx| {
        let manager = manager.clone();
        Box::pin(async move {
            let mut result = load_result(&ctx)?;
            result.analyst_runtime_state_mut(analyst_key).cleared = true;
            manager
                .persist_runtime_stage(&result, "analysts", clear_node_name(analyst_key))
                .await
                .map_err(graph_error)?;
            result_output(result)
        })
    }
}

pub(super) fn debate_node(
    manager: TaskManager,
    params: TaskRunParams,
    llm: LlmClient,
    bull: bool,
) -> impl Fn(NodeContext) -> futures::future::BoxFuture<'static, GraphResult<NodeOutput>>
+ Send
+ Sync
+ 'static {
    move |ctx| {
        let manager = manager.clone();
        let params = params.clone();
        let llm = llm.clone();
        Box::pin(async move {
            let mut result = load_result(&ctx)?;
            if bull {
                manager
                    .run_bull_researcher_node(&mut result, &params, &llm)
                    .await
                    .map_err(graph_error)?;
                // Persist the full result with all analyst data after parallel analysts merged
                manager
                    .persist_runtime_stage(&result, "analysts", "Bull Researcher")
                    .await
                    .map_err(graph_error)?;
            } else {
                manager
                    .run_bear_researcher_node(&mut result, &params, &llm)
                    .await
                    .map_err(graph_error)?;
            }
            result_output(result)
        })
    }
}

pub(super) fn research_node(
    manager: TaskManager,
    params: TaskRunParams,
    quick_llm: LlmClient,
    deep_llm: LlmClient,
) -> impl Fn(NodeContext) -> futures::future::BoxFuture<'static, GraphResult<NodeOutput>>
+ Send
+ Sync
+ 'static {
    move |ctx| {
        let manager = manager.clone();
        let params = params.clone();
        let quick_llm = quick_llm.clone();
        let deep_llm = deep_llm.clone();
        Box::pin(async move {
            let mut result = load_result(&ctx)?;
            manager
                .run_research_manager_stage(&mut result, &params, &quick_llm, &deep_llm)
                .await
                .map_err(graph_error)?;
            result_output(result)
        })
    }
}

pub(super) fn trader_node(
    manager: TaskManager,
    params: TaskRunParams,
    llm: LlmClient,
) -> impl Fn(NodeContext) -> futures::future::BoxFuture<'static, GraphResult<NodeOutput>>
+ Send
+ Sync
+ 'static {
    move |ctx| {
        let manager = manager.clone();
        let params = params.clone();
        let llm = llm.clone();
        Box::pin(async move {
            let mut result = load_result(&ctx)?;
            manager
                .run_trader_stage(&mut result, &params, &llm)
                .await
                .map_err(graph_error)?;
            result_output(result)
        })
    }
}

pub(super) fn risk_discussion_node(
    manager: TaskManager,
    params: TaskRunParams,
    llm: LlmClient,
) -> impl Fn(NodeContext) -> futures::future::BoxFuture<'static, GraphResult<NodeOutput>>
+ Send
+ Sync
+ 'static {
    move |ctx| {
        let manager = manager.clone();
        let params = params.clone();
        let llm = llm.clone();
        Box::pin(async move {
            let mut result = load_result(&ctx)?;
            let max_rounds = manager.max_risk_discuss_rounds;
            for _round in 0..max_rounds {
                manager
                    .run_risk_round(&mut result, &params, &llm)
                    .await
                    .map_err(graph_error)?;
            }
            result_output(result)
        })
    }
}

pub(super) fn portfolio_node(
    manager: TaskManager,
    params: TaskRunParams,
    deep_llm: LlmClient,
) -> impl Fn(NodeContext) -> futures::future::BoxFuture<'static, GraphResult<NodeOutput>>
+ Send
+ Sync
+ 'static {
    move |ctx| {
        let manager = manager.clone();
        let params = params.clone();
        let deep_llm = deep_llm.clone();
        Box::pin(async move {
            let mut result = load_result(&ctx)?;
            manager
                .run_portfolio_stage(&mut result, &params, &deep_llm)
                .await
                .map_err(graph_error)?;
            result_output(result)
        })
    }
}

async fn apply_analyst_report(
    manager: &TaskManager,
    result: &mut AnalysisResult,
    params: &TaskRunParams,
    analyst_key: &str,
    report: crate::llm::GeneratedRoleReport,
) -> anyhow::Result<()> {
    tracing::info!(
        task_id = %result.task_id,
        symbol = %result.symbol,
        analyst = analyst_key,
        "apply analyst report"
    );
    match analyst_key {
        "market" => {
            manager
                .update_task(
                    &result.task_id,
                    crate::TaskStatus::Running,
                    87,
                    "\u{5e02}\u{573a}\u{6280}\u{672f}\u{5206}\u{6790}",
                    "\u{751f}\u{6210}\u{5e02}\u{573a}\u{6280}\u{672f}\u{5206}\u{6790}\u{5e08}\u{62a5}\u{544a}",
                    "\u{5e02}\u{573a}\u{6280}\u{672f}\u{5206}\u{6790}\u{4e2d}",
                    None,
                )
                .await?;
            result.agent_state.market_report = report.detail.clone();
            crate::report::graph::push_analyst_node(result, report);
            result.sync_derived_fields();
            manager
                .persist_runtime_stage(result, "analyst:market", "Market Analyst")
                .await?;
        }
        "sentiment" => {
            manager
                .update_task(
                    &result.task_id,
                    crate::TaskStatus::Running,
                    88,
                    "\u{8d44}\u{91d1}\u{60c5}\u{7eea}\u{5206}\u{6790}",
                    "\u{751f}\u{6210}\u{8d44}\u{91d1}\u{60c5}\u{7eea}\u{5206}\u{6790}\u{5e08}\u{62a5}\u{544a}",
                    "\u{8d44}\u{91d1}\u{60c5}\u{7eea}\u{5206}\u{6790}\u{4e2d}",
                    None,
                )
                .await?;
            result.agent_state.sentiment_report = report.detail.clone();
            crate::report::graph::push_analyst_node(result, report);
            result.sync_derived_fields();
            manager
                .persist_runtime_stage(result, "analyst:sentiment", "Social Media Analyst")
                .await?;
        }
        "news" => {
            manager
                .update_task(
                    &result.task_id,
                    crate::TaskStatus::Running,
                    89,
                    "\u{65b0}\u{95fb}\u{4e8b}\u{4ef6}\u{5206}\u{6790}",
                    "\u{751f}\u{6210}\u{65b0}\u{95fb}\u{4e8b}\u{4ef6}\u{5206}\u{6790}\u{5e08}\u{62a5}\u{544a}",
                    "\u{65b0}\u{95fb}\u{4e8b}\u{4ef6}\u{5206}\u{6790}\u{4e2d}",
                    None,
                )
                .await?;
            result.agent_state.news_report = report.detail.clone();
            crate::report::graph::push_analyst_node(result, report);
            result.sync_derived_fields();
            manager
                .persist_runtime_stage(result, "analyst:news", "News Analyst")
                .await?;
        }
        "fundamentals" => {
            manager
                .update_task(
                    &result.task_id,
                    crate::TaskStatus::Running,
                    90,
                    "\u{57fa}\u{672c}\u{9762}\u{5206}\u{6790}",
                    "\u{751f}\u{6210}\u{57fa}\u{672c}\u{9762}\u{5206}\u{6790}\u{5e08}\u{62a5}\u{544a}",
                    "\u{57fa}\u{672c}\u{9762}\u{5206}\u{6790}\u{4e2d}",
                    None,
                )
                .await?;
            result.agent_state.fundamentals_report = report.detail.clone();
            crate::report::graph::push_analyst_node(result, report);
            result.sync_derived_fields();
            let selected_count = TaskManager::normalized_selected_analysts(params).len();
            if result.graph.analysts.len() >= selected_count
                && !result
                    .graph
                    .checkpoints
                    .iter()
                    .any(|item| item.stage_key == "analysts")
            {
                crate::report::graph::push_checkpoint(
                    result,
                    "analysts",
                    "\u{5206}\u{6790}\u{5e08}\u{9636}\u{6bb5}",
                    "completed",
                    format!(
                        "\u{5df2}\u{5b8c}\u{6210} {} \u{4e2a}\u{5206}\u{6790}\u{5e08}\u{8282}\u{70b9}",
                        result.graph.analysts.len()
                    ),
                );
            }
            manager
                .persist_runtime_stage(result, "analyst:fundamentals", "Fundamentals Analyst")
                .await?;
            manager
                .persist_runtime_stage(result, "analysts", "Msg Clear Fundamentals")
                .await?;
        }
        _ => {}
    }
    let runtime = result.analyst_runtime_state_mut(analyst_key);
    runtime.pending_tools.clear();
    runtime.cleared = false;
    runtime.final_messages.push(format!(
        "{} finalized analyst report at {}",
        analyst_key,
        Utc::now().to_rfc3339()
    ));
    let _ = params;
    Ok(())
}

fn analyst_context(
    result: &AnalysisResult,
    params: &TaskRunParams,
    analyst_key: &str,
) -> Vec<(String, String)> {
    let user_context = params.user_context_prompt.as_str();
    let sector_ctx = params.sector_context.as_str();
    let has_sector = !sector_ctx.is_empty();
    let sd = &result.artifacts.scenario_data;
    let fundamental_metrics = format_fundamental_metrics(sd);
    let volume_profile = format_volume_profile(sd);
    match analyst_key {
        "sentiment" => {
            let mut ctx: Vec<(String, String)> = vec![
                (
                    "\u{5e02}\u{573a}\u{6280}\u{672f}".into(),
                    result.agent_state.market_report.clone(),
                ),
                ("Past Context".into(), params.past_context.clone()),
                ("User Context".into(), user_context.into()),
            ];
            if has_sector {
                ctx.push(("Sector & Sentiment Context".into(), sector_ctx.into()));
            }
            if !sd.hot_rank_summary.is_empty() {
                ctx.push((
                    "\u{96ea}\u{7403}\u{70ed}\u{5ea6}".into(),
                    sd.hot_rank_summary.clone(),
                ));
            }
            if !sd.billboard_summary.is_empty() {
                ctx.push((
                    "\u{9f99}\u{864e}\u{699c}".into(),
                    sd.billboard_summary.clone(),
                ));
            }
            if !sd.margin_summary.is_empty() {
                ctx.push((
                    "\u{878d}\u{8d44}\u{878d}\u{5238}".into(),
                    sd.margin_summary.clone(),
                ));
            }
            if !fundamental_metrics.is_empty() {
                ctx.push(("Fundamental Metrics".into(), fundamental_metrics));
            }
            ctx
        }
        "news" => {
            let mut ctx: Vec<(String, String)> = vec![
                (
                    "\u{5e02}\u{573a}\u{6280}\u{672f}".into(),
                    result.agent_state.market_report.clone(),
                ),
                (
                    "\u{8d44}\u{91d1}\u{60c5}\u{7eea}".into(),
                    result.agent_state.sentiment_report.clone(),
                ),
                ("Past Context".into(), params.past_context.clone()),
                ("User Context".into(), user_context.into()),
            ];
            if has_sector {
                ctx.push(("Sector & Sentiment Context".into(), sector_ctx.into()));
            }
            if !sd.billboard_summary.is_empty() {
                ctx.push((
                    "\u{9f99}\u{864e}\u{699c}".into(),
                    sd.billboard_summary.clone(),
                ));
            }
            if !sd.earnings_forecast_summary.is_empty() {
                ctx.push((
                    "\u{4e1a}\u{7ee9}\u{9884}\u{544a}".into(),
                    sd.earnings_forecast_summary.clone(),
                ));
            }
            if !fundamental_metrics.is_empty() {
                ctx.push(("Fundamental Metrics".into(), fundamental_metrics));
            }
            ctx
        }
        "fundamentals" => {
            let mut ctx: Vec<(String, String)> = vec![
                (
                    "\u{5e02}\u{573a}\u{6280}\u{672f}".into(),
                    result.agent_state.market_report.clone(),
                ),
                (
                    "\u{8d44}\u{91d1}\u{60c5}\u{7eea}".into(),
                    result.agent_state.sentiment_report.clone(),
                ),
                (
                    "\u{65b0}\u{95fb}\u{4e8b}\u{4ef6}".into(),
                    result.agent_state.news_report.clone(),
                ),
                ("User Context".into(), user_context.into()),
            ];
            if has_sector {
                ctx.push(("Sector & Sentiment Context".into(), sector_ctx.into()));
            }
            if !fundamental_metrics.is_empty() {
                ctx.push(("Fundamental Metrics".into(), fundamental_metrics));
            }
            if !sd.earnings_forecast_summary.is_empty() {
                ctx.push((
                    "\u{4e1a}\u{7ee9}\u{9884}\u{544a}".into(),
                    sd.earnings_forecast_summary.clone(),
                ));
            }
            if !sd.shareholder_summary.is_empty() {
                ctx.push((
                    "\u{80a1}\u{4e1c}\u{5206}\u{6790}".into(),
                    sd.shareholder_summary.clone(),
                ));
            }
            ctx
        }
        _ => {
            let mut ctx: Vec<(String, String)> = vec![
                ("Past Context".into(), params.past_context.clone()),
                ("User Context".into(), user_context.into()),
            ];
            if has_sector {
                ctx.push(("Sector & Sentiment Context".into(), sector_ctx.into()));
            }
            if !fundamental_metrics.is_empty() {
                ctx.push(("Fundamental Metrics".into(), fundamental_metrics));
            }
            if !volume_profile.is_empty() {
                ctx.push(("Volume Profile".into(), volume_profile));
            }
            if !sd.fund_flow_summary.is_empty() {
                ctx.push((
                    "\u{8d44}\u{91d1}\u{6d41}\u{5411}".into(),
                    sd.fund_flow_summary.clone(),
                ));
            }
            if !sd.margin_summary.is_empty() {
                ctx.push((
                    "\u{878d}\u{8d44}\u{878d}\u{5238}".into(),
                    sd.margin_summary.clone(),
                ));
            }
            if !sd.technical_summary.is_empty() {
                ctx.push((
                    "\u{6280}\u{672f}\u{6307}\u{6807}".into(),
                    sd.technical_summary.clone(),
                ));
            }
            if !sd.limit_pool_summary.is_empty() {
                ctx.push((
                    "\u{6da8}\u{505c}\u{6c60}".into(),
                    sd.limit_pool_summary.clone(),
                ));
            }
            ctx
        }
    }
}

fn analyst_tools(analyst_key: &str) -> &'static [&'static str] {
    match analyst_key {
        "market" => &[
            "get_stock_data",
            "get_indicators",
            "get_fund_flow",
            "get_margin",
            "get_limit_pool",
        ],
        "sentiment" => &["get_news", "get_hot_rank", "get_billboard"],
        "news" => &[
            "get_news",
            "get_global_news",
            "get_insider_transactions",
            "get_billboard",
            "get_earnings_forecast",
        ],
        "fundamentals" => &[
            "get_fundamentals",
            "get_balance_sheet",
            "get_cashflow",
            "get_income_statement",
            "get_analyst_consensus",
            "get_earnings_forecast",
            "get_shareholder_analysis",
        ],
        _ => &[],
    }
}

fn analyst_title(analyst_key: &str) -> &'static str {
    match analyst_key {
        "market" => "\u{5e02}\u{573a}\u{6280}\u{672f}",
        "sentiment" => "\u{8d44}\u{91d1}\u{60c5}\u{7eea}",
        "news" => "\u{65b0}\u{95fb}\u{4e8b}\u{4ef6}",
        "fundamentals" => "\u{57fa}\u{672c}\u{9762}",
        _ => "\u{5206}\u{6790}",
    }
}

fn analyst_agent(analyst_key: &str) -> &'static str {
    match analyst_key {
        "market" => "Market Analyst",
        "sentiment" => "Social Media Analyst",
        "news" => "News Analyst",
        "fundamentals" => "Fundamentals Analyst",
        _ => "Analyst",
    }
}

fn analyst_mission(analyst_key: &str) -> &'static str {
    match analyst_key {
        "market" => {
            "\u{8d1f}\u{8d23}\u{8d8b}\u{52bf}\u{3001}\u{52a8}\u{91cf}\u{3001}\u{6ce2}\u{52a8}\u{3001}\u{91cf}\u{4ef7}\u{7ed3}\u{6784}\u{3001}\u{652f}\u{6491}\u{963b}\u{529b}\u{4e0e}\u{6280}\u{672f}\u{5931}\u{6548}\u{6761}\u{4ef6}\u{5206}\u{6790}\u{3002}"
        }
        "sentiment" => {
            "\u{8d1f}\u{8d23}\u{8d44}\u{91d1}\u{6d41}\u{3001}\u{6362}\u{624b}\u{3001}\u{677f}\u{5757}\u{70ed}\u{5ea6}\u{3001}\u{60c5}\u{7eea}\u{62e5}\u{6324}\u{5ea6}\u{3001}\u{9884}\u{671f}\u{6e29}\u{5ea6}\u{4e0e}\u{7b79}\u{7801}\u{7ed3}\u{6784}\u{5206}\u{6790}\u{3002}"
        }
        "news" => {
            "\u{8d1f}\u{8d23}\u{516c}\u{544a}\u{3001}\u{4ea7}\u{4e1a}\u{3001}\u{653f}\u{7b56}\u{3001}\u{5b8f}\u{89c2}\u{4e0e}\u{516c}\u{53f8}\u{4e8b}\u{4ef6}\u{50ac}\u{5316}\u{7684}\u{65f6}\u{95f4}\u{7ebf}\u{4e0e}\u{8fb9}\u{9645}\u{53d8}\u{5316}\u{5206}\u{6790}\u{3002}"
        }
        "fundamentals" => {
            "\u{8d1f}\u{8d23}\u{5546}\u{4e1a}\u{8d28}\u{91cf}\u{3001}\u{76c8}\u{5229}\u{9a71}\u{52a8}\u{3001}\u{8d44}\u{4ea7}\u{8d1f}\u{503a}\u{8868}\u{3001}\u{73b0}\u{91d1}\u{6d41}\u{3001}\u{4f30}\u{503c}\u{951a}\u{4e0e}\u{884c}\u{4e1a}\u{5730}\u{4f4d}\u{5206}\u{6790}\u{3002}"
        }
        _ => "\u{8d1f}\u{8d23}\u{4e13}\u{9879}\u{7814}\u{7a76}\u{3002}",
    }
}

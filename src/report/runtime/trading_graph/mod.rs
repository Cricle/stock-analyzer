mod graph_builder;
mod nodes;
mod routing;
mod summarize;

use adk_graph::{
    error::{GraphError, Result as GraphResult},
    node::{NodeContext, NodeOutput},
};
use serde_json::Value;

use crate::AnalysisResult;

pub(crate) use graph_builder::TradingAgentsGraph;
pub use summarize::summarize_stock_data_output;

// --- Node name constants ---

const CHANNEL_RESULT: &str = "result";

const NODE_MARKET: &str = "Market Analyst";
const NODE_SENTIMENT: &str = "Social Media Analyst";
const NODE_NEWS: &str = "News Analyst";
const NODE_FUNDAMENTALS: &str = "Fundamentals Analyst";
const NODE_TOOLS_MARKET: &str = "tools_market";
const NODE_TOOLS_SOCIAL: &str = "tools_social";
const NODE_TOOLS_NEWS: &str = "tools_news";
const NODE_TOOLS_FUNDAMENTALS: &str = "tools_fundamentals";
const NODE_CLEAR_MARKET: &str = "Msg Clear Market";
const NODE_CLEAR_SOCIAL: &str = "Msg Clear Social";
const NODE_CLEAR_NEWS: &str = "Msg Clear News";
const NODE_CLEAR_FUNDAMENTALS: &str = "Msg Clear Fundamentals";
const NODE_BULL: &str = "Bull Researcher";
const NODE_BEAR: &str = "Bear Researcher";
const NODE_RESEARCH: &str = "Research Manager";
const NODE_TRADER: &str = "Trader";
const NODE_RISK_DISCUSS: &str = "Risk Discussion";
const NODE_PORTFOLIO: &str = "Portfolio Manager";

// --- Shared helper functions ---

fn analyst_already_completed(result: &AnalysisResult, analyst_key: &str) -> bool {
    let has_node = result
        .graph
        .analysts
        .iter()
        .any(|node| node.key == analyst_key);
    if !has_node {
        return false;
    }
    // Also verify report content is non-empty to handle stale checkpoint resume
    
    match analyst_key {
        "market" => !result.agent_state.market_report.trim().is_empty(),
        "sentiment" | "social" => !result.agent_state.sentiment_report.trim().is_empty(),
        "news" => !result.agent_state.news_report.trim().is_empty(),
        "fundamentals" => !result.agent_state.fundamentals_report.trim().is_empty(),
        _ => true,
    }
}

fn analyst_node_name(analyst: &str) -> &'static str {
    match analyst {
        "market" => NODE_MARKET,
        "sentiment" | "social" => NODE_SENTIMENT,
        "news" => NODE_NEWS,
        "fundamentals" => NODE_FUNDAMENTALS,
        _ => NODE_MARKET,
    }
}

fn tool_node_name(analyst: &str) -> &'static str {
    match analyst {
        "market" => NODE_TOOLS_MARKET,
        "sentiment" | "social" => NODE_TOOLS_SOCIAL,
        "news" => NODE_TOOLS_NEWS,
        "fundamentals" => NODE_TOOLS_FUNDAMENTALS,
        _ => NODE_TOOLS_MARKET,
    }
}

fn clear_node_name(analyst: &str) -> &'static str {
    match analyst {
        "market" => NODE_CLEAR_MARKET,
        "sentiment" | "social" => NODE_CLEAR_SOCIAL,
        "news" => NODE_CLEAR_NEWS,
        "fundamentals" => NODE_CLEAR_FUNDAMENTALS,
        _ => NODE_CLEAR_MARKET,
    }
}

fn load_result(ctx: &NodeContext) -> GraphResult<AnalysisResult> {
    deserialize_result(ctx.get(CHANNEL_RESULT).cloned().ok_or_else(|| {
        GraphError::SerializationError("graph state missing analysis result".to_string())
    })?)
}

fn deserialize_result(value: Value) -> GraphResult<AnalysisResult> {
    serde_json::from_value(value).map_err(|error| {
        GraphError::SerializationError(format!(
            "failed to deserialize analysis result from graph state: {error}"
        ))
    })
}

fn result_output(result: AnalysisResult) -> GraphResult<NodeOutput> {
    Ok(NodeOutput::new().with_update(
        CHANNEL_RESULT,
        serde_json::to_value(result)
            .map_err(|error| GraphError::SerializationError(error.to_string()))?,
    ))
}

fn graph_error(error: anyhow::Error) -> GraphError {
    GraphError::NodeExecutionFailed {
        node: "trading-agents-runtime".to_string(),
        message: format!("{error:#}"),
    }
}

pub(crate) fn tool_history_text(history: &[crate::types::ToolObservation]) -> String {
    history
        .iter()
        .map(|item| {
            format!(
                "Tool: {}\nArgs: {}\nSuccess: {}\nOutput:\n{}",
                item.tool_name,
                item.arguments,
                item.success,
                summarize::summarize_tool_observation(item)
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

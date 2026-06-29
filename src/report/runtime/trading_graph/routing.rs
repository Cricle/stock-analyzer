use crate::AnalysisResult;
use adk_graph::state::State;

use super::{
    CHANNEL_RESULT, NODE_BEAR, NODE_BULL, NODE_RESEARCH, analyst_already_completed,
    clear_node_name, tool_node_name,
};

pub(super) fn analyst_route(state: &State, analyst_key: &str) -> String {
    let Some(result) = state
        .get(CHANNEL_RESULT)
        .cloned()
        .and_then(|value| serde_json::from_value::<AnalysisResult>(value).ok())
    else {
        return clear_node_name(analyst_key).to_string();
    };
    if analyst_already_completed(&result, analyst_key) {
        return clear_node_name(analyst_key).to_string();
    }
    let runtime = result.analyst_runtime_state(analyst_key);
    if runtime
        .and_then(|item| item.pending_tools.first())
        .is_some()
    {
        return tool_node_name(analyst_key).to_string();
    }
    clear_node_name(analyst_key).to_string()
}

pub(super) fn debate_route(state: &State, max_debate_rounds: usize) -> String {
    let Some(result) = state
        .get(CHANNEL_RESULT)
        .cloned()
        .and_then(|value| serde_json::from_value::<AnalysisResult>(value).ok())
    else {
        return NODE_RESEARCH.to_string();
    };

    if result.graph.investment_debate.count >= (max_debate_rounds as i32 * 2) {
        return NODE_RESEARCH.to_string();
    }
    if result.graph.investment_debate.count % 2 == 1 {
        return NODE_BEAR.to_string();
    }
    NODE_BULL.to_string()
}

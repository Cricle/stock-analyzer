mod helpers;
mod task_create;
mod task_run;
mod task_status;

use sa_models::{AnalysisParameters, AnalysisUserContext};

fn normalize_language(_value: Option<&str>) -> String {
    "zh-CN".to_string()
}

fn normalize_option(value: Option<&str>, allowed: &[&str], default_value: &str) -> String {
    let candidate = value.unwrap_or_default().trim().to_ascii_lowercase();
    allowed
        .iter()
        .find(|item| item.eq_ignore_ascii_case(&candidate))
        .copied()
        .unwrap_or(default_value)
        .to_string()
}

fn bounded_user_notes(value: Option<&str>) -> String {
    value
        .unwrap_or_default()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(600)
        .collect()
}

fn build_user_context(params: &AnalysisParameters) -> AnalysisUserContext {
    AnalysisUserContext {
        language: normalize_language(params.language.as_deref()),
        position_state: normalize_option(
            params.user_position_state.as_deref(),
            &["not_holding", "holding", "sold", "watching"],
            "not_holding",
        ),
        workflow_intent: normalize_option(
            params.workflow_intent.as_deref(),
            &[
                "stock_picking",
                "holding_review",
                "prepare_buy",
                "timing_watch",
                "risk_check",
            ],
            "stock_picking",
        ),
        holding_cost: params
            .holding_cost
            .filter(|value| value.is_finite() && *value > 0.0),
        holding_ratio_pct: params
            .holding_ratio_pct
            .filter(|value| value.is_finite() && *value >= 0.0),
        risk_preference: normalize_option(
            params.risk_preference.as_deref(),
            &["low", "medium", "high"],
            "medium",
        ),
        investment_horizon: normalize_option(
            params.investment_horizon.as_deref(),
            &["short_term", "swing", "position", "long_term"],
            "swing",
        ),
        notes: bounded_user_notes(params.user_notes.as_deref()),
    }
}

fn build_user_context_prompt(context: &AnalysisUserContext) -> String {
    let mut lines = vec![
        format!("language={}", context.language),
        format!("position_state={}", context.position_state),
        format!("workflow_intent={}", context.workflow_intent),
        format!("risk_preference={}", context.risk_preference),
        format!("investment_horizon={}", context.investment_horizon),
    ];
    if let Some(value) = context.holding_cost {
        lines.push(format!("holding_cost={value:.4}"));
    }
    if let Some(value) = context.holding_ratio_pct {
        lines.push(format!("holding_ratio_pct={value:.2}"));
    }
    if !context.notes.trim().is_empty() {
        lines.push(format!("notes={}", context.notes.trim()));
    }
    lines.join("\n")
}

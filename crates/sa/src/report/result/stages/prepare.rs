use crate::task_manager::TaskRunParams;
use crate::AnalysisResult;

pub(super) fn compact_decision_context(text: &str, max_chars: usize) -> String {
    let text = text.trim();
    if text.is_empty() || max_chars == 0 {
        return String::new();
    }
    if text.chars().count() <= max_chars {
        return text.to_string();
    }

    let lines = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();

    let priority_line = |line: &str| -> usize {
        let lower = line.to_ascii_lowercase();
        usize::from(lower.contains("recommend"))
            + usize::from(lower.contains("rating"))
            + usize::from(lower.contains("confidence"))
            + usize::from(lower.contains("risk"))
            + usize::from(lower.contains("trigger"))
            + usize::from(lower.contains("invalidation"))
            + usize::from(lower.contains("stop"))
            + usize::from(lower.contains("target"))
            + usize::from(lower.contains("entry"))
            + usize::from(lower.contains("price"))
            + usize::from(lower.contains("support"))
            + usize::from(lower.contains("resistance"))
            + usize::from(lower.contains("cash"))
            + usize::from(lower.contains("debt"))
            + usize::from(lower.contains("margin"))
            + usize::from(lower.contains("profit"))
            + usize::from(lower.contains("gap"))
            + usize::from(lower.contains("gap"))
            + usize::from(lower.contains("risk"))
            + usize::from(lower.contains("trigger"))
            + usize::from(lower.contains("stop-loss"))
            + usize::from(lower.contains("target"))
    };

    let mut selected = Vec::new();
    let mut used = 0usize;
    for line in lines.iter().filter(|line| priority_line(line) > 0) {
        let len = line.chars().count() + 1;
        if used + len > max_chars.saturating_sub(32) {
            break;
        }
        selected.push((*line).to_string());
        used += len;
        if selected.len() >= 12 {
            break;
        }
    }

    if selected.len() < 6 {
        for line in &lines {
            if selected.iter().any(|item| item == line) {
                continue;
            }
            let len = line.chars().count() + 1;
            if used + len > max_chars.saturating_sub(32) {
                break;
            }
            selected.push((*line).to_string());
            used += len;
            if selected.len() >= 12 {
                break;
            }
        }
    }

    if selected.is_empty() {
        let chars = text.chars().take(max_chars).collect::<String>();
        return format!("{chars}\n...[truncated]");
    }

    let mut compact = selected.join("\n");
    if compact.chars().count() > max_chars {
        compact = compact.chars().take(max_chars).collect::<String>();
    }
    compact
}

impl crate::TaskManager {
    pub(super) fn research_manager_needs_deep_llm(result: &AnalysisResult, params: &TaskRunParams) -> bool {
        if crate::env_config::analysis_debug_quick_only() {
            return false;
        }
        let report = &result.report;
        let memory = &params.memory_context;
        let user = &params.user_context;
        let confidence = report.confidence_score;
        let action = report.action_score;
        let direction_abs = report.direction_score.abs();
        let reward_risk = report.profit_risk.reward_risk_ratio;
        let setup_history_weak = memory.setup_resolved_match_count > 0
            && (memory.setup_resolved_match_count < 2
                || memory.setup_match_hit_rate < 0.5
                || memory.setup_match_avg_alpha_return <= 0.0);
        let directional_conflict = memory.setup_resolved_match_count >= 2
            && ((report.direction_score > 20
                && memory.setup_short_match_count > memory.setup_long_match_count)
                || (report.direction_score < -20
                    && memory.setup_long_match_count > memory.setup_short_match_count));
        let boundary_case = (45..=70).contains(&confidence)
            || (40..=60).contains(&action)
            || (15..=35).contains(&direction_abs)
            || reward_risk.is_some_and(|value| (0.7..=1.3).contains(&value));
        let capital_impact = matches!(
            user.position_state.as_str(),
            "holding" | "bought" | "already_bought"
        ) || user.holding_ratio_pct.is_some_and(|value| value >= 20.0);
        let incomplete_but_actionable = !report.execution_readiness.execution_boundary_complete
            && direction_abs >= 25
            && confidence >= 45;
        setup_history_weak
            || directional_conflict
            || boundary_case
            || capital_impact
            || incomplete_but_actionable
    }
}

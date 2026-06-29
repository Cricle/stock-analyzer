use serde_json::Value;

use super::collector::AnalysisDataCollector;
use super::schema::*;

/// Execute a tool call from the LLM and update the collector.
pub fn execute_tool_call(
    collector: &AnalysisDataCollector,
    tool_name: &str,
    arguments: &Value,
) -> Result<(), String> {
    match tool_name {
        // === Rating & Confidence ===
        "set_rating" => {
            let rating = arguments.get("rating").and_then(Value::as_str).ok_or("missing 'rating'")?;
            validate_rating(rating)?;
            collector.set_rating(rating);
            Ok(())
        }
        "set_confidence" => {
            let score = arguments.get("score").and_then(Value::as_f64).ok_or("missing 'score'")?;
            if !(0.0..=100.0).contains(&score) {
                return Err("score must be 0-100".into());
            }
            collector.set_confidence(score);
            Ok(())
        }
        "set_action" => {
            let action = arguments.get("value").and_then(Value::as_str).ok_or("missing 'value'")?;
            collector.set_action(action);
            Ok(())
        }

        // === Price Levels ===
        "set_entry_price" => set_price(collector, arguments, "set_entry_price"),
        "set_stop_loss" => set_price(collector, arguments, "set_stop_loss"),
        "set_target_price" => set_price(collector, arguments, "set_target_price"),
        "set_confirmation_level" => set_price(collector, arguments, "set_confirmation_level"),
        "set_invalidation_level" => set_price(collector, arguments, "set_invalidation_level"),
        "set_risk_reward_ratio" => {
            let value = arguments.get("value").and_then(Value::as_f64).ok_or("missing 'value'")?;
            if value < 0.0 {
                return Err("ratio must be >= 0".into());
            }
            collector.set_risk_reward_ratio(value);
            Ok(())
        }

        // === Text Fields ===
        "set_executive_summary" => set_text(collector, arguments, "set_executive_summary"),
        "set_investment_thesis" => set_text(collector, arguments, "set_investment_thesis"),
        "set_rationale" => set_text(collector, arguments, "set_rationale"),
        "set_risk_assessment" => set_text(collector, arguments, "set_risk_assessment"),
        "set_summary" => set_text(collector, arguments, "set_summary"),
        "set_detail" => set_text(collector, arguments, "set_detail"),
        "set_strategic_actions" => set_text(collector, arguments, "set_strategic_actions"),
        "set_trader_plan" => set_text(collector, arguments, "set_trader_plan"),

        // === Evidence & Lists ===
        "add_evidence_point" => add_to_list(collector, arguments, "add_evidence_point"),
        "add_key_risk" => add_to_list(collector, arguments, "add_key_risk"),
        "add_trigger" => add_to_list(collector, arguments, "add_trigger"),
        "add_next_step" => add_to_list(collector, arguments, "add_next_step"),
        "add_blocking_gap" => add_to_list(collector, arguments, "add_blocking_gap"),
        "add_tolerable_gap" => add_to_list(collector, arguments, "add_tolerable_gap"),
        "add_manageable_gap" => add_to_list(collector, arguments, "add_manageable_gap"),
        "add_key_number" => add_to_list(collector, arguments, "add_key_number"),
        "add_reference" => add_to_list(collector, arguments, "add_reference"),

        // === Probability ===
        "set_probability" => {
            let up = arguments.get("up").and_then(Value::as_f64).ok_or("missing 'up'")?;
            let down = arguments.get("down").and_then(Value::as_f64).ok_or("missing 'down'")?;
            let sideways = arguments.get("sideways").and_then(Value::as_f64).ok_or("missing 'sideways'")?;
            if up < 0.0 || up > 1.0 || down < 0.0 || down > 1.0 || sideways < 0.0 || sideways > 1.0 {
                return Err("probabilities must be 0-1".into());
            }
            collector.set_probability(up, down, sideways);
            Ok(())
        }

        // === Scores ===
        "set_score" => {
            let dim = arguments.get("dimension").and_then(Value::as_str).ok_or("missing 'dimension'")?;
            let score = arguments.get("score").and_then(Value::as_f64).ok_or("missing 'score'")?;
            if !(0.0..=100.0).contains(&score) {
                return Err("score must be 0-100".into());
            }
            collector.set_score(dim, score);
            Ok(())
        }

        // === Scenario Paths ===
        "add_scenario_path" => {
            let key = arguments.get("key").and_then(Value::as_str).ok_or("missing 'key'")?;
            let name = arguments.get("name").and_then(Value::as_str).ok_or("missing 'name'")?;
            let action = arguments.get("action").and_then(Value::as_str).ok_or("missing 'action'")?;
            if key.is_empty() || name.is_empty() {
                return Err("key and name cannot be empty".into());
            }
            collector.add_scenario_path(ScenarioPathData {
                key: key.to_string(),
                name: name.to_string(),
                trigger: arguments.get("trigger").and_then(Value::as_str).unwrap_or("").to_string(),
                action: action.to_string(),
                risk_boundary: arguments.get("risk_boundary").and_then(Value::as_str).unwrap_or("").to_string(),
                position_sizing: arguments.get("position_sizing").and_then(Value::as_str).unwrap_or("").to_string(),
                stop_level: arguments.get("stop_level").and_then(Value::as_str).unwrap_or("").to_string(),
                entry_price: arguments.get("entry_price").and_then(Value::as_f64),
                target: arguments.get("target").and_then(Value::as_f64),
            });
            Ok(())
        }

        // === Meta ===
        "set_time_horizon" => set_text(collector, arguments, "set_time_horizon"),
        "set_position_sizing" => set_text(collector, arguments, "set_position_sizing"),
        "set_time_stop" => {
            let deadline = arguments.get("deadline").and_then(Value::as_str).ok_or("missing 'deadline'")?;
            let reason = arguments.get("reason").and_then(Value::as_str).ok_or("missing 'reason'")?;
            collector.set_time_stop(deadline, reason);
            Ok(())
        }
        "set_reflection" => {
            let strongest = arguments.get("strongest_part").and_then(Value::as_str).ok_or("missing 'strongest_part'")?;
            let weakest = arguments.get("weakest_uncertainty").and_then(Value::as_str).ok_or("missing 'weakest_uncertainty'")?;
            let lesson = arguments.get("next_lesson").and_then(Value::as_str).ok_or("missing 'next_lesson'")?;
            collector.set_reflection(ReflectionData {
                strongest_part: strongest.to_string(),
                weakest_uncertainty: weakest.to_string(),
                next_lesson: lesson.to_string(),
            });
            Ok(())
        }
        "set_accounting_scope_hypothesis" => set_text(collector, arguments, "set_accounting_scope_hypothesis"),
        "set_speaker" => set_text(collector, arguments, "set_speaker"),
        "set_stance" => set_text(collector, arguments, "set_stance"),
        "set_response" => set_text(collector, arguments, "set_response"),

        other => Err(format!("unknown tool: {other}")),
    }
}

fn set_price(collector: &AnalysisDataCollector, args: &Value, tool: &str) -> Result<(), String> {
    let price = args.get("value").and_then(Value::as_f64).ok_or("missing 'value'")?;
    if price <= 0.0 || !price.is_finite() {
        return Err("price must be positive and finite".into());
    }
    match tool {
        "set_entry_price" => collector.set_entry_price(price),
        "set_stop_loss" => collector.set_stop_loss(price),
        "set_target_price" => collector.set_target_price(price),
        "set_confirmation_level" => collector.set_confirmation_level(price),
        "set_invalidation_level" => collector.set_invalidation_level(price),
        _ => unreachable!(),
    }
    Ok(())
}

fn set_text(collector: &AnalysisDataCollector, args: &Value, tool: &str) -> Result<(), String> {
    let value = args.get("value").and_then(Value::as_str).ok_or("missing 'value'")?;
    if value.trim().is_empty() {
        return Err("value cannot be empty".into());
    }
    match tool {
        "set_executive_summary" => collector.set_executive_summary(value),
        "set_investment_thesis" => collector.set_investment_thesis(value),
        "set_rationale" => collector.set_rationale(value),
        "set_risk_assessment" => collector.set_risk_assessment(value),
        "set_summary" => collector.set_summary(value),
        "set_detail" => collector.set_detail(value),
        "set_strategic_actions" => collector.set_strategic_actions(value),
        "set_trader_plan" => collector.set_trader_plan(value),
        "set_time_horizon" => collector.set_time_horizon(value),
        "set_position_sizing" => collector.set_position_sizing(value),
        "set_accounting_scope_hypothesis" => collector.set_accounting_scope_hypothesis(value),
        "set_speaker" => collector.set_speaker(value),
        "set_stance" => collector.set_stance(value),
        "set_response" => collector.set_response(value),
        _ => unreachable!(),
    }
    Ok(())
}

fn add_to_list(collector: &AnalysisDataCollector, args: &Value, tool: &str) -> Result<(), String> {
    let value = args.get("value").and_then(Value::as_str).ok_or("missing 'value'")?;
    if value.trim().is_empty() {
        return Err("value cannot be empty".into());
    }
    match tool {
        "add_evidence_point" => collector.add_evidence_point(value),
        "add_key_risk" => collector.add_key_risk(value),
        "add_trigger" => collector.add_trigger(value),
        "add_next_step" => collector.add_next_step(value),
        "add_blocking_gap" => collector.add_blocking_gap(value),
        "add_tolerable_gap" => collector.add_tolerable_gap(value),
        "add_manageable_gap" => collector.add_manageable_gap(value),
        "add_key_number" => collector.add_key_number(value),
        "add_reference" => collector.add_reference(value),
        _ => unreachable!(),
    }
    Ok(())
}

fn validate_rating(rating: &str) -> Result<(), String> {
    match rating {
        "Buy" | "Overweight" | "Hold" | "Underweight" | "Sell" => Ok(()),
        _ => Err(format!("invalid rating '{rating}', must be: Buy, Overweight, Hold, Underweight, Sell")),
    }
}

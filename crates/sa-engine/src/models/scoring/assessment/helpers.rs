
fn score_market_direction(
    analyst: Option<&AgentReportNode>,
    final_rating: &Rating,
) -> SignedScoreDimension {
    let analyst_score = score_analyst_net(analyst, 20);
    let score = analyst_score + rating_bias(final_rating, 5);
    SignedScoreDimension {
        score: score.clamp(-25, 25),
        min_score: -25,
        max_score: 25,
        rationale: LocalText::new("market_direction_rationale")
            .with_f64("net_probability", analyst.map_or(0.0, analyst_net_probability))
            .with_str("final_rating", final_rating.to_string()),
    }
}

fn score_analyst_direction(
    analyst: Option<&AgentReportNode>,
    label: &str,
    max_abs: i32,
) -> SignedScoreDimension {
    let score = score_analyst_net(analyst, max_abs);
    SignedScoreDimension {
        score,
        min_score: -max_abs,
        max_score: max_abs,
        rationale: match analyst {
            Some(node) => LocalText::new("analyst_direction_rationale")
                .with_str("analyst_label", label)
                .with_f64("up_probability", node.up_probability)
                .with_f64("down_probability", node.down_probability)
                .with_f64("sideways_probability", node.sideways_probability)
                .with_f64("net_strength", analyst_net_probability(node)),
            None => LocalText::new("analyst_direction_missing")
                .with_str("analyst_label", label),
        },
    }
}

fn score_risk_adjustment(result: &AnalysisResult, recommendation: &Rating) -> SignedScoreDimension {
    let aggressive = result
        .graph
        .risk_debate
        .turns
        .iter()
        .filter(|turn| normalized_key(&turn.stance) == "aggressive")
        .count() as i32;
    let conservative = result
        .graph
        .risk_debate
        .turns
        .iter()
        .filter(|turn| normalized_key(&turn.stance) == "conservative")
        .count() as i32;
    let base = rating_bias(recommendation, 8);
    let stance_delta = (aggressive - conservative).clamp(-2, 2) * 2;
    SignedScoreDimension {
        score: (base + stance_delta).clamp(-15, 15),
        min_score: -15,
        max_score: 15,
        rationale: LocalText::new("risk_adjustment_rationale")
            .with_str("recommendation", recommendation.to_string())
            .with_i32("aggressive_turns", aggressive)
            .with_i32("conservative_turns", conservative),
    }
}

fn score_action_alignment(
    result: &AnalysisResult,
    trader_plan: &StructuredTraderPlan,
    direction_score: i32,
    confidence_score: i32,
) -> ScoreDimension {
    let recommendation = &result.structured_portfolio_decision().rating;
    let recommendation_bias = semantic_direction(recommendation);
    let action = trader_plan.action.trim();
    let action_rating = Rating::parse(action);
    let action_bias = if action.is_empty() { 0 } else { semantic_direction(&action_rating) };
    let direction_bias = if direction_score >= 20 {
        1
    } else if direction_score <= -20 {
        -1
    } else {
        0
    };

    // Action-recommendation alignment — continuous (0-14 points)
    let rec_alignment = if action_bias == recommendation_bias {
        1.0
    } else if recommendation_bias == 0 && action_bias != 0 && action_bias == direction_bias {
        0.5
    } else {
        0.0
    };
    let rec_score = rec_alignment * 14.0;

    // Action-direction alignment — continuous (0-10 points)
    let dir_alignment = if action_bias == direction_bias {
        1.0
    } else if direction_bias == 0 && action_bias == 0 {
        0.8
    } else if direction_bias == 0 && action_bias == recommendation_bias {
        0.4
    } else {
        0.0
    };
    let dir_score = dir_alignment * 10.0;

    // Confidence bonus — sigmoid continuous (0-8 points)
    let conf_factor = sigmoid(confidence_score as f64, 65.0, 0.08);
    let conf_bonus = if action_bias != 0 && action_bias == direction_bias {
        conf_factor * 8.0
    } else if action_bias == 0 {
        (1.0 - conf_factor) * 6.0
    } else {
        4.0
    };

    let score = (rec_score + dir_score + conf_bonus).clamp(0.0, 30.0) as i32;

    ScoreDimension {
        score,
        max_score: 30,
        rationale: LocalText::new("action_alignment_rationale")
            .with_str("recommendation", recommendation.to_string())
            .with_str("action", if action.is_empty() { "not_extracted" } else { action })
            .with_i32("direction_score", direction_score)
            .with_i32("confidence_score", confidence_score),
    }
}

fn score_execution_levels(
    trader_plan: &StructuredTraderPlan,
    portfolio_decision: &StructuredPortfolioDecision,
) -> ScoreDimension {
    let action = trader_plan.action.trim();
    let entry = parse_first_number(&trader_plan.entry_price);
    let stop = parse_first_number(&trader_plan.stop_loss);
    let target = parse_first_number(&portfolio_decision.price_target);
    let confirmation = parse_first_number(&portfolio_decision.confirmation_level);
    let boundary_count = usize::from(entry.is_some())
        + usize::from(stop.is_some())
        + usize::from(target.is_some() || confirmation.is_some())
        + usize::from(!portfolio_decision.time_horizon.trim().is_empty());
    let numeric_levels = trader_plan.entry_price_numeric_count()
        + trader_plan.stop_loss_numeric_count()
        + portfolio_decision.price_target_numeric_count()
        + count_numeric_levels(&portfolio_decision.confirmation_level);

    let mut score = 0;
    if entry.is_some() {
        score += 8;
    }
    if stop.is_some() {
        score += 7;
    }
    if target.is_some() || confirmation.is_some() {
        score += 5;
    }
    if boundary_count >= 3 {
        score += 5;
    }
    if action == "Hold" && score < 10 {
        score = 10 + numeric_levels.min(3);
    }

    ScoreDimension {
        score: score.clamp(0, 25),
        max_score: 25,
        rationale: LocalText::new("execution_levels_rationale")
            .with_str("entry", entry.map(|v| format!("{v:.2}")).unwrap_or_default())
            .with_str("stop_loss", stop.map(|v| format!("{v:.2}")).unwrap_or_default())
            .with_str("target", target
                .map(|v| format!("{v:.2}"))
                .or_else(|| confirmation.map(|v| format!("{v:.2}")))
                .unwrap_or_default())
            .with_i32("boundary_count", boundary_count as i32)
            .with_i32("numeric_levels", numeric_levels),
    }
}

fn score_sizing_discipline(trader_plan: &StructuredTraderPlan) -> ScoreDimension {
    let sizing = trader_plan.position_sizing.trim();
    let parsed_percent = parse_position_percentage(sizing);

    let mut score = 0;
    if !sizing.is_empty() {
        score += 8;
    }
    if parsed_percent.is_some() {
        score += 8;
    }
    if !trader_plan.proposal.trim().is_empty() {
        score += 2;
    }
    if !trader_plan.reasoning.trim().is_empty() {
        score += 2;
    }

    ScoreDimension {
        score: score.clamp(0, 20),
        max_score: 20,
        rationale: LocalText::new("sizing_discipline_rationale")
            .with_str("sizing", if sizing.is_empty() { "" } else { sizing })
            .with_str("parsed_percent", parsed_percent
                .map(|value| format!("{:.1}%", value * 100.0))
                .unwrap_or_default())
            .with_bool("has_proposal", !trader_plan.proposal.trim().is_empty())
            .with_bool("has_reasoning", !trader_plan.reasoning.trim().is_empty()),
    }
}

fn score_horizon_clarity(portfolio_decision: &StructuredPortfolioDecision) -> ScoreDimension {
    let horizon = portfolio_decision.time_horizon.trim();
    let token_count = horizon.split_whitespace().count();
    let numeric_count = count_numeric_levels(horizon);

    let mut score = 0;
    if !horizon.is_empty() {
        score += 7;
    }
    if numeric_count > 0 || (1..=4).contains(&token_count) {
        score += 3;
    }

    ScoreDimension {
        score: score.clamp(0, 10),
        max_score: 10,
        rationale: LocalText::new("horizon_clarity_rationale")
            .with_str("horizon", if horizon.is_empty() { "" } else { horizon })
            .with_i32("token_count", token_count as i32)
            .with_i32("numeric_count", numeric_count),
    }
}

fn score_reward_to_risk(
    trader_plan: &StructuredTraderPlan,
    portfolio_decision: &StructuredPortfolioDecision,
    action: &str,
) -> ScoreDimension {
    let entry = parse_first_number(&trader_plan.entry_price);
    let stop = parse_first_number(&trader_plan.stop_loss);
    let target = parse_first_number(&portfolio_decision.price_target);
    let confirmation = parse_first_number(&portfolio_decision.confirmation_level);
    let has_entry = entry.is_some();
    let has_stop = stop.is_some();
    let has_horizon = !portfolio_decision.time_horizon.trim().is_empty();

    let mut score = 0.0_f64;
    let mut rr_value: Option<f64> = None;
    let mut rr_status = "missing_fields";
    if let (Some(entry), Some(stop), Some(target)) = (entry, stop, target) {
        let rr = if action == "Buy" && target > entry && entry > stop {
            Some((target - entry) / (entry - stop))
        } else if action == "Sell" && stop > entry && entry > target {
            Some((entry - target) / (stop - entry))
        } else {
            None
        };

        if let Some(rr) = rr {
            // Continuous mapping: rr=1→~8, rr=2→~12, rr=3→~14, rr=4+→~15
            score = sigmoid(rr, 1.5, 1.5) * 15.0;
            rr_value = Some(rr);
            rr_status = "computed";
        } else {
            if action == "Hold" && has_horizon {
                score = 8.0;
            } else if has_entry && has_stop {
                score = 6.0;
            }
            rr_status = "cannot_close";
        }
    } else if let (Some(_entry), Some(_stop), Some(confirm)) = (entry, stop, confirmation) {
        if action == "Hold" && has_horizon {
            score = 9.0;
        } else if has_entry && has_stop {
            score = 7.0;
        }
        rr_value = Some(confirm);
        rr_status = "confirmation_used";
    } else {
        if action == "Hold" && has_horizon {
            score = 8.0;
        } else if has_entry && has_stop {
            score = 6.0;
        }
    };

    let mut rationale = LocalText::new("reward_to_risk_rationale")
        .with_str("action", if action.is_empty() { "not_extracted" } else { action })
        .with_str("rr_status", rr_status);
    if let Some(rr) = rr_value {
        rationale = rationale.with_f64("rr_value", rr);
    }

    ScoreDimension {
        score: score.clamp(0.0, 15.0) as i32,
        max_score: 15,
        rationale,
    }
}

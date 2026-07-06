
pub(super) fn technical_bias(indicators: &TechnicalIndicatorView) -> i32 {
    let mut bias: i32 = 0;
    for cat in &indicators.categories {
        for item in &cat.indicators {
            match item.key.as_str() {
                "macd" => match item.signal_code.as_str() {
                    "bullish_cross" => bias += 4,
                    "bearish_cross" => bias -= 4,
                    "above_zero" => bias += 1,
                    "below_zero" => bias -= 1,
                    _ => {}
                },
                "rsi" => match item.signal_code.as_str() {
                    "oversold" => bias += 3,
                    "overbought" => bias -= 3,
                    "neutral" => {
                        if let Some(v) = item.value {
                            if v < 40.0 { bias += 1; } else if v > 60.0 { bias -= 1; }
                        }
                    }
                    _ => {}
                },
                "kdj_k" => match item.signal_code.as_str() {
                    "bullish_cross" => bias += 3,
                    "bearish_cross" => bias -= 3,
                    "oversold" => bias += 1,
                    "overbought" => bias -= 1,
                    _ => {}
                },
                "cci" => match item.signal_code.as_str() {
                    "oversold" => bias += 2,
                    "overbought" => bias -= 2,
                    "neutral" => {
                        if let Some(v) = item.value {
                            if v < -100.0 { bias += 1; } else if v > 100.0 { bias -= 1; }
                        }
                    }
                    _ => {}
                },
                "boll_mid" => match item.signal_code.as_str() {
                    "above_reference" => bias += 1,
                    "below_reference" => bias -= 1,
                    _ => {}
                },
                "obv" => match item.signal_code.as_str() {
                    "volume_accumulation" => bias += 1,
                    "volume_distribution" => bias -= 1,
                    _ => {}
                },
                "wr" => match item.signal_code.as_str() {
                    "oversold" => bias += 1,
                    "overbought" => bias -= 1,
                    _ => {}
                },
                _ => {}
            }
        }
    }
    bias.clamp(-20, 20)
}

fn score_market_direction(
    analyst: Option<&AgentReportNode>,
    final_rating: &Rating,
    tech_indicators: &TechnicalIndicatorView,
) -> SignedScoreDimension {
    let analyst_score = score_analyst_net(analyst, 20);
    let tech = technical_bias(tech_indicators);
    let dampened_analyst = if tech.signum() == -analyst_score.signum()
        && tech.abs() >= 6
        && analyst_score.abs() >= 12
    {
        analyst_score * 6 / 10
    } else {
        analyst_score
    };
    let score = dampened_analyst + rating_bias(final_rating, 5) + tech;
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
    uniformity_flag: bool,
) -> ScoreDimension {
    let recommendation = &result.structured_portfolio_decision().rating;
    let recommendation_bias = semantic_direction(recommendation);
    let action = trader_plan.action.trim();
    let action_rating = Rating::parse(action);
    let action_bias = if action.is_empty() { 0 } else { semantic_direction(&action_rating) };
    let direction_bias = if direction_score >= 25 {
        1
    } else if direction_score <= -25 {
        -1
    } else {
        0
    };

    let mut score = 0;
    if action_bias == recommendation_bias {
        score += 14;
    } else if recommendation_bias == 0 && action_bias != 0 && action_bias == direction_bias {
        score += 6;
    }

    if action_bias == direction_bias {
        score += 10;
    } else if direction_bias == 0 && action_bias == 0 {
        score += 8;
    } else if direction_bias == 0 && action_bias == recommendation_bias {
        score += 4;
    }

    if confidence_score >= 75 && action_bias != 0 && action_bias == direction_bias {
        score += 8;
    } else if confidence_score < 55 && action_bias == 0 {
        score += 6;
    } else if (55..75).contains(&confidence_score)
        && (action_bias == 0 || action_bias == direction_bias)
    {
        score += 4;
    }

    // Penalize uniform outputs that suggest LLM isn't differentiating stocks
    if uniformity_flag {
        score = score.min(12);
    }

    ScoreDimension {
        score: score.clamp(0, 30),
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

    let mut score = 0;
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
            score = if rr >= 3.0 {
                15
            } else if rr >= 2.0 {
                13
            } else if rr >= 1.5 {
                11
            } else if rr >= 1.0 {
                8
            } else {
                4
            };
            rr_value = Some(rr);
            rr_status = "computed";
        } else {
            if action == "Hold" && has_horizon {
                score = 8;
            } else if has_entry && has_stop {
                score = 6;
            }
            rr_status = "cannot_close";
        }
    } else if let (Some(_entry), Some(_stop), Some(confirm)) = (entry, stop, confirmation) {
        if action == "Hold" && has_horizon {
            score = 9;
        } else if has_entry && has_stop {
            score = 7;
        }
        rr_value = Some(confirm);
        rr_status = "confirmation_used";
    } else {
        if action == "Hold" && has_horizon {
            score = 8;
        } else if has_entry && has_stop {
            score = 6;
        }
    };

    let mut rationale = LocalText::new("reward_to_risk_rationale")
        .with_str("action", if action.is_empty() { "not_extracted" } else { action })
        .with_str("rr_status", rr_status);
    if let Some(rr) = rr_value {
        rationale = rationale.with_f64("rr_value", rr);
    }

    ScoreDimension {
        score: score.clamp(0, 15),
        max_score: 15,
        rationale,
    }
}


fn derive_execution_confidence(
    _result: &AnalysisResult,
    trader_plan: &StructuredTraderPlan,
    portfolio_decision: &StructuredPortfolioDecision,
    execution_boundary: ExecutionBoundaryLevel,
) -> ScoreDimension {
    let mut score = 48.0_f64;
    match execution_boundary {
        ExecutionBoundaryLevel::Complete => score += 18.0,
        ExecutionBoundaryLevel::Partial => score += 10.0,
        ExecutionBoundaryLevel::Missing => {}
    }
    // Trigger checklists — sigmoid (replaces min(5)*N)
    score += sigmoid(trader_plan.execution_trigger_checklist.len() as f64, 3.0, 1.0) * 15.0;
    score += sigmoid(portfolio_decision.trigger_checklist.len() as f64, 3.0, 1.0) * 10.0;
    if !trader_plan.entry_price.trim().is_empty() {
        score += 8.0;
    }
    if !trader_plan.stop_loss.trim().is_empty() {
        score += 7.0;
    }
    if !portfolio_decision.time_horizon.trim().is_empty() {
        score += 5.0;
    }
    ScoreDimension {
        score: score.clamp(0.0, 100.0) as i32,
        max_score: 100,
        rationale: LocalText::new("execution_confidence_rationale"),
    }
}

fn derive_evidence_completeness(
    non_empty_core: usize,
    tool_failures: usize,
    fundamentals_diagnostics: &[ReportDiagnosticItem],
    research_plan: &StructuredResearchPlan,
    portfolio_decision: &StructuredPortfolioDecision,
) -> ScoreDimension {
    // Core desks — sigmoid (replaces linear * 18)
    let mut score = sigmoid(non_empty_core as f64, 3.0, 1.5) * 72.0;

    // Tool failures — exponential decay penalty
    score -= exponential_decay(tool_failures as f64, 2.0) * 16.0;

    if !research_plan.missing_evidence_ladder.manageable_gaps.is_empty() {
        score -= 10.0;
    }
    if !research_plan.missing_evidence_ladder.blocking_gaps.is_empty()
        || !portfolio_decision.missing_evidence_ladder.blocking_gaps.is_empty()
    {
        score -= 12.0;
    }
    if fundamentals_diagnostics
        .iter()
        .any(|item| item.code == "fundamentals_period_mixed")
    {
        score -= 10.0;
    }
    ScoreDimension {
        score: score.clamp(0.0, 100.0) as i32,
        max_score: 100,
        rationale: LocalText::new("evidence_completeness_rationale"),
    }
}

fn derive_historical_calibration(
    result: &AnalysisResult,
    historical_transferability: &ScoreDimension,
) -> ScoreDimension {
    let memory = &result.artifacts.memory_context;

    let score = if memory.setup_resolved_match_count == 0 {
        // No verified setup history — sigmoid based on seed samples
        let seed = (memory.same_ticker_count + memory.cross_ticker_count) as f64;
        sigmoid(seed, 2.0, 1.0) * 25.0 + 20.0
    } else if memory.used_setup_fallback_calibration {
        // Weak calibration — continuous
        let hit_rate_score = sigmoid(memory.setup_match_hit_rate, 0.5, 8.0) * 12.0;
        let alpha_bonus = if memory.setup_match_avg_alpha_return > 0.0 { 4.0 } else { 0.0 };
        historical_transferability.score as f64 * 5.0 + hit_rate_score + alpha_bonus
    } else {
        // Normal calibration — continuous
        let base = historical_transferability.score as f64 * 8.0;
        let hit_rate_bonus = sigmoid(memory.setup_match_hit_rate, 0.5, 8.0) * 20.0;
        let alpha_bonus = if memory.setup_match_avg_alpha_return > 0.0 { 10.0 } else { 0.0 };
        base + hit_rate_bonus + alpha_bonus
    };

    ScoreDimension {
        score: score.clamp(0.0, 100.0) as i32,
        max_score: 100,
        rationale: if memory.used_setup_fallback_calibration {
            LocalText::new("historical_calibration_fallback_rationale")
        } else {
            LocalText::new("historical_calibration_rationale")
        },
    }
}

pub fn evaluate_direction_score(result: &AnalysisResult) -> DirectionAssessment {
    let recommendation = result.structured_portfolio_decision().rating.clone();
    let market = score_market_direction(select_analyst(result, &["market"]), &recommendation);
    let fundamentals = score_analyst_direction(
        select_analyst(result, &["fundamentals", "fundamental"]),
        "direction_score_fundamentals",
        25,
    );
    let news = score_analyst_direction(select_analyst(result, &["news"]), "direction_score_news", 20);
    let sentiment = score_analyst_direction(select_analyst(result, &["sentiment"]), "direction_score_sentiment", 15);
    let risk_adjustment = score_risk_adjustment(result, &recommendation);

    let total_score = [
        market.score,
        fundamentals.score,
        news.score,
        sentiment.score,
        risk_adjustment.score,
    ]
    .into_iter()
    .sum::<i32>()
    .clamp(-100, 100);
    let implied_rating = LocalText::new(format!("rating_{}", map_direction_score_to_rating(total_score).to_string().to_ascii_lowercase()));

    DirectionAssessment {
        final_score: total_score,
        breakdown: DirectionBreakdown {
            market,
            fundamentals,
            news,
            sentiment,
            risk_adjustment,
            total_score,
            implied_rating,
        },
    }
}

pub fn evaluate_action_score(
    result: &AnalysisResult,
    trader_plan: &StructuredTraderPlan,
    portfolio_decision: &StructuredPortfolioDecision,
    direction_score: i32,
    confidence_score: i32,
) -> ActionAssessment {
    let alignment = score_action_alignment(result, trader_plan, direction_score, confidence_score);
    let execution_levels = score_execution_levels(trader_plan, portfolio_decision);
    let sizing_discipline = score_sizing_discipline(trader_plan);
    let horizon_clarity = score_horizon_clarity(portfolio_decision);
    let reward_to_risk =
        score_reward_to_risk(trader_plan, portfolio_decision, trader_plan.action.trim());

    let total_score = [
        alignment.score,
        execution_levels.score,
        sizing_discipline.score,
        horizon_clarity.score,
        reward_to_risk.score,
    ]
    .into_iter()
    .sum::<i32>()
    .clamp(0, 100);

    ActionAssessment {
        final_score: total_score,
        breakdown: ActionBreakdown {
            alignment,
            execution_levels,
            sizing_discipline,
            horizon_clarity,
            reward_to_risk,
            total_score,
        },
    }
}

#[cfg(test)]
pub fn calibrate_recommendation(
    raw_llm_recommendation: &str,
    direction_score: i32,
    confidence_score: i32,
    action_score: i32,
    execution_boundary_complete: bool,
) -> RecommendationCalibration {
    let level = if execution_boundary_complete {
        ExecutionBoundaryLevel::Complete
    } else {
        ExecutionBoundaryLevel::Missing
    };
    calibrate_recommendation_with_profile(
        raw_llm_recommendation,
        direction_score,
        confidence_score,
        action_score,
        level,
        &CalibrationProfile::default(),
        0,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn calibrate_recommendation_with_profile(
    raw_llm_recommendation: &str,
    direction_score: i32,
    confidence_score: i32,
    action_score: i32,
    execution_boundary: ExecutionBoundaryLevel,
    profile: &CalibrationProfile,
    direction_threshold_penalty: i32,
    reward_risk_hint: Option<f64>,
) -> RecommendationCalibration {
    let raw_rating = Rating::parse(raw_llm_recommendation);
    let raw_score = rating_to_score(&raw_rating);
    let evidence_score = direction_score_to_evidence_score(direction_score);
    let history_requires_caution = history_requires_caution(profile);
    let direction_floor_abs = if history_requires_caution {
        (profile.direction_floor_abs + 5).min(profile.strong_direction_abs)
    } else {
        profile.direction_floor_abs
    }
    + direction_threshold_penalty;
    let direction_floor_abs = direction_floor_abs.clamp(12, 85);
    // When reward/risk is excellent, relax the direction floor so that
    // great risk/reward structures aren't blocked by moderate direction scores.
    let direction_floor_abs = if let Some(rr) = reward_risk_hint {
        if rr >= 5.0 && confidence_score >= 30 {
            (direction_floor_abs - 8).max(12)
        } else if rr >= 3.0 && confidence_score >= 30 {
            (direction_floor_abs - 4).max(12)
        } else {
            direction_floor_abs
        }
    } else {
        direction_floor_abs
    };
    let strong_direction_abs = if history_requires_caution {
        (profile.strong_direction_abs + 5).min(85)
    } else {
        profile.strong_direction_abs
    }
    + direction_threshold_penalty;
    let strong_direction_abs = strong_direction_abs.clamp(24, 90);
    let effective_action_score = match execution_boundary {
        ExecutionBoundaryLevel::Complete => action_score,
        ExecutionBoundaryLevel::Partial => action_score.min(profile.min_action_score + 25),
        ExecutionBoundaryLevel::Missing => action_score.min(profile.min_action_score + 15),
    };

    let final_score = if confidence_score < profile.min_confidence_score
        || effective_action_score < profile.min_action_score
    {
        0
    } else if execution_boundary == ExecutionBoundaryLevel::Missing {
        // Missing execution boundary forces Hold.
        0
    } else if confidence_score >= (profile.min_confidence_score + 15).min(75)
        && effective_action_score >= (profile.min_action_score + 15).min(70)
        && direction_score.abs() >= strong_direction_abs
    {
        evidence_score
    } else if confidence_score >= (profile.min_confidence_score + 3)
        && effective_action_score >= (profile.min_action_score + 10)
        && direction_score.abs() >= direction_floor_abs
    {
        evidence_score.signum()
    } else {
        0
    };
    let final_rating = score_to_rating(final_score);
    let final_action = rating_to_action(&final_rating);
    let direction_view_key = if direction_score >= strong_direction_abs {
        "direction_strongly_bullish"
    } else if direction_score >= direction_floor_abs {
        "direction_mildly_bullish"
    } else if direction_score <= -strong_direction_abs {
        "direction_strongly_bearish"
    } else if direction_score <= -direction_floor_abs {
        "direction_mildly_bearish"
    } else {
        "direction_unclear"
    };
    let execution_view_key = if execution_boundary.is_complete()
        && confidence_score >= (profile.min_confidence_score + 3)
        && effective_action_score >= (profile.min_action_score + 10)
    {
        "execution_ready"
    } else if execution_boundary.is_at_least_partial() {
        "execution_partial"
    } else {
        "execution_incomplete"
    };
    let raw_rating_str = if raw_llm_recommendation.is_empty() {
        "not_extracted"
    } else {
        raw_llm_recommendation
    };
    let rationale = LocalText::new("calibration_rationale")
        .with_str("raw_rating", raw_rating_str)
        .with_i32("direction_score", direction_score)
        .with_i32("confidence_score", confidence_score)
        .with_i32("action_score", action_score)
        .with_bool("execution_boundary_complete", execution_boundary.is_complete())
        .with_str("direction_view", direction_view_key)
        .with_str("execution_view", execution_view_key)
        .with_bool("rating_calibrated", raw_score != final_score)
        .with_str("final_rating", final_rating.to_string())
        .with_bool("execution_blocks_upgrade", !execution_boundary.is_complete() && final_rating == Rating::Hold)
        .with_bool("history_caution", history_requires_caution);
    let decision_narrative = if final_rating == Rating::Hold {
        if direction_score.abs() >= direction_floor_abs
            && confidence_score >= profile.min_confidence_score
            && effective_action_score >= profile.min_action_score
        {
            if direction_score > 0 {
                LocalText::new("hold_narrative_bullish_direction")
            } else {
                LocalText::new("hold_narrative_bearish_direction")
            }
        } else if direction_score.abs() >= direction_floor_abs {
            LocalText::new("hold_narrative_direction_present")
        } else {
            LocalText::new("hold_narrative_no_clear_direction")
        }
    } else if final_score.abs() >= 2 {
        if final_score > 0 {
            LocalText::new("active_bullish_narrative")
        } else {
            LocalText::new("active_bearish_narrative")
        }
    } else {
        LocalText::new("mild_direction_narrative")
    };

    RecommendationCalibration {
        final_rating: final_rating.to_string(),
        final_action: final_action.to_string(),
        rationale,
        decision_narrative,
    }
}

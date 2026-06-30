
fn derive_execution_confidence(
    result: &AnalysisResult,
    trader_plan: &StructuredTraderPlan,
    portfolio_decision: &StructuredPortfolioDecision,
    execution_boundary_complete: bool,
) -> ScoreDimension {
    let mut score = 20;
    if execution_boundary_complete {
        score += 18;
    }
    score += trader_plan.execution_trigger_checklist.len().min(5) as i32 * 3;
    score += portfolio_decision.trigger_checklist.len().min(5) as i32 * 2;
    if !trader_plan.entry_price.trim().is_empty() {
        score += 8;
    }
    if !trader_plan.stop_loss.trim().is_empty() {
        score += 7;
    }
    if !portfolio_decision.time_horizon.trim().is_empty() {
        score += 5;
    }
    if result
        .structured_portfolio_decision()
        .rating == Rating::Hold
        && !execution_boundary_complete
    {
        score = score.min(65);
    }
    ScoreDimension {
        score: score.clamp(0, 100),
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
    let mut score = (non_empty_core as i32) * 18 - (tool_failures as i32 * 4);
    if !research_plan.missing_evidence_ladder.manageable_gaps.is_empty() {
        score -= 10;
    }
    if !research_plan.missing_evidence_ladder.blocking_gaps.is_empty()
        || !portfolio_decision.missing_evidence_ladder.blocking_gaps.is_empty()
    {
        score -= 12;
    }
    if fundamentals_diagnostics
        .iter()
        .any(|item| item.code == "fundamentals_period_mixed")
    {
        score -= 10;
    }
    ScoreDimension {
        score: score.clamp(0, 100),
        max_score: 100,
        rationale: LocalText::new("evidence_completeness_rationale"),
    }
}

fn derive_historical_calibration(
    result: &AnalysisResult,
    historical_transferability: &ScoreDimension,
) -> ScoreDimension {
    let memory = &result.artifacts.memory_context;
    let mut score = historical_transferability.score * 8;
    if memory.setup_resolved_match_count == 0 {
        // No verified setup history — give a modest base score when seed samples exist
        // instead of zeroing out, which would crush the overall confidence to single digits.
        score = if memory.same_ticker_count > 0 || memory.cross_ticker_count > 0 {
            25
        } else {
            20
        };
    } else if memory.used_setup_fallback_calibration {
        score = (historical_transferability.score * 5)
            + (memory.setup_match_hit_rate * 12.0).round() as i32
            + if memory.setup_match_avg_alpha_return > 0.0 {
                4
            } else {
                0
            };
    } else {
        score += (memory.setup_match_hit_rate * 20.0).round() as i32;
        if memory.setup_match_avg_alpha_return > 0.0 {
            score += 10;
        }
    }
    ScoreDimension {
        score: score.clamp(0, 100),
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
    // When the LLM outputs Hold/Unknown, consider debate results AND all analyst
    // probabilities (not just market) to determine direction.
    let direction_rating = if matches!(recommendation, Rating::Hold | Rating::Unknown) {
        // Count bull/bear debate turns
        let bull_turns = result.graph.investment_debate.turns.iter()
            .filter(|t| t.stance == "bull").count();
        let bear_turns = result.graph.investment_debate.turns.iter()
            .filter(|t| t.stance == "bear").count();
        let debate_bias = bull_turns as i32 - bear_turns as i32;

        // Weighted analyst probabilities
        let market_net = select_analyst(result, &["market"])
            .map(|a| a.up_probability - a.down_probability).unwrap_or(0.0);
        let fund_net = select_analyst(result, &["fundamentals", "fundamental"])
            .map(|a| a.up_probability - a.down_probability).unwrap_or(0.0);
        let news_net = select_analyst(result, &["news"])
            .map(|a| a.up_probability - a.down_probability).unwrap_or(0.0);
        let sent_net = select_analyst(result, &["sentiment"])
            .map(|a| a.up_probability - a.down_probability).unwrap_or(0.0);

        // Weighted average: market 40%, fundamentals 30%, news 15%, sentiment 15%
        let weighted_net = market_net * 0.4 + fund_net * 0.3
            + news_net * 0.15 + sent_net * 0.15;

        // Debate can shift the threshold slightly
        let threshold = 0.15 - (debate_bias as f64 * 0.02).clamp(-0.05, 0.05);

        if weighted_net >= threshold {
            Rating::Buy
        } else if weighted_net <= -threshold {
            Rating::Sell
        } else {
            Rating::Hold
        }
    } else {
        recommendation.clone()
    };
    let market = score_market_direction(select_analyst(result, &["market"]), &direction_rating);
    let fundamentals = score_analyst_direction(
        select_analyst(result, &["fundamentals", "fundamental"]),
        "direction_score_fundamentals",
        25,
    );
    let news = score_analyst_direction(select_analyst(result, &["news"]), "direction_score_news", 20);
    let sentiment = score_analyst_direction(select_analyst(result, &["sentiment"]), "direction_score_sentiment", 15);
    let risk_adjustment = score_risk_adjustment(result, &direction_rating);

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
    let alignment = score_action_alignment(result, trader_plan, direction_score, confidence_score, false); // TODO: pass actual uniformity_flag
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

pub fn calibrate_recommendation(
    raw_llm_recommendation: &str,
    direction_score: i32,
    confidence_score: i32,
    action_score: i32,
    execution_boundary_complete: bool,
) -> RecommendationCalibration {
    calibrate_recommendation_with_profile(
        raw_llm_recommendation,
        direction_score,
        confidence_score,
        action_score,
        execution_boundary_complete,
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
    execution_boundary_complete: bool,
    profile: &CalibrationProfile,
    direction_threshold_penalty: i32,
    reward_risk_hint: Option<f64>,
) -> RecommendationCalibration {
    let raw_rating = Rating::parse(raw_llm_recommendation);
    let raw_score = rating_to_score(&raw_rating);
    let pm_says_hold = matches!(raw_rating, Rating::Hold);
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
    let effective_action_score = if execution_boundary_complete {
        action_score
    } else {
        action_score.min(profile.min_action_score + 15)
    };

    // Respect PM's Hold decision when direction is not extreme.
    // Only override Hold when direction is very strong (>= 50).
    let final_score = if pm_says_hold && direction_score.abs() < 50 {
        // PM said Hold and direction is not extreme — respect the decision
        0
    } else if confidence_score < profile.min_confidence_score - 15
        || effective_action_score < profile.min_action_score - 10
    {
        // Very low confidence or action — Hold regardless of direction
        0
    } else if confidence_score >= (profile.min_confidence_score + 25).min(85)
        && effective_action_score >= (profile.min_action_score + 30).min(85)
        && direction_score.abs() >= strong_direction_abs
    {
        if execution_boundary_complete {
            evidence_score
        } else {
            evidence_score.signum()
        }
    } else if confidence_score >= (profile.min_confidence_score + 3)
        && effective_action_score >= (profile.min_action_score + 10)
        && direction_score.abs() >= direction_floor_abs
    {
        if pm_says_hold {
            0 // Respect PM's Hold for moderate direction
        } else if execution_boundary_complete {
            evidence_score.signum()
        } else {
            evidence_score.signum()
        }
    } else if confidence_score >= profile.min_confidence_score - 15
        && effective_action_score >= profile.min_action_score - 10
        && direction_score.abs() >= strong_direction_abs
    {
        if pm_says_hold {
            0 // Respect PM's Hold even for strong direction if confidence is low
        } else {
            evidence_score.signum()
        }
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
    let execution_view_key = if execution_boundary_complete
        && confidence_score >= (profile.min_confidence_score + 3)
        && effective_action_score >= (profile.min_action_score + 10)
    {
        "execution_ready"
    } else if execution_boundary_complete {
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
        .with_bool("execution_boundary_complete", execution_boundary_complete)
        .with_str("direction_view", direction_view_key)
        .with_str("execution_view", execution_view_key)
        .with_bool("rating_calibrated", raw_score != final_score)
        .with_str("final_rating", final_rating.to_string())
        .with_bool("execution_blocks_upgrade", !execution_boundary_complete && final_rating == Rating::Hold)
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

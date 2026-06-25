
pub fn derive_calibration_bias(
    memory_context: &MemoryContextSnapshot,
    memory_threshold_tightened: bool,
    memory_direction_misaligned: bool,
    positive_setup_support: bool,
) -> CalibrationBias {
    if memory_direction_misaligned {
        CalibrationBias {
            direction: LocalText::new("negative"),
            magnitude: LocalText::new("high"),
            rationale: LocalText::new("calibration_bias_misaligned"),
        }
    } else if memory_threshold_tightened {
        CalibrationBias {
            direction: LocalText::new("negative"),
            magnitude: LocalText::new("medium"),
            rationale: LocalText::new("calibration_bias_threshold_tightened"),
        }
    } else if positive_setup_support {
        CalibrationBias {
            direction: LocalText::new("positive"),
            magnitude: LocalText::new("low"),
            rationale: LocalText::new("calibration_bias_positive_support")
                .with_i32("count", memory_context.setup_resolved_match_count as i32)
                .with_f64("hit_rate", memory_context.setup_match_hit_rate * 100.0)
                .with_f64("avg_alpha", memory_context.setup_match_avg_alpha_return * 100.0),
        }
    } else {
        CalibrationBias {
            direction: LocalText::new("neutral"),
            magnitude: LocalText::new("low"),
            rationale: LocalText::new("calibration_bias_neutral"),
        }
    }
}


pub fn fallback_sizing_reference(existing: &str, rating: &Rating, blocker_present: bool) -> LocalText {
    if blocker_present {
        return LocalText::new("sizing_reference_blockers");
    }
    let trimmed = existing.trim();
    if !trimmed.is_empty() {
        return LocalText::new("sizing_reference_from_plan").with_str("sizing", trimmed);
    }
    if rating.is_bullish() {
        LocalText::new("sizing_reference_bullish")
    } else if rating.is_bearish() {
        LocalText::new("sizing_reference_bearish")
    } else {
        LocalText::new("sizing_reference_neutral")
    }
}

pub fn derive_action_guides(
    _result: &AnalysisResult,
    research_plan: &StructuredResearchPlan,
    trader_plan: &StructuredTraderPlan,
    portfolio_decision: &StructuredPortfolioDecision,
    _profile: &ConfidenceProfile,
    caps: &[ConfidenceCap],
) -> ReportActionGuides {
    let weak_history = caps.iter().any(|cap| {
        matches!(
            cap.key.as_str(),
            "thin_setup_history" | "zero_resolved_setup_history" | "execution_boundary_missing"
        )
    });
    let blocker_present = !portfolio_decision
        .missing_evidence_ladder
        .blocking_gaps
        .is_empty()
        || !trader_plan.blocking_gaps.is_empty();
    let rating = &portfolio_decision.rating;
    let sizing_ref = fallback_sizing_reference(&trader_plan.position_sizing, rating, blocker_present);
    let review_points = collect_key_review_points(research_plan, trader_plan, portfolio_decision);

    let entry_ref = trader_plan.entry_price.trim().to_string();
    let invalidation_ref = visible_invalidation_reference(portfolio_decision, Some(trader_plan))
        .unwrap_or_default();
    let target_ref = visible_target_reference(portfolio_decision).unwrap_or_default();
    let confirmation_ref = visible_confirmation_reference(portfolio_decision).unwrap_or_default();
    let time_horizon = portfolio_decision.time_horizon.trim().to_string();

    let holder_actions = build_holder_actions(trader_plan, portfolio_decision, blocker_present);
    let holder_paths = build_scenario_paths(trader_plan, portfolio_decision, "holder", blocker_present, weak_history);
    let holders = AudienceActionGuide {
        audience: LocalText::new("audience_holders"),
        user_state: LocalText::new("user_state_holders"),
        priority: "high".to_string(),
        stance: if rating.is_bullish() {
            LocalText::new("stance_holder_bullish")
        } else if rating.is_bearish() {
            LocalText::new("stance_holder_bearish")
        } else {
            LocalText::new("stance_holder_neutral")
        },
        summary: if weak_history {
            LocalText::new("summary_holders_weak")
        } else if blocker_present {
            LocalText::new("summary_holders_blocker")
        } else if rating.is_bullish() {
            LocalText::new("summary_holders_bullish")
        } else if rating.is_bearish() {
            LocalText::new("summary_holders_bearish")
        } else {
            LocalText::new("summary_holders_neutral")
        },
        principle: LocalText::new("principle_holder"),
        entry_reference: entry_ref.clone(),
        invalidation_reference: invalidation_ref.clone(),
        target_reference: target_ref.clone(),
        confirmation_reference: confirmation_ref.clone(),
        time_horizon: time_horizon.clone(),
        sizing_reference: sizing_ref.clone(),
        actions: holder_actions,
        avoid: Vec::new(),
        review_points: review_points.clone(),
        scenario_paths: holder_paths,
    };

    let buyer_actions = build_buyer_actions(trader_plan, portfolio_decision, blocker_present);
    let buyer_paths = build_scenario_paths(trader_plan, portfolio_decision, "buyer", blocker_present, weak_history);
    let buyers = AudienceActionGuide {
        audience: LocalText::new("audience_buyers"),
        user_state: LocalText::new("user_state_buyers"),
        priority: "medium".to_string(),
        stance: if rating.is_bullish() {
            LocalText::new("stance_buyer_bullish")
        } else if rating.is_bearish() {
            LocalText::new("stance_buyer_bearish")
        } else {
            LocalText::new("stance_buyer_neutral")
        },
        summary: if weak_history {
            LocalText::new("summary_buyers_weak")
        } else if blocker_present {
            LocalText::new("summary_buyers_blocker")
        } else if rating.is_bullish() {
            LocalText::new("summary_buyers_bullish")
        } else if rating.is_bearish() {
            LocalText::new("summary_buyers_bearish")
        } else {
            LocalText::new("summary_buyers_neutral")
        },
        principle: LocalText::new("principle_buyer"),
        entry_reference: entry_ref.clone(),
        invalidation_reference: invalidation_ref.clone(),
        target_reference: target_ref.clone(),
        confirmation_reference: confirmation_ref.clone(),
        time_horizon: time_horizon.clone(),
        sizing_reference: sizing_ref.clone(),
        actions: buyer_actions,
        avoid: Vec::new(),
        review_points: review_points.clone(),
        scenario_paths: buyer_paths,
    };

    let watcher_actions = build_watcher_actions(research_plan, portfolio_decision, weak_history);
    let watcher_paths = build_scenario_paths(trader_plan, portfolio_decision, "watcher", blocker_present, weak_history);
    let watchers = AudienceActionGuide {
        audience: LocalText::new("audience_watchers"),
        user_state: LocalText::new("user_state_watchers"),
        priority: "low".to_string(),
        stance: if rating.is_bullish() {
            LocalText::new("stance_watcher_bullish")
        } else if rating.is_bearish() {
            LocalText::new("stance_watcher_bearish")
        } else {
            LocalText::new("stance_watcher_neutral")
        },
        summary: if weak_history {
            LocalText::new("summary_watchers_weak")
        } else if blocker_present {
            LocalText::new("summary_watchers_blocker")
        } else if rating.is_bullish() {
            LocalText::new("summary_watchers_bullish")
        } else if rating.is_bearish() {
            LocalText::new("summary_watchers_bearish")
        } else {
            LocalText::new("summary_watchers_neutral")
        },
        principle: LocalText::new("principle_watcher"),
        entry_reference: entry_ref,
        invalidation_reference: invalidation_ref,
        target_reference: target_ref,
        confirmation_reference: confirmation_ref,
        time_horizon,
        sizing_reference: sizing_ref,
        actions: watcher_actions,
        avoid: Vec::new(),
        review_points,
        scenario_paths: watcher_paths,
    };

    ReportActionGuides {
        holders,
        buyers,
        watchers,
    }
}

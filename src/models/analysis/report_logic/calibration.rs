
fn derive_calibration_bias(
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


fn fallback_sizing_reference(existing: &str, rating: &Rating, blocker_present: bool) -> LocalText {
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

fn derive_action_guides(
    _result: &AnalysisResult,
    research_plan: &StructuredResearchPlan,
    trader_plan: &StructuredTraderPlan,
    portfolio_decision: &StructuredPortfolioDecision,
    confidence_profile: &ConfidenceProfile,
    confidence_caps: &[ConfidenceCap],
) -> ReportActionGuides {
    let rating = portfolio_decision.rating.clone();
    let blocker_present = !portfolio_decision
        .missing_evidence_ladder
        .blocking_gaps
        .is_empty()
        || !research_plan
            .missing_evidence_ladder
            .blocking_gaps
            .is_empty();
    let weak_history = confidence_caps
        .iter()
        .any(|item| item.key == "zero_resolved_setup_history" || item.key == "thin_setup_history");
    let key_review_points =
        collect_key_review_points(research_plan, trader_plan, portfolio_decision);
    let invalidation_reference =
        visible_invalidation_reference(portfolio_decision, Some(trader_plan)).unwrap_or_default();
    let holder_stance = if rating == Rating::Hold {
        "stance_holder_hold"
    } else if rating.is_bullish() {
        "stance_holder_bullish"
    } else {
        "stance_holder_bearish"
    };
    let buyer_stance = if blocker_present || confidence_profile.execution_confidence.score < 65 {
        "stance_buyer_blocked"
    } else {
        "stance_buyer_allowed"
    };
    let watcher_stance = if weak_history {
        "stance_watcher_weak_history"
    } else {
        "stance_watcher_normal"
    };

    ReportActionGuides {
        holders: AudienceActionGuide {
            audience: LocalText::new("audience_holders"),
            user_state: LocalText::new("user_state_holders"),
            priority: if blocker_present {
                "高".to_string()
            } else {
                "中".to_string()
            },
            stance: LocalText::new(holder_stance),
            summary: LocalText::new("summary_holders"),
            principle: LocalText::new("principle_holders"),
            entry_reference: trader_plan.entry_price.trim().to_string(),
            invalidation_reference: invalidation_reference.clone(),
            target_reference: visible_target_reference(portfolio_decision).unwrap_or_default(),
            confirmation_reference: visible_confirmation_reference(portfolio_decision)
                .unwrap_or_default(),
            time_horizon: portfolio_decision.time_horizon.trim().to_string(),
            sizing_reference: fallback_sizing_reference(&trader_plan.position_sizing, &rating, blocker_present),
            actions: build_holder_actions(trader_plan, portfolio_decision, blocker_present),
            avoid: vec![
                LocalText::new("avoid_holder_chase"),
                LocalText::new("avoid_holder_ignore_finance"),
            ],
            review_points: key_review_points.clone(),
            scenario_paths: build_scenario_paths(trader_plan, portfolio_decision, "holder", blocker_present, weak_history),
        },
        buyers: AudienceActionGuide {
            audience: LocalText::new("audience_buyers"),
            user_state: LocalText::new("user_state_buyers"),
            priority: if blocker_present || confidence_profile.execution_confidence.score < 65 {
                "高".to_string()
            } else {
                "中".to_string()
            },
            stance: LocalText::new(buyer_stance),
            summary: LocalText::new("summary_buyers"),
            principle: LocalText::new("principle_buyers"),
            entry_reference: trader_plan.entry_price.trim().to_string(),
            invalidation_reference: invalidation_reference.clone(),
            target_reference: visible_target_reference(portfolio_decision).unwrap_or_default(),
            confirmation_reference: visible_confirmation_reference(portfolio_decision)
                .unwrap_or_default(),
            time_horizon: portfolio_decision.time_horizon.trim().to_string(),
            sizing_reference: fallback_sizing_reference(&trader_plan.position_sizing, &rating, blocker_present),
            actions: build_buyer_actions(trader_plan, portfolio_decision, blocker_present),
            avoid: vec![
                LocalText::new("avoid_buyer_misread_trend"),
                LocalText::new("avoid_buyer_no_confirmation"),
            ],
            review_points: key_review_points.clone(),
            scenario_paths: build_scenario_paths(trader_plan, portfolio_decision, "buyer", blocker_present, weak_history),
        },
        watchers: AudienceActionGuide {
            audience: LocalText::new("audience_watchers"),
            user_state: LocalText::new("user_state_watchers"),
            priority: if weak_history { "高".to_string() } else { "中".to_string() },
            stance: LocalText::new(watcher_stance),
            summary: if weak_history {
                LocalText::new("summary_watchers_weak")
            } else {
                LocalText::new("summary_watchers_normal")
            },
            principle: LocalText::new("principle_watchers"),
            entry_reference: trader_plan.entry_price.trim().to_string(),
            invalidation_reference,
            target_reference: visible_target_reference(portfolio_decision).unwrap_or_default(),
            confirmation_reference: visible_confirmation_reference(portfolio_decision)
                .unwrap_or_default(),
            time_horizon: portfolio_decision.time_horizon.trim().to_string(),
            sizing_reference: fallback_sizing_reference(&trader_plan.position_sizing, &rating, blocker_present),
            actions: build_watcher_actions(research_plan, portfolio_decision, weak_history),
            avoid: vec![
                LocalText::new("avoid_watcher_unvalidated_setup"),
                LocalText::new("avoid_watcher_unverified_finance"),
            ],
            review_points: key_review_points,
            scenario_paths: build_scenario_paths(trader_plan, portfolio_decision, "watcher", blocker_present, weak_history),
        },
    }
}

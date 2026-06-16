
pub fn evaluate_confidence_score(result: &AnalysisResult) -> ConfidenceAssessment {
    let research_plan = result.structured_research_plan();
    let trader_plan = result.structured_trader_plan();
    let portfolio_decision = result.structured_portfolio_decision();
    let core_reports = [
        result.agent_state.market_report.as_str(),
        result.agent_state.fundamentals_report.as_str(),
        result.agent_state.news_report.as_str(),
        result.agent_state.sentiment_report.as_str(),
    ];
    let non_empty_core = core_reports
        .iter()
        .filter(|item| !item.trim().is_empty())
        .count();
    let tool_successes = result
        .artifacts
        .analyst_runtime_states
        .iter()
        .flat_map(|state| state.tool_history.iter())
        .filter(|item| item.success)
        .count();
    let tool_failures = result
        .artifacts
        .analyst_runtime_states
        .iter()
        .flat_map(|state| state.tool_history.iter())
        .filter(|item| !item.success)
        .count();
    let analyst_count = result.graph.analysts.len();
    let evidence_density = average_evidence_density(&result.graph.analysts);
    let execution_boundary = has_execution_boundary(&trader_plan, &portfolio_decision);
    let next_steps_count = result
        .graph
        .analysts
        .iter()
        .map(|item| item.next_steps.len())
        .sum::<usize>();

    let data_quality =
        score_data_quality(non_empty_core, analyst_count, tool_successes, tool_failures);
    let trend_confirmation = score_trend_confirmation(
        select_analyst(result, &["market"]),
        &result.agent_state.market_report,
        &trader_plan,
        &portfolio_decision,
    );
    let fundamental_confirmation = score_fundamentals(
        select_analyst(result, &["fundamentals", "fundamental"]),
        &result.agent_state.fundamentals_report,
    );
    let catalyst_quality = score_catalyst_quality(
        select_analyst(result, &["news"]),
        &result.agent_state.news_report,
        &portfolio_decision,
    );
    let historical_transferability = score_historical_transferability(result);
    let cross_agent_consistency = score_cross_agent_consistency(result);
    let risk_clarity =
        score_risk_clarity(result, &research_plan, &trader_plan, &portfolio_decision);
    let diagnostics = crate::models::derive_report_diagnostics(result);

    let total_before_caps = [
        data_quality.score,
        trend_confirmation.score,
        fundamental_confirmation.score,
        catalyst_quality.score,
        historical_transferability.score,
        cross_agent_consistency.score,
        risk_clarity.score,
    ]
    .into_iter()
    .sum::<i32>();
    let direction_confidence = derive_direction_confidence(
        result,
        &trend_confirmation,
        &fundamental_confirmation,
        &catalyst_quality,
        &cross_agent_consistency,
    );
    let execution_confidence = derive_execution_confidence(
        result,
        &trader_plan,
        &portfolio_decision,
        execution_boundary,
    );
    let evidence_completeness = derive_evidence_completeness(
        non_empty_core,
        tool_failures,
        &diagnostics.fundamentals,
        &research_plan,
        &portfolio_decision,
    );
    let (historical_calibration, has_history_data) =
        derive_historical_calibration(result, &historical_transferability);

    let mut caps = Vec::new();
    if non_empty_core < 3 || tool_failures >= 2 {
        caps.push(ConfidenceCap {
            key: "missing_core_data".to_string(),
            label: LocalText::new("cap_label_missing_core_data"),
            cap: if non_empty_core < 3 { 80 } else { 82 },
            reason: LocalText::new("missing_core_data_reason")
                .with_i32("core_count", non_empty_core as i32)
                .with_i32("total", 4)
                .with_i32("tool_failures", tool_failures as i32),
        });
    }
    if evidence_density < 1.5 {
        caps.push(ConfidenceCap {
            key: "thin_evidence_density".to_string(),
            label: LocalText::new("cap_label_thin_evidence_density"),
            cap: 82,
            reason: LocalText::new("thin_evidence_density_reason")
                .with_f64("evidence_density", evidence_density),
        });
    }
    if execution_boundary == ExecutionBoundaryLevel::Missing {
        caps.push(ConfidenceCap {
            key: "execution_boundary_missing".to_string(),
            label: LocalText::new("cap_label_execution_boundary_missing"),
            cap: 83,
            reason: LocalText::new("execution_boundary_missing_reason"),
        });
    } else if execution_boundary == ExecutionBoundaryLevel::Partial {
        caps.push(ConfidenceCap {
            key: "execution_boundary_partial".to_string(),
            label: LocalText::new("cap_label_execution_boundary_missing"),
            cap: 90,
            reason: LocalText::new("execution_boundary_missing_reason"),
        });
    }
    if cross_agent_consistency.score <= 8 {
        caps.push(ConfidenceCap {
            key: "cross_agent_divergence".to_string(),
            label: LocalText::new("cap_label_cross_agent_divergence"),
            cap: 85,
            reason: LocalText::new("cross_agent_divergence_reason")
                .with_str("consistency_detail", &cross_agent_consistency.rationale.key),
        });
    }
    if next_steps_count == 0 {
        caps.push(ConfidenceCap {
            key: "missing_follow_up_plan".to_string(),
            label: LocalText::new("cap_label_missing_follow_up_plan"),
            cap: 82,
            reason: LocalText::new("missing_follow_up_plan_reason"),
        });
    }
    if !portfolio_decision
        .missing_evidence_ladder
        .blocking_gaps
        .is_empty()
        || !research_plan.missing_evidence_ladder.blocking_gaps.is_empty()
    {
        caps.push(ConfidenceCap {
            key: "decision_blocking_gaps_present".to_string(),
            label: LocalText::new("cap_label_decision_blocking_gaps_present"),
            cap: 82,
            reason: LocalText::new("decision_blocking_gaps_present_reason"),
        });
    }
    if diagnostics
        .fundamentals
        .iter()
        .any(|item| item.code == "fundamentals_period_mixed")
    {
        caps.push(ConfidenceCap {
            key: "fundamentals_period_mixed".to_string(),
            label: LocalText::new("cap_label_fundamentals_period_mixed"),
            cap: 80,
            reason: LocalText::new("fundamentals_period_mixed_reason"),
        });
    }
    if result
        .structured_portfolio_decision()
        .missing_evidence_ladder
        .blocking_gaps
        .iter()
        .any(|gap| {
            let lower = gap.to_lowercase();
            gap.contains("突破") || gap.contains("催化")
                || lower.contains("breakout") || lower.contains("catalyst")
                || lower.contains("resistance")
        })
    {
        caps.push(ConfidenceCap {
            key: "near_resistance_without_fresh_catalyst".to_string(),
            label: LocalText::new("cap_label_near_resistance_without_fresh_catalyst"),
            cap: 80,
            reason: LocalText::new("near_resistance_without_fresh_catalyst_reason"),
        });
    }
    let applied_cap = caps.iter().map(|item| item.cap).min().unwrap_or(100);
    // Dynamic weight: when no history data, redistribute its 20% to other dimensions.
    let (dir_w, exec_w, evid_w, hist_w) = if has_history_data {
        (0.30, 0.25, 0.25, 0.20)
    } else {
        (0.38, 0.31, 0.31, 0.0)
    };
    let pre_cap_score = (direction_confidence.score as f64 * dir_w
        + execution_confidence.score as f64 * exec_w
        + evidence_completeness.score as f64 * evid_w
        + historical_calibration.score as f64 * hist_w)
        .round() as i32;
    let final_score = pre_cap_score.clamp(0, applied_cap);

    ConfidenceAssessment {
        final_score,
        breakdown: ConfidenceBreakdown {
            data_quality,
            trend_confirmation,
            fundamental_confirmation,
            catalyst_quality,
            historical_transferability,
            cross_agent_consistency,
            risk_clarity,
            total_before_caps,
            final_score,
            applied_cap,
        },
        profile: ConfidenceProfile {
            direction_confidence,
            execution_confidence,
            evidence_completeness,
            historical_calibration,
            total_confidence: final_score,
            methodology: LocalText::new("confidence_methodology"),
        },
        caps,
    }
}

fn derive_direction_confidence(
    _result: &AnalysisResult,
    trend_confirmation: &ScoreDimension,
    fundamental_confirmation: &ScoreDimension,
    catalyst_quality: &ScoreDimension,
    cross_agent_consistency: &ScoreDimension,
) -> ScoreDimension {
    // Use full weights regardless of raw LLM recommendation.
    // The raw rating is already evaluated separately in calibration;
    // penalizing confidence based on it creates a self-reinforcing Hold loop.
    let score = trend_confirmation.score
        + fundamental_confirmation.score
        + catalyst_quality.score
        + cross_agent_consistency.score;
    ScoreDimension {
        score: score.clamp(0, 100),
        max_score: 100,
        rationale: LocalText::new("direction_confidence_rationale"),
    }
}

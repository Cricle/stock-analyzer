
/// Parameters for [`build_overview_section`].
pub(crate) struct OverviewSectionParams<'a> {
    pub result: &'a AnalysisResult,
    pub portfolio_decision: &'a StructuredPortfolioDecision,
    pub recommendation: &'a str,
    pub confidence_score: i32,
    pub research_confidence_score: i32,
    pub research_reliability: &'a ResearchReliability,
    pub core_research_call: &'a CoreResearchCall,
    pub decision_view: &'a DecisionView,
    pub decision_narrative: &'a str,
    pub mispricing_claim: &'a LocalText,
    pub why_now: &'a LocalText,
    pub required_confirmation: &'a LocalText,
    pub max_initial_risk_budget: &'a LocalText,
}

fn build_overview_section(
    params: OverviewSectionParams<'_>,
) -> Option<ReportSection> {
    let summary = first_non_empty_sentence(&[
        params.portfolio_decision.executive_summary.as_str(),
        params.portfolio_decision.investment_thesis.as_str(),
        params.portfolio_decision.rationale.as_str(),
        params.result.derived_summary().as_str(),
    ]);
    let risk = first_non_empty_sentence(&[
        params.portfolio_decision.risk_assessment.as_str(),
        params.result.derived_risk_assessment().as_str(),
    ]);
    let rationale = first_non_empty_sentence(&[params.result.derived_rationale().as_str()]);

    let mut lines = Vec::new();
    let display_rating = if params.recommendation.trim().is_empty() {
        "Hold"
    } else {
        params.recommendation.trim()
    };
    lines.push(format!("- Core Research Call: **{}**", params.core_research_call));
    lines.push(format!("- Action: **{}**", decision_action_code(&params.decision_view.action)));
    lines.push(format!("- Rating: **{}**", display_rating));
    lines.push(format!("- Decision Rationale: {}", params.decision_narrative));
    lines.push(format!(
        "- Research Reliability: **{} / {}** ({})",
        params.research_reliability.score, params.research_reliability.max_score, params.research_reliability.label.key
    ));
    lines.push(format!("- Research Raw Score: **{} / 100**", params.research_confidence_score));
    lines.push(format!("- Execution Confidence: **{} / 100**", params.confidence_score));
    if params.research_reliability.score >= 75 && params.confidence_score <= 35 {
        lines.push(
            "- Note: Low score reflects execution timing uncertainty, not research distortion."
                .to_string(),
        );
    }
    if let Some(summary) = summary.as_ref() {
        lines.push(format!("- Core Conclusion: {}", summary));
    }
    if !params.mispricing_claim.key.is_empty() {
        lines.push(format!("- Mispricing Claim: {}", params.mispricing_claim.key));
    }
    if !params.why_now.key.is_empty() {
        lines.push(format!("- Why Now: {}", params.why_now.key));
    }
    if !params.required_confirmation.key.is_empty() {
        lines.push(format!("- Required Confirmation: {}", params.required_confirmation.key));
    }
    if !params.max_initial_risk_budget.key.is_empty() {
        lines.push(format!("- Initial Risk Budget: {}", params.max_initial_risk_budget.key));
    }
    lines.push("- Governance: Historical setups constrain sizing and upgrade thresholds only, not the main narrative.".to_string());
    if let Some(risk) = risk {
        lines.push(format!("- Key Risk: {}", risk));
    }
    if let Some(rationale) =
        rationale.filter(|item| !is_semantically_similar(Some(item), summary.as_ref()))
    {
        lines.push(format!("- Current Basis: {}", rationale));
    }
    lines.push(format!(
        "- Completed Stages: {}",
        summarize_stage_state(&params.result.report_stage())
    ));

    (!lines.is_empty()).then(|| ReportSection {
        key: "overview".to_string(),
        title: "Overview".to_string(),
        content: lines.join("\n"),
    })
}

fn decision_action_code(action: &DecisionAction) -> &'static str {
    match action {
        DecisionAction::BuyNow => "buy_now",
        DecisionAction::ProbePosition => "probe_position",
        DecisionAction::WaitBreakout => "wait_breakout",
        DecisionAction::WaitRetest => "wait_retest",
        DecisionAction::Hold => "hold",
        DecisionAction::Reduce => "reduce",
        DecisionAction::Exit => "exit",
    }
}

fn describe_core_research_call(call: &CoreResearchCall) -> &'static str {
    match call {
        CoreResearchCall::LeanBuy => "Lean Buy",
        CoreResearchCall::BuyOnConfirmation => "Buy on Confirmation",
        CoreResearchCall::Neutral => "Neutral",
        CoreResearchCall::LeanSell => "Lean Sell",
        CoreResearchCall::SellOnBreak => "Sell on Break",
    }
}

fn describe_decision_action(action: &DecisionAction) -> &'static str {
    match action {
        DecisionAction::BuyNow => "Buy Now",
        DecisionAction::ProbePosition => "Probe Position",
        DecisionAction::WaitBreakout => "Wait Breakout",
        DecisionAction::WaitRetest => "Wait Retest",
        DecisionAction::Hold => "Hold / Observe",
        DecisionAction::Reduce => "Reduce",
        DecisionAction::Exit => "Exit",
    }
}

fn describe_execution_state(state: &DecisionExecutionState) -> &'static str {
    match state {
        DecisionExecutionState::Ready => "Ready",
        DecisionExecutionState::Conditional => "Conditional",
        DecisionExecutionState::Watchlist => "Watchlist",
        DecisionExecutionState::Blocked => "Blocked",
    }
}

fn build_ic_report_summary(report: &StructuredReport) -> String {
    let main_path = preferred_scenario_path(&report.action_guides)
        .map(|path| path.name.clone())
        .unwrap_or_else(|| LocalText::new("ic_waiting_for_confirmation"));
    let research_call = describe_core_research_call(&report.decision_view.tilt);
    let action = describe_decision_action(&report.decision_view.action);
    let execution_state = describe_execution_state(&report.decision_view.execution_state);
    format!(
        "Chair assessment: {research_call}, execution: {execution_state}, next action: {action}. The priority path is \u{201c}{main_path}\u{201d}; no risk expansion until confirmation."
    )
}

fn build_primary_path_call(
    core_research_call: &CoreResearchCall,
    guides: &ReportActionGuides,
    confidence_score: i32,
) -> LocalText {
    let Some(path) = preferred_scenario_path(guides) else {
        return LocalText::default();
    };
    let path_name = path.name.trim().to_string();
    match core_research_call {
        CoreResearchCall::LeanBuy | CoreResearchCall::BuyOnConfirmation => {
            let probability_band = if confidence_score >= 60 { "high" } else if confidence_score >= 45 { "medium" } else { "low" };
            LocalText::new("primary_path_call_bullish")
                .with_str("path_name", path_name)
                .with_str("probability_band", probability_band)
        }
        CoreResearchCall::LeanSell | CoreResearchCall::SellOnBreak => {
            LocalText::new("primary_path_call_bearish").with_str("path_name", path_name)
        }
        CoreResearchCall::Neutral => {
            LocalText::new("primary_path_call_neutral").with_str("path_name", path_name)
        }
    }
}

fn build_path_bias_rationale(
    core_research_call: &CoreResearchCall,
    guides: &ReportActionGuides,
    trader_plan: &StructuredTraderPlan,
    portfolio_decision: &StructuredPortfolioDecision,
    confidence_score: i32,
) -> LocalText {
    let preferred = preferred_scenario_path(guides)
        .map(|path| path.name.trim().to_string())
        .unwrap_or_default();
    let confirmation = visible_confirmation_reference(portfolio_decision)
        .unwrap_or_default();
    let invalidation = visible_invalidation_reference(portfolio_decision, Some(trader_plan))
        .unwrap_or_default();
    match core_research_call {
        CoreResearchCall::LeanBuy | CoreResearchCall::BuyOnConfirmation => {
            LocalText::new("path_bias_bullish")
                .with_str("path_name", preferred)
                .with_str("confirmation", confirmation)
        }
        CoreResearchCall::LeanSell | CoreResearchCall::SellOnBreak => {
            LocalText::new("path_bias_bearish").with_str("path_name", preferred)
        }
        CoreResearchCall::Neutral => {
            let confidence_band = if confidence_score >= 45 { "moderate" } else { "weak" };
            LocalText::new("path_bias_neutral")
                .with_str("confidence_band", confidence_band)
                .with_str("confirmation", confirmation)
                .with_str("invalidation", invalidation)
        }
    }
}

fn build_advance_probe_opinion(
    core_research_call: &CoreResearchCall,
    action: &DecisionAction,
    trader_plan: &StructuredTraderPlan,
    portfolio_decision: &StructuredPortfolioDecision,
    confidence_score: i32,
    forced_hold: bool,
) -> LocalText {
    if forced_hold {
        return LocalText::new("advance_probe_forced_hold");
    }
    let confirmation = visible_confirmation_reference(portfolio_decision)
        .unwrap_or_default();
    let entry = trader_plan.entry_price.trim().to_string();
    match action {
        DecisionAction::ProbePosition => {
            LocalText::new("advance_probe_position")
                .with_str("entry", entry)
                .with_str("confirmation", confirmation)
        }
        DecisionAction::WaitBreakout => match core_research_call {
            CoreResearchCall::LeanBuy | CoreResearchCall::BuyOnConfirmation if confidence_score >= 45 => {
                LocalText::new("advance_probe_wait_breakout_conditional").with_str("confirmation", confirmation)
            }
            _ => {
                LocalText::new("advance_probe_wait_breakout_strict").with_str("confirmation", confirmation)
            }
        },
        _ => LocalText::new("advance_probe_default"),
    }
}

fn build_abort_plan(
    action: &DecisionAction,
    trader_plan: &StructuredTraderPlan,
    portfolio_decision: &StructuredPortfolioDecision,
) -> LocalText {
    let invalidation = visible_invalidation_reference(portfolio_decision, Some(trader_plan))
        .unwrap_or_else(|| trader_plan.stop_loss.trim().to_string());
    if invalidation.trim().is_empty() {
        return LocalText::new("abort_plan_generic");
    }
    match action {
        DecisionAction::ProbePosition | DecisionAction::BuyNow => {
            LocalText::new("abort_plan_probe_or_buy").with_str("invalidation", invalidation)
        }
        _ => {
            LocalText::new("abort_plan_default").with_str("invalidation", invalidation)
        }
    }
}

fn build_ic_report_sections(result: &AnalysisResult, report: &StructuredReport) -> Vec<ReportSection> {
    let preferred_path = preferred_scenario_path(&report.action_guides).cloned();
    let alternate_paths = all_scenario_paths(&report.action_guides)
        .into_iter()
        .filter(|path| preferred_path.as_ref().is_none_or(|current| current.name != path.name))
        .collect::<Vec<_>>();
    let validated_target = visible_target_reference(&report.portfolio_decision);
    let confirmation = visible_confirmation_reference(&report.portfolio_decision);
    let setup_summary = report.calibration_summary.setup_match_explanation.summary.trim();
    let execution_state = if report.execution_readiness.execution_boundary_complete {
        "Execution boundary largely complete, but must act on path conditions."
    } else {
        "Execution boundary not yet closed; cannot upgrade directional view to active risk increase."
    };
    let decision_tension = [
        format!(
            "{} on {}: core tension is whether trends, crowded trades, event windows, and execution odds allow more risk.",
            result.symbol, result.analysis_date
        ),
        first_non_empty_sentence(&[
            report.summary.as_str(),
            report.portfolio_decision.investment_thesis.as_str(),
            report.portfolio_decision.rationale.as_str(),
        ])
        .unwrap_or_default(),
    ]
    .into_iter()
    .filter(|item| !item.trim().is_empty())
    .map(|item| format!("- {item}"))
    .collect::<Vec<_>>()
    .join("\n");

    let current_judgement = {
        let mut lines = vec![format!(
            "- Current Judgement: **{}**. Long-term thesis intact, but not the time for unconditional risk expansion.",
            fallback_rating(&report.portfolio_decision)
        )];
        // Surface the code-computed reward-risk ratio as the authoritative value.
        // This prevents the LLM from generating its own conflicting ratio in
        // reasoning text (e.g. 0.13 when the computed value is 4.98).
        if let Some(rr) = report.profit_risk.reward_risk_ratio {
            let rr_label = if rr >= 2.0 {
                "Favorable R:R"
            } else if rr >= 1.2 {
                "Acceptable R:R"
            } else if rr >= 0.5 {
                "Weak R:R"
            } else {
                "Poor R:R"
            };
            let current_rr = report.profit_risk.current_position_reward_risk_ratio;
            if let Some(crr) = current_rr
                && (crr - rr).abs() > 0.01
            {
                let crr_label = if crr >= 2.0 {
                    "Favorable R:R"
                } else if crr >= 1.2 {
                    "Acceptable R:R"
                } else if crr >= 0.5 {
                    "Weak R:R"
                } else {
                    "Poor R:R"
                };
                lines.push(format!(
                    "- Computed R:R (current→confirmation): **{:.2}**（{}）；R:R (current→target): **{:.2}**（{}）。The former is odds from current price to confirmation breakout, the latter from breakout to target. Use code-computed values as authoritative.",
                    crr, crr_label, rr, rr_label
                ));
            } else {
                lines.push(format!(
                    "- Computed R:R: **{:.2}**（{}）。Use code-computed values as authoritative. Do not derive your own.",
                    rr, rr_label
                ));
            }
        }
        if let Some(path) = preferred_path.as_ref() {
            lines.push(format!("- Active path: **{}**。", path.name));
            lines.push(format!("- Trigger: {}", path.trigger.key));
            lines.push(format!("- Action: {}", path.action.key));
        }
        if let Some(level) = confirmation.as_ref() {
            lines.push(format!("- Key confirmation level: {level}"));
        }
        if let Some(level) = validated_target.as_ref() {
            lines.push(format!("- Target/take-profit reference if trend extends: {level}"));
        }
        lines.join("\n")
    };

    let scenario_section = {
        let mut lines = Vec::new();
        if let Some(path) = preferred_path.as_ref() {
            lines.push(format!("### Primary Path: {}", path.name));
            lines.push(format!("- Trigger: {}", path.trigger.key));
            lines.push(format!("- Action: {}", path.action.key));
            lines.push(format!("- Risk boundary: {}", path.risk_boundary.key));
            if !path.position_sizing.key.is_empty() {
                lines.push(format!("- Position sizing: {}", path.position_sizing.key));
            }
            if !path.stop_level.key.is_empty() {
                lines.push(format!("- Stop/Target: {}", path.stop_level.key));
            }
        }
        for path in alternate_paths {
            lines.push(String::new());
            lines.push(format!("### Alternate (not adopted): {}", path.name));
            lines.push(format!("- Trigger: {}", path.trigger.key));
            lines.push(format!("- Action if triggered: {}", path.action.key));
            lines.push(format!("- Reason not adopted: {}", path.risk_boundary.key));
            if !path.position_sizing.key.is_empty() {
                lines.push(format!("- Position sizing: {}", path.position_sizing.key));
            }
            if !path.stop_level.key.is_empty() {
                lines.push(format!("- Stop/Target: {}", path.stop_level.key));
            }
        }
        lines.join("\n")
    };

    let system_conservatism = [
        format!("- System remains conservative because {}。", execution_state),
        format!(
            "- Historical setup serves only as governance constraint, not the primary conclusion engine. Current historical reading: {}",
            if setup_summary.is_empty() { "No strong historical backing" } else { setup_summary }
        ),
        "- This means the system will not auto-upgrade on a few positive samples, but neutral historical samples should not dominate the conclusion.".to_string(),
    ]
    .join("\n");

    let override_questions = [
        "- Does this opportunity have paradigm-shift characteristics beyond a typical high-position momentum stock?".to_string(),
        "- If only small position risk is allowed, is the current R:R already worth a conditional probe?".to_string(),
        "- Which new evidence, once it appears, should trigger an immediate switch from wait to execute?".to_string(),
        "- If we do not act now, what is the real opportunity cost?".to_string(),
    ]
    .join("\n");

    let execution_discipline = {
        let mut lines = Vec::new();
        for item in report.portfolio_decision.trigger_checklist.iter().take(4) {
            lines.push(format!("- Upgrade condition: {item}"));
        }
        for item in report
            .portfolio_decision
            .missing_evidence_ladder
            .blocking_gaps
            .iter()
            .take(3)
        {
            lines.push(format!("- Must review gap: {item}"));
        }
        // Time-based stop-loss
        if !report.trader_plan.time_stop_deadline.trim().is_empty() {
            lines.push(format!("- Time stop: {}", report.trader_plan.time_stop_deadline));
            if !report.trader_plan.time_stop_reason.trim().is_empty() {
                lines.push(format!("- After time stop: {}", report.trader_plan.time_stop_reason));
            }
        }
        if lines.is_empty() {
            lines.push("- No structured triggers; next review must focus on key levels, event realization, and risk boundaries.".to_string());
        }
        lines.join("\n")
    };

    // Catalyst scoring card section
    let catalyst_section = {
        let card = &report.catalyst_score_card;
        let mut lines = Vec::new();
        if !card.event_name.trim().is_empty() {
            lines.push(format!("### Catalyst Assessment: {}", card.event_name));
            for item in &card.items {
                let mark = if item.score > 0 { "✓" } else { "✗" };
                lines.push(format!("- [{}] {}", mark, item.question));
                if !item.evidence.trim().is_empty() {
                    lines.push(format!("  Evidence: {}", item.evidence));
                }
            }
            lines.push(format!("Total: **{} / {}**", card.total_score, card.max_score));
            if !card.interpretation.trim().is_empty() {
                lines.push(format!("Interpretation: {}", card.interpretation));
            }
            if !card.recommended_action.trim().is_empty() {
                lines.push(format!("Recommended Action: {}", card.recommended_action));
            }
        } else {
            lines.push("No catalyst score card. The system auto-generates an assessment framework when key events (earnings calls, reports) approach.".to_string());
        }
        lines.join("\n")
    };

    // Review checklist section
    let review_section = {
        let checklist = &report.review_checklist;
        let mut lines = Vec::new();
        if !checklist.daily.is_empty() || !checklist.weekly.is_empty() {
            if !checklist.daily.is_empty() {
                lines.push("### Daily Review (After Close)".to_string());
                for item in &checklist.daily {
                    lines.push(format!("- [{}] {}", item.category, item.check));
                }
            }
            if !checklist.weekly.is_empty() {
                lines.push("### Weekly Review (Weekend)".to_string());
                for item in &checklist.weekly {
                    lines.push(format!("- [{}] {}", item.category, item.check));
                }
            }
        } else {
            lines.push("Daily: Monitor price near confirmation/invalidation levels, volume changes.".to_string());
            lines.push("Weekly: Check technical trends, fundamental marginal changes, discipline execution.".to_string());
        }
        lines.join("\n")
    };

    vec![
        ReportSection {
            key: "ic_decision_tension".to_string(),
            title: "Decision Tension".to_string(),
            content: decision_tension,
        },
        ReportSection {
            key: "ic_current_judgement".to_string(),
            title: "Current Judgement".to_string(),
            content: current_judgement,
        },
        ReportSection {
            key: "ic_scenario_paths".to_string(),
            title: "Scenario Paths".to_string(),
            content: scenario_section,
        },
        ReportSection {
            key: "ic_system_conservatism".to_string(),
            title: "System Conservatism".to_string(),
            content: system_conservatism,
        },
        ReportSection {
            key: "ic_override_questions".to_string(),
            title: "Override Questions".to_string(),
            content: override_questions,
        },
        ReportSection {
            key: "ic_execution_discipline".to_string(),
            title: "Execution Discipline".to_string(),
            content: execution_discipline,
        },
        ReportSection {
            key: "ic_catalyst_scoring".to_string(),
            title: "Catalyst Scoring".to_string(),
            content: catalyst_section,
        },
        ReportSection {
            key: "ic_review_checklist".to_string(),
            title: "Review Checklist".to_string(),
            content: review_section,
        },
    ]
}

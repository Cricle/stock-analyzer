
impl std::fmt::Display for CoreResearchCall {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let value = match self {
            CoreResearchCall::LeanBuy => "lean_buy",
            CoreResearchCall::BuyOnConfirmation => "buy_on_confirmation",
            CoreResearchCall::Neutral => "neutral",
            CoreResearchCall::LeanSell => "lean_sell",
            CoreResearchCall::SellOnBreak => "sell_on_break",
        };
        write!(f, "{value}")
    }
}

impl std::fmt::Display for DecisionMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let value = match self {
            DecisionMode::CoreResearch => "core_research",
            DecisionMode::Execution => "execution",
            DecisionMode::Blocked => "blocked",
        };
        write!(f, "{value}")
    }
}

#[allow(clippy::too_many_arguments)]
fn build_decision_view(
    trader_plan: &StructuredTraderPlan,
    portfolio_decision: &StructuredPortfolioDecision,
    action_guides: &ReportActionGuides,
    confidence_score: i32,
    execution_boundary_complete: bool,
    forced_hold: bool,
    core_research_call: &CoreResearchCall,
    current_price: Option<f64>,
    first_target: Option<String>,
) -> DecisionView {
    let rating = fallback_rating(portfolio_decision);
    let confirmation_reference = visible_confirmation_reference(portfolio_decision);
    let confirmation_price = confirmation_reference
        .as_deref()
        .and_then(parse_first_numeric)
        .filter(|&price| {
            // Reject confirmation prices that are unreasonably far from current price.
            // This catches LLM hallucinations like a confirmation at 683 when price is 425.
            current_price.is_none_or(|current| {
                current > 0.0 && (price / current - 1.0).abs() < 0.5
            })
        });
    let invalidation_price_raw =
        visible_invalidation_reference(portfolio_decision, Some(trader_plan));
    let invalidation_price = invalidation_price_raw
        .as_deref()
        .and_then(parse_first_numeric);
    let has_confirmation_gate = !confirmation_reference.clone().unwrap_or_default().is_empty();
    let primary_path = preferred_scenario_path(action_guides)
        .map(|path| path.name.key.clone())
        .unwrap_or_default();
    let primary_path_key = preferred_scenario_path(action_guides)
        .map(|path| path.key.clone())
        .filter(|key| !key.trim().is_empty())
        .unwrap_or_else(|| "base_case".to_string());
    let next_upgrade_condition = preferred_scenario_path(action_guides)
        .map(|path| {
            let trigger = normalize_trigger_phrase(&path.trigger.key);
            LocalText::new("next_upgrade_from_path").with_str("trigger", trigger)
        })
        .unwrap_or_else(|| {
            confirmation_reference
                .clone()
                .map(|level| LocalText::new("next_upgrade_with_confirmation").with_str("level", level))
                .unwrap_or_else(|| LocalText::new("next_upgrade_generic"))
        });
    let next_downgrade_condition = if let Some(invalidation_level) =
        visible_invalidation_reference(portfolio_decision, Some(trader_plan))
    {
        let is_hold = matches!(rating, Rating::Hold | Rating::Unknown);
        LocalText::new("next_downgrade_with_invalidation")
            .with_str("invalidation", invalidation_level)
            .with_bool("is_hold", is_hold)
    } else {
        LocalText::new("next_downgrade_no_invalidation")
    };
    let research_waiting_confirmation = matches!(
        core_research_call,
        CoreResearchCall::BuyOnConfirmation | CoreResearchCall::SellOnBreak
    ) || (matches!(
        core_research_call,
        CoreResearchCall::LeanBuy | CoreResearchCall::LeanSell
    ) && has_confirmation_gate
        && matches!(rating, Rating::Hold | Rating::Unknown));
    let execution_ready_now = execution_boundary_complete && !research_waiting_confirmation;
    let probe_position_allowed =
        !forced_hold && allows_probe_position_before_confirmation(trader_plan, portfolio_decision);
    let execution_state = if forced_hold {
        DecisionExecutionState::Blocked
    } else if execution_ready_now {
        DecisionExecutionState::Ready
    } else if has_confirmation_gate || !trader_plan.entry_price.trim().is_empty()
    {
        DecisionExecutionState::Conditional
    } else {
        DecisionExecutionState::Watchlist
    };
    let action = match core_research_call {
        CoreResearchCall::LeanBuy => {
            if execution_ready_now {
                DecisionAction::BuyNow
            } else if probe_position_allowed {
                DecisionAction::ProbePosition
            } else if has_confirmation_gate {
                DecisionAction::WaitBreakout
            } else {
                DecisionAction::WaitRetest
            }
        }
        CoreResearchCall::BuyOnConfirmation => {
            if probe_position_allowed {
                DecisionAction::ProbePosition
            } else if has_confirmation_gate {
                DecisionAction::WaitBreakout
            } else {
                DecisionAction::WaitRetest
            }
        }
        CoreResearchCall::LeanSell | CoreResearchCall::SellOnBreak => {
            if execution_ready_now {
                DecisionAction::Exit
            } else {
                DecisionAction::Reduce
            }
        }
        _ => DecisionAction::Hold,
    };
    let action_bias = match core_research_call {
        CoreResearchCall::LeanBuy | CoreResearchCall::BuyOnConfirmation => {
            if execution_ready_now {
                DecisionActionBias::AddRisk
            } else if probe_position_allowed {
                DecisionActionBias::KeepRisk
            } else {
                DecisionActionBias::NoTrade
            }
        }
        CoreResearchCall::LeanSell | CoreResearchCall::SellOnBreak => DecisionActionBias::ReduceRisk,
        _ => DecisionActionBias::KeepRisk,
    };
    let view = match core_research_call {
        CoreResearchCall::LeanBuy | CoreResearchCall::BuyOnConfirmation => DecisionViewDirection::Bullish,
        CoreResearchCall::LeanSell | CoreResearchCall::SellOnBreak => DecisionViewDirection::Bearish,
        _ => DecisionViewDirection::Neutral,
    };
    let confidence_band = if confidence_score >= 75 {
        DecisionConfidenceBand::High
    } else if confidence_score >= 45 {
        DecisionConfidenceBand::Medium
    } else {
        DecisionConfidenceBand::Low
    };
    let timeframe = infer_timeframe(portfolio_decision.time_horizon.as_str());
    let target_type = infer_target_type(portfolio_decision, execution_boundary_complete);
    let reader_summary = LocalText::new("reader_summary_text")
        .with_str("text", portfolio_decision.executive_summary.trim());
    let decision_mode = if forced_hold {
        DecisionMode::Blocked
    } else if execution_ready_now {
        DecisionMode::Execution
    } else {
        DecisionMode::CoreResearch
    };
    let state_line = build_decision_state_line(core_research_call, execution_ready_now, portfolio_decision);
    let action_line = build_decision_action_line(&action, portfolio_decision, execution_ready_now);
    let risk_line = build_decision_risk_line(portfolio_decision);
    let primary_path_call =
        build_primary_path_call(core_research_call, action_guides, confidence_score);
    let path_bias_rationale = build_path_bias_rationale(
        core_research_call,
        action_guides,
        trader_plan,
        portfolio_decision,
        confidence_score,
    );
    let advance_probe_opinion = build_advance_probe_opinion(
        core_research_call,
        &action,
        trader_plan,
        portfolio_decision,
        confidence_score,
        forced_hold,
    );
    let abort_plan = build_abort_plan(&action, trader_plan, portfolio_decision);
    let early_probe_allowed = matches!(action, DecisionAction::ProbePosition)
        || matches!(core_research_call, CoreResearchCall::LeanBuy | CoreResearchCall::BuyOnConfirmation)
            && !forced_hold
            && confidence_score >= 45;
    let early_probe_trigger = build_early_probe_trigger(
        &action,
        trader_plan,
        portfolio_decision,
        current_price,
        confirmation_price,
    );
    let early_probe_max_size = build_early_probe_max_size(&action, trader_plan);
    let wait_cost = build_wait_cost(
        current_price,
        confirmation_price,
        confidence_score,
        early_probe_allowed,
    );
    let distance_to_confirmation_pct = price_distance_pct(current_price, confirmation_price);
    let distance_to_invalidation_pct = downside_distance_pct(current_price, invalidation_price);

    // Entry derivation: track where entry_reference came from for transparency
    let raw_entry = trader_plan.entry_price.trim().to_string();
    let entry_price_val = parse_first_numeric(&raw_entry);
    // Guard: if entry == invalidation, it's a data error — derive a reasonable
    // entry instead of showing contradictory "enter at stop-loss" guidance.
    // Guard: if entry == confirmation (within 0.5%), derive entry slightly below
    // confirmation to create an observation window before breakout.
    let (entry_reference, entry_derived) = if entry_price_val.is_some()
        && entry_price_val == invalidation_price
        && entry_price_val != confirmation_price
    {
        match (current_price, confirmation_price) {
            (Some(current), Some(confirm)) if confirm > current => {
                let midpoint = current + (confirm - current) * 0.5;
                (format_price_reference(midpoint), true)
            }
            (Some(current), Some(confirm)) if confirm <= current && confirm > 0.0 => {
                (format_price_reference(confirm * 0.98), true)
            }
            _ => (String::new(), false),
        }
    } else if let (Some(entry), Some(confirm)) = (entry_price_val, confirmation_price) {
        // Entry == confirmation: derive entry below for observation window.
        // Enforce at least 1% gap from confirmation to avoid degenerate cases
        // where current_price ≈ confirmation produces a near-identical pullback.
        if confirm > 0.0 && (entry - confirm).abs() / confirm < 0.005 {
            let derived = if let Some(current) = current_price.filter(|&c| c > 0.0) && confirm > current {
                let pullback = current + (confirm - current) * 0.8;
                // If pullback is too close to confirmation (< 1% gap), use 3% discount
                if (confirm - pullback) / confirm < 0.01 {
                    confirm * 0.97
                } else {
                    pullback
                }
            } else {
                confirm * 0.97
            };
            (format_price_reference(derived), true)
        } else {
            (raw_entry.clone(), false)
        }
    } else {
        (raw_entry.clone(), false)
    };
    let entry_derivation = if !entry_reference.is_empty() {
        if entry_derived {
            LocalText::new("entry_derived_from_confirmation")
        } else {
            LocalText::new("entry_from_trader_plan")
        }
    } else {
        LocalText::new("entry_not_specified")
    };
    DecisionView {
        view,
        execution_state,
        action,
        action_bias,
        confidence_band,
        timeframe,
        entry_reference,
        entry_derivation,
        confirmation_level: confirmation_reference.unwrap_or_default(),
        invalidation_level: visible_invalidation_reference(portfolio_decision, Some(trader_plan))
            .unwrap_or_default(),
        target_type,
        target_reference: LocalText::new("target_reference_value")
            .with_str("value", portfolio_decision.target_reference.trim()),
        first_target: first_target.unwrap_or_default(),
        target_condition: LocalText::new("target_condition_value")
            .with_str("value", portfolio_decision.target_condition.trim()),
        thesis_state: infer_thesis_state(rating),
        primary_path,
        primary_path_key,
        primary_path_call,
        path_bias_rationale,
        advance_probe_opinion,
        abort_plan,
        next_upgrade_condition,
        next_downgrade_condition,
        sizing_guidance: {
            let has_blockers = !portfolio_decision.missing_evidence_ladder.blocking_gaps.is_empty()
                || !trader_plan.blocking_gaps.is_empty();
            if has_blockers {
                LocalText::new("sizing_guidance_blockers")
            } else {
                LocalText::new("sizing_guidance_from_plan")
                    .with_str("sizing", trader_plan.position_sizing.trim())
            }
        },
        reader_summary,
        tilt: core_research_call.clone(),
        decision_mode,
        state_line,
        action_line,
        risk_line,
        current_price: current_price
            .map(format_price_reference)
            .unwrap_or_default(),
        confirmation_price: confirmation_price
            .map(format_price_reference)
            .unwrap_or_default(),
        invalidation_price: invalidation_price
            .map(format_price_reference)
            .unwrap_or_default(),
        distance_to_confirmation_pct,
        distance_to_invalidation_pct,
        early_probe_allowed,
        early_probe_trigger,
        early_probe_max_size,
        wait_cost,
    }
}

fn price_distance_pct(current_price: Option<f64>, target_price: Option<f64>) -> f64 {
    match (current_price, target_price) {
        (Some(current), Some(target)) if current > 0.0 && target > current => ((target - current) / current) * 100.0,
        _ => 0.0,
    }
}

fn downside_distance_pct(current_price: Option<f64>, invalidation_price: Option<f64>) -> f64 {
    match (current_price, invalidation_price) {
        (Some(current), Some(invalidation)) if current > 0.0 && invalidation < current => ((current - invalidation) / current) * 100.0,
        _ => 0.0,
    }
}

fn build_early_probe_trigger(
    action: &DecisionAction,
    trader_plan: &StructuredTraderPlan,
    portfolio_decision: &StructuredPortfolioDecision,
    current_price: Option<f64>,
    confirmation_price: Option<f64>,
) -> LocalText {
    if matches!(action, DecisionAction::ProbePosition) {
        return LocalText::new("early_probe_trigger_entry")
            .with_str("entry", trader_plan.entry_price.trim());
    }
    match (current_price, confirmation_price) {
        (Some(current), Some(confirm)) if confirm > current => {
            let midpoint = current + (confirm - current) * 0.5;
            LocalText::new("early_probe_trigger_midpoint")
                .with_str("confirm", format_price_reference(confirm))
                .with_str("midpoint", format_price_reference(midpoint))
        }
        _ => {
            let level = visible_confirmation_reference(portfolio_decision).unwrap_or_default();
            LocalText::new("early_probe_trigger_confirmation")
                .with_str("confirmation", level)
        }
    }
}

fn build_early_probe_max_size(action: &DecisionAction, trader_plan: &StructuredTraderPlan) -> LocalText {
    let has_blockers = !trader_plan.blocking_gaps.is_empty();
    if has_blockers {
        return LocalText::new("early_probe_max_size_blocked");
    }
    if matches!(action, DecisionAction::ProbePosition) {
        if !trader_plan.position_sizing.trim().is_empty() {
            return LocalText::new("early_probe_max_size_from_plan")
                .with_str("sizing", trader_plan.position_sizing.trim());
        }
        return LocalText::new("early_probe_max_size_default");
    }
    LocalText::new("early_probe_max_size_zero")
}

fn build_wait_cost(
    current_price: Option<f64>,
    confirmation_price: Option<f64>,
    confidence_score: i32,
    early_probe_allowed: bool,
) -> LocalText {
    let distance = price_distance_pct(current_price, confirmation_price);
    if distance <= 0.0 {
        return LocalText::new("wait_cost_none");
    }
    if early_probe_allowed && confidence_score >= 45 {
        return LocalText::new("wait_cost_opportunity")
            .with_f64("distance_pct", distance);
    }
    LocalText::new("wait_cost_space_remaining")
        .with_f64("distance_pct", distance)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum AnalystConsensus {
    StrongBearish,
    ModerateBearish,
    Mixed,
    ModerateBullish,
    StrongBullish,
    NoData,
}

pub(super) fn analyst_consensus(analysts: &[AgentReportNode]) -> AnalystConsensus {
    if analysts.is_empty() {
        return AnalystConsensus::NoData;
    }
    let bearish_count = analysts.iter()
        .filter(|a| a.down_probability > 0.5)
        .count();
    let bullish_count = analysts.iter()
        .filter(|a| a.up_probability > 0.5)
        .count();
    let total = analysts.len();

    if bearish_count >= total * 3 / 4 {
        AnalystConsensus::StrongBearish
    } else if bearish_count >= total / 2 {
        AnalystConsensus::ModerateBearish
    } else if bullish_count >= total * 3 / 4 {
        AnalystConsensus::StrongBullish
    } else if bullish_count >= total / 2 {
        AnalystConsensus::ModerateBullish
    } else {
        AnalystConsensus::Mixed
    }
}

fn derive_core_research_call(
    research_plan: &StructuredResearchPlan,
    raw_llm_recommendation: &str,
    direction_score: i32,
    research_confidence_score: i32,
    research_reliability: &ResearchReliability,
    portfolio_decision: &StructuredPortfolioDecision,
    consensus: AnalystConsensus,
) -> CoreResearchCall {
    let research_anchor = primary_research_rating(
        research_plan,
        raw_llm_recommendation,
        portfolio_decision,
    );
    let confirmation_gated_bullish_hold =
        hold_language_implies_buy_on_confirmation(research_plan, portfolio_decision);
    if research_anchor.is_bearish() || direction_score <= -45 {
        if matches!(consensus, AnalystConsensus::StrongBearish | AnalystConsensus::ModerateBearish) {
            if !portfolio_decision.invalidation_level.trim().is_empty() {
                return CoreResearchCall::SellOnBreak;
            }
            return CoreResearchCall::LeanSell;
        }
        return CoreResearchCall::LeanSell;
    }
    if research_anchor.is_bullish() || direction_score >= 45 {
        if !portfolio_decision.confirmation_level.trim().is_empty() && research_confidence_score < 85 {
            return CoreResearchCall::BuyOnConfirmation;
        }
        return CoreResearchCall::LeanBuy;
    }
    if direction_score >= 25 && research_reliability.score >= 70 {
        return CoreResearchCall::BuyOnConfirmation;
    }
    if direction_score <= -35 && research_reliability.score >= 70 {
        if matches!(consensus, AnalystConsensus::StrongBearish | AnalystConsensus::ModerateBearish) {
            return CoreResearchCall::SellOnBreak;
        }
        return CoreResearchCall::LeanSell;
    }
    if research_anchor == Rating::Hold
        && !portfolio_decision.confirmation_level.trim().is_empty()
        && !portfolio_decision.invalidation_level.trim().is_empty()
        && direction_score >= 20
        && research_reliability.score >= 60
    {
        return CoreResearchCall::BuyOnConfirmation;
    }
    if confirmation_gated_bullish_hold {
        return CoreResearchCall::BuyOnConfirmation;
    }
    if research_anchor == Rating::Hold
        && !portfolio_decision.invalidation_level.trim().is_empty()
        && direction_score <= -20
        && research_reliability.score >= 60
    {
        if matches!(consensus, AnalystConsensus::StrongBearish | AnalystConsensus::ModerateBearish) {
            return CoreResearchCall::SellOnBreak;
        }
        return CoreResearchCall::LeanSell;
    }
    CoreResearchCall::Neutral
}

fn is_regulatory_reference_source(item: &ReferenceFactItem) -> bool {
    let source = item.emphasis.trim().to_ascii_lowercase();
    if source == "sec" {
        return true;
    }
    parse_url_host(&item.url).is_some_and(|host| host == "sec.gov" || host.ends_with(".sec.gov"))
}

fn derive_risk_controls(
    decision: &DecisionView,
    _portfolio_decision: &StructuredPortfolioDecision,
    reliability: &ResearchReliability,
    price_context: &PriceContext,
    probability: &ProbabilityView,
) -> Vec<RiskControl> {
    let mut controls = Vec::new();
    controls.push(RiskControl {
        risk_name: LocalText::new("risk_name_price_invalidation"),
        probability_pct: probability.risk_probability_pct,
        impact: LocalText::new("risk_impact_thesis_downgrade"),
        trigger: decision.next_downgrade_condition.clone(),
        defense_action: decision.abort_plan.clone(),
        invalidation_level: decision.invalidation_level.clone(),
        monitoring_signal: LocalText::new("risk_signal_price_below_invalidation"),
    });
    if let Some(distance) = price_context.distance_to_low_pct {
        controls.push(RiskControl {
            risk_name: LocalText::new("risk_name_drawdown_to_recent_low"),
            probability_pct: probability.downside_probability_pct,
            impact: LocalText::new("risk_impact_distance_pct").with_str("distance", format!("{distance:.1}%")),
            trigger: LocalText::new("risk_trigger_low_date").with_str("date", &price_context.low_date),
            defense_action: if decision.abort_plan.key.is_empty() { LocalText::new("risk_defense_generic") } else { decision.abort_plan.clone() },
            invalidation_level: price_context
                .low_price
                .map(format_price_reference)
                .unwrap_or_default(),
            monitoring_signal: LocalText::new("risk_signal_recent_low_retest"),
        });
    }
    for item in reliability.constraints.iter().take(3) {
        controls.push(RiskControl {
            risk_name: LocalText::new("risk_name_evidence_gap"),
            probability_pct: (100 - reliability.score).clamp(5, 80) as f64,
            impact: LocalText::new("risk_impact_evidence_gap").with_str("gap", item.to_string()),
            trigger: LocalText::new("risk_trigger_evidence_gap").with_str("gap", item.to_string()),
            defense_action: LocalText::new("risk_defense_evidence_gap"),
            invalidation_level: decision.invalidation_level.clone(),
            monitoring_signal: LocalText::new("risk_signal_missing_evidence"),
        });
    }
    controls
}

fn collect_price_anchors(
    result: &AnalysisResult,
    trader_plan: &StructuredTraderPlan,
    portfolio_decision: &StructuredPortfolioDecision,
) -> Vec<f64> {
    let mut anchors = Vec::new();
    for text in [
        trader_plan.entry_price.as_str(),
        trader_plan.stop_loss.as_str(),
        portfolio_decision.price_target.as_str(),
        portfolio_decision.confirmation_level.as_str(),
        portfolio_decision.executive_summary.as_str(),
        portfolio_decision.investment_thesis.as_str(),
        portfolio_decision.rationale.as_str(),
        result.agent_state.market_report.as_str(),
        result.agent_state.trader_investment_plan.as_str(),
    ] {
        anchors.extend(extract_price_like_numbers(text));
    }
    // Also pull technical-level anchors from market chart indicators
    for item in &result.artifacts.market_chart.indicators {
        match item.key.as_str() {
            "vwap" | "vwma_20" | "boll_upper" | "boll_lower" | "boll_mid" => {
                anchors.extend(extract_price_like_numbers(&item.value));
            }
            _ => {}
        }
    }
    anchors.sort_by(|left, right| left.partial_cmp(right).unwrap_or(std::cmp::Ordering::Equal));
    anchors.dedup_by(|left, right| (*left - *right).abs() < 0.01);
    anchors
}

fn target_passes_sanity_checks(
    target: f64,
    current_price: Option<f64>,
    rating: &Rating,
    anchors: &[f64],
) -> bool {
    if target <= 0.0 {
        return false;
    }
    if let Some(current) = current_price {
        let relative_gap = (target - current).abs() / current.max(1.0);
        if relative_gap > 1.2 {
            return false;
        }
        if current >= 20.0 && target < current * 0.4 {
            return false;
        }
        if rating.is_bullish() && target <= current * 0.9 {
            return false;
        }
        if rating.is_bearish() && target >= current * 1.1 {
            return false;
        }
        if target < 100.0 && current >= 120.0 && !anchors.iter().any(|anchor| (*anchor - target).abs() < 0.5)
        {
            return false;
        }
    }
    true
}

fn rebuild_confirmation_level(
    current_price: Option<f64>,
    anchors: &[f64],
    trader_plan: &StructuredTraderPlan,
    portfolio_decision: &StructuredPortfolioDecision,
) -> String {
    if let Some(existing) = parse_first_numeric(&portfolio_decision.confirmation_level) {
        return format_price_reference(existing);
    }
    if let Some(value) = [
        first_numeric_after_keywords(
            portfolio_decision.executive_summary.as_str(),
            &["突破", "站稳", "确认", "阻力", "前高"],
        ),
        first_numeric_after_keywords(
            portfolio_decision.investment_thesis.as_str(),
            &["突破", "站稳", "确认", "阻力", "前高"],
        ),
        parse_first_numeric(&trader_plan.entry_price),
        nearest_anchor_above(current_price, anchors),
    ].into_iter().flatten().next() {
        return format_price_reference(value);
    }
    String::new()
}

fn rebuild_directional_target(
    current_price: Option<f64>,
    anchors: &[f64],
    trader_plan: &StructuredTraderPlan,
    portfolio_decision: &StructuredPortfolioDecision,
) -> String {
    if let Some(value) = [
        first_numeric_after_keywords(
            portfolio_decision.investment_thesis.as_str(),
            &["目标", "目标位", "上看", "下看", "止盈"],
        ),
        first_numeric_after_keywords(
            portfolio_decision.rationale.as_str(),
            &["目标", "目标位", "上看", "下看", "止盈"],
        ),
        nearest_anchor_above(current_price, anchors),
        parse_first_numeric(&trader_plan.entry_price).zip(current_price).map(|(entry, current)| {
            if entry > current { entry } else { current * 1.05 }
        }),
    ].into_iter().flatten().next() {
        return format_price_reference(value);
    }
    // Moderate target fallback: when no bullish anchor exists above price,
    // use the nearest anchor below as a conservative target (e.g. support bounce)
    if let Some(below) = nearest_anchor_below(current_price, anchors) {
        return format_price_reference(below);
    }
    String::new()
}

fn visible_target_reference(portfolio_decision: &StructuredPortfolioDecision) -> Option<String> {
    let target = if portfolio_decision.target_reference.trim().is_empty() {
        portfolio_decision.price_target.trim()
    } else {
        portfolio_decision.target_reference.trim()
    };
    (!target.is_empty()).then(|| target.to_string())
}

fn visible_confirmation_reference(portfolio_decision: &StructuredPortfolioDecision) -> Option<String> {
    let value = normalize_level_phrase(portfolio_decision.confirmation_level.trim());
    if value.is_empty() {
        return None;
    }
    // Truncate overly long confirmation text to keep UI cards readable.
    // Extract just the price-level portion when the LLM produced a paragraph.
    let truncated = truncate_confirmation_for_display(&value);
    Some(truncated)
}

/// Keep confirmation-reference strings short enough for action-guide cards.
/// If the text is already concise (<= 80 chars), return as-is.  Otherwise
/// try to extract the first price-like sentence; failing that, hard-truncate
/// at 80 characters with an ellipsis.
fn truncate_confirmation_for_display(text: &str) -> String {
    if text.len() <= 80 {
        return text.to_string();
    }
    // Try to find the first sentence ending with a Chinese period or ASCII period.
    for delim in ["。", "，", ". ", "; "] {
        if let Some(pos) = text.find(delim) {
            let candidate = text[..pos + delim.len()].trim_end_matches(delim).trim();
            if !candidate.is_empty() && candidate.len() <= 80 {
                return candidate.to_string();
            }
        }
    }
    // Hard truncate as last resort.
    let mut end = 80.min(text.len());
    // Don't cut in the middle of a multi-byte char.
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &text[..end])
}

fn visible_invalidation_reference(
    portfolio_decision: &StructuredPortfolioDecision,
    trader_plan: Option<&StructuredTraderPlan>,
) -> Option<String> {
    let value = if !portfolio_decision.invalidation_level.trim().is_empty() {
        normalize_level_phrase(portfolio_decision.invalidation_level.trim())
    } else if let Some(trader_plan) = trader_plan {
        normalize_level_phrase(trader_plan.stop_loss.trim())
    } else {
        String::new()
    };
    (!value.is_empty()).then_some(value)
}

fn fallback_rating(portfolio_decision: &StructuredPortfolioDecision) -> Rating {
    portfolio_decision.rating.clone()
}

fn preferred_scenario_path(guides: &ReportActionGuides) -> Option<&ActionScenarioPath> {
    guides
        .buyers
        .scenario_paths
        .iter()
        .chain(guides.holders.scenario_paths.iter())
        .chain(guides.watchers.scenario_paths.iter())
        .find(|item| {
            matches!(
                item.key.as_str(),
                "retest_confirmation" | "breakout_continuation"
            )
        })
        .or_else(|| {
            guides
                .buyers
                .scenario_paths
                .iter()
                .chain(guides.holders.scenario_paths.iter())
                .chain(guides.watchers.scenario_paths.iter())
                .find(|item| !item.key.trim().is_empty())
        })
}

fn all_scenario_paths(guides: &ReportActionGuides) -> Vec<ActionScenarioPath> {
    let mut seen = std::collections::BTreeSet::new();
    guides
        .buyers
        .scenario_paths
        .iter()
        .chain(guides.holders.scenario_paths.iter())
        .chain(guides.watchers.scenario_paths.iter())
        .filter(|path| seen.insert(if path.key.trim().is_empty() {
            path.name.trim().to_string()
        } else {
            path.key.trim().to_string()
        }))
        .cloned()
        .collect::<Vec<_>>()
}

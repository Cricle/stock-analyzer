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

#[cfg(test)]
mod tests {
    use super::*;

    // --- is_regulatory_reference_source ---

    #[test]
    fn regulatory_sec_source() {
        let item = ReferenceFactItem {
            emphasis: "SEC".into(),
            ..Default::default()
        };
        assert!(is_regulatory_reference_source(&item));
    }

    #[test]
    fn regulatory_sec_gov_url() {
        let item = ReferenceFactItem {
            emphasis: "".into(),
            url: "https://sec.gov/filing/123".into(),
            ..Default::default()
        };
        assert!(is_regulatory_reference_source(&item));
    }

    #[test]
    fn regulatory_subdomain() {
        let item = ReferenceFactItem {
            emphasis: "".into(),
            url: "https://www.sec.gov/filing".into(),
            ..Default::default()
        };
        assert!(is_regulatory_reference_source(&item));
    }

    #[test]
    fn regulatory_non_sec() {
        let item = ReferenceFactItem {
            emphasis: "Reuters".into(),
            url: "https://reuters.com/news".into(),
            ..Default::default()
        };
        assert!(!is_regulatory_reference_source(&item));
    }

    // --- target_passes_sanity_checks ---

    #[test]
    fn sanity_zero_target() {
        assert!(!target_passes_sanity_checks(0.0, Some(100.0), &Rating::Buy, &[]));
    }

    #[test]
    fn sanity_negative_target() {
        assert!(!target_passes_sanity_checks(-5.0, Some(100.0), &Rating::Buy, &[]));
    }

    #[test]
    fn sanity_too_far_from_current() {
        assert!(!target_passes_sanity_checks(500.0, Some(100.0), &Rating::Buy, &[]));
    }

    #[test]
    fn sanity_bullish_target_too_low() {
        assert!(!target_passes_sanity_checks(80.0, Some(100.0), &Rating::Buy, &[]));
    }

    #[test]
    fn sanity_bearish_target_too_high() {
        assert!(!target_passes_sanity_checks(130.0, Some(100.0), &Rating::Sell, &[]));
    }

    #[test]
    fn sanity_valid_bullish() {
        assert!(target_passes_sanity_checks(120.0, Some(100.0), &Rating::Buy, &[]));
    }

    #[test]
    fn sanity_valid_no_current_price() {
        assert!(target_passes_sanity_checks(120.0, None, &Rating::Buy, &[]));
    }

    // --- truncate_confirmation_for_display ---

    #[test]
    fn truncate_short_text() {
        assert_eq!(truncate_confirmation_for_display("short text"), "short text");
    }

    #[test]
    fn truncate_long_text_with_period() {
        let long = "a".repeat(50) + "。" + &"b".repeat(50);
        let result = truncate_confirmation_for_display(&long);
        assert!(result.len() <= 80 || result.ends_with('…'));
    }

    #[test]
    fn truncate_long_text_no_delim() {
        let long = "a".repeat(200);
        let result = truncate_confirmation_for_display(&long);
        assert!(result.ends_with('…'));
        assert!(result.len() <= 82); // 80 chars + "…"
    }

    // --- visible_target_reference ---

    #[test]
    fn visible_target_from_reference() {
        let pd = StructuredPortfolioDecision {
            target_reference: "120".into(),
            price_target: "130".into(),
            ..Default::default()
        };
        assert_eq!(visible_target_reference(&pd), Some("120".into()));
    }

    #[test]
    fn visible_target_fallback_to_price_target() {
        let pd = StructuredPortfolioDecision {
            target_reference: "".into(),
            price_target: "130".into(),
            ..Default::default()
        };
        assert_eq!(visible_target_reference(&pd), Some("130".into()));
    }

    #[test]
    fn visible_target_empty() {
        let pd = StructuredPortfolioDecision {
            target_reference: "".into(),
            price_target: "".into(),
            ..Default::default()
        };
        assert_eq!(visible_target_reference(&pd), None);
    }

    // --- visible_confirmation_reference ---

    #[test]
    fn visible_confirmation_with_level() {
        let pd = StructuredPortfolioDecision {
            confirmation_level: "105".into(),
            ..Default::default()
        };
        assert_eq!(visible_confirmation_reference(&pd), Some("105".into()));
    }

    #[test]
    fn visible_confirmation_empty() {
        let pd = StructuredPortfolioDecision {
            confirmation_level: "".into(),
            ..Default::default()
        };
        assert_eq!(visible_confirmation_reference(&pd), None);
    }

    // --- visible_invalidation_reference ---

    #[test]
    fn visible_invalidation_from_portfolio() {
        let pd = StructuredPortfolioDecision {
            invalidation_level: "95".into(),
            ..Default::default()
        };
        assert_eq!(visible_invalidation_reference(&pd, None), Some("95".into()));
    }

    #[test]
    fn visible_invalidation_from_trader_plan() {
        let pd = StructuredPortfolioDecision {
            invalidation_level: "".into(),
            ..Default::default()
        };
        let tp = StructuredTraderPlan {
            stop_loss: "90".into(),
            ..Default::default()
        };
        assert_eq!(visible_invalidation_reference(&pd, Some(&tp)), Some("90".into()));
    }

    #[test]
    fn visible_invalidation_empty() {
        let pd = StructuredPortfolioDecision {
            invalidation_level: "".into(),
            ..Default::default()
        };
        assert_eq!(visible_invalidation_reference(&pd, None), None);
    }

    // --- fallback_rating ---

    #[test]
    fn fallback_rating_from_portfolio() {
        let pd = StructuredPortfolioDecision {
            rating: Rating::Buy,
            ..Default::default()
        };
        assert!(matches!(fallback_rating(&pd), Rating::Buy));
    }

    // --- preferred_scenario_path ---

    #[test]
    fn preferred_path_retest_confirmation() {
        let mut guides = ReportActionGuides::default();
        guides.buyers.scenario_paths.push(ActionScenarioPath {
            key: "retest_confirmation".into(),
            ..Default::default()
        });
        assert_eq!(preferred_scenario_path(&guides).unwrap().key, "retest_confirmation");
    }

    #[test]
    fn preferred_path_first_non_empty() {
        let mut guides = ReportActionGuides::default();
        guides.holders.scenario_paths.push(ActionScenarioPath {
            key: "base_case".into(),
            ..Default::default()
        });
        assert_eq!(preferred_scenario_path(&guides).unwrap().key, "base_case");
    }

    #[test]
    fn preferred_path_empty() {
        let guides = ReportActionGuides::default();
        assert!(preferred_scenario_path(&guides).is_none());
    }
}

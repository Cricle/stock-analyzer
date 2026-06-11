fn derive_trade_setup_quality(
    trader_plan: &StructuredTraderPlan,
    portfolio_decision: &StructuredPortfolioDecision,
    action_breakdown: &ActionBreakdown,
    execution_boundary_complete: bool,
    execution_blocking_gaps: &[String],
) -> TradeSetupQuality {
    let score = action_breakdown.execution_levels.score
        + action_breakdown.sizing_discipline.score
        + action_breakdown.horizon_clarity.score
        + action_breakdown.reward_to_risk.score;
    let max_score = action_breakdown.execution_levels.max_score
        + action_breakdown.sizing_discipline.max_score
        + action_breakdown.horizon_clarity.max_score
        + action_breakdown.reward_to_risk.max_score;
    let trigger_count =
        portfolio_decision.trigger_checklist.len() + trader_plan.execution_trigger_checklist.len();
    let has_sizing = !trader_plan.position_sizing.trim().is_empty();
    let has_entry = !trader_plan.entry_price.trim().is_empty();
    let has_stop = !trader_plan.stop_loss.trim().is_empty();
    let has_target = !portfolio_decision.price_target.trim().is_empty()
        || !portfolio_decision.confirmation_level.trim().is_empty();
    let has_horizon = !portfolio_decision.time_horizon.trim().is_empty();

    let mut strengths = Vec::new();
    let mut gaps = Vec::new();

    if has_entry && has_stop && has_target {
        strengths.push(LocalText::new("setup_strength_price_loop"));
    } else {
        gaps.push(LocalText::new("setup_gap_price_loop_incomplete"));
    }
    if has_sizing {
        strengths.push(LocalText::new("setup_strength_sizing_structured"));
    } else {
        gaps.push(LocalText::new("setup_gap_sizing_missing"));
    }
    if has_horizon {
        strengths.push(LocalText::new("setup_strength_horizon_clear"));
    } else {
        gaps.push(LocalText::new("setup_gap_horizon_missing"));
    }
    if trigger_count >= 2 {
        strengths.push(LocalText::new("setup_strength_triggers").with_i32("count", trigger_count as i32));
    } else if trigger_count == 1 {
        gaps.push(LocalText::new("setup_gap_trigger_too_few"));
    } else {
        gaps.push(LocalText::new("setup_gap_no_triggers"));
    }
    if action_breakdown.reward_to_risk.score >= 11 {
        strengths.push(LocalText::new("setup_strength_rr_acceptable"));
    } else if action_breakdown.reward_to_risk.score <= 6 {
        gaps.push(LocalText::new("setup_gap_rr_weak"));
    }
    // Penalize when cash flow data is missing and the setup relies on fundamentals
    if !has_entry || !has_stop {
        // Only flag cash flow gap when execution levels are weak
        gaps.push(LocalText::new("setup_gap_entry_stop_missing"));
    }
    if !execution_boundary_complete {
        if execution_blocking_gaps.is_empty() {
            gaps.push(LocalText::new("setup_gap_execution_boundary_incomplete"));
        } else {
            for gap in execution_blocking_gaps.iter().take(4) {
                if !gaps.iter().any(|existing| existing.as_str() == gap.as_str()) {
                    gaps.push(LocalText::new(gap.as_str()));
                }
            }
        }
    }

    let label = LocalText::new(if score >= 52 && execution_boundary_complete {
        "trade_setup_label_execution_ready"
    } else if score >= 36 {
        "trade_setup_label_conditional"
    } else {
        "trade_setup_label_watchlist"
    });

    let rationale = LocalText::new("setup_rationale")
        .with_i32("score", score)
        .with_i32("max_score", max_score)
        .with_i32("trigger_count", trigger_count as i32)
        .with_str("boundary_complete", if execution_boundary_complete { "yes" } else { "no" });

    TradeSetupQuality {
        score,
        max_score,
        ready: label == LocalText::new("trade_setup_label_execution_ready"),
        label,
        rationale,
        strengths,
        gaps,
    }
}

fn collect_execution_blocking_gaps(
    research_plan: &StructuredResearchPlan,
    trader_plan: &StructuredTraderPlan,
    portfolio_decision: &StructuredPortfolioDecision,
    diagnostics: &ReportDiagnostics,
) -> Vec<String> {
    let mut gaps = Vec::new();
    for gap in research_plan
        .missing_evidence_ladder
        .blocking_gaps
        .iter()
        .chain(trader_plan.blocking_gaps.iter())
        .chain(
            portfolio_decision
                .missing_evidence_ladder
                .blocking_gaps
                .iter(),
        )
    {
        let trimmed = gap.trim();
        if trimmed.is_empty() {
            continue;
        }
        if !gaps.iter().any(|existing: &String| existing == trimmed) {
            gaps.push(trimmed.to_string());
        }
    }
    for item in diagnostics.availability.iter().filter(|item| {
        item.severity.eq_ignore_ascii_case("error") || item.code.starts_with("scenario_minimum_")
    }) {
        let trimmed = item.message.trim();
        if trimmed.is_empty() {
            continue;
        }
        if !gaps.iter().any(|existing: &String| existing == trimmed) {
            gaps.push(trimmed.to_string());
        }
    }
    // Also check news diagnostics for elevated items (e.g. critically sparse coverage).
    for item in diagnostics.news.iter().filter(|item| {
        item.elevated_to_execution_blocking_gap
    }) {
        let trimmed = item.message.trim();
        if trimmed.is_empty() {
            continue;
        }
        if !gaps.iter().any(|existing: &String| existing == trimmed) {
            gaps.push(trimmed.to_string());
        }
    }
    gaps
}

fn normalize_gap_match_text(value: &str) -> String {
    value.trim()
        .to_ascii_lowercase()
        .replace(
            ['，', '。', '：', ':', ';', '；', ',', '.', '/', '\\', '(', ')'],
            " ",
        )
}

fn tokenize_gap_match_text(value: &str) -> Vec<String> {
    normalize_gap_match_text(value)
        .split_whitespace()
        .filter(|token| token.len() >= 2)
        .map(|token| token.to_string())
        .collect()
}

fn score_related_gap_match(base_tokens: &[String], candidate: &str) -> usize {
    let candidate_tokens = tokenize_gap_match_text(candidate);
    base_tokens
        .iter()
        .filter(|token| candidate_tokens.iter().any(|candidate| candidate == *token))
        .count()
}

fn related_gap_items(item: &ReportDiagnosticItem, pool: &[String]) -> Vec<String> {
    let source = [item.code.as_str(), item.message.as_str()]
        .into_iter()
        .chain(item.details.iter().map(String::as_str))
        .collect::<Vec<_>>()
        .join(" ");
    let tokens = tokenize_gap_match_text(&source);
    if tokens.is_empty() {
        return Vec::new();
    }
    let mut ranked = pool
        .iter()
        .map(|entry| (entry, score_related_gap_match(&tokens, entry)))
        .filter(|(_, score)| *score > 0)
        .collect::<Vec<_>>();
    ranked.sort_by_key(|right| std::cmp::Reverse(right.1));
    ranked
        .into_iter()
        .take(2)
        .map(|(entry, _)| entry.clone())
        .collect()
}

fn enrich_diagnostic_linkage(
    diagnostics: &mut ReportDiagnostics,
    research_plan: &StructuredResearchPlan,
    trader_plan: &StructuredTraderPlan,
    portfolio_decision: &StructuredPortfolioDecision,
) {
    let blocking_pool = research_plan
        .missing_evidence_ladder
        .blocking_gaps
        .iter()
        .chain(trader_plan.blocking_gaps.iter())
        .chain(
            portfolio_decision
                .missing_evidence_ladder
                .blocking_gaps
                .iter(),
        )
        .filter_map(|item| {
            let trimmed = item.trim();
            (!trimmed.is_empty()).then(|| trimmed.to_string())
        })
        .fold(Vec::new(), |mut acc, item| {
            if !acc.iter().any(|existing| existing == &item) {
                acc.push(item);
            }
            acc
        });
    let trigger_pool = research_plan
        .trigger_checklist
        .iter()
        .chain(trader_plan.execution_trigger_checklist.iter())
        .chain(portfolio_decision.trigger_checklist.iter())
        .filter_map(|item| {
            let trimmed = item.trim();
            (!trimmed.is_empty()).then(|| trimmed.to_string())
        })
        .fold(Vec::new(), |mut acc, item| {
            if !acc.iter().any(|existing| existing == &item) {
                acc.push(item);
            }
            acc
        });

    for item in &mut diagnostics.availability {
        item.related_blocking_gaps = related_gap_items(item, &blocking_pool);
        item.related_trigger_checklist = related_gap_items(item, &trigger_pool);
        item.elevated_to_execution_blocking_gap = item.severity.eq_ignore_ascii_case("error")
            || item.code.starts_with("scenario_minimum_");
    }
    // Elevate critically sparse news coverage to execution blocking gap.
    // When news comes from ≤1 source AND <5 items, the analysis lacks the
    // factual basis needed for directional probability estimates.
    for item in &mut diagnostics.news {
        item.related_blocking_gaps = related_gap_items(item, &blocking_pool);
        item.related_trigger_checklist = related_gap_items(item, &trigger_pool);
        if item.code == "news_sparse_coverage" && !item.elevated_to_execution_blocking_gap {
            let item_count = item.details.iter().find_map(|d| {
                d.strip_prefix("item_count=").and_then(|s| s.parse::<usize>().ok())
            });
            // Parse source count from message: "returned N items across M source(s)"
            let src_count = item.message
                .split("across ")
                .nth(1)
                .and_then(|s| s.split(' ').next())
                .and_then(|s| s.parse::<usize>().ok());
            let critically_sparse = src_count.is_none_or(|s| s <= 1)
                && item_count.is_none_or(|n| n < 5);
            if critically_sparse {
                item.elevated_to_execution_blocking_gap = true;
            }
        }
    }
}

fn scenario_gap_messages(diagnostics: &ReportDiagnostics) -> Vec<String> {
    diagnostics
        .availability
        .iter()
        .filter(|item| {
            item.severity.eq_ignore_ascii_case("error")
                || item.code.starts_with("scenario_minimum_")
        })
        .map(|item| item.message.trim().to_string())
        .chain(
            diagnostics
                .news
                .iter()
                .filter(|item| item.elevated_to_execution_blocking_gap)
                .map(|item| item.message.trim().to_string()),
        )
        .filter(|item| !item.is_empty())
        .fold(Vec::new(), |mut acc, item| {
            if !acc.iter().any(|existing| existing == &item) {
                acc.push(item);
            }
            acc
        })
}

fn append_scenario_gap_narrative(
    target: &mut LocalText,
    diagnostics: &ReportDiagnostics,
    prefix: &str,
) {
    let messages = scenario_gap_messages(diagnostics);
    if messages.is_empty() {
        return;
    }
    let suffix = format!("{prefix}：{}。", messages.join("；"));
    let trimmed = target.trim();
    if trimmed.is_empty() {
        target.key = suffix;
    } else if !trimmed.contains(&suffix) {
        target.key = format!("{trimmed} {suffix}");
    }
}


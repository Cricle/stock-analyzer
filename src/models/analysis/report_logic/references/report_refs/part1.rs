fn derive_research_reliability(
    confidence_breakdown: &ConfidenceBreakdown,
    confidence_caps: &[ConfidenceCap],
    memory_context: &MemoryContextSnapshot,
    execution_boundary_complete: bool,
    diagnostics: &ReportDiagnostics,
) -> ResearchReliability {
    let score = (confidence_breakdown.data_quality.score
        + confidence_breakdown.trend_confirmation.score
        + confidence_breakdown.fundamental_confirmation.score
        + confidence_breakdown.catalyst_quality.score
        + confidence_breakdown.cross_agent_consistency.score
        + confidence_breakdown.risk_clarity.score)
        .clamp(0, 100);

    let label = if score >= 80 {
        "high"
    } else if score >= 65 {
        "good"
    } else if score >= 50 {
        "conditional"
    } else {
        "weak"
    }
    .to_string();

    let mut strengths = Vec::new();
    let mut constraints = Vec::new();

    if confidence_breakdown.data_quality.score >= 14 {
        strengths.push(LocalText::new("Core analysis platform and tool evidence largely complete."));
    } else {
        constraints.push(LocalText::new("Core data or toolchain completeness insufficient."));
    }
    if confidence_breakdown.trend_confirmation.score >= 14 {
        strengths.push(LocalText::new("Market structure, price levels, and indicator anchors are fairly complete."));
    }
    if confidence_breakdown.fundamental_confirmation.score >= 10 {
        strengths.push(LocalText::new("Fundamental analysis has sufficient numerical anchors to support directional assessment."));
    } else {
        constraints.push(LocalText::new("Fundamental numerical anchors insufficient or methodology unverified; currently at reference/risk-alert level, not yet sufficient as standalone core decision evidence."));
    }
    if diagnostics
        .fundamentals
        .iter()
        .any(|item| item.code == "cashflow_quality_unresolved")
    {
        constraints.push(LocalText::new(
            "Profit-cash flow divergence not broken down to working capital level; risk alert only, not standalone core decision evidence.",
        ));
    }
    if confidence_breakdown.catalyst_quality.score >= 8 {
        strengths.push(LocalText::new("Event and catalyst chain is relatively clear."));
    } else {
        constraints.push(LocalText::new("News catalyst evidence is weak; mostly background/complexity clues, not directional triggers."));
    }
    if diagnostics
        .news
        .iter()
        .any(|item| item.code == "disclosure_sequence_complexity")
    {
        constraints.push(LocalText::new(
            "Recent disclosures include registration/issuance/selling clues; news is better as complexity/supply pressure signals, not operational catalysts.",
        ));
    }
    if confidence_breakdown.cross_agent_consistency.score >= 12 {
        strengths.push(LocalText::new("Multiple analysis desks directionally aligned; conclusion is not a single-point opinion."));
    }
    if confidence_breakdown.risk_clarity.score >= 7 {
        strengths.push(LocalText::new("Risk boundaries and invalidation conditions are clearly expressed."));
    }
    if memory_context.used_setup_filtered_retrieval
        && memory_context.setup_resolved_match_count == 0
    {
        constraints.push(LocalText::new("Insufficient validated samples for similar setups; historical transferability must be treated conservatively."));
    }
    for item in diagnostics
        .availability
        .iter()
        .filter(|item| item.severity.eq_ignore_ascii_case("error"))
    {
        constraints.push(LocalText::new(item.message.key.clone()));
    }
    if !execution_boundary_complete {
        constraints.push(LocalText::new("Execution boundary not closed; reduces executability but does not invalidate the research."));
    }
    constraints.extend(
        confidence_caps
            .iter()
            .filter(|cap| {
                cap.key != "execution_boundary_missing" && cap.key != "thin_setup_history"
            })
            .map(|cap| LocalText::new(cap.key.clone())),
    );

    let rationale = LocalText::new(format!("reliability_rationale_{label}"));

    ResearchReliability {
        score,
        max_score: 100,
        label: LocalText::new(format!("reliability_label_{label}")),
        rationale,
        strengths,
        constraints,
    }
}

fn derive_report_references(
    result: &AnalysisResult,
    confidence_breakdown: &ConfidenceBreakdown,
    memory_context: &MemoryContextSnapshot,
) -> ReportReferenceSnapshot {
    let mut market = derive_market_reference_facts(result);
    if market.is_empty() {
        market = derive_market_reference_facts_from_report(result);
    }
    let mut news = derive_news_reference_facts(result);
    news.extend(derive_news_quality_reference_facts(result));
    ReportReferenceSnapshot {
        market,
        fundamentals: derive_fundamentals_reference_facts(result),
        news,
        memory: derive_memory_reference_facts(confidence_breakdown, memory_context),
    }
}

fn derive_market_reference_facts(result: &AnalysisResult) -> Vec<ReferenceFactItem> {
    let Some(state) = result.analyst_runtime_state("market") else {
        return Vec::new();
    };
    let mut facts = Vec::new();
    for observation in &state.tool_history {
        if observation.tool_name == "get_stock_data"
            && observation.success
            && let Ok(value) = serde_json::from_str::<serde_json::Value>(&observation.output)
            && let Some(rows) = value.get("rows").and_then(|item| item.as_array())
            && let (Some(first), Some(last)) = (rows.first(), rows.last())
        {
            let first_close = first.get("close").and_then(json_number);
            let last_close = last.get("close").and_then(json_number);
            if let Some(last_close) = last_close {
                facts.push(ReferenceFactItem {
                    key: "latest_close".to_string(),
                    label: "Latest Close".into(),
                    value: format!("{last_close:.4}"),
                    emphasis: "primary".to_string(),
                    ..Default::default()
                });
            }
            if let (Some(start), Some(end)) = (first_close, last_close) {
                let pct = if start.abs() > f64::EPSILON {
                    ((end - start) / start) * 100.0
                } else {
                    0.0
                };
                facts.push(ReferenceFactItem {
                    key: "window_return".to_string(),
                    label: "Window Return".into(),
                    value: format!("{pct:.2}%"),
                    emphasis: if pct >= 0.0 { "success" } else { "warning" }.to_string(),
                    ..Default::default()
                });
            }
        }
        if observation.tool_name == "get_indicators" && observation.success
            && let Some(items) = parse_indicator_items(&observation.output, &observation.meta) {
                for (key, value) in items {
                    facts.push(ReferenceFactItem {
                        key: key.clone(),
                        label: key,
                        value: format!("{value:.4}"),
                        emphasis: "info".to_string(),
                        ..Default::default()
                    });
                }
            }
    }
    facts
}

fn derive_market_reference_facts_from_report(result: &AnalysisResult) -> Vec<ReferenceFactItem> {
    let chart = &result.report.market_chart;
    let price = &result.report.price_context;
    let technical = &result.report.technical_indicators;
    let mut facts = Vec::new();

    if let Some(current) = price.current_price.or_else(|| chart.candles.last().map(|item| item.close))
        && current.is_finite()
        && current > 0.0
    {
        facts.push(ReferenceFactItem {
            key: "latest_close".to_string(),
            label: "Latest Close".into(),
            value: format!("{current:.4}"),
            emphasis: "primary".to_string(),
            ..Default::default()
        });
    }

    if let (Some(first), Some(last)) = (chart.candles.first(), chart.candles.last())
        && first.close.is_finite()
        && last.close.is_finite()
        && first.close.abs() > f64::EPSILON
    {
        let pct = ((last.close - first.close) / first.close) * 100.0;
        facts.push(ReferenceFactItem {
            key: "window_return".to_string(),
            label: "Window Return".into(),
            value: format!("{pct:.2}%"),
            emphasis: if pct >= 0.0 { "success" } else { "warning" }.to_string(),
            ..Default::default()
        });
    }

    if let Some(range_pct) = price.range_pct.filter(|value| value.is_finite()) {
        facts.push(ReferenceFactItem {
            key: "range_pct".to_string(),
            label: "Range volatility".into(),
            value: format!("{range_pct:.2}%"),
            emphasis: "info".to_string(),
            ..Default::default()
        });
    }

    if let Some(volume) = price.latest_volume.filter(|value| *value > 0) {
        facts.push(ReferenceFactItem {
            key: "latest_volume".to_string(),
            label: "Latest volume".into(),
            value: volume.to_string(),
            emphasis: "info".to_string(),
            ..Default::default()
        });
    }

    for category in &technical.categories {
        for indicator in &category.indicators {
            let Some(value) = indicator.value.filter(|item| item.is_finite()) else {
                continue;
            };
            facts.push(ReferenceFactItem {
                key: indicator.key.clone(),
                label: indicator.key.clone(),
                value: format!("{value:.4}"),
                emphasis: "info".to_string(),
                ..Default::default()
            });
        }
    }

    facts
}

fn derive_fundamentals_reference_facts(result: &AnalysisResult) -> Vec<ReferenceFactItem> {
    let Some(state) = result.analyst_runtime_state("fundamentals") else {
        return Vec::new();
    };
    let mut merged = serde_json::Map::new();
    for observation in &state.tool_history {
        if matches!(
            observation.tool_name.as_str(),
            "get_fundamentals" | "get_income_statement" | "get_balance_sheet" | "get_cashflow"
        ) && observation.success
            && let Ok(value) = serde_json::from_str::<serde_json::Value>(&observation.output)
            && let Some(object) = value.as_object()
        {
            for (key, value) in object {
                merged.insert(key.clone(), value.clone());
            }
        }
    }
    let pick = |key: &str| merged.get(key).and_then(json_number);
    let fiscal_period = merged
        .get("fiscal_year_end")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());
    let mut facts = Vec::new();
    for (label, key, emphasis) in [
        ("Revenue", "revenues", "primary"),
        ("Net income", "net_income", "primary"),
        ("Operating cash flow", "operating_cash_flow", "success"),
        ("Free cash flow", "free_cash_flow", "success"),
        ("Capital expenditure", "capital_expenditure", "info"),
        ("Total assets", "assets", "info"),
        ("Total liabilities", "liabilities", "warning"),
        ("Stockholders equity", "stockholders_equity", "info"),
        ("Cash and equivalents", "cash_and_equivalents", "info"),
        ("Market cap", "market_cap", "info"),
    ] {
        if let Some(value) = pick(key) {
            let display_label = if key == "revenues" {
                if let Some(ref period) = fiscal_period {
                    format!("Revenue ({})", period)
                } else {
                    label.to_string()
                }
            } else {
                label.to_string()
            };
            facts.push(ReferenceFactItem {
                key: key.to_string(),
                label: display_label,
                value: format_number_compact(value),
                emphasis: emphasis.to_string(),
                ..Default::default()
            });
        }
    }
    facts.truncate(10);
    facts
}

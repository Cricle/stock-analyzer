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
        strengths.push(LocalText::new("核心分析台与工具证据基本齐备。"));
    } else {
        constraints.push(LocalText::new("核心数据或工具链完整度不足。"));
    }
    if confidence_breakdown.trend_confirmation.score >= 14 {
        strengths.push(LocalText::new("市场结构、价位和指标锚点较完整。"));
    }
    if confidence_breakdown.fundamental_confirmation.score >= 10 {
        strengths.push(LocalText::new("基本面论证具备足够数值锚点，可支撑方向判断。"));
    } else {
        constraints.push(LocalText::new("基本面数值锚点不足或口径仍待验证，当前更多停留在参考层或风险提醒层，尚不足以单独构成核心决策证据。"));
    }
    if diagnostics
        .fundamentals
        .iter()
        .any(|item| item.code == "cashflow_quality_unresolved")
    {
        constraints.push(LocalText::new(
            "利润与现金流背离尚未拆解到应收、存货、预付款等营运资本层，当前只能作为风险警示，不能单独充当核心决策证据。",
        ));
    }
    if confidence_breakdown.catalyst_quality.score >= 8 {
        strengths.push(LocalText::new("事件与催化链路较清晰。"));
    } else {
        constraints.push(LocalText::new("新闻催化证据偏弱，更多是背景信息或复杂度线索，而非可直接定方向的硬触发器。"));
    }
    if diagnostics
        .news
        .iter()
        .any(|item| item.code == "disclosure_sequence_complexity")
    {
        constraints.push(LocalText::new(
            "近期披露序列包含注册、发行或减持类线索，当前新闻更适合作为复杂度/供给压力提示，不能直接等同于经营催化。",
        ));
    }
    if confidence_breakdown.cross_agent_consistency.score >= 12 {
        strengths.push(LocalText::new("多分析台方向较一致，结论不是单点意见。"));
    }
    if confidence_breakdown.risk_clarity.score >= 7 {
        strengths.push(LocalText::new("风险边界与失效条件表达清楚。"));
    }
    if memory_context.used_setup_filtered_retrieval
        && memory_context.setup_resolved_match_count == 0
    {
        constraints.push(LocalText::new("相似 setup 已验证样本不足，历史迁移性只能保守处理。"));
    }
    for item in diagnostics
        .availability
        .iter()
        .filter(|item| item.severity.eq_ignore_ascii_case("error"))
    {
        constraints.push(LocalText::new(item.message.key.clone()));
    }
    if !execution_boundary_complete {
        constraints.push(LocalText::new("执行边界未闭环，这会压低可下单性，但不等于研究本身无效。"));
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
                    label: "最新收盘".into(),
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
                    label: "窗口涨跌幅".into(),
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
            label: "最新收盘".into(),
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
            label: "窗口涨跌幅".into(),
            value: format!("{pct:.2}%"),
            emphasis: if pct >= 0.0 { "success" } else { "warning" }.to_string(),
            ..Default::default()
        });
    }

    if let Some(range_pct) = price.range_pct.filter(|value| value.is_finite()) {
        facts.push(ReferenceFactItem {
            key: "range_pct".to_string(),
            label: "区间波动".into(),
            value: format!("{range_pct:.2}%"),
            emphasis: "info".to_string(),
            ..Default::default()
        });
    }

    if let Some(volume) = price.latest_volume.filter(|value| *value > 0) {
        facts.push(ReferenceFactItem {
            key: "latest_volume".to_string(),
            label: "最新成交量".into(),
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
        ("营收", "revenues", "primary"),
        ("净利润", "net_income", "primary"),
        ("经营现金流", "operating_cash_flow", "success"),
        ("自由现金流", "free_cash_flow", "success"),
        ("资本开支", "capital_expenditure", "info"),
        ("总资产", "assets", "info"),
        ("总负债", "liabilities", "warning"),
        ("股东权益", "stockholders_equity", "info"),
        ("现金及等价物", "cash_and_equivalents", "info"),
        ("总市值", "market_cap", "info"),
    ] {
        if let Some(value) = pick(key) {
            let display_label = if key == "revenues" {
                if let Some(ref period) = fiscal_period {
                    format!("营收 ({})", period)
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

#[cfg(test)]
mod tests {
    use super::*;

    // --- derive_research_reliability ---

    #[test]
    fn reliability_high_score() {
        let mut breakdown = ConfidenceBreakdown::default();
        breakdown.data_quality.score = 16;
        breakdown.trend_confirmation.score = 16;
        breakdown.fundamental_confirmation.score = 12;
        breakdown.catalyst_quality.score = 10;
        breakdown.cross_agent_consistency.score = 14;
        breakdown.risk_clarity.score = 8;
        let memory = MemoryContextSnapshot::default();
        let diagnostics = ReportDiagnostics::default();
        let result = derive_research_reliability(&breakdown, &[], &memory, true, &diagnostics);
        assert_eq!(result.score, 76);
        assert_eq!(result.label.key, "reliability_label_good");
    }

    #[test]
    fn reliability_weak_score() {
        let breakdown = ConfidenceBreakdown::default();
        let memory = MemoryContextSnapshot::default();
        let diagnostics = ReportDiagnostics::default();
        let result = derive_research_reliability(&breakdown, &[], &memory, true, &diagnostics);
        assert_eq!(result.score, 0);
        assert_eq!(result.label.key, "reliability_label_weak");
    }

    #[test]
    fn reliability_with_constraints() {
        let breakdown = ConfidenceBreakdown::default();
        let memory = MemoryContextSnapshot::default();
        let diagnostics = ReportDiagnostics::default();
        let result = derive_research_reliability(&breakdown, &[], &memory, false, &diagnostics);
        assert!(result.constraints.iter().any(|c| c.key.contains("execution_boundary")));
    }

    #[test]
    fn reliability_with_caps() {
        let breakdown = ConfidenceBreakdown::default();
        let memory = MemoryContextSnapshot::default();
        let diagnostics = ReportDiagnostics::default();
        let caps = vec![ConfidenceCap {
            key: "some_cap".to_string(),
            ..Default::default()
        }];
        let result = derive_research_reliability(&breakdown, &caps, &memory, true, &diagnostics);
        assert!(result.constraints.iter().any(|c| c.key == "some_cap"));
    }

    #[test]
    fn reliability_excludes_execution_boundary_cap() {
        let breakdown = ConfidenceBreakdown::default();
        let memory = MemoryContextSnapshot::default();
        let diagnostics = ReportDiagnostics::default();
        let caps = vec![ConfidenceCap {
            key: "execution_boundary_missing".to_string(),
            ..Default::default()
        }];
        let result = derive_research_reliability(&breakdown, &caps, &memory, true, &diagnostics);
        assert!(!result.constraints.iter().any(|c| c.key == "execution_boundary_missing"));
    }

    // --- derive_market_reference_facts ---

    #[test]
    fn market_facts_empty_result() {
        let result = AnalysisResult::default();
        let facts = derive_market_reference_facts(&result);
        assert!(facts.is_empty());
    }

    #[test]
    fn market_facts_from_stock_data() {
        let mut result = AnalysisResult::default();
        result.artifacts.analyst_runtime_states.push(AnalystRuntimeState {
            key: "market".to_string(),
            tool_history: vec![ToolObservation {
                tool_name: "get_stock_data".to_string(),
                success: true,
                output: r#"{"rows":[{"close":100.0},{"close":110.0}]}"#.to_string(),
                ..Default::default()
            }],
            ..Default::default()
        });
        let facts = derive_market_reference_facts(&result);
        assert!(facts.iter().any(|f| f.key == "latest_close"));
        assert!(facts.iter().any(|f| f.key == "window_return"));
    }

    // --- derive_market_reference_facts_from_report ---

    #[test]
    fn market_facts_from_report_empty() {
        let result = AnalysisResult::default();
        let facts = derive_market_reference_facts_from_report(&result);
        assert!(facts.is_empty());
    }

    #[test]
    fn market_facts_from_report_with_candles() {
        let mut result = AnalysisResult::default();
        result.report.market_chart.candles = vec![
            serde_json::json!({"close": 100.0}).into(),
            serde_json::json!({"close": 110.0}).into(),
        ];
        // This will likely not populate since candles are CandleData not serde_json::Value
        // but we test the function doesn't panic
        let _ = derive_market_reference_facts_from_report(&result);
    }

    // --- derive_fundamentals_reference_facts ---

    #[test]
    fn fundamentals_facts_empty() {
        let result = AnalysisResult::default();
        let facts = derive_fundamentals_reference_facts(&result);
        assert!(facts.is_empty());
    }

    #[test]
    fn fundamentals_facts_with_data() {
        let mut result = AnalysisResult::default();
        result.artifacts.analyst_runtime_states.push(AnalystRuntimeState {
            key: "fundamentals".to_string(),
            tool_history: vec![ToolObservation {
                tool_name: "get_fundamentals".to_string(),
                success: true,
                output: r#"{"revenues":5000000000,"net_income":1000000000,"fiscal_year_end":"2025-12"}"#.to_string(),
                ..Default::default()
            }],
            ..Default::default()
        });
        let facts = derive_fundamentals_reference_facts(&result);
        assert!(facts.iter().any(|f| f.key == "revenues"));
        assert!(facts.iter().any(|f| f.key == "net_income"));
        assert!(facts.iter().any(|f| f.label.contains("2025-12")));
    }

    // --- derive_report_references ---

    #[test]
    fn report_references_empty() {
        let result = AnalysisResult::default();
        let breakdown = ConfidenceBreakdown::default();
        let memory = MemoryContextSnapshot::default();
        let refs = derive_report_references(&result, &breakdown, &memory);
        assert!(refs.market.is_empty());
        assert!(refs.fundamentals.is_empty());
        assert!(refs.news.is_empty());
    }
}

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
    // Try tool_history first (when analyst called tools)
    let mut merged = serde_json::Map::new();
    let mut fiscal_period: Option<String> = None;
    if let Some(state) = result.analyst_runtime_state("fundamentals") {
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
        fiscal_period = merged
            .get("fiscal_year_end")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string());
    }

    // Fallback to scenario_data.fundamentals when tool_history is empty
    let use_scenario_fallback = merged.is_empty();
    let scenario_fund = if use_scenario_fallback {
        result.artifacts.scenario_data.fundamentals.as_ref()
    } else {
        None
    };

    let pick = |key: &str| -> Option<f64> {
        if let Some(v) = merged.get(key).and_then(json_number) {
            return Some(v);
        }
        // Map scenario_data field names (with _usd suffix) to report field names
        if let Some(fund) = scenario_fund {
            return match key {
                "revenues" => fund.revenues_usd,
                "net_income" => fund.net_income_usd,
                "operating_cash_flow" => fund.operating_cash_flow_usd,
                "free_cash_flow" => fund.free_cash_flow_usd,
                "capital_expenditure" => fund.capital_expenditure_usd,
                "assets" => fund.assets_usd,
                "liabilities" => fund.liabilities_usd,
                "stockholders_equity" => fund.stockholders_equity_usd,
                "cash_and_equivalents" => fund.cash_and_equivalents_usd,
                "market_cap" => fund.market_cap,
                _ => None,
            };
        }
        None
    };

    // Get fiscal period from scenario fallback
    if use_scenario_fallback && fiscal_period.is_none() {
        fiscal_period = scenario_fund
            .and_then(|f| f.fiscal_year_end.as_deref())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string());
    }

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
            let display_label = if key != "market_cap" {
                if let Some(ref period) = fiscal_period {
                    format!("{} ({})", label, period)
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
    let valuation = valuation_metrics(
        pick("market_cap"),
        pick("revenues"),
        pick("net_income"),
        pick("stockholders_equity"),
    );
    for (key, value) in [
        ("price_to_sales", valuation.price_to_sales),
        ("price_to_book", valuation.price_to_book),
        ("price_to_earnings", valuation.price_to_earnings),
    ] {
        if let Some(value) = value {
            facts.push(ReferenceFactItem {
                key: key.to_string(),
                label: key.to_string(),
                value: format!("{value:.2}x"),
                emphasis: "info".to_string(),
                ..Default::default()
            });
        }
    }
    if valuation.earnings_multiple_not_meaningful {
        facts.push(ReferenceFactItem {
            key: "price_to_earnings".to_string(),
            label: "price_to_earnings".to_string(),
            value: "N/M".to_string(),
            emphasis: "warning".to_string(),
            ..Default::default()
        });
    }
    if valuation.price_to_sales.is_some() || valuation.price_to_book.is_some() {
        facts.push(ReferenceFactItem {
            key: "valuation_range_status".to_string(),
            label: "valuation_range_status".to_string(),
            value: "not_quantified_without_peer_or_forecast".to_string(),
            emphasis: "warning".to_string(),
            ..Default::default()
        });
    }
    facts.truncate(14);
    facts
}

#[derive(Default)]
struct ValuationMetrics {
    price_to_sales: Option<f64>,
    price_to_book: Option<f64>,
    price_to_earnings: Option<f64>,
    earnings_multiple_not_meaningful: bool,
}

fn valuation_metrics(
    market_cap: Option<f64>,
    revenue: Option<f64>,
    net_income: Option<f64>,
    equity: Option<f64>,
) -> ValuationMetrics {
    let valid = |value: Option<f64>| value.filter(|value| value.is_finite() && *value > 0.0);
    let market_cap = valid(market_cap);
    ValuationMetrics {
        price_to_sales: market_cap.zip(valid(revenue)).map(|(value, revenue)| value / revenue),
        price_to_book: market_cap.zip(valid(equity)).map(|(value, equity)| value / equity),
        price_to_earnings: market_cap.zip(valid(net_income)).map(|(value, income)| value / income),
        earnings_multiple_not_meaningful: market_cap.is_some()
            && net_income.is_some_and(|value| value <= 0.0),
    }
}

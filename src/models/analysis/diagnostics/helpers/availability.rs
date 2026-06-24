
fn derive_availability_diagnostics(result: &AnalysisResult) -> Vec<ReportDiagnosticItem> {
    let mut diagnostics = Vec::new();

    for issue in &result.artifacts.scenario_data.issues {
        diagnostics.push(ReportDiagnosticItem {
            code: issue.code.clone(),
            severity: issue.severity.clone(),
            message: LocalText::new(&issue.code).with_str("message", &issue.message),
            details: vec![format!("domain={}", issue.domain)],
        ..Default::default()
        });
    }

    diagnostics.extend(derive_scenario_minimum_diagnostics(result));

    for state in &result.artifacts.analyst_runtime_states {
        for observation in &state.tool_history {
            let lower = observation.output.to_ascii_lowercase();

            if lower.contains("tushare token missing") {
                diagnostics.push(ReportDiagnosticItem {
                    code: "missing_credentials".to_string(),
                    severity: "info".to_string(),
                    message: LocalText::new("missing_credentials_message"),
                    details: vec![
                        format!("analyst={}", state.key),
                        format!("tool={}", observation.tool_name),
                    ],
                    ..Default::default()
                });
            }

            if matches!(
                result.artifacts.scenario_context.market,
                crate::AnalysisScenarioMarket::AShare
            ) && observation.tool_name == "get_insider_transactions"
            {
                continue;
            }

            if observation.success
                && observation.tool_name == "get_fundamentals"
                && let Ok(value) = serde_json::from_str::<serde_json::Value>(&observation.output)
                && let Some(object) = value.as_object()
            {
                let key_fields = [
                    "revenues",
                    "net_income",
                    "assets",
                    "liabilities",
                    "operating_cash_flow",
                    "free_cash_flow",
                ];
                let populated = key_fields
                    .iter()
                    .filter(|key| object.get(**key).is_some_and(|v| !v.is_null()))
                    .count();
                if populated <= 1 {
                    diagnostics.push(ReportDiagnosticItem {
                        code: "fundamentals_sparse".to_string(),
                        severity: "warning".to_string(),
                        message: LocalText::new("fundamentals_sparse_message"),
                        details: vec![
                            format!("analyst={}", state.key),
                            format!("tool={}", observation.tool_name),
                            format!("populated_key_fields={}", populated),
                        ],
                        ..Default::default()
                    });
                }
            }

            if !observation.success
                && (observation.tool_name == "get_news"
                    || observation.tool_name == "get_global_news")
                && lower.contains("no hk company news available")
            {
                diagnostics.push(ReportDiagnosticItem {
                    code: "hk_news_sparse".to_string(),
                    severity: "info".to_string(),
                    message: LocalText::new("hk_news_sparse_message"),
                    details: vec![format!("tool={}", observation.tool_name)],
                ..Default::default()
                });
            }
        }
    }

    diagnostics
}

fn derive_scenario_minimum_diagnostics(result: &AnalysisResult) -> Vec<ReportDiagnosticItem> {
    let mut diagnostics = Vec::new();
    let scenario = &result.artifacts.scenario_context;
    let data = &result.artifacts.scenario_data;

    let has_quote = data.quote.is_some();
    let has_fundamentals = data.fundamentals.is_some();
    let has_company_news = !data.company_news.is_empty();
    let has_candles = !data.candles.is_empty();

    let mut missing = Vec::new();
    if !has_quote {
        missing.push("quote");
    }
    if !has_candles {
        missing.push("candles");
    }

    match scenario.market {
        crate::AnalysisScenarioMarket::AShare => {
            if !has_fundamentals {
                missing.push("fundamentals");
            }
            if !has_company_news {
                missing.push("company_news");
            }
        }
        crate::AnalysisScenarioMarket::HongKong => {
            if !has_company_news {
                missing.push("company_news");
            }
            if !has_fundamentals {
                diagnostics.push(ReportDiagnosticItem {
                    code: "scenario_minimum_hk_fundamentals_soft_gap".to_string(),
                    severity: "warning".to_string(),
                    message: "港股基础分析缺少基本面快照，当前更偏事件和价格驱动判断。".into(),
                    details: vec![
                        "market=hk_equity".to_string(),
                        "missing=fundamentals".to_string(),
                    ],
                    ..Default::default()
                });
            }
        }
        crate::AnalysisScenarioMarket::UsEquity => {
            if !has_fundamentals {
                missing.push("fundamentals");
            }
            if !has_company_news {
                missing.push("company_news");
            }
        }
        crate::AnalysisScenarioMarket::Unknown => {
            if !has_company_news {
                diagnostics.push(ReportDiagnosticItem {
                    code: "scenario_minimum_unknown_news_gap".to_string(),
                    severity: "warning".to_string(),
                    message: "当前市场类型未明确，且公司级新闻证据不足。".into(),
                    details: vec!["market=unknown".to_string()],
                ..Default::default()
                });
            }
        }
    }

    if !missing.is_empty() {
        let market_key = scenario.market.key();
        let market_label = scenario.market.label();
        diagnostics.push(ReportDiagnosticItem {
            code: format!("scenario_minimum_{}_incomplete", market_key),
            severity: "error".to_string(),
            message: LocalText::new("scenario_minimum_incomplete")
                .with_str("market", if market_label.trim().is_empty() { "current_market" } else { market_label })
                .with_str("missing", missing.join(", ")),
            details: vec![
                format!("market={}", market_key),
                format!("missing={}", missing.join(",")),
            ],
            ..Default::default()
        });
    }

    diagnostics
}

fn collect_tool_meta_details(analyst_key: &str, observation: &ToolObservation) -> Vec<String> {
    let mut details = vec![
        format!("analyst={analyst_key}"),
        format!("tool={}", observation.tool_name),
    ];
    if let Some(item_count) = observation.meta.get("item_count").and_then(|value| value.as_u64()) {
        details.push(format!("item_count={item_count}"));
    }
    if let Some(sources) = observation.meta.get("sources").and_then(|value| value.as_array()) {
        let sources = sources
            .iter()
            .filter_map(|value| value.as_str())
            .collect::<Vec<_>>();
        if !sources.is_empty() {
            details.push(format!("sources={}", sources.join(",")));
        }
    }
    if let Some(fallback_used) = observation
        .meta
        .get("fallback_used")
        .and_then(|value| value.as_bool())
        && fallback_used {
            details.push("used_alternate_public_source=true".to_string());
        }
    if let Some(fallback_kind) = observation
        .meta
        .get("fallback_kind")
        .and_then(|value| value.as_str())
    {
        details.push(format!("fallback_kind={fallback_kind}"));
        if fallback_kind == "hkex_recent_high_value" {
            details.push("近窗无港股公司公告，已回补最近高价值HKEX公司公告".to_string());
        }
    }
    if let Some(scope) = observation.meta.get("scope").and_then(|value| value.as_str()) {
        details.push(format!("scope={scope}"));
    }
    if let Some(attempts) = observation.meta.get("attempts").and_then(|value| value.as_array()) {
        let failed_attempts = attempts
            .iter()
            .filter(|attempt| {
                !attempt
                    .get("success")
                    .and_then(|value| value.as_bool())
                    .unwrap_or(false)
            })
            .count();
        if failed_attempts > 0 {
            details.push(format!("failed_attempts={failed_attempts}"));
        }
        for attempt in attempts.iter().take(4) {
            let source = attempt
                .get("source")
                .and_then(|value| value.as_str())
                .unwrap_or("unknown");
            let success = attempt
                .get("success")
                .and_then(|value| value.as_bool())
                .unwrap_or(false);
            let item_count = attempt
                .get("item_count")
                .and_then(|value| value.as_u64())
                .unwrap_or_default();
            if success {
                details.push(format!(
                    "attempt source={source}, success=true, item_count={item_count}"
                ));
            }
        }
    }
    details
}

fn json_number(value: &serde_json::Value) -> Option<f64> {
    match value {
        serde_json::Value::Number(number) => number.as_f64(),
        serde_json::Value::String(text) => text.parse::<f64>().ok(),
        _ => None,
    }
}

fn parse_first_numeric(value: &str) -> Option<f64> {
    extract_price_like_numbers(value).into_iter().next()
}

fn extract_price_like_numbers(text: &str) -> Vec<f64> {
    let Ok(regex) = Regex::new(r"(-?\d{1,5}(?:\.\d{1,4})?)") else {
        return Vec::new();
    };
    regex
        .captures_iter(text)
        .filter_map(|caps| {
            let m = caps.get(1)?;
            let start = m.start();
            if start > 0 {
                let prev = text.as_bytes()[start - 1] as char;
                if prev.is_ascii_alphabetic() || prev == '%' {
                    return None;
                }
            }
            // Skip numbers followed by period/MA indicator characters
            // (e.g. "200日均线" where 200 is a period, not a price)
            let end = m.end();
            if end < text.len() {
                let next = text[end..].chars().next().unwrap_or('\0');
                if matches!(next, '日' | '天' | '周' | '月' | '年' | '均' | '线') {
                    return None;
                }
            }
            m.as_str().parse::<f64>().ok()
        })
        .filter(|value| value.is_finite() && *value > 0.0)
        .collect()
}

fn first_numeric_after_keywords(text: &str, keywords: &[&str]) -> Option<f64> {
    for keyword in keywords {
        if let Some(index) = text.find(keyword) {
            let tail = &text[index..];
            if let Some(value) = parse_first_numeric(tail) {
                return Some(value);
            }
        }
    }
    None
}

fn nearest_anchor_above(current_price: Option<f64>, anchors: &[f64]) -> Option<f64> {
    let current = current_price?;
    anchors
        .iter()
        .copied()
        .filter(|anchor| *anchor > current * 1.01)
        .min_by(|left, right| {
            left.partial_cmp(right)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
}

fn format_price_reference(value: f64) -> String {
    if value >= 100.0 {
        format!("{value:.2}")
    } else {
        format!("{value:.4}")
    }
}

fn collect_missing_execution_fields(
    trader_plan: &StructuredTraderPlan,
    portfolio_decision: &StructuredPortfolioDecision,
) -> Vec<String> {
    let mut missing = Vec::new();
    if trader_plan.entry_price.trim().is_empty() {
        missing.push("entry_price".to_string());
    }
    if trader_plan.stop_loss.trim().is_empty() {
        missing.push("stop_loss".to_string());
    }
    if portfolio_decision.price_target.trim().is_empty()
        && portfolio_decision.confirmation_level.trim().is_empty()
    {
        missing.push("price_target".to_string());
    }
    if portfolio_decision.time_horizon.trim().is_empty() {
        missing.push("time_horizon".to_string());
    }
    missing
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- json_number ---

    #[test]
    fn json_number_from_number() {
        let val = serde_json::json!(42.5);
        assert_eq!(json_number(&val), Some(42.5));
    }

    #[test]
    fn json_number_from_string() {
        let val = serde_json::json!("3.14");
        assert_eq!(json_number(&val), Some(3.14));
    }

    #[test]
    fn json_number_from_invalid() {
        let val = serde_json::json!(true);
        assert_eq!(json_number(&val), None);
    }

    #[test]
    fn json_number_from_invalid_string() {
        let val = serde_json::json!("not_a_number");
        assert_eq!(json_number(&val), None);
    }

    // --- parse_first_numeric ---

    #[test]
    fn parse_first_numeric_basic() {
        assert_eq!(parse_first_numeric("价格 105.5 一带"), Some(105.5));
    }

    #[test]
    fn parse_first_numeric_none() {
        assert_eq!(parse_first_numeric("没有数字"), None);
    }

    #[test]
    fn parse_first_numeric_skips_alpha_prefix() {
        assert_eq!(parse_first_numeric("abc105.5"), None);
    }

    // --- extract_price_like_numbers ---

    #[test]
    fn extract_prices_basic() {
        let result = extract_price_like_numbers("支撑位 105.5，阻力位 110.0");
        assert_eq!(result, vec![105.5, 110.0]);
    }

    #[test]
    fn extract_prices_empty() {
        let result = extract_price_like_numbers("没有数字");
        assert!(result.is_empty());
    }

    #[test]
    fn extract_prices_skips_ma_period() {
        let result = extract_price_like_numbers("200日均线");
        assert!(result.is_empty());
    }

    #[test]
    fn extract_prices_skips_percent() {
        let result = extract_price_like_numbers("上涨10%");
        assert!(result.is_empty());
    }

    // --- first_numeric_after_keywords ---

    #[test]
    fn first_numeric_after_keyword() {
        let result = first_numeric_after_keywords("目标价 120.0 元", &["目标价"]);
        assert_eq!(result, Some(120.0));
    }

    #[test]
    fn first_numeric_after_multiple_keywords() {
        let result = first_numeric_after_keywords("some text 目标 130.0", &["目标价", "目标"]);
        assert_eq!(result, Some(130.0));
    }

    #[test]
    fn first_numeric_after_keyword_not_found() {
        let result = first_numeric_after_keywords("some text", &["目标价"]);
        assert_eq!(result, None);
    }

    // --- nearest_anchor_above ---

    #[test]
    fn nearest_anchor_above_basic() {
        let result = nearest_anchor_above(Some(100.0), &[105.0, 110.0, 95.0]);
        assert_eq!(result, Some(105.0));
    }

    #[test]
    fn nearest_anchor_above_none_close() {
        let result = nearest_anchor_above(None, &[105.0, 110.0]);
        assert_eq!(result, None);
    }

    #[test]
    fn nearest_anchor_above_empty() {
        let result = nearest_anchor_above(Some(100.0), &[]);
        assert_eq!(result, None);
    }

    #[test]
    fn nearest_anchor_above_all_below() {
        let result = nearest_anchor_above(Some(100.0), &[90.0, 95.0]);
        assert_eq!(result, None);
    }

    // --- format_price_reference ---

    #[test]
    fn format_price_above_100() {
        assert_eq!(format_price_reference(105.5), "105.50");
    }

    #[test]
    fn format_price_below_100() {
        assert_eq!(format_price_reference(42.5), "42.50");
    }

    // --- collect_missing_execution_fields ---

    #[test]
    fn missing_fields_all_empty() {
        let trader = StructuredTraderPlan::default();
        let portfolio = StructuredPortfolioDecision::default();
        let result = collect_missing_execution_fields(&trader, &portfolio);
        assert!(result.contains(&"entry_price".to_string()));
        assert!(result.contains(&"stop_loss".to_string()));
        assert!(result.contains(&"price_target".to_string()));
        assert!(result.contains(&"time_horizon".to_string()));
    }

    #[test]
    fn missing_fields_all_present() {
        let mut trader = StructuredTraderPlan::default();
        trader.entry_price = "105".to_string();
        trader.stop_loss = "95".to_string();
        let mut portfolio = StructuredPortfolioDecision::default();
        portfolio.price_target = "120".to_string();
        portfolio.time_horizon = "3个月".to_string();
        let result = collect_missing_execution_fields(&trader, &portfolio);
        assert!(result.is_empty());
    }

    #[test]
    fn missing_fields_partial() {
        let mut trader = StructuredTraderPlan::default();
        trader.entry_price = "105".to_string();
        let portfolio = StructuredPortfolioDecision::default();
        let result = collect_missing_execution_fields(&trader, &portfolio);
        assert!(!result.contains(&"entry_price".to_string()));
        assert!(result.contains(&"stop_loss".to_string()));
    }

    // --- derive_scenario_minimum_diagnostics ---

    #[test]
    fn scenario_minimum_us_complete() {
        let mut result = AnalysisResult::default();
        result.artifacts.scenario_context.market = crate::AnalysisScenarioMarket::UsEquity;
        result.artifacts.scenario_data.quote = Some(serde_json::json!({"price": 100.0}));
        result.artifacts.scenario_data.company_news = vec![serde_json::json!({"title": "test"})];
        result.artifacts.scenario_data.fundamentals = Some(serde_json::json!({"revenues": 1000}));
        result.artifacts.scenario_data.candles = vec![serde_json::json!({"close": 100.0})];
        let diagnostics = derive_scenario_minimum_diagnostics(&result);
        let errors: Vec<_> = diagnostics.iter().filter(|d| d.severity == "error").collect();
        assert!(errors.is_empty());
    }

    #[test]
    fn scenario_minimum_us_missing_quote() {
        let mut result = AnalysisResult::default();
        result.artifacts.scenario_context.market = crate::AnalysisScenarioMarket::UsEquity;
        result.artifacts.scenario_data.company_news = vec![serde_json::json!({"title": "test"})];
        result.artifacts.scenario_data.fundamentals = Some(serde_json::json!({"revenues": 1000}));
        result.artifacts.scenario_data.candles = vec![serde_json::json!({"close": 100.0})];
        let diagnostics = derive_scenario_minimum_diagnostics(&result);
        let errors: Vec<_> = diagnostics.iter().filter(|d| d.severity == "error").collect();
        assert!(!errors.is_empty());
    }

    #[test]
    fn scenario_minimum_hk_missing_fundamentals_soft_gap() {
        let mut result = AnalysisResult::default();
        result.artifacts.scenario_context.market = crate::AnalysisScenarioMarket::HongKong;
        result.artifacts.scenario_data.quote = Some(serde_json::json!({"price": 100.0}));
        result.artifacts.scenario_data.company_news = vec![serde_json::json!({"title": "test"})];
        result.artifacts.scenario_data.candles = vec![serde_json::json!({"close": 100.0})];
        let diagnostics = derive_scenario_minimum_diagnostics(&result);
        let warnings: Vec<_> = diagnostics.iter().filter(|d| d.severity == "warning").collect();
        assert!(warnings.iter().any(|d| d.code.contains("hk_fundamentals_soft_gap")));
    }

    // --- collect_tool_meta_details ---

    #[test]
    fn tool_meta_basic() {
        let observation = ToolObservation {
            tool_name: "get_news".to_string(),
            success: true,
            output: "test".to_string(),
            ..Default::default()
        };
        let details = collect_tool_meta_details("news", &observation);
        assert!(details.iter().any(|d| d.contains("analyst=news")));
        assert!(details.iter().any(|d| d.contains("tool=get_news")));
    }

    #[test]
    fn tool_meta_with_item_count() {
        let mut observation = ToolObservation {
            tool_name: "get_news".to_string(),
            success: true,
            output: "test".to_string(),
            ..Default::default()
        };
        observation.meta.insert("item_count".to_string(), serde_json::json!(5));
        let details = collect_tool_meta_details("news", &observation);
        assert!(details.iter().any(|d| d.contains("item_count=5")));
    }

    #[test]
    fn tool_meta_with_sources() {
        let mut observation = ToolObservation {
            tool_name: "get_news".to_string(),
            success: true,
            output: "test".to_string(),
            ..Default::default()
        };
        observation.meta.insert("sources".to_string(), serde_json::json!(["Reuters", "Bloomberg"]));
        let details = collect_tool_meta_details("news", &observation);
        assert!(details.iter().any(|d| d.contains("sources=Reuters,Bloomberg")));
    }

    #[test]
    fn tool_meta_with_fallback() {
        let mut observation = ToolObservation {
            tool_name: "get_news".to_string(),
            success: true,
            output: "test".to_string(),
            ..Default::default()
        };
        observation.meta.insert("fallback_used".to_string(), serde_json::json!(true));
        let details = collect_tool_meta_details("news", &observation);
        assert!(details.iter().any(|d| d.contains("used_alternate_public_source=true")));
    }

    #[test]
    fn tool_meta_with_failed_attempts() {
        let mut observation = ToolObservation {
            tool_name: "get_news".to_string(),
            success: true,
            output: "test".to_string(),
            ..Default::default()
        };
        observation.meta.insert("attempts".to_string(), serde_json::json!([
            {"source": "Reuters", "success": false, "item_count": 0},
            {"source": "Bloomberg", "success": true, "item_count": 3}
        ]));
        let details = collect_tool_meta_details("news", &observation);
        assert!(details.iter().any(|d| d.contains("failed_attempts=1")));
    }
}

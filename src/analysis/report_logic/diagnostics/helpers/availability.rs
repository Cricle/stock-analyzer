
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
                diagnostics.push(ReportDiagnosticItem {
                    code: "scenario_minimum_us_fundamentals_soft_gap".to_string(),
                    severity: "warning".to_string(),
                    message: "US equity analysis missing fundamentals snapshot, analysis is more price and technical driven.".into(),
                    details: vec![
                        "market=us_equity".to_string(),
                        "missing=fundamentals".to_string(),
                    ],
                    ..Default::default()
                });
            }
            if !has_company_news {
                diagnostics.push(ReportDiagnosticItem {
                    code: "scenario_minimum_us_news_soft_gap".to_string(),
                    severity: "warning".to_string(),
                    message: "US equity analysis missing company news, catalyst assessment may be incomplete.".into(),
                    details: vec![
                        "market=us_equity".to_string(),
                        "missing=company_news".to_string(),
                    ],
                    ..Default::default()
                });
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

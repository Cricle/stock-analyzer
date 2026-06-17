pub fn derive_report_diagnostics(result: &AnalysisResult) -> ReportDiagnostics {
    ReportDiagnostics {
        market: derive_market_diagnostics(result),
        fundamentals: derive_fundamentals_diagnostics(result),
        news: derive_news_diagnostics(result),
        availability: derive_availability_diagnostics(result),
    }
}

fn derive_market_diagnostics(result: &AnalysisResult) -> Vec<ReportDiagnosticItem> {
    let mut diagnostics = Vec::new();
    let state = result.analyst_runtime_state("market");

    for observation in state.into_iter().flat_map(|item| item.tool_history.iter()) {
        if observation.tool_name == "get_stock_data" && !observation.success {
            diagnostics.push(ReportDiagnosticItem {
                code: "market_data_unavailable".to_string(),
                severity: "info".to_string(),
                message: LocalText::new("market_data_incomplete"),
                details: collect_tool_meta_details("market", observation),
            ..Default::default()
            });
        }

        if observation.tool_name == "get_indicators" && observation.success {
            diagnostics.extend(parse_indicator_unavailable_diagnostics(
                &observation.output,
                &observation.meta,
            ));
        }
    }

    diagnostics
}

fn parse_indicator_unavailable_diagnostics(output: &str, meta: &Value) -> Vec<ReportDiagnosticItem> {
    let mut diagnostics = Vec::new();
    let Some(payload) = parse_indicator_payload(output, meta) else {
        return diagnostics;
    };
    if let Some(unavailable) = payload
        .get("data_gap")
        .and_then(|item| item.get("unavailable_indicators"))
        .and_then(Value::as_array)
    {
        for key in unavailable.iter().filter_map(Value::as_str) {
            diagnostics.push(ReportDiagnosticItem {
                code: "indicator_unavailable".to_string(),
                severity: "warning".to_string(),
                message: LocalText::new("indicator_unavailable_message")
                    .with_str("indicator", key.trim()),
                details: vec![format!(
                    "history_candle_count={}",
                    payload
                        .get("history_candle_count")
                        .and_then(Value::as_u64)
                        .unwrap_or_default()
                )],
                ..Default::default()
            });
        }
    }
    diagnostics
}

fn parse_indicator_payload<'a>(output: &'a str, meta: &'a Value) -> Option<Value> {
    serde_json::from_str::<Value>(output)
        .ok()
        .or_else(|| meta.get("payload").cloned())
}

fn parse_indicator_items(output: &str, meta: &Value) -> Option<Vec<(String, f64)>> {
    let payload = parse_indicator_payload(output, meta)?;
    let items = payload.get("indicators")?.as_array()?;
    Some(
        items
            .iter()
            .filter_map(|item| {
                let key = item.get("key")?.as_str()?.trim().to_string();
                let value = item.get("value").and_then(json_number)?;
                Some((key, value))
            })
            .collect(),
    )
}

fn derive_fundamentals_diagnostics(result: &AnalysisResult) -> Vec<ReportDiagnosticItem> {
    let state = result.analyst_runtime_state("fundamentals");
    let mut merged = serde_json::Map::new();

    for observation in state.into_iter().flat_map(|item| item.tool_history.iter()) {
        if (observation.tool_name == "get_fundamentals"
            || observation.tool_name == "get_income_statement")
            && let Ok(value) = serde_json::from_str::<serde_json::Value>(&observation.output)
                && let Some(object) = value.as_object()
            {
                for (key, value) in object {
                    merged.insert(key.clone(), value.clone());
                }
            }
    }

    if merged.is_empty() {
        return Vec::new();
    }

    let revenues = merged.get("revenues").and_then(json_number);
    let gross_profit = merged.get("gross_profit").and_then(json_number);
    let operating_income = merged.get("operating_income").and_then(json_number);
    let net_income = merged.get("net_income").and_then(json_number);
    let operating_cash_flow = merged.get("operating_cash_flow").and_then(json_number);
    let free_cash_flow = merged.get("free_cash_flow").and_then(json_number);
    let accounts_receivable = merged.get("accounts_receivable").and_then(json_number);
    let inventory = merged.get("inventory").and_then(json_number);
    let prepayments = merged
        .get("prepayments")
        .and_then(json_number)
        .or_else(|| merged.get("prepaid_expenses").and_then(json_number))
        .or_else(|| merged.get("prepaid_assets").and_then(json_number));
    let mut details = Vec::new();

    if let (Some(left), Some(right)) = (gross_profit, revenues)
        && left > right
    {
        details.push("gross_profit > revenues".to_string());
    }
    if let (Some(left), Some(right)) = (operating_income, revenues)
        && left > right
    {
        details.push("operating_income > revenues".to_string());
    }
    if let (Some(left), Some(right)) = (net_income, revenues)
        && left > right
    {
        details.push("net_income > revenues".to_string());
    }
    if let (Some(left), Some(right)) = (free_cash_flow, operating_cash_flow)
        && left > right * 1.2
    {
        details.push("free_cash_flow >> operating_cash_flow".to_string());
    }

    // Validate P/E range — abnormally low (<5) or high (>200) values indicate
    // period mixing, one-time items, or data source inconsistency.
    let market_cap = merged.get("market_cap").and_then(json_number);
    if let (Some(mc), Some(ni)) = (market_cap, net_income)
        && ni > 0.0
        && mc > 0.0
    {
        let pe = mc / ni;
        if pe < 5.0 {
            details.push(format!("pe_ratio={pe:.1}<5_suspicious_low"));
        } else if pe > 200.0 {
            details.push(format!("pe_ratio={pe:.1}>200_suspicious_high"));
        }
    }

    if details.is_empty() {
        if let (Some(net_income), Some(operating_cash_flow)) = (net_income, operating_cash_flow)
            && net_income > 0.0
            && operating_cash_flow < 0.0
            && accounts_receivable.is_none()
            && inventory.is_none()
            && prepayments.is_none()
        {
            vec![ReportDiagnosticItem {
                code: "cashflow_quality_unresolved".to_string(),
                severity: "warning".to_string(),
                message: "Profit and operating cash flow diverge, but without receivables, inventory, and prepayment breakdown, this is only reasonable suspicion, not definitive fundamental deterioration.".into(),
                details: vec![
                    "missing_working_capital_breakdown=accounts_receivable,inventory,prepayments".to_string(),
                    "follow_up_priority=separate_receivable_inventory_prepayment_drivers".to_string(),
                    "possible_branches=receivable_collection_slowdown,inventory_build_for_delivery,prepayment_capacity_lockup,revenue_recognition_timing".to_string(),
                    "first_check=accounts_receivable_days,inventory_turnover,prepayment_delta".to_string(),
                ],
                ..Default::default()
            }]
        } else {
            Vec::new()
        }
    } else {
        vec![ReportDiagnosticItem {
            code: "fundamentals_period_mixed".to_string(),
            severity: "warning".to_string(),
            message: "Period mixing or methodology conflict between fundamental fields.".into(),
            details,
            ..Default::default()
        }]
    }
}

fn derive_news_diagnostics(result: &AnalysisResult) -> Vec<ReportDiagnosticItem> {
    let mut diagnostics = Vec::new();
    let mut source_set = std::collections::BTreeSet::new();
    let mut successful_attempts = 0usize;
    let mut failed_attempts = 0usize;

    for state in result.artifacts.analyst_runtime_states.iter() {
        if state.key != "news" && state.key != "sentiment" {
            continue;
        }
        for observation in &state.tool_history {
            if let Some(data_gap) = observation.meta.get("data_gap").and_then(|value| value.as_object()) {
                diagnostics.push(ReportDiagnosticItem {
                    code: data_gap
                        .get("kind")
                        .and_then(|value| value.as_str())
                        .unwrap_or("news_data_gap")
                        .to_string(),
                    severity: "warning".to_string(),
                    message: LocalText::new("news_data_gap_message")
                        .with_str("detail", data_gap
                            .get("message")
                            .and_then(|value| value.as_str())
                            .unwrap_or("news_data_gap")),
                    details: collect_tool_meta_details(state.key.as_str(), observation),
                ..Default::default()
                });
            }
            if !observation.success
                && (observation.tool_name == "get_global_news"
                    || observation.tool_name == "get_news")
            {
                diagnostics.push(ReportDiagnosticItem {
                    code: "news_upstream_unavailable".to_string(),
                    severity: "warning".to_string(),
                    message: "News or macro upstream unavailable in this run.".into(),
                    details: collect_tool_meta_details(state.key.as_str(), observation),
                ..Default::default()
                });
            }
            if let Some(sources) = observation.meta.get("sources").and_then(|value| value.as_array()) {
                for source in sources.iter().filter_map(Value::as_str) {
                    let normalized = source.trim();
                    if !normalized.is_empty() {
                        source_set.insert(normalized.to_string());
                    }
                }
            }
            if let Some(attempts) = observation.meta.get("attempts").and_then(|value| value.as_array()) {
                for attempt in attempts {
                    if attempt
                        .get("success")
                        .and_then(Value::as_bool)
                        .unwrap_or(false)
                    {
                        successful_attempts += 1;
                    } else {
                        failed_attempts += 1;
                    }
                }
            }
        }
    }

    if !source_set.is_empty() && source_set.len() <= 1 {
        diagnostics.push(ReportDiagnosticItem {
            code: "news_source_concentration".to_string(),
            severity: "info".to_string(),
            message: "News evidence sources are limited; conclusions are better treated as clues than high-confidence catalyst assessments.".into(),
            details: vec![format!(
                "sources={}",
                source_set.iter().cloned().collect::<Vec<_>>().join(",")
            )],
            ..Default::default()
        });
    }

    if failed_attempts > successful_attempts && (successful_attempts + failed_attempts) > 0 {
        diagnostics.push(ReportDiagnosticItem {
            code: "news_fetch_coverage_weak".to_string(),
            severity: "warning".to_string(),
            message: "News retrieval failures exceed successes; news evidence coverage is weak.".into(),
            details: vec![
                format!("successful_attempts={successful_attempts}"),
                format!("failed_attempts={failed_attempts}"),
            ],
            ..Default::default()
        });
    }

    let mut deduped = Vec::new();
    for item in diagnostics {
        let duplicate = deduped.iter().any(|existing: &ReportDiagnosticItem| {
            existing.code == item.code
                && existing.message == item.message
                && existing.details == item.details
        });
        if !duplicate {
            deduped.push(item);
        }
    }

    if let Some(item) = detect_disclosure_sequence_complexity(result, &derive_news_reference_facts(result)) {
        let duplicate = deduped.iter().any(|existing: &ReportDiagnosticItem| {
            existing.code == item.code
                && existing.message == item.message
                && existing.details == item.details
        });
        if !duplicate {
            deduped.push(item);
        }
    }

    deduped
}

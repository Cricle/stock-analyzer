
pub fn detect_disclosure_sequence_complexity(
    result: &AnalysisResult,
    news_items: &[ReferenceFactItem],
) -> Option<ReportDiagnosticItem> {
    let mut filing_dates = Vec::new();
    let mut sec_host_count = std::collections::HashSet::new();

    for item in news_items {
        if item.key == "news_item"
            && is_regulatory_reference_source(item)
            && let Some(date) = parse_news_date(&item.label)
        {
            filing_dates.push(date);
            if let Some(host) = parse_url_host(&item.url) {
                sec_host_count.insert(host);
            }
        }
    }

    if filing_dates.len() < 2 {
        return None;
    }

    let insider_signal_present = result
        .artifacts
        .analyst_runtime_states
        .iter()
        .flat_map(|state| state.tool_history.iter())
        .any(|observation| {
            observation.tool_name == "get_insider_transactions"
                && observation.success
                && observation
                    .meta
                    .get("item_count")
                    .and_then(Value::as_u64)
                    .unwrap_or_default()
                    > 0
        });

    filing_dates.sort_unstable();
    let clustered = filing_dates
        .windows(2)
        .any(|pair| pair[1].signed_duration_since(pair[0]).num_days().abs() <= 14);
    if !clustered || (!insider_signal_present && filing_dates.len() < 3) {
        return None;
    }

    let span_days = filing_dates
        .first()
        .zip(filing_dates.last())
        .map(|(first, last)| last.signed_duration_since(*first).num_days())
        .unwrap_or_default();

    Some(ReportDiagnosticItem {
        code: "disclosure_sequence_complexity".to_string(),
        severity: "warning".to_string(),
        message: "近期披露更像注册、发行或减持等资本市场序列，当前应先厘清供给与融资安排，不能把它直接视为经营催化。".into(),
        details: vec![
            format!("filing_count={}", filing_dates.len()),
            format!("regulatory_source_count={}", sec_host_count.len()),
            format!("insider_signal_present={insider_signal_present}"),
            format!("cluster_span_days={span_days}"),
        ],
        ..Default::default()
    })
}

fn parse_news_date(value: &str) -> Option<NaiveDate> {
    let date = value.trim();
    NaiveDate::parse_from_str(date, "%Y-%m-%d")
        .ok()
        .or_else(|| NaiveDate::parse_from_str(date, "%Y/%m/%d").ok())
}

fn parse_url_host(value: &str) -> Option<String> {
    Url::parse(value.trim())
        .ok()
        .and_then(|url| url.host_str().map(|host| host.to_ascii_lowercase()))
}

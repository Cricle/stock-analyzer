
fn derive_news_reference_facts(result: &AnalysisResult) -> Vec<ReferenceFactItem> {
    let mut facts = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let analysis_date = result.analysis_date.trim();
    let is_a_share = matches!(
        result.artifacts.scenario_context.market,
        crate::AnalysisScenarioMarket::AShare
    );

    // Try tool_history first
    for state in &result.artifacts.analyst_runtime_states {
        if !matches!(state.key.as_str(), "news" | "sentiment") {
            continue;
        }
        for observation in &state.tool_history {
            if is_a_share && observation.tool_name == "get_insider_transactions" {
                continue;
            }
            if !matches!(
                observation.tool_name.as_str(),
                "get_news" | "get_insider_transactions"
            ) || !observation.success
            {
                continue;
            }
            let mut current: Option<ReferenceFactItem> = None;
            for line in observation.output.lines().map(str::trim) {
                if line.is_empty() {
                    continue;
                }
                let parts = line.split('|').map(str::trim).collect::<Vec<_>>();
                if parts.len() >= 3 && parts[0].contains('.') {
                    if let Some(item) = current.take()
                        && !item.label.is_empty()
                        && !item.value.is_empty()
                        && seen.insert(news_dedupe_key(&item))
                    {
                        facts.push(item);
                    }
                    let date = parts[0]
                        .split_whitespace()
                        .nth(1)
                        .unwrap_or_default()
                        .to_string();
                    let timing_prefix = if !date.is_empty() && !analysis_date.is_empty() && *date <= *analysis_date {
                        "[已公布] "
                    } else if !date.is_empty() && !analysis_date.is_empty() {
                        "[待公布] "
                    } else {
                        ""
                    };
                    current = Some(ReferenceFactItem {
                        key: "news_item".to_string(),
                        label: date,
                        value: format!("{timing_prefix}{}", parts[2..].join(" | ")),
                        emphasis: parts.get(1).copied().unwrap_or("info").to_string(),
                        ..Default::default()
                    });
                    continue;
                }
                if let Some(item) = current.as_mut() {
                    if let Some(summary) = line.strip_prefix("Summary:") {
                        item.summary = summary.trim().to_string();
                    } else if let Some(url) = line.strip_prefix("URL:") {
                        item.url = url.trim().to_string();
                    }
                }
            }
            if let Some(item) = current.take()
                && !item.label.is_empty()
                && !item.value.is_empty()
                && seen.insert(news_dedupe_key(&item))
            {
                facts.push(item);
            }
        }
    }

    // Fallback to scenario_data.company_news when tool_history produced nothing
    if facts.is_empty() {
        for item in &result.artifacts.scenario_data.company_news {
            let date = item.published_at.trim();
            let timing_prefix = if !date.is_empty() && !analysis_date.is_empty() && *date <= *analysis_date {
                "[已公布] "
            } else if !date.is_empty() && !analysis_date.is_empty() {
                "[待公布] "
            } else {
                ""
            };
            let title = if !item.title.trim().is_empty() {
                item.title.trim()
            } else {
                item.summary.trim()
            };
            if title.is_empty() {
                continue;
            }
            let dedup_key = format!("{}|{}", date, title).to_ascii_lowercase();
            if !seen.insert(dedup_key) {
                continue;
            }
            facts.push(ReferenceFactItem {
                key: "news_item".to_string(),
                label: date.to_string(),
                value: format!("{timing_prefix}{title}"),
                emphasis: item.source.clone(),
                summary: item.summary.clone(),
                url: item.url.clone().unwrap_or_default(),
            });
            if facts.len() >= 6 {
                break;
            }
        }
    }

    facts.truncate(6);
    facts
}

fn news_dedupe_key(item: &ReferenceFactItem) -> String {
    if !item.url.trim().is_empty() {
        return item.url.trim().to_ascii_lowercase();
    }
    format!(
        "{}|{}|{}",
        item.label.trim(),
        item.emphasis.trim(),
        item.value.trim()
    )
    .to_ascii_lowercase()
}

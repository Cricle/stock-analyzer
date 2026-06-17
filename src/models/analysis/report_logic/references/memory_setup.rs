
fn derive_news_quality_reference_facts(result: &AnalysisResult) -> Vec<ReferenceFactItem> {
    let mut source_set = std::collections::BTreeSet::new();
    let mut regulatory_count = 0usize;
    let mut successful_attempts = 0usize;
    let mut failed_attempts = 0usize;

    for state in &result.artifacts.analyst_runtime_states {
        if !matches!(state.key.as_str(), "news" | "sentiment") {
            continue;
        }
        for observation in &state.tool_history {
            if !matches!(
                observation.tool_name.as_str(),
                "get_news" | "get_global_news" | "get_insider_transactions"
            ) {
                continue;
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

    let news_items = derive_news_reference_facts(result);
    for item in &news_items {
        if item.key == "news_item" && is_regulatory_reference_source(item) {
            regulatory_count += 1;
        }
        let source = item.emphasis.trim();
        if !source.is_empty() {
            source_set.insert(source.to_string());
        }
    }

    let total_news_items = news_items
        .iter()
        .filter(|item| item.key == "news_item")
        .count();

    let mut facts = Vec::new();
    if total_news_items > 0 {
        facts.push(ReferenceFactItem {
            key: "news_source_diversity".to_string(),
            label: "News Source Count".to_string(),
            value: source_set.len().to_string(),
            emphasis: if source_set.len() >= 3 {
                "success".to_string()
            } else {
                "warning".to_string()
            },
            summary: if source_set.is_empty() {
                String::new()
            } else {
                source_set.iter().take(5).cloned().collect::<Vec<_>>().join(", ")
            },
            ..Default::default()
        });
        facts.push(ReferenceFactItem {
            key: "regulatory_news_share".to_string(),
            label: "Regulatory/Disclosure Clue Ratio".to_string(),
            value: format!("{}/{}", regulatory_count, total_news_items),
            emphasis: if regulatory_count * 2 >= total_news_items {
                "warning".to_string()
            } else {
                "info".to_string()
            },
            summary: "High ratio means news leans toward disclosure/supply complexity clues, not operational catalysts".to_string(),
            ..Default::default()
        });
    }
    if successful_attempts + failed_attempts > 0 {
        facts.push(ReferenceFactItem {
            key: "news_fetch_attempts".to_string(),
            label: "News Retrieval Success/Failure".to_string(),
            value: format!("{successful_attempts}/{failed_attempts}"),
            emphasis: if failed_attempts == 0 {
                "success".to_string()
            } else {
                "warning".to_string()
            },
            summary: "Determines whether news evidence coverage is complete".to_string(),
            ..Default::default()
        });
    }
    facts
}

fn derive_memory_reference_facts(
    confidence_breakdown: &ConfidenceBreakdown,
    memory_context: &MemoryContextSnapshot,
) -> Vec<ReferenceFactItem> {
    let mut facts = vec![
        ReferenceFactItem {
            key: "research_raw_score".to_string(),
            label: "Research Raw Score".to_string(),
            value: format!("{}/100", confidence_breakdown.total_before_caps),
            emphasis: "primary".to_string(),
            ..Default::default()
        },
        ReferenceFactItem {
            key: "verified_setup_samples".to_string(),
            label: "Validated Calibration Samples".to_string(),
            value: memory_context
                .setup_calibration_sample_count
                .max(memory_context.setup_resolved_match_count)
                .to_string(),
            emphasis: "info".to_string(),
            ..Default::default()
        },
        ReferenceFactItem {
            key: "setup_hit_rate".to_string(),
            label: "Similar Sample Hit Rate".to_string(),
            value: format!("{:.0}%", memory_context.setup_match_hit_rate * 100.0),
            emphasis: "info".to_string(),
            ..Default::default()
        },
        ReferenceFactItem {
            key: "setup_avg_alpha".to_string(),
            label: "Similar Sample Avg Alpha".to_string(),
            value: format!(
                "{:.1}%",
                memory_context.setup_match_avg_alpha_return * 100.0
            ),
            emphasis: if memory_context.setup_match_avg_alpha_return > 0.0 {
                "success".to_string()
            } else {
                "warning".to_string()
            },
            ..Default::default()
        },
        ReferenceFactItem {
            key: "same_vs_cross_ticker_samples".to_string(),
            label: "Same/Cross-Ticker Samples".to_string(),
            value: format!(
                "{} / {}",
                memory_context.same_ticker_count, memory_context.cross_ticker_count
            ),
            emphasis: "info".to_string(),
            ..Default::default()
        },
    ];

    for item in memory_context.historical_same_ticker_highlights.iter().take(2) {
        facts.push(ReferenceFactItem {
            key: "same_ticker_history".to_string(),
            label: format!("Same-Ticker History {} {}", item.trade_date, item.ticker),
            value: if item.summary.trim().is_empty() {
                format!(
                    "{} / {}",
                    item.rating,
                    if item.action.trim().is_empty() {
                        "na"
                    } else {
                        item.action.trim()
                    }
                )
            } else {
                item.summary.clone()
            },
            emphasis: "info".to_string(),
            summary: [
                (!item.key_risk.trim().is_empty()).then(|| format!("Risk: {}", item.key_risk.trim())),
                (!item.lesson.trim().is_empty()).then(|| format!("Review: {}", item.lesson.trim())),
            ]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(" | "),
            ..Default::default()
        });
    }

    for item in memory_context.historical_cross_ticker_highlights.iter().take(2) {
        facts.push(ReferenceFactItem {
            key: "cross_ticker_lesson".to_string(),
            label: format!("Cross-Ticker Sample {} {}", item.trade_date, item.ticker),
            value: if item.lesson.trim().is_empty() {
                item.summary.clone()
            } else {
                item.lesson.clone()
            },
            emphasis: "info".to_string(),
            summary: [
                (!item.summary.trim().is_empty()).then(|| format!("Summary: {}", item.summary.trim())),
                (!item.key_risk.trim().is_empty()).then(|| format!("Risk: {}", item.key_risk.trim())),
            ]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(" | "),
            ..Default::default()
        });
    }

    facts
}

fn format_number_compact(value: f64) -> String {
    let abs = value.abs();
    if abs >= 1_000_000_000_000.0 {
        format!("{:.2}T", value / 1_000_000_000_000.0)
    } else if abs >= 1_000_000_000.0 {
        format!("{:.2}B", value / 1_000_000_000.0)
    } else if abs >= 1_000_000.0 {
        format!("{:.2}M", value / 1_000_000.0)
    } else if abs >= 1_000.0 {
        format!("{:.2}K", value / 1_000.0)
    } else {
        format!("{value:.2}")
    }
}

fn humanize_setup_tag(tag: &str) -> &'static str {
    match tag.trim() {
        "trend_confirmed" => "Trend confirmed",
        "fundamental_quality" => "Strong fundamentals",
        "event_driven" => "Event catalyst present",
        "watchlist_only" => "Better suited for watchlist",
        "execution_ready" => "Execution conditions largely ready",
        "conditional_entry" => "Waiting for better entry",
        "conditional_breakout" => "Awaiting breakout confirmation",
        "conditional_pullback_zone" => "Awaiting pullback zone confirmation",
        "high_crowding" => "High crowding",
        "overextended" => "Price overheated near-term",
        _ => "",
    }
}

fn summarize_setup_tags(tags: &[String]) -> Option<String> {
    let items = tags
        .iter()
        .filter_map(|item| {
            let humanized = humanize_setup_tag(item);
            (!humanized.is_empty()).then_some(humanized)
        })
        .collect::<Vec<_>>();
    (!items.is_empty()).then(|| items.join("、"))
}

fn derive_setup_match_explanation(
    memory_context: &MemoryContextSnapshot,
    fallback_sample_count: usize,
) -> SetupMatchExplanation {
    let mut details = Vec::new();
    if let Some(summary) = summarize_setup_tags(&memory_context.resolved_setup_tags) {
        details.push(format!("Resolved structural features this round:: {summary}"));
    } else if let Some(summary) = summarize_setup_tags(&memory_context.setup_tags) {
        details.push(format!("Preliminary structural features this round:: {summary}"));
    }
    if memory_context.setup_pending_match_count > 0 {
        details.push(format!(
            "Pending settlement samples:: {}",
            memory_context.setup_pending_match_count
        ));
    }
    if memory_context.setup_resolved_match_count > 0 {
        details.push(format!(
            "Verified samples:: {}，Hit rate {:.0}%，Avg alpha return {:.1}%",
            memory_context.setup_resolved_match_count,
            memory_context.setup_match_hit_rate * 100.0,
            memory_context.setup_match_avg_alpha_return * 100.0
        ));
    }

    if !memory_context.used_setup_filtered_retrieval {
        return SetupMatchExplanation {
            reason_code: "setup_filter_not_used".to_string(),
            summary: "No strict setup filter enabled this round; history serves as broad background reference.".to_string(),
            details,
            fallback_used: false,
            fallback_sample_count,
        };
    }

    if memory_context.setup_resolved_match_count > 0 {
        return SetupMatchExplanation {
            reason_code: "resolved_setup_matches_available".to_string(),
            summary: "Found reviewable similar history this round; these samples serve as direct statistical reference.".to_string(),
            details,
            fallback_used: false,
            fallback_sample_count,
        };
    }

    if memory_context.setup_calibration_sample_count > 0 {
        details.push(format!(
            "Verified fallback samples for weak calibration:: {}",
            memory_context.setup_calibration_sample_count
        ));
        return SetupMatchExplanation {
            reason_code: "pending_only_with_verified_fallback_samples".to_string(),
            summary: "Strictly similar setups not yet settled, but verified same-ticker or same-market seed samples provide historical boundary reference.".to_string(),
            details,
            fallback_used: true,
            fallback_sample_count: memory_context.setup_calibration_sample_count,
        };
    }

    if memory_context.setup_pending_match_count > 0 {
        return SetupMatchExplanation {
            reason_code: "pending_only_setup_matches".to_string(),
            summary: "Found similar setups, but samples not yet settled; can only serve as pending clues for now.".to_string(),
            details,
            fallback_used: fallback_sample_count > 0,
            fallback_sample_count,
        };
    }

    if fallback_sample_count > 0 {
        details.push(format!("Fallback samples for weak calibration:: {fallback_sample_count}"));
        return SetupMatchExplanation {
            reason_code: "no_strict_match_fallback_to_market_samples".to_string(),
            summary: "No strict setup match; borrowed verified same-ticker or same-market samples to supplement historical boundary.".to_string(),
            details,
            fallback_used: true,
            fallback_sample_count,
        };
    }

    SetupMatchExplanation {
        reason_code: "no_matching_setup_history".to_string(),
        summary: "No directly reviewable similar history; conclusion relies mainly on current evidence with history as weak reference.".to_string(),
        details,
        fallback_used: false,
        fallback_sample_count,
    }
}

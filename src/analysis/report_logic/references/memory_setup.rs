
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
            label: "新闻来源数".to_string(),
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
            label: "监管/披露线索占比".to_string(),
            value: format!("{}/{}", regulatory_count, total_news_items),
            emphasis: if regulatory_count * 2 >= total_news_items {
                "warning".to_string()
            } else {
                "info".to_string()
            },
            summary: "占比高时，新闻更偏披露/供给复杂度线索，不等于经营催化".to_string(),
            ..Default::default()
        });
    }
    if successful_attempts + failed_attempts > 0 {
        facts.push(ReferenceFactItem {
            key: "news_fetch_attempts".to_string(),
            label: "新闻检索成功/失败".to_string(),
            value: format!("{successful_attempts}/{failed_attempts}"),
            emphasis: if failed_attempts == 0 {
                "success".to_string()
            } else {
                "warning".to_string()
            },
            summary: "用于判断本次新闻证据覆盖是否完整".to_string(),
            ..Default::default()
        });
    }
    facts
}

pub fn derive_memory_reference_facts(
    confidence_breakdown: &ConfidenceBreakdown,
    memory_context: &MemoryContextSnapshot,
) -> Vec<ReferenceFactItem> {
    let mut facts = vec![
        ReferenceFactItem {
            key: "research_raw_score".to_string(),
            label: "研究原始分".to_string(),
            value: format!("{}/100", confidence_breakdown.total_before_caps),
            emphasis: "primary".to_string(),
            ..Default::default()
        },
        ReferenceFactItem {
            key: "verified_setup_samples".to_string(),
            label: "已验证校准样本".to_string(),
            value: memory_context
                .setup_calibration_sample_count
                .max(memory_context.setup_resolved_match_count)
                .to_string(),
            emphasis: "info".to_string(),
            ..Default::default()
        },
        ReferenceFactItem {
            key: "setup_hit_rate".to_string(),
            label: "相似样本命中率".to_string(),
            value: format!("{:.0}%", memory_context.setup_match_hit_rate * 100.0),
            emphasis: "info".to_string(),
            ..Default::default()
        },
        ReferenceFactItem {
            key: "setup_avg_alpha".to_string(),
            label: "相似样本平均超额收益".to_string(),
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
            label: "同票/跨票样本".to_string(),
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
            label: format!("同票历史 {} {}", item.trade_date, item.ticker),
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
                (!item.key_risk.trim().is_empty()).then(|| format!("风险: {}", item.key_risk.trim())),
                (!item.lesson.trim().is_empty()).then(|| format!("复盘: {}", item.lesson.trim())),
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
            label: format!("跨票样本 {} {}", item.trade_date, item.ticker),
            value: if item.lesson.trim().is_empty() {
                item.summary.clone()
            } else {
                item.lesson.clone()
            },
            emphasis: "info".to_string(),
            summary: [
                (!item.summary.trim().is_empty()).then(|| format!("摘要: {}", item.summary.trim())),
                (!item.key_risk.trim().is_empty()).then(|| format!("风险: {}", item.key_risk.trim())),
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
        "trend_confirmed" => "趋势已确认",
        "fundamental_quality" => "基本面质量较强",
        "event_driven" => "存在事件催化",
        "watchlist_only" => "暂时更适合观察名单",
        "execution_ready" => "执行条件较完整",
        "conditional_entry" => "需要等待更好入场条件",
        "conditional_breakout" => "等待条件突破确认",
        "conditional_pullback_zone" => "等待条件回踩区间确认",
        "high_crowding" => "拥挤度较高",
        "overextended" => "价格阶段性偏热",
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

pub fn derive_setup_match_explanation(
    memory_context: &MemoryContextSnapshot,
    fallback_sample_count: usize,
) -> SetupMatchExplanation {
    let mut details = Vec::new();
    if let Some(summary) = summarize_setup_tags(&memory_context.resolved_setup_tags) {
        details.push(format!("本轮最终更接近的结构特征: {summary}"));
    } else if let Some(summary) = summarize_setup_tags(&memory_context.setup_tags) {
        details.push(format!("本轮初步识别到的结构特征: {summary}"));
    }
    if memory_context.setup_pending_match_count > 0 {
        details.push(format!(
            "待结算样本: {}",
            memory_context.setup_pending_match_count
        ));
    }
    if memory_context.setup_resolved_match_count > 0 {
        details.push(format!(
            "已验证样本: {}，命中率 {:.0}%，平均超额收益 {:.1}%",
            memory_context.setup_resolved_match_count,
            memory_context.setup_match_hit_rate * 100.0,
            memory_context.setup_match_avg_alpha_return * 100.0
        ));
    }

    if !memory_context.used_setup_filtered_retrieval {
        return SetupMatchExplanation {
            reason_code: "setup_filter_not_used".to_string(),
            summary: "本轮没有启用严格 setup 过滤，历史部分主要作为宽口径背景参考。".to_string(),
            details,
            fallback_used: false,
            fallback_sample_count,
        };
    }

    if memory_context.setup_resolved_match_count > 0 {
        return SetupMatchExplanation {
            reason_code: "resolved_setup_matches_available".to_string(),
            summary: "本轮找到了可复盘的相似历史，这批样本可以直接作为本次判断的统计参照。".to_string(),
            details,
            fallback_used: false,
            fallback_sample_count,
        };
    }

    if memory_context.setup_calibration_sample_count > 0 {
        details.push(format!(
            "用于弱校准的已验证回退样本: {}",
            memory_context.setup_calibration_sample_count
        ));
        return SetupMatchExplanation {
            reason_code: "pending_only_with_verified_fallback_samples".to_string(),
            summary: "严格相似的 setup 还没结算完，但已有同票或同市场的已验证 seed 样本可提供历史边界参考。".to_string(),
            details,
            fallback_used: true,
            fallback_sample_count: memory_context.setup_calibration_sample_count,
        };
    }

    if memory_context.setup_pending_match_count > 0 {
        return SetupMatchExplanation {
            reason_code: "pending_only_setup_matches".to_string(),
            summary: "已经找到相似 setup，但这些样本尚未结算，因此暂时只能当作待验证线索。".to_string(),
            details,
            fallback_used: fallback_sample_count > 0,
            fallback_sample_count,
        };
    }

    if fallback_sample_count > 0 {
        details.push(format!("用于弱校准的回退样本: {fallback_sample_count}"));
        return SetupMatchExplanation {
            reason_code: "no_strict_match_fallback_to_market_samples".to_string(),
            summary: "没有命中严格相似 setup，但已借用同票或同市场的已验证样本补足历史边界。".to_string(),
            details,
            fallback_used: true,
            fallback_sample_count,
        };
    }

    SetupMatchExplanation {
        reason_code: "no_matching_setup_history".to_string(),
        summary: "目前缺少可直接复盘的相似历史，这次结论主要依赖当期证据，历史部分只能弱参考。".to_string(),
        details,
        fallback_used: false,
        fallback_sample_count,
    }
}

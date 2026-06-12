//! News fetching and impact classification for guidance reports.

use super::*;

impl DailyGuidanceGenerator {
    pub(super) async fn fetch_guidance_news(
        &self,
        market: &GuidanceMarket,
        date: &str,
    ) -> (Vec<GuidanceNewsItem>, Vec<String>) {
        // Use akshare global news (CLS, THS, Sina, Futu) as primary source
        let market_symbol = match market {
            GuidanceMarket::AShare => "000001.SH",
            GuidanceMarket::HongKong => "2800.HK",
            GuidanceMarket::UsEquity => "SPY",
            GuidanceMarket::All => "000001.SH",
        };
        let items = self
            .market_data
            .fetch_global_news(market_symbol, date, 7, 30)
            .await
            .unwrap_or_default();

        let mut guidance_items = Vec::new();
        let mut sources = Vec::new();
        let mut dedup = std::collections::HashSet::new();

        for item in items {
            let normalized_title = item
                .title
                .replace("&amp;", "&")
                .replace("&lt;", "<")
                .replace("&gt;", ">")
                .replace("&quot;", "\"")
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
                .to_ascii_lowercase();
            let dedup_key = format!(
                "{}:{}",
                normalized_title,
                item.source.to_ascii_lowercase()
            );
            if dedup.insert(dedup_key) {
                let impact = self.classify_news_impact(&item.title, &item.summary);
                guidance_items.push(GuidanceNewsItem {
                    title: item.title,
                    summary: item.summary,
                    source: item.source.clone(),
                    published_at: item.published_at,
                    url: item.url,
                    impact,
                    affected_entities: Vec::new(),
                });
                if !sources.contains(&item.source) {
                    sources.push(item.source);
                }
            }
        }

        guidance_items.sort_by(|a, b| b.published_at.cmp(&a.published_at));
        guidance_items.truncate(30);

        // LLM-based sentiment enrichment (batch classify top items)
        if let Some(llm) = &self.llm
            && let Err(e) = self.enrich_sentiment_with_llm(llm, &mut guidance_items).await
        {
            tracing::warn!("LLM sentiment enrichment failed, keeping keyword results: {e}");
        }

        (guidance_items, sources)
    }

    /// Use LLM to batch-classify news sentiment for items still marked "neutral".
    async fn enrich_sentiment_with_llm(
        &self,
        llm: &crate::engine::llm::LlmClient,
        items: &mut [GuidanceNewsItem],
    ) -> anyhow::Result<()> {
        // Only re-classify items that the keyword classifier marked as neutral
        let neutral_indices: Vec<usize> = items
            .iter()
            .enumerate()
            .filter(|(_, n)| n.impact == "neutral")
            .take(15)
            .map(|(i, _)| i)
            .collect();

        if neutral_indices.is_empty() {
            return Ok(());
        }

        let items_json: Vec<serde_json::Value> = neutral_indices
            .iter()
            .enumerate()
            .map(|(i, &idx)| {
                let summary = if items[idx].summary.len() > 200 {
                    let end = items[idx].summary.floor_char_boundary(200);
                    &items[idx].summary[..end]
                } else {
                    &items[idx].summary
                };
                serde_json::json!({
                    "id": i,
                    "title": items[idx].title,
                    "summary": summary,
                })
            })
            .collect();

        let prompt = format!(
            r#"You are a financial news sentiment classifier. Classify each news item's market impact.

Return a JSON array of objects with keys: id (int), impact ("positive"|"negative"|"neutral"), entities (array of stock/market names mentioned).

Rules:
- "positive" = bullish signals: earnings beat, upgrades, policy stimulus, institutional buying, sector rotation into
- "negative" = bearish signals: earnings miss, downgrades, policy tightening, institutional selling, scandals
- "neutral" = informational only: website descriptions, general market commentary without clear direction
- Only classify based on actual financial content, not website metadata or navigation text

News items:
{}"#,
            serde_json::to_string_pretty(&items_json)?
        );

        let response = llm.generate(&prompt).await?;

        // Parse JSON array from response (handle markdown code blocks)
        let json_str = response
            .trim()
            .strip_prefix("```json")
            .or_else(|| response.trim().strip_prefix("```"))
            .unwrap_or(response.trim())
            .strip_suffix("```")
            .unwrap_or(response.trim())
            .trim();

        let parsed: Vec<serde_json::Value> = serde_json::from_str(json_str)
            .map_err(|e| anyhow::anyhow!("failed to parse LLM sentiment JSON: {e}"))?;

        for entry in &parsed {
            if let (Some(id), Some(impact)) = (
                entry.get("id").and_then(|v| v.as_u64()),
                entry.get("impact").and_then(|v| v.as_str()),
            ) {
                let llm_idx = id as usize;
                if let Some(&orig_idx) = neutral_indices.get(llm_idx) {
                    if impact == "positive" || impact == "negative" {
                        items[orig_idx].impact = impact.to_string();
                    }
                    // Extract affected entities
                    if let Some(entities) = entry.get("entities").and_then(|v| v.as_array()) {
                        let entity_list: Vec<String> = entities
                            .iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect();
                        if !entity_list.is_empty() {
                            items[orig_idx].affected_entities = entity_list;
                        }
                    }
                }
            }
        }

        let reclassified = parsed
            .iter()
            .filter(|e| {
                e.get("impact")
                    .and_then(|v| v.as_str())
                    .is_some_and(|i| i != "neutral")
            })
            .count();
        tracing::info!(
            reclassified,
            total_neutral = neutral_indices.len(),
            "LLM sentiment enrichment applied"
        );

        Ok(())
    }

    fn classify_news_impact(&self, title: &str, summary: &str) -> String {
        classify_impact(title, summary)
    }
}

/// Check whether `keyword` is preceded by a negation word within the
/// preceding 10 characters of `text`.
fn has_negation_before(text: &str, keyword: &str) -> bool {
    if let Some(pos) = text.find(keyword) {
        // Find the char boundary at or before `pos - 10` to avoid panicking
        // on multi-byte UTF-8 (e.g. Chinese characters).
        let target = pos.saturating_sub(10);
        let start = text.floor_char_boundary(target);
        let prefix = &text[start..pos];
        ["not ", "no ", "\u{975e}", "\u{4e0d}", "\u{65e0}"]
            .iter()
            .any(|n| prefix.contains(n))
    } else {
        false
    }
}

/// Standalone impact classification so it can be unit-tested without
/// constructing a full `DailyGuidanceGenerator`.
fn classify_impact(title: &str, summary: &str) -> String {
    let text = format!("{} {}", title, summary).to_ascii_lowercase();
    let positive_words = [
        "surge", "rally", "gain", "rise", "bullish", "upgrade", "outperform",
        "上涨", "大涨", "利好", "突破", "增长", "看多", "上调", "反弹", "走强",
        "涨停", "新高", "放量", "资金流入", "机构买入", "增持", "回购",
    ];
    let negative_words = [
        "crash", "plunge", "drop", "fall", "bearish", "downgrade", "underperform",
        "下跌", "暴跌", "利空", "跌破", "下滑", "看空", "下调", "回调", "走弱",
        "跌停", "新低", "缩量", "资金流出", "机构卖出", "减持", "爆雷",
    ];

    let mut pos = 0usize;
    let mut neg = 0usize;
    for w in &positive_words {
        if text.contains(*w) {
            if has_negation_before(&text, w) {
                neg += 1; // "not bullish" => negative
            } else {
                pos += 1;
            }
        }
    }
    for w in &negative_words {
        if text.contains(*w) {
            if has_negation_before(&text, w) {
                pos += 1; // "not bearish" => positive
            } else {
                neg += 1;
            }
        }
    }

    if pos > neg {
        "positive".to_string()
    } else if neg > pos {
        "negative".to_string()
    } else {
        "neutral".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stock_crash_is_negative() {
        assert_eq!(classify_impact("Stock crash wipes billions", ""), "negative");
    }

    #[test]
    fn not_bullish_detects_negation() {
        // "bullish" alone would be positive, but "not bullish" should flip.
        assert_eq!(classify_impact("Analysts not bullish on tech sector", ""), "negative");
    }

    #[test]
    fn empty_text_is_neutral() {
        assert_eq!(classify_impact("", ""), "neutral");
    }

    #[test]
    fn plain_positive() {
        assert_eq!(classify_impact("Markets rally on strong earnings", ""), "positive");
    }

    #[test]
    fn negation_of_negative_flips_to_positive() {
        assert_eq!(classify_impact("Analysts say not bearish outlook", ""), "positive");
    }
}

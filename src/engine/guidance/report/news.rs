//! News fetching and classification for guidance reports.

use super::*;

impl DailyGuidanceGenerator {
    pub(super) async fn fetch_guidance_news(
        &self,
        market: &GuidanceMarket,
        date: &str,
    ) -> (Vec<GuidanceNewsItem>, Vec<String>) {
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
                guidance_items.push(GuidanceNewsItem {
                    title: item.title,
                    summary: item.summary,
                    source: item.source.clone(),
                    published_at: item.published_at,
                    url: item.url,
                    impact: "neutral".to_string(),
                    affected_entities: Vec::new(),
                    sector: None,
                });
                if !sources.contains(&item.source) {
                    sources.push(item.source);
                }
            }
        }

        guidance_items.sort_by(|a, b| b.published_at.cmp(&a.published_at));
        guidance_items.truncate(30);

        // LLM batch classification: relevance + sentiment + sector in one call
        if let Some(llm) = &self.llm {
            match self.classify_news_batch_with_llm(llm, &mut guidance_items).await {
                Ok(()) => {
                    // Filter out irrelevant news as determined by LLM
                    guidance_items.retain(|item| item.impact != "irrelevant");
                }
                Err(e) => {
                    tracing::warn!("LLM news classification failed, keeping all items: {e}");
                }
            }
        }

        (guidance_items, sources)
    }

    /// Batch-classify news items with LLM for relevance, sentiment, and sector.
    /// Single LLM call replaces keyword-based is_market_relevant + classify_news_sentiment + extract_sector_highlights.
    async fn classify_news_batch_with_llm(
        &self,
        llm: &crate::engine::llm::LlmClient,
        items: &mut [GuidanceNewsItem],
    ) -> anyhow::Result<()> {
        if items.is_empty() {
            return Ok(());
        }

        let indices: Vec<usize> = (0..items.len()).take(25).collect();

        let items_json: Vec<serde_json::Value> = indices
            .iter()
            .map(|&idx| {
                serde_json::json!({
                    "id": idx,
                    "title": items[idx].title,
                    "summary": items[idx].summary,
                })
            })
            .collect();

        let prompt = format!(
            r#"You are a financial news classifier. For each news item below, determine:
1. **relevant**: Is this related to financial markets, stocks, economy, or specific companies? (true/false)
2. **impact**: Market sentiment — "positive", "negative", or "neutral"
3. **sector**: Best-matching sector — one of: "technology", "finance", "healthcare", "energy", "consumer", "real_estate", "industrial", "materials", "utilities", "telecom"
4. **entities**: Array of stock tickers or company names mentioned (e.g. ["NVDA", "AAPL"]). Empty array if none.

Classification rules — be CONSERVATIVE with "neutral", most financial news has a direction:
- "positive" = bullish signal: earnings beat, analyst upgrade, policy stimulus, institutional buying, strong economic data, stock rally, index record high, peace deal boosting markets, buyback, net buying by institutions
- "negative" = bearish signal: earnings miss, analyst downgrade, policy tightening, scandal, fraud, fine, weak data, stock decline, layoffs, net selling, delisting risk
- "neutral" = truly no direction: general industry commentary with no bullish/bearish angle, factual reporting of past events without sentiment
- Irrelevant (politics with no market angle, sports, entertainment) → relevant=false, impact="irrelevant"
- "增长放缓" is negative despite containing "增长"
- Stock buybacks and net institutional buying are POSITIVE signals
- Record highs and strong rallies are POSITIVE

Return ONLY a JSON array, no markdown, no explanation:
[{{"id":0,"relevant":true,"impact":"positive","sector":"technology","entities":["NVDA"]}}]

News items:
{}"#,
            serde_json::to_string_pretty(&items_json)?
        );

        let response = llm.generate(&prompt).await?;

        let json_str = response
            .trim()
            .strip_prefix("```json")
            .or_else(|| response.trim().strip_prefix("```"))
            .unwrap_or(response.trim())
            .strip_suffix("```")
            .unwrap_or(response.trim())
            .trim();

        // If stripping markdown fences didn't find a JSON array, try to extract it
        let json_str = if json_str.starts_with('[') {
            json_str
        } else if let Some(start) = json_str.find('[') {
            if let Some(end) = json_str.rfind(']') {
                &json_str[start..=end]
            } else {
                json_str
            }
        } else {
            json_str
        };

        let parsed: Vec<serde_json::Value> = serde_json::from_str(json_str)
            .map_err(|e| {
                tracing::warn!(response = %response.chars().take(200).collect::<String>(), "LLM classification raw response");
                anyhow::anyhow!("failed to parse LLM news classification JSON: {e}")
            })?;

        let valid_sectors = [
            "technology", "finance", "healthcare", "energy", "consumer",
            "real_estate", "industrial", "materials", "utilities", "telecom",
        ];

        for entry in &parsed {
            let Some(id) = entry.get("id").and_then(|v| v.as_u64()) else {
                continue;
            };
            let idx = id as usize;
            if idx >= items.len() {
                continue;
            }

            // Relevance
            let relevant = entry.get("relevant").and_then(|v| v.as_bool()).unwrap_or(true);
            if !relevant {
                items[idx].impact = "irrelevant".to_string();
                continue;
            }

            // Impact/sentiment
            if let Some(impact) = entry.get("impact").and_then(|v| v.as_str())
                && matches!(impact, "positive" | "negative" | "neutral")
            {
                items[idx].impact = impact.to_string();
            }

            // Sector
            if let Some(sector) = entry.get("sector").and_then(|v| v.as_str())
                && valid_sectors.contains(&sector)
            {
                items[idx].sector = Some(sector.to_string());
            }

            // Entities
            if let Some(entities) = entry.get("entities").and_then(|v| v.as_array()) {
                let entity_list: Vec<String> = entities
                    .iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect();
                if !entity_list.is_empty() {
                    items[idx].affected_entities = entity_list;
                }
            }
        }

        let relevant_count = items.iter().filter(|i| i.impact != "irrelevant").count();
        let pos_count = items.iter().filter(|i| i.impact == "positive").count();
        let neg_count = items.iter().filter(|i| i.impact == "negative").count();
        let neutral_count = items.iter().filter(|i| i.impact == "neutral").count();
        let with_sector = items.iter().filter(|i| i.sector.is_some()).count();
        tracing::info!(
            total = items.len(),
            relevant = relevant_count,
            positive = pos_count,
            negative = neg_count,
            neutral = neutral_count,
            with_sector = with_sector,
            "LLM news classification applied"
        );

        Ok(())
    }
}

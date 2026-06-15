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

        // Filter out irrelevant international political/social news
        guidance_items.retain(|item| is_market_relevant(&item.title, &item.summary));

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

    /// Use LLM to batch-classify news sentiment for all items.
    async fn enrich_sentiment_with_llm(
        &self,
        llm: &crate::engine::llm::LlmClient,
        items: &mut [GuidanceNewsItem],
    ) -> anyhow::Result<()> {
        if items.is_empty() {
            return Ok(());
        }

        // Process all items (up to 20) to allow LLM to override keyword classification
        let indices: Vec<usize> = (0..items.len()).take(20).collect();

        let items_json: Vec<serde_json::Value> = indices
            .iter()
            .map(|&idx| {
                let summary = if items[idx].summary.len() > 200 {
                    let end = items[idx].summary.floor_char_boundary(200);
                    &items[idx].summary[..end]
                } else {
                    &items[idx].summary
                };
                serde_json::json!({
                    "id": idx,
                    "title": items[idx].title,
                    "summary": summary,
                    "current_classification": items[idx].impact,
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

        let mut changed = 0usize;
        for entry in &parsed {
            if let (Some(id), Some(impact)) = (
                entry.get("id").and_then(|v| v.as_u64()),
                entry.get("impact").and_then(|v| v.as_str()),
            ) {
                let idx = id as usize;
                if idx < items.len() && (impact == "positive" || impact == "negative" || impact == "neutral") {
                    if items[idx].impact != impact {
                        items[idx].impact = impact.to_string();
                        changed += 1;
                    }
                    // Extract affected entities
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
            }
        }

        tracing::info!(
            changed,
            total = indices.len(),
            "LLM sentiment enrichment applied"
        );

        Ok(())
    }

    fn classify_news_impact(&self, title: &str, summary: &str) -> String {
        crate::data::news::classify_news_sentiment(title, summary).as_str().to_string()
    }
}

/// Check if a news item is relevant to financial markets.
/// Uses blacklist-first: items matching irrelevant keywords are filtered regardless of other content.
fn is_market_relevant(title: &str, summary: &str) -> bool {
    let text = format!("{} {}", title, summary).to_ascii_lowercase();

    // Blacklist: filter out these categories regardless of other keywords
    let irrelevant_keywords = [
        // International geopolitics/military
        "以色列", "伊朗", "黎巴嫩", "真主党", "哈马斯", "叙利亚", "伊拉克",
        "朝鲜", "俄罗斯外长", "乌克兰", "军事打击", "火箭弹", "无人机袭击",
        "军事演习", "军事训练", "航行警告",
        "israel", "iran", "lebanon", "hezbollah", "hamas", "military strike",
        // Pure diplomacy (no market angle)
        "会见王毅", "外交部长", "总统会见", "国务卿", "外交部发言人",
        // Sports/entertainment
        "世界杯", "夺冠", "锦标赛", "机车", "摩托车", "赛车",
        "演唱会", "综艺", "选秀", "潮玩", "labubu", "泡泡玛特",
        // Social/celebrity
        "审查调查", "违纪违法", "接受审查",
    ];

    if irrelevant_keywords.iter().any(|kw| text.contains(kw)) {
        return false;
    }

    // Whitelist: financial/market keywords — keep if present
    let market_keywords = [
        // Market/indices
        "股", "a股", "大盘", "上证", "深证", "创业板", "科创板", "恒生", "纳斯达克",
        "标普", "道琼斯", "stock", "market", "index", "nasdaq", "s&p", "dow",
        // Economy/policy
        "gdp", "cpi", "pmi", "利率", "降息", "加息", "央行", "货币政策", "财政",
        "经济", "inflation", "interest rate", "fed", "federal reserve",
        "降准", "mlf", "lpr", "逆回购",
        // Companies/sectors
        " earnings", "营收", "利润", "财报", "业绩", "分红", "回购", "增发",
        "ipo", "上市", "退市", "停牌", "复牌",
        // Trading
        "涨停", "跌停", "涨幅", "跌幅", "成交量", "成交额", "换手率", "主力",
        "资金流", "北向", "融资融券", "龙虎榜",
        // Industry
        "科技", "人工智能", "芯片", "半导体", "新能源", "光伏", "锂电",
        "消费", "白酒", "医药", "金融", "银行", "保险", "券商", "地产",
        "能源", "石油", "煤炭", "ai", "semiconductor", "chip", "tech",
        // Commodities
        "黄金", "原油", "铜", "期货", "gold", "oil", "crude",
        // Policy/regulation
        "证监会", "银保监", "国资委", "国务院", "政策", "监管", "改革",
    ];

    market_keywords.iter().any(|kw| text.contains(kw))
}


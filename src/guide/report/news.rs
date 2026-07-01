//! News fetching and impact classification for guidance reports.

use super::*;

impl DailyGuidanceGenerator {
    pub(super) async fn fetch_guidance_news(
        &self,
        market: &GuidanceMarket,
        _date: &str,
    ) -> (Vec<GuidanceNewsItem>, Vec<String>) {
        let queries = self.news_queries_for_market(market);

        // Use the full search pipeline with multi-provider fallback
        let (items, _attempts) = self
            .market_data
            .fetch_news_search_queries_with_attempts(
                &queries,
                "zh-CN",
                Some("week"),
                None,
                None,
                crate::data::GeneralSearchIntent::MacroEvidence,
            )
            .await;

        // Supplement with Sina Finance news if available
        let sina_items = self.fetch_sina_finance_news(market).await;
        let items: Vec<crate::types::NewsItem> = items.into_iter().chain(sina_items).collect();

        let mut guidance_items = Vec::new();
        let mut sources = Vec::new();
        let mut dedup = std::collections::HashSet::new();

        // Domains to exclude (irrelevant content)
        let excluded_domains = [
            "zdic.net",
            "baidu.com/hanyu",
            "fanyi.baidu.com",
            "dict.baidu.com",
            "xueshu.baidu.com",
            "wenku.baidu.com",
            "baike.baidu.com",
            "zhidao.baidu.com",
        ];

        for item in items {
            // Skip irrelevant results (dictionary pages, translation sites, etc.)
            let url_lower = item.url.as_deref().unwrap_or("").to_ascii_lowercase();
            if excluded_domains.iter().any(|d| url_lower.contains(d)) {
                continue;
            }

            // Skip items with no finance/stock-related content
            let title_summary = format!("{} {}", item.title, item.summary).to_ascii_lowercase();
            let has_finance_keyword = [
                "股", "市", "涨", "跌", "资金", "主力", "板块", "涨停", "跌停", "stock", "market",
                "rally", "earnings", "trade", "invest", "指数", "行情", "基金", "银行", "保险",
                "券商",
            ]
            .iter()
            .any(|kw| title_summary.contains(kw));
            if !has_finance_keyword && !item.title.is_empty() {
                // Allow items from known finance sources
                let is_finance_source = [
                    "东方财富",
                    "新浪财经",
                    "腾讯财经",
                    "华尔街",
                    "CNBC",
                    "Reuters",
                    "Bloomberg",
                    "MarketWatch",
                    "Yahoo Finance",
                    "证券时报",
                    "上海证券报",
                    "中国证券报",
                ]
                .iter()
                .any(|s| item.source.contains(s));
                if !is_finance_source {
                    continue;
                }
            }

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
            let dedup_key = format!("{}:{}", normalized_title, item.source.to_ascii_lowercase());
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
        (guidance_items, sources)
    }

    fn news_queries_for_market(&self, market: &GuidanceMarket) -> Vec<String> {
        match market {
            GuidanceMarket::AShare => vec![
                "A股 涨停 板块 热点".to_string(),
                "沪深 资金流向 主力".to_string(),
                "中国股市 政策 利好 利空".to_string(),
                "A股 机构 研报 观点".to_string(),
            ],
            GuidanceMarket::HongKong => vec![
                "港股 恒生指数 涨跌".to_string(),
                "港股 科技股 腾讯 阿里".to_string(),
                "港股 南向资金 异动".to_string(),
                "Hong Kong stock market rally drop".to_string(),
            ],
            GuidanceMarket::UsEquity => vec![
                "美股 科技股 涨跌".to_string(),
                "US stock market rally earnings".to_string(),
                "Wall Street Federal Reserve rates".to_string(),
                "S&P 500 Nasdaq market movement".to_string(),
            ],
            GuidanceMarket::All => vec![
                "A股 涨停 热点".to_string(),
                "港股 恒生指数".to_string(),
                "US stock market today".to_string(),
                "global financial markets".to_string(),
            ],
        }
    }

    fn classify_news_impact(&self, title: &str, summary: &str) -> String {
        classify_impact(title, summary)
    }
}

/// Check whether `keyword` is preceded by a negation word within the
/// preceding 10 characters of `text`.
pub fn has_negation_before(text: &str, keyword: &str) -> bool {
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
pub fn classify_impact(title: &str, summary: &str) -> String {
    let text = format!("{} {}", title, summary).to_ascii_lowercase();
    let positive_words = [
        "surge",
        "rally",
        "gain",
        "rise",
        "bullish",
        "upgrade",
        "outperform",
        "上涨",
        "大涨",
        "利好",
        "突破",
        "增长",
        "看多",
        "上调",
    ];
    let negative_words = [
        "crash",
        "plunge",
        "drop",
        "fall",
        "bearish",
        "downgrade",
        "underperform",
        "下跌",
        "暴跌",
        "利空",
        "跌破",
        "下滑",
        "看空",
        "下调",
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

impl DailyGuidanceGenerator {
    /// Fetch news from Sina Finance as supplementary source.
    pub(super) async fn fetch_sina_finance_news(
        &self,
        market: &GuidanceMarket,
    ) -> Vec<crate::types::NewsItem> {
        let url = match market {
            GuidanceMarket::AShare => "https://finance.sina.com.cn/stock/",
            GuidanceMarket::HongKong => "https://finance.sina.com.cn/stock/hkstock/",
            GuidanceMarket::UsEquity => "https://finance.sina.com.cn/stock/usstock/",
            GuidanceMarket::All => "https://finance.sina.com.cn/",
        };

        let Ok(resp) = self.http.get(url).send().await else {
            return Vec::new();
        };
        let Ok(html) = resp.text().await else {
            return Vec::new();
        };

        let mut items = Vec::new();
        // Extract news titles from Sina Finance HTML
        // Pattern: <a href="..." target="_blank">Title</a>
        let re = regex::Regex::new(r#"<a[^>]*href="([^"]*)"[^>]*>([^<]{10,100})</a>"#).unwrap();
        for cap in re.captures_iter(&html) {
            let url = cap[1].to_string();
            let title = cap[2].trim().to_string();

            // Filter: must contain finance keywords
            let title_lower = title.to_ascii_lowercase();
            let has_keyword = [
                "股",
                "涨",
                "跌",
                "市场",
                "板块",
                "资金",
                "主力",
                "指数",
                "行情",
                "基金",
                "银行",
                "保险",
                "券商",
                "涨停",
                "跌停",
                "ETF",
                "芯片",
                "科技",
                "医疗",
                "军工",
                "新能源",
            ]
            .iter()
            .any(|kw| title_lower.contains(kw));
            if !has_keyword {
                continue;
            }

            // Skip navigation/menu items
            if title.len() < 15 || title.contains(">>") || title.contains("更多") {
                continue;
            }

            items.push(crate::types::NewsItem {
                title: title.clone(),
                summary: String::new(),
                source: "新浪财经".to_string(),
                published_at: chrono::Utc::now().to_rfc3339(),
                url: Some(url),
            });

            if items.len() >= 10 {
                break;
            }
        }

        tracing::info!(
            market = market.as_str(),
            count = items.len(),
            "fetched Sina Finance news"
        );
        items
    }
}

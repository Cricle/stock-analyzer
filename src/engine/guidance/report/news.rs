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
                "A股 今日 行情".to_string(),
                "中国股市 新闻".to_string(),
                "沪深 大盘".to_string(),
            ],
            GuidanceMarket::HongKong => vec![
                "Hong Kong stock market today".to_string(),
                "恒生指数 行情 新闻".to_string(),
                "港股 异动 公告".to_string(),
                "港股 科技股 金融股".to_string(),
                "Hang Seng Index".to_string(),
            ],
            GuidanceMarket::UsEquity => vec![
                "US stock market today".to_string(),
                "Wall Street news".to_string(),
                "S&P 500 market".to_string(),
            ],
            GuidanceMarket::All => vec![
                "A股 今日 行情".to_string(),
                "港股 今日 行情".to_string(),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stock_crash_is_negative() {
        assert_eq!(
            classify_impact("Stock crash wipes billions", ""),
            "negative"
        );
    }

    #[test]
    fn not_bullish_detects_negation() {
        // "bullish" alone would be positive, but "not bullish" should flip.
        assert_eq!(
            classify_impact("Analysts not bullish on tech sector", ""),
            "negative"
        );
    }

    #[test]
    fn empty_text_is_neutral() {
        assert_eq!(classify_impact("", ""), "neutral");
    }

    #[test]
    fn plain_positive() {
        assert_eq!(
            classify_impact("Markets rally on strong earnings", ""),
            "positive"
        );
    }

    #[test]
    fn negation_of_negative_flips_to_positive() {
        assert_eq!(
            classify_impact("Analysts say not bearish outlook", ""),
            "positive"
        );
    }
}

//! Shared news utility functions used across stock_pick, guidance, and analysis.

use super::NewsItem;

const HARD_NEGATIVE_KEYWORDS: &[&str] = &[
    "investigation",
    "fraud",
    "default",
    "bankruptcy",
    "delist",
    "downgrade",
    "lawsuit",
    "recall",
    "probe",
];

const POSITIVE_KEYWORDS: &[&str] = &[
    "surge", "rally", "gain", "rise", "bullish", "upgrade", "outperform",
    "上涨", "大涨", "利好", "突破", "增长", "看多", "上调", "反弹", "走强",
    "涨停", "新高", "放量", "资金流入", "机构买入", "增持", "回购",
];

const NEGATIVE_KEYWORDS: &[&str] = &[
    "crash", "plunge", "drop", "fall", "bearish", "downgrade", "underperform",
    "下跌", "暴跌", "利空", "跌破", "下滑", "看空", "下调", "回调", "走弱",
    "跌停", "新低", "缩量", "资金流出", "机构卖出", "减持", "爆雷",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NewsSentiment {
    Positive,
    Negative,
    Neutral,
}

impl NewsSentiment {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Positive => "positive",
            Self::Negative => "negative",
            Self::Neutral => "neutral",
        }
    }
}

/// Generate a deduplication key from news item fields.
pub(crate) fn news_dedupe_key(
    title: &str,
    source: &str,
    published_at: &str,
    url: Option<&str>,
) -> String {
    format!(
        "{}|{}|{}|{}",
        title.trim().to_ascii_lowercase(),
        source.trim().to_ascii_lowercase(),
        published_at.trim(),
        url.unwrap_or_default()
            .trim()
            .to_ascii_lowercase()
    )
}

/// Check whether a keyword is preceded by a negation word within the
/// preceding 10 characters of `text`.
fn has_negation_before(text: &str, keyword: &str) -> bool {
    if let Some(pos) = text.find(keyword) {
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

/// Keyword-based news sentiment classification with negation detection.
pub(crate) fn classify_news_sentiment(title: &str, summary: &str) -> NewsSentiment {
    let text = format!("{} {}", title, summary).to_ascii_lowercase();
    let mut pos = 0usize;
    let mut neg = 0usize;
    for w in POSITIVE_KEYWORDS {
        if text.contains(w) {
            if has_negation_before(&text, w) {
                neg += 1;
            } else {
                pos += 1;
            }
        }
    }
    for w in NEGATIVE_KEYWORDS {
        if text.contains(w) {
            if has_negation_before(&text, w) {
                pos += 1;
            } else {
                neg += 1;
            }
        }
    }
    if pos > neg {
        NewsSentiment::Positive
    } else if neg > pos {
        NewsSentiment::Negative
    } else {
        NewsSentiment::Neutral
    }
}

/// Check if a single news item contains hard negative keywords.
pub(crate) fn is_hard_negative(item: &NewsItem) -> bool {
    let title = item.title.to_ascii_lowercase();
    let summary = item.summary.to_ascii_lowercase();
    HARD_NEGATIVE_KEYWORDS
        .iter()
        .any(|keyword| title.contains(keyword) || summary.contains(keyword))
}

/// Check if any news item contains hard negative keywords.
#[cfg(test)]
pub(crate) fn has_hard_negative_news(news: &[NewsItem]) -> bool {
    news.iter().any(|item| {
        let title = item.title.to_ascii_lowercase();
        let summary = item.summary.to_ascii_lowercase();
        HARD_NEGATIVE_KEYWORDS
            .iter()
            .any(|keyword| title.contains(keyword) || summary.contains(keyword))
    })
}

/// Deduplicate news items by title+source+date+url, then sort by date descending.
pub(crate) fn dedupe_news_items(items: Vec<NewsItem>) -> Vec<NewsItem> {
    let mut dedup = std::collections::HashSet::new();
    let mut output = Vec::new();
    for item in items {
        let key = news_dedupe_key(&item.title, &item.source, &item.published_at, item.url.as_deref());
        if dedup.insert(key) {
            output.push(item);
        }
    }
    output.sort_by(|left, right| right.published_at.cmp(&left.published_at));
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crash_is_negative() {
        assert_eq!(
            classify_news_sentiment("Stock crash wipes billions", ""),
            NewsSentiment::Negative
        );
    }

    #[test]
    fn not_bullish_detects_negation() {
        assert_eq!(
            classify_news_sentiment("Analysts not bullish on tech sector", ""),
            NewsSentiment::Negative
        );
    }

    #[test]
    fn empty_text_is_neutral() {
        assert_eq!(
            classify_news_sentiment("", ""),
            NewsSentiment::Neutral
        );
    }

    #[test]
    fn plain_positive() {
        assert_eq!(
            classify_news_sentiment("Markets rally on strong earnings", ""),
            NewsSentiment::Positive
        );
    }

    #[test]
    fn negation_of_negative_flips_to_positive() {
        assert_eq!(
            classify_news_sentiment("Analysts say not bearish outlook", ""),
            NewsSentiment::Positive
        );
    }

    #[test]
    fn hard_negative_detects_fraud() {
        let news = vec![NewsItem {
            title: "Company under investigation for fraud".to_string(),
            summary: String::new(),
            source: "Reuters".to_string(),
            published_at: "2024-01-15".to_string(),
            url: None,
        }];
        assert!(has_hard_negative_news(&news));
    }

    #[test]
    fn hard_negative_no_false_positive() {
        let news = vec![NewsItem {
            title: "Company reports strong earnings".to_string(),
            summary: String::new(),
            source: "Reuters".to_string(),
            published_at: "2024-01-15".to_string(),
            url: None,
        }];
        assert!(!has_hard_negative_news(&news));
    }

    #[test]
    fn dedupe_removes_duplicates() {
        let items = vec![
            NewsItem {
                title: "Same Title".to_string(),
                summary: "A".to_string(),
                source: "Reuters".to_string(),
                published_at: "2024-01-15".to_string(),
                url: None,
            },
            NewsItem {
                title: "Same Title".to_string(),
                summary: "B".to_string(),
                source: "Reuters".to_string(),
                published_at: "2024-01-15".to_string(),
                url: None,
            },
        ];
        let result = dedupe_news_items(items);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn dedupe_key_case_insensitive() {
        let key1 = news_dedupe_key("Hello World", "Reuters", "2024-01-15", None);
        let key2 = news_dedupe_key("hello world", "reuters", "2024-01-15", None);
        assert_eq!(key1, key2);
    }
}

//! Market sentiment assessment and sector highlight extraction.

use super::*;

/// Truncate and join titles to fit within a character budget.
fn truncate_titles(titles: &[&str], max_chars: usize) -> String {
    let mut result = String::new();
    for (i, title) in titles.iter().enumerate() {
        if i > 0 {
            result.push_str("; ");
        }
        let truncated = if title.len() > 40 {
            format!("{}...", &title[..title.floor_char_boundary(40)])
        } else {
            title.to_string()
        };
        if result.len() + truncated.len() > max_chars {
            break;
        }
        result.push_str(&truncated);
    }
    result
}

/// Compute sentiment score from positive/negative/total counts.
///
/// Enforces a minimum sample threshold: with fewer than 2 news items the
/// formula would be overly volatile (e.g. 1 positive item = score 100),
/// so we return 0 (neutral) instead.
fn sentiment_score(pos: usize, neg: usize, total: usize) -> i32 {
    if total < 2 {
        return 0;
    }
    let ratio = (pos as f64 - neg as f64) / total.max(1) as f64;
    (ratio * 100.0).clamp(-100.0, 100.0) as i32
}

fn sentiment_label(score: i32) -> (&'static str, &'static str) {
    if score > 30 {
        ("bullish", "guidance.sentiment.bullish")
    } else if score > 10 {
        ("slightly_bullish", "guidance.sentiment.slightly_bullish")
    } else if score > -10 {
        ("neutral", "guidance.sentiment.neutral")
    } else if score > -30 {
        ("slightly_bearish", "guidance.sentiment.slightly_bearish")
    } else {
        ("bearish", "guidance.sentiment.bearish")
    }
}

/// Keyword-based sentiment fallback when LLM classifies all items as neutral.
/// Updates items' impact field in-place and returns (positive_count, negative_count).
fn keyword_sentiment_update(news: &mut [GuidanceNewsItem]) -> (usize, usize) {
    let pos_keywords = [
        "surge", "rally", "beat", "upgrade", "stimulus", "growth", "record high",
        "bullish", "outperform", "buy", "buyback", "gain", "profit", "revenue up", "jump", "soar",
        "peace deal", "boost", "strong data", "institutional buying",
        "上涨", "涨停", "利好", "增长", "突破", "新高", "增持", "买入", "回购", "大涨", "净买入",
    ];
    let neg_keywords = [
        "crash", "plunge", "miss", "downgrade", "recession", "decline", "record low",
        "bearish", "underperform", "sell-off", "loss", "layoff", "scandal", "fraud", "fine", "penalty",
        "tariff", "sanctions", "bankruptcy", "default", "delisting",
        "下跌", "跌停", "利空", "下滑", "暴跌", "净卖出", "亏损", "爆雷", "罚款", "处罚",
        "制裁", "退市", "破产", "违约",
    ];

    let mut pos = 0usize;
    let mut neg = 0usize;

    for item in news.iter_mut() {
        let text = format!("{} {}", item.title, item.summary).to_ascii_lowercase();
        let has_pos = pos_keywords.iter().any(|kw| text.contains(kw));
        let has_neg = neg_keywords.iter().any(|kw| text.contains(kw));
        if has_pos && !has_neg {
            item.impact = "positive".to_string();
            pos += 1;
        } else if has_neg && !has_pos {
            item.impact = "negative".to_string();
            neg += 1;
        }
    }

    (pos, neg)
}

impl DailyGuidanceGenerator {
    pub(super) async fn assess_market_sentiment(
        &self,
        news: &mut [GuidanceNewsItem],
        market: &GuidanceMarket,
    ) -> MarketSentiment {
        let mut pos = news.iter().filter(|n| n.impact == "positive").count();
        let mut neg = news.iter().filter(|n| n.impact == "negative").count();
        let total = news.len();

        // Fallback: if LLM classified everything as neutral, try keyword-based detection
        // and update the items' impact so downstream (drivers, sector highlights) sees them
        if pos == 0 && neg == 0 && total >= 2 {
            let (kw_pos, kw_neg) = keyword_sentiment_update(news);
            if kw_pos > 0 || kw_neg > 0 {
                tracing::info!(
                    kw_pos, kw_neg,
                    "LLM classified all news as neutral, using keyword-based fallback sentiment"
                );
                pos = kw_pos;
                neg = kw_neg;
            }
        }

        let score = sentiment_score(pos, neg, total);
        let (label, label_key) = sentiment_label(score);

        // Extract specific events as drivers
        let mut drivers = Vec::new();
        let mut driver_keys = Vec::new();
        let pos_events: Vec<&str> = news
            .iter()
            .filter(|n| n.impact == "positive")
            .take(3)
            .map(|n| n.title.as_str())
            .collect();
        let neg_events: Vec<&str> = news
            .iter()
            .filter(|n| n.impact == "negative")
            .take(3)
            .map(|n| n.title.as_str())
            .collect();

        if !pos_events.is_empty() {
            let events = truncate_titles(&pos_events, 80);
            drivers.push(format!("positive: {events}"));
            driver_keys.push(serde_json::json!({
                "i18n_key": "guidance.drivers.positive",
                "events": events,
            }));
        }
        if !neg_events.is_empty() {
            let events = truncate_titles(&neg_events, 80);
            drivers.push(format!("negative: {events}"));
            driver_keys.push(serde_json::json!({
                "i18n_key": "guidance.drivers.negative",
                "events": events,
            }));
        }

        let neutral_count = total - pos - neg;
        let rationale_key = serde_json::json!({
            "i18n_key": "guidance.rationale",
            "total": total,
            "pos": pos,
            "neg": neg,
            "neutral": neutral_count,
            "market": market.as_str(),
        });

        MarketSentiment {
            score,
            label: label.to_string(),
            label_key: Some(label_key.to_string()),
            rationale: format!(
                "Based on {} news items: {} positive, {} negative, {} neutral for {} market",
                total, pos, neg, neutral_count, market.as_str()
            ),
            rationale_key: Some(rationale_key),
            drivers,
            driver_keys,
        }
    }

    pub(super) fn extract_sector_highlights(
        &self,
        news: &[GuidanceNewsItem],
    ) -> Vec<SectorHighlight> {
        // Aggregate news by LLM-assigned sector
        let mut sector_news: std::collections::HashMap<String, Vec<&GuidanceNewsItem>> =
            std::collections::HashMap::new();

        for item in news {
            if let Some(ref sector) = item.sector {
                sector_news.entry(sector.clone()).or_default().push(item);
            }
        }

        let mut highlights = Vec::new();
        for (sector_name, matching) in &sector_news {
            let pos = matching.iter().filter(|n| n.impact == "positive").count();
            let neg = matching.iter().filter(|n| n.impact == "negative").count();
            let (direction, direction_key) = if pos > neg {
                ("positive", "guidance.direction.positive")
            } else if neg > pos {
                ("negative", "guidance.direction.negative")
            } else {
                ("mixed", "guidance.direction.mixed")
            };

            let key_driver = matching
                .iter()
                .filter(|n| n.impact != "neutral")
                .map(|n| n.title.as_str())
                .next()
                .or_else(|| matching.first().map(|n| n.title.as_str()))
                .unwrap_or("")
                .to_string();

            // Collect representative stocks from LLM-assigned entities
            let mut stocks: Vec<String> = Vec::new();
            for item in matching {
                for entity in &item.affected_entities {
                    if stocks.len() >= 3 {
                        break;
                    }
                    let e = entity.trim().to_uppercase();
                    if !e.is_empty() && !stocks.contains(&e) {
                        stocks.push(e);
                    }
                }
                if stocks.len() >= 3 {
                    break;
                }
            }

            highlights.push(SectorHighlight {
                sector_name: sector_name.to_string(),
                sector_key: Some(format!("guidance.sector.{}", sector_name)),
                direction: direction.to_string(),
                direction_key: Some(direction_key.to_string()),
                key_driver,
                representative_stocks: stocks,
            });
        }

        highlights
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_news_returns_zero() {
        assert_eq!(sentiment_score(0, 0, 0), 0);
    }

    #[test]
    fn single_positive_news_neutral_due_to_threshold() {
        // With only 1 item total < 2, score must be 0 regardless of polarity.
        assert_eq!(sentiment_score(1, 0, 1), 0);
    }

    #[test]
    fn two_items_at_threshold_now_scored() {
        // With 2 items (>= threshold of 2), sentiment is now computed.
        // 2 positive / 2 total => ratio = 1.0 => score = 100
        assert_eq!(sentiment_score(2, 0, 2), 100);
    }

    #[test]
    fn mixed_news_balanced_near_zero() {
        // 3 positive, 3 negative out of 10 total => ratio = 0 => score 0
        assert_eq!(sentiment_score(3, 3, 10), 0);
    }

    #[test]
    fn all_negative_large_set_near_minus_100() {
        // 10 negative, 0 positive out of 10 => ratio = -1.0 => score = -100
        assert_eq!(sentiment_score(0, 10, 10), -100);
    }

    #[test]
    fn all_positive_large_set_near_plus_100() {
        assert_eq!(sentiment_score(10, 0, 10), 100);
    }

    #[test]
    fn score_clamped() {
        // Should never exceed [-100, 100]
        let s = sentiment_score(100, 0, 100);
        assert!(s >= -100 && s <= 100);
    }
}

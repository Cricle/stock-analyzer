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
/// Enforces a minimum sample threshold: with fewer than 3 news items the
/// formula would be overly volatile (e.g. 1 positive item = score 100),
/// so we return 0 (neutral) instead.
fn sentiment_score(pos: usize, neg: usize, total: usize) -> i32 {
    if total < 3 {
        return 0;
    }
    let ratio = (pos as f64 - neg as f64) / total.max(1) as f64;
    (ratio * 100.0).clamp(-100.0, 100.0) as i32
}

fn sentiment_label(score: i32) -> &'static str {
    if score > 30 {
        "bullish"
    } else if score > 10 {
        "slightly_bullish"
    } else if score > -10 {
        "neutral"
    } else if score > -30 {
        "slightly_bearish"
    } else {
        "bearish"
    }
}

impl DailyGuidanceGenerator {
    pub(super) async fn assess_market_sentiment(
        &self,
        news: &[GuidanceNewsItem],
        market: &GuidanceMarket,
    ) -> MarketSentiment {
        let pos = news.iter().filter(|n| n.impact == "positive").count();
        let neg = news.iter().filter(|n| n.impact == "negative").count();
        let total = news.len();
        let score = sentiment_score(pos, neg, total);
        let label = sentiment_label(score);

        // Extract specific events as drivers
        let mut drivers = Vec::new();
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
            drivers.push(format!("positive: {}", truncate_titles(&pos_events, 80)));
        }
        if !neg_events.is_empty() {
            drivers.push(format!("negative: {}", truncate_titles(&neg_events, 80)));
        }

        MarketSentiment {
            score,
            label: label.to_string(),
            rationale: format!(
                "Based on {} news items: {} positive, {} negative, {} neutral for {} market",
                total,
                pos,
                neg,
                total - pos - neg,
                market.as_str()
            ),
            drivers,
        }
    }

    pub(super) fn extract_sector_highlights(
        &self,
        news: &[GuidanceNewsItem],
    ) -> Vec<SectorHighlight> {
        let sector_keywords = [
            (
                "technology",
                vec![
                    "tech", "ai", "semiconductor", "chip", "科技", "人工智能", "芯片",
                    "nvidia", "apple", "microsoft", "google", "tesla", "英伟达", "苹果",
                ],
            ),
            (
                "finance",
                vec![
                    "bank", "insurance", "金融", "银行", "保险", "券商", "基金",
                    "jpmorgan", "goldman", "berkshire",
                ],
            ),
            (
                "healthcare",
                vec![
                    "pharma", "biotech", "health", "医药", "生物", "医疗",
                    "pfizer", "johnson", "unitedhealth",
                ],
            ),
            (
                "energy",
                vec![
                    "oil", "gas", "energy", "renewable", "能源", "石油", "光伏",
                    "exxon", "chevron", "宁德时代", "比亚迪",
                ],
            ),
            (
                "consumer",
                vec![
                    "retail", "consumer", "luxury", "消费", "零售", "白酒", "茅台",
                    "walmart", "costco", "lvmh",
                ],
            ),
            (
                "real_estate",
                vec![
                    "property", "real estate", "房地产", "地产", "物业",
                ],
            ),
        ];

        // Step 1: Assign each news item to its best-matching sector
        let mut sector_news: std::collections::HashMap<&str, Vec<&GuidanceNewsItem>> =
            std::collections::HashMap::new();

        for item in news {
            let text = format!("{} {}", item.title, item.summary).to_ascii_lowercase();
            let mut best_sector = None;
            let mut best_score = 0usize;

            for (sector_name, keywords) in &sector_keywords {
                let score = keywords.iter().filter(|kw| text.contains(*kw)).count();
                if score > best_score {
                    best_score = score;
                    best_sector = Some(*sector_name);
                }
            }

            if let Some(sector) = best_sector {
                sector_news.entry(sector).or_default().push(item);
            }
        }

        // Step 2: Build SectorHighlight for each sector with news
        let mut highlights = Vec::new();
        for (sector_name, matching) in &sector_news {
            let pos = matching.iter().filter(|n| n.impact == "positive").count();
            let neg = matching.iter().filter(|n| n.impact == "negative").count();
            let direction = if pos > neg {
                "positive"
            } else if neg > pos {
                "negative"
            } else {
                "mixed"
            };

            // Pick key_driver: prefer non-neutral news, then most recent
            let key_driver = matching
                .iter()
                .filter(|n| n.impact != "neutral")
                .map(|n| n.title.as_str())
                .next()
                .or_else(|| matching.first().map(|n| n.title.as_str()))
                .unwrap_or("")
                .to_string();

            // Extract representative stock names
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
                let title_lower = item.title.to_ascii_lowercase();
                let known_stocks: &[(&str, &str)] = match *sector_name {
                    "technology" => &[
                        ("nvidia", "NVDA"), ("apple", "AAPL"), ("microsoft", "MSFT"),
                        ("google", "GOOGL"), ("tesla", "TSLA"), ("meta", "META"),
                        ("英伟达", "NVDA"), ("苹果", "AAPL"), ("特斯拉", "TSLA"),
                    ],
                    "finance" => &[
                        ("jpmorgan", "JPM"), ("goldman", "GS"), ("berkshire", "BRK"),
                        ("汇丰", "00005.HK"), ("渣打", "2888.HK"), ("平安", "601318"),
                    ],
                    "energy" => &[
                        ("exxon", "XOM"), ("chevron", "CVX"),
                        ("宁德时代", "300750"), ("比亚迪", "002594"),
                        ("中石油", "601857"), ("中石化", "600028"),
                    ],
                    "consumer" => &[
                        ("walmart", "WMT"), ("costco", "COST"), ("lvmh", "MC"),
                        ("茅台", "600519"), ("五粮液", "000858"),
                    ],
                    "healthcare" => &[
                        ("pfizer", "PFE"), ("johnson", "JNJ"), ("unitedhealth", "UNH"),
                    ],
                    _ => &[],
                };
                for (pattern, ticker) in known_stocks {
                    if stocks.len() >= 3 {
                        break;
                    }
                    if title_lower.contains(pattern) && !stocks.contains(&ticker.to_string()) {
                        stocks.push(ticker.to_string());
                    }
                }
                if stocks.len() >= 3 {
                    break;
                }
            }

            highlights.push(SectorHighlight {
                sector_name: sector_name.to_string(),
                direction: direction.to_string(),
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
        // With only 1 item total < 3, score must be 0 regardless of polarity.
        assert_eq!(sentiment_score(1, 0, 1), 0);
    }

    #[test]
    fn two_items_still_below_threshold() {
        assert_eq!(sentiment_score(2, 0, 2), 0);
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

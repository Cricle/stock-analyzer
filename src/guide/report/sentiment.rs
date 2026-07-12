//! Market sentiment assessment and sector highlight extraction.

use super::*;

/// Compute sentiment score from positive/negative/total counts.
///
/// Enforces a minimum sample threshold: with fewer than 3 news items the
/// formula would be overly volatile (e.g. 1 positive item = score 100),
/// so we return 0 (neutral) instead.
pub fn sentiment_score(pos: usize, neg: usize, total: usize) -> i32 {
    if total < 3 {
        return 0;
    }
    let ratio = (pos as f64 - neg as f64) / total.max(1) as f64;
    (ratio * 100.0).clamp(-100.0, 100.0) as i32
}

/// Compute Sentiment_label.
pub fn sentiment_label(score: i32) -> &'static str {
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

        let mut drivers = Vec::new();
        if pos > 0 {
            drivers.push(format!("{} positive news items", pos));
        }
        if neg > 0 {
            drivers.push(format!("{} negative news items", neg));
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
        let mut highlights = Vec::new();
        let sector_keywords = [
            (
                "technology",
                vec![
                    "tech",
                    "ai",
                    "semiconductor",
                    "chip",
                    "科技",
                    "人工智能",
                    "芯片",
                    "nvidia",
                    "apple",
                    "microsoft",
                    "google",
                    "tesla",
                    "英伟达",
                    "苹果",
                ],
            ),
            (
                "finance",
                vec![
                    "bank",
                    "insurance",
                    "金融",
                    "银行",
                    "保险",
                    "券商",
                    "基金",
                    "jpmorgan",
                    "goldman",
                    "berkshire",
                ],
            ),
            (
                "healthcare",
                vec![
                    "pharma",
                    "biotech",
                    "health",
                    "医药",
                    "生物",
                    "医疗",
                    "pfizer",
                    "johnson",
                    "unitedhealth",
                ],
            ),
            (
                "energy",
                vec![
                    "oil",
                    "gas",
                    "energy",
                    "renewable",
                    "能源",
                    "石油",
                    "光伏",
                    "exxon",
                    "chevron",
                    "宁德时代",
                    "比亚迪",
                ],
            ),
            (
                "consumer",
                vec![
                    "retail", "consumer", "luxury", "消费", "零售", "白酒", "茅台", "walmart",
                    "costco", "lvmh",
                ],
            ),
            (
                "real_estate",
                vec!["property", "real estate", "房地产", "地产", "物业"],
            ),
        ];

        for (sector_name, keywords) in &sector_keywords {
            let matching: Vec<&GuidanceNewsItem> = news
                .iter()
                .filter(|n| {
                    let text = format!("{} {}", n.title, n.summary).to_ascii_lowercase();
                    keywords.iter().any(|kw| text.contains(kw))
                })
                .collect();

            if !matching.is_empty() {
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

                // Extract representative stock names from news titles
                let mut stocks: Vec<String> = Vec::new();
                for item in &matching {
                    // First try affected_entities
                    for entity in &item.affected_entities {
                        if stocks.len() >= 3 {
                            break;
                        }
                        let e = entity.trim().to_uppercase();
                        if !e.is_empty() && !stocks.contains(&e) {
                            stocks.push(e);
                        }
                    }
                    // Also extract well-known stock names from title
                    let title_lower = item.title.to_ascii_lowercase();
                    let known_stocks: &[(&str, &str)] = match *sector_name {
                        "technology" => &[
                            ("nvidia", "NVDA"),
                            ("apple", "AAPL"),
                            ("microsoft", "MSFT"),
                            ("google", "GOOGL"),
                            ("tesla", "TSLA"),
                            ("meta", "META"),
                            ("英伟达", "NVDA"),
                            ("苹果", "AAPL"),
                            ("特斯拉", "TSLA"),
                        ],
                        "finance" => &[
                            ("jpmorgan", "JPM"),
                            ("goldman", "GS"),
                            ("berkshire", "BRK"),
                            ("汇丰", "00005.HK"),
                            ("渣打", "2888.HK"),
                            ("平安", "601318"),
                        ],
                        "energy" => &[
                            ("exxon", "XOM"),
                            ("chevron", "CVX"),
                            ("宁德时代", "300750"),
                            ("比亚迪", "002594"),
                            ("中石油", "601857"),
                            ("中石化", "600028"),
                        ],
                        "consumer" => &[
                            ("walmart", "WMT"),
                            ("costco", "COST"),
                            ("lvmh", "MC"),
                            ("茅台", "600519"),
                            ("五粮液", "000858"),
                        ],
                        "healthcare" => &[
                            ("pfizer", "PFE"),
                            ("johnson", "JNJ"),
                            ("unitedhealth", "UNH"),
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
        }

        highlights
    }
}

fn select_analyst<'a>(
    result: &'a AnalysisResult,
    candidates: &[&str],
) -> Option<&'a AgentReportNode> {
    result.graph.analysts.iter().find(|item| analyst_matches(item, candidates))
}

fn analyst_matches(item: &AgentReportNode, candidates: &[&str]) -> bool {
    let key = normalized_key(&item.key);
    let title = normalized_key(&item.title);
    let agent = normalized_key(&item.agent);

    candidates.iter().any(|candidate| {
        let candidate = normalized_key(candidate);
        if candidate.is_empty() {
            return false;
        }

        key == candidate
            || title == candidate
            || agent == candidate
            || key.contains(&candidate)
            || title.contains(&candidate)
            || agent.contains(&candidate)
            || candidate.contains(&key)
            || candidate.contains(&title)
            || candidate.contains(&agent)
            || matches_semantic_alias(&candidate, &key, &title, &agent)
    })
}

fn matches_semantic_alias(candidate: &str, key: &str, title: &str, agent: &str) -> bool {
    // Prefer structured key/agent matching; fall back to Chinese title matching
    // only when key and agent are empty (legacy data).
    if key == candidate || agent.contains(candidate) {
        return true;
    }
    match candidate {
        "market" => {
            title.contains("市场") || title.contains("技术")
        }
        "fundamentals" | "fundamental" => {
            title.contains("基本面")
        }
        "news" => title.contains("新闻"),
        "sentiment" => {
            title.contains("情绪") || title.contains("资金")
        }
        _ => false,
    }
}

fn normalized_key(value: &str) -> String {
    value
        .trim()
        .to_lowercase()
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || is_cjk(*ch))
        .collect()
}

fn is_cjk(ch: char) -> bool {
    matches!(
        ch as u32,
        0x4E00..=0x9FFF
            | 0x3400..=0x4DBF
            | 0x20000..=0x2A6DF
            | 0x2A700..=0x2B73F
            | 0x2B740..=0x2B81F
            | 0x2B820..=0x2CEAF
            | 0xF900..=0xFAFF
            | 0x2F800..=0x2FA1F
    )
}

fn average_evidence_density(analysts: &[AgentReportNode]) -> f64 {
    if analysts.is_empty() {
        return 0.0;
    }
    analysts
        .iter()
        .map(|item| item.evidence_points.len() as f64)
        .sum::<f64>()
        / analysts.len() as f64
}

pub fn has_execution_boundary(
    trader_plan: &StructuredTraderPlan,
    portfolio_decision: &StructuredPortfolioDecision,
) -> bool {
    let has_target = !portfolio_decision.price_target.trim().is_empty();
    let has_confirmation = !portfolio_decision.confirmation_level.trim().is_empty();
    !trader_plan.entry_price.trim().is_empty()
        && !trader_plan.stop_loss.trim().is_empty()
        && (has_target || has_confirmation)
        && !portfolio_decision.time_horizon.trim().is_empty()
}

fn analyst_probability_quality(analyst: Option<&AgentReportNode>) -> i32 {
    let Some(analyst) = analyst else {
        return 0;
    };
    let sum = analyst.up_probability + analyst.down_probability + analyst.sideways_probability;
    let gap = (sum - 1.0).abs();
    if gap <= 0.05 {
        4
    } else if gap <= 0.15 {
        2
    } else {
        0
    }
}

fn analyst_net_probability(analyst: &AgentReportNode) -> f64 {
    (analyst.up_probability - analyst.down_probability).clamp(-1.0, 1.0)
}

fn score_analyst_net(analyst: Option<&AgentReportNode>, max_abs: i32) -> i32 {
    analyst
        .map(|item| ((analyst_net_probability(item) * max_abs as f64).round()) as i32)
        .unwrap_or(0)
        .clamp(-max_abs, max_abs)
}

fn rating_bias(rating: &Rating, magnitude: i32) -> i32 {
    match rating {
        Rating::Buy => magnitude,
        Rating::Overweight => (magnitude * 3) / 4,
        Rating::Hold => 0,
        Rating::Underweight => -((magnitude * 3) / 4),
        Rating::Sell => -magnitude,
    }
}

fn map_direction_score_to_rating(score: i32) -> Rating {
    match score {
        60..=100 => Rating::Buy,
        20..=59 => Rating::Overweight,
        -19..=19 => Rating::Hold,
        -59..=-20 => Rating::Underweight,
        _ => Rating::Sell,
    }
}

fn direction_score_to_evidence_score(score: i32) -> i32 {
    match score {
        60..=100 => 2,
        20..=59 => 1,
        -19..=19 => 0,
        -59..=-20 => -1,
        _ => -2,
    }
}

fn rating_to_score(rating: &Rating) -> i32 {
    rating.to_score()
}

fn score_to_rating(score: i32) -> Rating {
    match score {
        2 => Rating::Buy,
        1 => Rating::Overweight,
        0 => Rating::Hold,
        -1 => Rating::Underweight,
        _ => Rating::Sell,
    }
}

fn rating_to_action(rating: &Rating) -> &'static str {
    rating.to_action_group()
}

fn semantic_direction(rating: &Rating) -> i32 {
    rating.bias(1)
}

#[cfg(test)]
mod technical_tests {
    use super::*;

    // --- normalized_key ---

    #[test]
    fn normalized_key_lowercases() {
        assert_eq!(normalized_key("Hello World"), "helloworld");
    }

    #[test]
    fn normalized_key_preserves_cjk() {
        assert_eq!(normalized_key("市场分析"), "市场分析");
    }

    #[test]
    fn normalized_key_strips_special() {
        assert_eq!(normalized_key("hello-world!"), "helloworld");
    }

    // --- is_cjk ---

    #[test]
    fn is_cjk_basic() {
        assert!(is_cjk('中'));
        assert!(is_cjk('文'));
        assert!(!is_cjk('A'));
        assert!(!is_cjk('1'));
    }

    // --- analyst_matches ---

    #[test]
    fn analyst_matches_by_key() {
        let node = AgentReportNode {
            key: "market_analysis".into(),
            title: "Market Report".into(),
            agent: "market".into(),
            ..Default::default()
        };
        assert!(analyst_matches(&node, &["market"]));
    }

    #[test]
    fn analyst_matches_by_chinese_title() {
        let node = AgentReportNode {
            key: "".into(),
            title: "NVDA 基本面分析".into(),
            agent: "".into(),
            ..Default::default()
        };
        assert!(analyst_matches(&node, &["fundamentals"]));
    }

    #[test]
    fn analyst_matches_no_match() {
        let node = AgentReportNode {
            key: "market".into(),
            title: "Market".into(),
            agent: "market".into(),
            ..Default::default()
        };
        assert!(!analyst_matches(&node, &["fundamentals"]));
    }

    #[test]
    fn analyst_matches_empty_candidate_ignored() {
        let node = AgentReportNode {
            key: "market".into(),
            ..Default::default()
        };
        assert!(!analyst_matches(&node, &[""]));
    }

    // --- matches_semantic_alias ---

    #[test]
    fn matches_semantic_alias_market() {
        assert!(matches_semantic_alias("market", "", "市场分析", ""));
        assert!(matches_semantic_alias("market", "", "技术面报告", ""));
    }

    #[test]
    fn matches_semantic_alias_fundamentals() {
        assert!(matches_semantic_alias("fundamentals", "", "基本面分析", ""));
        assert!(matches_semantic_alias("fundamental", "", "基本面", ""));
    }

    #[test]
    fn matches_semantic_alias_news() {
        assert!(matches_semantic_alias("news", "", "新闻催化", ""));
    }

    #[test]
    fn matches_semantic_alias_sentiment() {
        assert!(matches_semantic_alias("sentiment", "", "情绪分析", ""));
        assert!(matches_semantic_alias("sentiment", "", "资金面", ""));
    }

    #[test]
    fn matches_semantic_alias_unknown() {
        assert!(!matches_semantic_alias("unknown", "", "市场", ""));
    }

    // --- analyst_probability_quality ---

    #[test]
    fn analyst_probability_quality_none() {
        assert_eq!(analyst_probability_quality(None), 0);
    }

    #[test]
    fn analyst_probability_quality_perfect() {
        let node = AgentReportNode {
            up_probability: 0.5,
            down_probability: 0.3,
            sideways_probability: 0.2,
            ..Default::default()
        };
        assert_eq!(analyst_probability_quality(Some(&node)), 4);
    }

    #[test]
    fn analyst_probability_quality_off_by_010() {
        let node = AgentReportNode {
            up_probability: 0.5,
            down_probability: 0.3,
            sideways_probability: 0.3,
            ..Default::default()
        };
        assert_eq!(analyst_probability_quality(Some(&node)), 2);
    }

    #[test]
    fn analyst_probability_quality_off_by_more() {
        let node = AgentReportNode {
            up_probability: 0.5,
            down_probability: 0.5,
            sideways_probability: 0.5,
            ..Default::default()
        };
        assert_eq!(analyst_probability_quality(Some(&node)), 0);
    }

    // --- analyst_net_probability ---

    #[test]
    fn analyst_net_probability_bullish() {
        let node = AgentReportNode {
            up_probability: 0.7,
            down_probability: 0.2,
            ..Default::default()
        };
        assert!((analyst_net_probability(&node) - 0.5).abs() < 0.01);
    }

    #[test]
    fn analyst_net_probability_clamped() {
        let node = AgentReportNode {
            up_probability: 0.0,
            down_probability: 1.5,
            ..Default::default()
        };
        assert!((analyst_net_probability(&node) - (-1.0)).abs() < 0.01);
    }

    // --- score_analyst_net ---

    #[test]
    fn score_analyst_net_none() {
        assert_eq!(score_analyst_net(None, 20), 0);
    }

    #[test]
    fn score_analyst_net_bullish() {
        let node = AgentReportNode {
            up_probability: 0.7,
            down_probability: 0.2,
            ..Default::default()
        };
        let score = score_analyst_net(Some(&node), 20);
        assert!(score > 0, "expected positive, got {}", score);
    }

    // --- rating_bias ---

    #[test]
    fn rating_bias_buy() {
        assert_eq!(rating_bias(&Rating::Buy, 10), 10);
    }

    #[test]
    fn rating_bias_sell() {
        assert_eq!(rating_bias(&Rating::Sell, 10), -10);
    }

    #[test]
    fn rating_bias_hold() {
        assert_eq!(rating_bias(&Rating::Hold, 10), 0);
    }

    #[test]
    fn rating_bias_overweight() {
        assert_eq!(rating_bias(&Rating::Overweight, 10), 7);
    }

    #[test]
    fn rating_bias_underweight() {
        assert_eq!(rating_bias(&Rating::Underweight, 10), -7);
    }

    // --- map_direction_score_to_rating ---

    #[test]
    fn map_direction_score_to_rating_buy() {
        assert_eq!(map_direction_score_to_rating(80), Rating::Buy);
    }

    #[test]
    fn map_direction_score_to_rating_hold() {
        assert_eq!(map_direction_score_to_rating(0), Rating::Hold);
    }

    #[test]
    fn map_direction_score_to_rating_sell() {
        assert_eq!(map_direction_score_to_rating(-80), Rating::Sell);
    }

    // --- direction_score_to_evidence_score ---

    #[test]
    fn direction_score_to_evidence_score_buy_range() {
        assert_eq!(direction_score_to_evidence_score(80), 2);
    }

    #[test]
    fn direction_score_to_evidence_score_hold_range() {
        assert_eq!(direction_score_to_evidence_score(0), 0);
    }

    #[test]
    fn direction_score_to_evidence_score_sell_range() {
        assert_eq!(direction_score_to_evidence_score(-80), -2);
    }

    // --- score_to_rating ---

    #[test]
    fn score_to_rating_all_variants() {
        assert_eq!(score_to_rating(2), Rating::Buy);
        assert_eq!(score_to_rating(1), Rating::Overweight);
        assert_eq!(score_to_rating(0), Rating::Hold);
        assert_eq!(score_to_rating(-1), Rating::Underweight);
        assert_eq!(score_to_rating(-2), Rating::Sell);
    }

    // --- has_execution_boundary ---

    #[test]
    fn has_execution_boundary_complete() {
        let trader = StructuredTraderPlan {
            entry_price: "100".into(),
            stop_loss: "95".into(),
            ..Default::default()
        };
        let portfolio = StructuredPortfolioDecision {
            price_target: "110".into(),
            time_horizon: "1 week".into(),
            ..Default::default()
        };
        assert!(has_execution_boundary(&trader, &portfolio));
    }

    #[test]
    fn has_execution_boundary_with_confirmation() {
        let trader = StructuredTraderPlan {
            entry_price: "100".into(),
            stop_loss: "95".into(),
            ..Default::default()
        };
        let portfolio = StructuredPortfolioDecision {
            price_target: "".into(),
            confirmation_level: "108".into(),
            time_horizon: "1 week".into(),
            ..Default::default()
        };
        assert!(has_execution_boundary(&trader, &portfolio));
    }

    #[test]
    fn has_execution_boundary_missing_stop() {
        let trader = StructuredTraderPlan {
            entry_price: "100".into(),
            stop_loss: "".into(),
            ..Default::default()
        };
        let portfolio = StructuredPortfolioDecision {
            price_target: "110".into(),
            time_horizon: "1 week".into(),
            ..Default::default()
        };
        assert!(!has_execution_boundary(&trader, &portfolio));
    }

    #[test]
    fn has_execution_boundary_missing_horizon() {
        let trader = StructuredTraderPlan {
            entry_price: "100".into(),
            stop_loss: "95".into(),
            ..Default::default()
        };
        let portfolio = StructuredPortfolioDecision {
            price_target: "110".into(),
            time_horizon: "".into(),
            ..Default::default()
        };
        assert!(!has_execution_boundary(&trader, &portfolio));
    }

    // --- average_evidence_density ---

    #[test]
    fn average_evidence_density_empty() {
        assert_eq!(average_evidence_density(&[]), 0.0);
    }

    #[test]
    fn average_evidence_density_nonempty() {
        let analysts = vec![
            AgentReportNode {
                evidence_points: vec!["a".into(), "b".into()],
                ..Default::default()
            },
            AgentReportNode {
                evidence_points: vec!["c".into()],
                ..Default::default()
            },
        ];
        assert!((average_evidence_density(&analysts) - 1.5).abs() < 0.01);
    }
}

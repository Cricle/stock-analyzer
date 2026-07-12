use stock_analyzer::analysis::{
    AgentReportNode, Rating, StructuredPortfolioDecision, StructuredTraderPlan,
};
use stock_analyzer::scoring::{
    analyst_matches, analyst_net_probability, analyst_probability_quality,
    average_evidence_density, direction_score_to_evidence_score, has_execution_boundary, is_cjk,
    map_direction_score_to_rating, matches_semantic_alias, normalized_key, rating_bias,
    score_analyst_net, score_to_rating,
};

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
    assert_eq!(analyst_probability_quality(Some(&node)), 6);
}

#[test]
fn analyst_probability_quality_off_by_010() {
    let node = AgentReportNode {
        up_probability: 0.5,
        down_probability: 0.3,
        sideways_probability: 0.3,
        ..Default::default()
    };
    assert_eq!(analyst_probability_quality(Some(&node)), 4);
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

use stock_analyzer::data::{CandlePoint, FundamentalsSnapshot, NewsItem};
use stock_analyzer::guide::I18nText;
use stock_analyzer::pick::objective::{
    evaluate_stock_pick_objective_assessment, format_valuation_line, stock_pick_objective_gaps,
    stock_pick_objective_grade, stock_pick_objective_headline,
};
use stock_analyzer::pick::{EnrichedCandidate, FactorBreakdown};
use stock_analyzer::{
    StockPickDataQualitySnapshot, StockPickFundamentalSnapshot, StockPickHistoryMatchSnapshot,
    StockPickItem, StockPickMarketSnapshot, StockPickNewsSnapshot, StockPickRiskSnapshot,
    StockPickTechnicalSnapshot,
};

fn make_pick(
    thesis: &str,
    catalysts: Vec<&str>,
    risks: Vec<&str>,
    evidence: Vec<&str>,
) -> StockPickItem {
    StockPickItem {
        symbol: "TEST".to_string(),
        name: "Test".to_string(),
        market: "US".to_string(),
        exchange: "US".to_string(),
        thesis: I18nText::new(thesis),
        catalysts: catalysts.into_iter().map(I18nText::new).collect(),
        risks: risks.into_iter().map(I18nText::new).collect(),
        evidence_points: evidence.into_iter().map(String::from).collect(),
        price: Some(100.0),
        change_pct: Some(1.5),
        market_cap: Some(5_000_000_000.0),
        confidence: 65.0,
        ..StockPickItem::default()
    }
}

fn make_candidate(
    candles_len: usize,
    news_len: usize,
    fundamentals: Option<FundamentalsSnapshot>,
    factor: FactorBreakdown,
) -> EnrichedCandidate {
    EnrichedCandidate {
        symbol: "TEST".to_string(),
        name: "Test Corp".to_string(),
        market: "US".to_string(),
        exchange: "US".to_string(),
        industry: "Technology".to_string(),
        price: Some(100.0),
        change_pct: Some(1.5),
        market_cap: Some(5_000_000_000.0),
        theme_key: "tech".to_string(),
        fundamentals: fundamentals.clone(),
        news: (0..news_len)
            .map(|i| NewsItem {
                published_at: format!("2024-01-{:02}", i + 1),
                title: format!("News {i}"),
                summary: "summary".to_string(),
                source: "Reuters".to_string(),
                url: Some(format!("http://u{i}")),
            })
            .collect(),
        evidence_records: Vec::new(),
        candles: (0..candles_len)
            .map(|i| CandlePoint {
                trade_date: format!("2024-01-{:02}", i + 1),
                open: 10.0 + i as f64,
                close: 10.5 + i as f64,
                high: 11.0 + i as f64,
                low: 9.5 + i as f64,
                volume: 1000,
                amount: 10000.0,
                amplitude_pct: 4.0,
                change_pct: 2.0,
                change_amount: 0.5,
                turnover_pct: 1.0,
            })
            .collect(),
        technical_snapshot: StockPickTechnicalSnapshot::default(),
        market_snapshot: StockPickMarketSnapshot::default(),
        fundamental_snapshot: StockPickFundamentalSnapshot::default(),
        news_snapshot: StockPickNewsSnapshot::default(),
        history_match_snapshot: StockPickHistoryMatchSnapshot::default(),
        risk_snapshot: StockPickRiskSnapshot::default(),
        data_quality_snapshot: StockPickDataQualitySnapshot::default(),
        factor,
        pass_filter: true,
        rejected_reasons: Vec::new(),
        description: String::new(),
    }
}

// --- stock_pick_objective_grade ---

#[test]
fn grade_a() {
    assert_eq!(stock_pick_objective_grade(85), "A");
    assert_eq!(stock_pick_objective_grade(100), "A");
}

#[test]
fn grade_b() {
    assert_eq!(stock_pick_objective_grade(75), "B");
    assert_eq!(stock_pick_objective_grade(84), "B");
}

#[test]
fn grade_c() {
    assert_eq!(stock_pick_objective_grade(60), "C");
    assert_eq!(stock_pick_objective_grade(74), "C");
}

#[test]
fn grade_d() {
    assert_eq!(stock_pick_objective_grade(59), "D");
    assert_eq!(stock_pick_objective_grade(0), "D");
}

// --- stock_pick_objective_headline ---

#[test]
fn headline_ready_high_score() {
    let h = stock_pick_objective_headline(90, true, &[]);
    assert!(h.contains("High-quality"));
}

#[test]
fn headline_ready_normal_score() {
    let h = stock_pick_objective_headline(78, true, &[]);
    assert!(h.contains("Usable"));
}

#[test]
fn headline_not_ready_no_gaps() {
    let h = stock_pick_objective_headline(50, false, &[]);
    assert!(h.contains("mixed"));
}

#[test]
fn headline_not_ready_with_gaps() {
    let gaps = vec!["missing_fundamentals".to_string(), "thin_news".to_string()];
    let h = stock_pick_objective_headline(40, false, &gaps);
    assert!(h.contains("Not fully ready"));
    assert!(h.contains("missing_fundamentals"));
}

// --- format_valuation_line ---

#[test]
fn valuation_line_none_value() {
    assert!(format_valuation_line("PE", None, 20.0).is_none());
}

#[test]
fn valuation_line_zero_value() {
    assert!(format_valuation_line("PE", Some(0.0), 20.0).is_none());
}

#[test]
fn valuation_line_negative_value() {
    assert!(format_valuation_line("PE", Some(-5.0), 20.0).is_none());
}

#[test]
fn valuation_line_infinite_value() {
    assert!(format_valuation_line("PE", Some(f64::INFINITY), 20.0).is_none());
}

#[test]
fn valuation_line_premium() {
    let line = format_valuation_line("PE", Some(30.0), 20.0).unwrap();
    assert!(line.contains("PE"));
    assert!(line.contains("30.0x"));
    assert!(line.contains("premium"));
}

#[test]
fn valuation_line_discount() {
    let line = format_valuation_line("PS", Some(5.0), 10.0).unwrap();
    assert!(line.contains("PS"));
    assert!(line.contains("discount"));
}

// --- stock_pick_objective_gaps ---

#[test]
fn gaps_no_fundamentals() {
    let pick = make_pick(
        "thesis",
        vec!["c1", "c2"],
        vec!["r1", "r2"],
        vec!["e1", "e2", "e3", "e4"],
    );
    let item = make_candidate(15, 5, None, FactorBreakdown::default());
    let gaps = stock_pick_objective_gaps(&pick, &item);
    assert!(gaps.contains(&"missing_fundamentals".to_string()));
}

#[test]
fn gaps_thin_news() {
    let pick = make_pick(
        "thesis",
        vec!["c1", "c2"],
        vec!["r1", "r2"],
        vec!["e1", "e2", "e3", "e4"],
    );
    let fund = FundamentalsSnapshot {
        industry: Some("Tech".to_string()),
        revenues_usd: Some(1_000_000.0),
        assets_usd: Some(2_000_000.0),
        ..FundamentalsSnapshot::default()
    };
    let item = make_candidate(15, 1, Some(fund), FactorBreakdown::default());
    let gaps = stock_pick_objective_gaps(&pick, &item);
    assert!(gaps.contains(&"thin_news_coverage".to_string()));
}

#[test]
fn gaps_short_price_history() {
    let pick = make_pick(
        "thesis",
        vec!["c1", "c2"],
        vec!["r1", "r2"],
        vec!["e1", "e2", "e3", "e4"],
    );
    let fund = FundamentalsSnapshot {
        industry: Some("Tech".to_string()),
        revenues_usd: Some(1_000_000.0),
        assets_usd: Some(2_000_000.0),
        ..FundamentalsSnapshot::default()
    };
    let item = make_candidate(5, 5, Some(fund), FactorBreakdown::default());
    let gaps = stock_pick_objective_gaps(&pick, &item);
    assert!(gaps.contains(&"short_price_history".to_string()));
}

#[test]
fn gaps_thin_evidence_points() {
    let pick = make_pick("thesis", vec!["c1", "c2"], vec!["r1", "r2"], vec!["e1"]);
    let fund = FundamentalsSnapshot {
        industry: Some("Tech".to_string()),
        revenues_usd: Some(1_000_000.0),
        assets_usd: Some(2_000_000.0),
        ..FundamentalsSnapshot::default()
    };
    let item = make_candidate(15, 5, Some(fund), FactorBreakdown::default());
    let gaps = stock_pick_objective_gaps(&pick, &item);
    assert!(gaps.contains(&"thin_evidence_points".to_string()));
}

#[test]
fn gaps_no_gaps_with_good_data() {
    let pick = make_pick(
        "A long thesis that is well over one hundred and twenty characters to clear the thesis length cap requirement for full score",
        vec!["c1", "c2"],
        vec!["r1", "r2"],
        vec!["e1", "e2", "e3", "e4"],
    );
    let fund = FundamentalsSnapshot {
        industry: Some("Tech".to_string()),
        revenues_usd: Some(1_000_000.0),
        assets_usd: Some(2_000_000.0),
        ..FundamentalsSnapshot::default()
    };
    let item = make_candidate(20, 5, Some(fund), FactorBreakdown::default());
    let gaps = stock_pick_objective_gaps(&pick, &item);
    assert!(gaps.is_empty());
}

// --- evaluate_stock_pick_objective_assessment ---

#[test]
fn assessment_smoke_test() {
    let pick = make_pick(
        "A solid thesis with enough chars for good score",
        vec!["c1", "c2"],
        vec!["r1", "r2"],
        vec!["e1", "e2", "e3", "e4"],
    );
    let fund = FundamentalsSnapshot {
        industry: Some("Tech".to_string()),
        revenues_usd: Some(1_000_000.0),
        assets_usd: Some(2_000_000.0),
        ..FundamentalsSnapshot::default()
    };
    let item = make_candidate(
        20,
        5,
        Some(fund),
        FactorBreakdown {
            momentum: 65.0,
            quality: 60.0,
            value: 55.0,
            profitability: 50.0,
            risk: 65.0,
            event: 60.0,
            evidence: 50.0,
            history: 45.0,
            penalty: 0.0,
            total: 62.0,
        },
    );
    let assessment = evaluate_stock_pick_objective_assessment(&pick, &item);
    assert!(assessment.final_score > 0);
    assert!(!assessment.grade.is_empty());
    assert!(!assessment.headline.is_empty());
}

#[test]
fn assessment_minimal_data_low_score() {
    let pick = make_pick("short", vec![], vec![], vec![]);
    let item = make_candidate(0, 0, None, FactorBreakdown::default());
    let assessment = evaluate_stock_pick_objective_assessment(&pick, &item);
    assert!(assessment.final_score < 75);
    assert_eq!(assessment.grade, "D");
}

#[test]
fn assessment_grade_reflects_score() {
    let pick = make_pick("thesis here", vec!["c1"], vec!["r1"], vec!["e1"]);
    let item = make_candidate(10, 3, None, FactorBreakdown::default());
    let assessment = evaluate_stock_pick_objective_assessment(&pick, &item);
    let expected_grade = stock_pick_objective_grade(assessment.final_score);
    assert_eq!(assessment.grade, expected_grade);
}

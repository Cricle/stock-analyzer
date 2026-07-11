use stock_analyzer::scoring::dimensions::fundamental::{FundamentalInput, score_fundamental};

#[test]
fn test_strong_fundamentals() {
    let input = FundamentalInput {
        pe_like: Some(12.0),
        ps_like: None,
        roe: Some(25.0),
        leverage: Some(0.8),
        market_cap: Some(10_000_000_000.0),
        revenues_usd: Some(5_000_000_000.0),
        net_income_usd: Some(1_000_000_000.0),
    };
    let result = score_fundamental(&input);
    assert!(
        result.score >= 75,
        "expected high score, got {}",
        result.score
    );
}

#[test]
fn test_weak_fundamentals() {
    let input = FundamentalInput {
        pe_like: Some(80.0),
        ps_like: None,
        roe: Some(-5.0),
        leverage: Some(4.0),
        market_cap: Some(100_000_000.0),
        revenues_usd: Some(0.0),
        net_income_usd: Some(-50_000_000.0),
    };
    let result = score_fundamental(&input);
    assert!(
        result.score <= 30,
        "expected low score, got {}",
        result.score
    );
}

#[test]
fn test_no_data_neutral() {
    let input = FundamentalInput {
        pe_like: None,
        ps_like: None,
        roe: None,
        leverage: None,
        market_cap: None,
        revenues_usd: None,
        net_income_usd: None,
    };
    let result = score_fundamental(&input);
    assert_eq!(result.score, 50);
}

#[test]
fn test_negative_pe() {
    let input = FundamentalInput {
        pe_like: Some(-10.0),
        ps_like: None,
        roe: None,
        leverage: None,
        market_cap: None,
        revenues_usd: None,
        net_income_usd: None,
    };
    let result = score_fundamental(&input);
    assert_eq!(result.score, 50); // 12.5/25 * 100 = 50
    assert!(
        result.reason.contains("PE为负"),
        "expected negative PE reason, got {}",
        result.reason
    );
}

#[test]
fn test_partial_data_pe_only() {
    let input = FundamentalInput {
        pe_like: Some(10.0),
        ps_like: None,
        roe: None,
        leverage: None,
        market_cap: None,
        revenues_usd: None,
        net_income_usd: None,
    };
    let result = score_fundamental(&input);
    assert_eq!(result.score, 100); // 25/25 * 100 = 100
}

#[test]
fn test_revenue_positive_net_loss() {
    let input = FundamentalInput {
        pe_like: None,
        ps_like: None,
        roe: None,
        leverage: None,
        market_cap: None,
        revenues_usd: Some(1_000_000_000.0),
        net_income_usd: Some(-200_000_000.0),
    };
    let result = score_fundamental(&input);
    assert_eq!(result.score, 48); // 12/25 * 100 = 48
    assert!(
        result.reason.contains("净利亏损"),
        "expected net loss reason, got {}",
        result.reason
    );
}

#[test]
fn test_high_leverage() {
    let input = FundamentalInput {
        pe_like: None,
        ps_like: None,
        roe: None,
        leverage: Some(5.0),
        market_cap: None,
        revenues_usd: None,
        net_income_usd: None,
    };
    let result = score_fundamental(&input);
    assert_eq!(result.score, 12); // 3/25 * 100 = 12
    assert!(
        result.reason.contains("过高"),
        "expected high leverage reason, got {}",
        result.reason
    );
}

#[test]
fn test_borderline_pe() {
    let input = FundamentalInput {
        pe_like: Some(15.0), // exactly at boundary
        ps_like: None,
        roe: None,
        leverage: None,
        market_cap: None,
        revenues_usd: None,
        net_income_usd: None,
    };
    let result = score_fundamental(&input);
    // PE 15.0 falls into <25 bucket (18/25 * 100 = 72)
    assert_eq!(result.score, 72);
}

#[test]
fn test_zero_roe() {
    let input = FundamentalInput {
        pe_like: None,
        ps_like: None,
        roe: Some(0.0),
        leverage: None,
        market_cap: None,
        revenues_usd: None,
        net_income_usd: None,
    };
    let result = score_fundamental(&input);
    // ROE 0.0 falls into >0? no, it's exactly 0 → >0 is false → 2/25*100 = 8
    assert_eq!(result.score, 8);
}

#[test]
fn test_mixed_signals() {
    // Good PE, bad ROE, good leverage, bad revenue
    let input = FundamentalInput {
        pe_like: Some(10.0),
        ps_like: None,
        roe: Some(-10.0),
        leverage: Some(0.5),
        market_cap: None,
        revenues_usd: Some(0.0),
        net_income_usd: None,
    };
    let result = score_fundamental(&input);
    // PE: 25/25, ROE: 2/25, leverage: 25/25, revenue: 3/25
    // total=55, weight=100, score=55
    assert_eq!(result.score, 55);
    assert!(result.reason.contains("估值偏低"), "expected low PE reason");
    assert!(result.reason.contains("亏损"), "expected ROE loss reason");
}

#[test]
fn test_score_range() {
    // Test with all max values
    let input = FundamentalInput {
        pe_like: Some(5.0),
        ps_like: Some(1.0),
        roe: Some(50.0),
        leverage: Some(0.3),
        market_cap: Some(100_000_000_000.0),
        revenues_usd: Some(50_000_000_000.0),
        net_income_usd: Some(10_000_000_000.0),
    };
    let result = score_fundamental(&input);
    assert!(
        result.score <= 100,
        "score should be <= 100, got {}",
        result.score
    );
    assert!(
        result.score >= 90,
        "expected very high score, got {}",
        result.score
    );
}

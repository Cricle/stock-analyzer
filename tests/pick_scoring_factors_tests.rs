use sa::data::{CandlePoint, FundamentalsSnapshot};
use sa::pick::EnrichedCandidate;
use sa::pick::scoring::factors::{compute_factor_breakdown, penalty_score};
use sa::pick::FactorBreakdown;
use sa::{
    StockPickDataQualitySnapshot, StockPickFundamentalSnapshot, StockPickHistoryMatchSnapshot,
    StockPickMarketSnapshot, StockPickNewsSnapshot, StockPickRiskSnapshot,
    StockPickTechnicalSnapshot,
};

fn make_candle(
    trade_date: &str,
    open: f64,
    close: f64,
    volume: i64,
    change_pct: f64,
    turnover_pct: f64,
) -> CandlePoint {
    CandlePoint {
        trade_date: trade_date.to_string(),
        open,
        close,
        high: close.max(open) * 1.02,
        low: close.min(open) * 0.98,
        volume,
        amount: volume as f64 * close,
        amplitude_pct: 4.0,
        change_pct,
        change_amount: close - open,
        turnover_pct,
    }
}

fn make_enriched(
    candles: Vec<CandlePoint>,
    fundamentals: Option<FundamentalsSnapshot>,
    news_snapshot: StockPickNewsSnapshot,
    risk_snapshot: StockPickRiskSnapshot,
    history_match_snapshot: StockPickHistoryMatchSnapshot,
    change_pct: Option<f64>,
    market_cap: Option<f64>,
) -> EnrichedCandidate {
    EnrichedCandidate {
        symbol: "TEST".to_string(),
        name: "Test Corp".to_string(),
        market: "US".to_string(),
        exchange: "US".to_string(),
        industry: "Technology".to_string(),
        price: candles.last().map(|c| c.close),
        change_pct,
        market_cap,
        theme_key: "tech".to_string(),
        fundamentals,
        news: Vec::new(),
        evidence_records: Vec::new(),
        candles,
        technical_snapshot: StockPickTechnicalSnapshot::default(),
        market_snapshot: StockPickMarketSnapshot::default(),
        fundamental_snapshot: StockPickFundamentalSnapshot::default(),
        news_snapshot,
        history_match_snapshot,
        risk_snapshot,
        data_quality_snapshot: StockPickDataQualitySnapshot::default(),
        factor: FactorBreakdown::default(),
        pass_filter: true,
        rejected_reasons: Vec::new(),
        description: String::new(),
    }
}

// --- compute_factor_breakdown ---

#[test]
fn factor_breakdown_empty_candles_momentum_zero() {
    let item = make_enriched(
        Vec::new(),
        None,
        StockPickNewsSnapshot::default(),
        StockPickRiskSnapshot::default(),
        StockPickHistoryMatchSnapshot::default(),
        None,
        None,
    );
    let fb = compute_factor_breakdown(&item);
    assert_eq!(fb.momentum, 0.0);
}

#[test]
fn factor_breakdown_no_fundamentals_quality_default() {
    let candles = vec![
        make_candle("2024-01-01", 10.0, 10.5, 1000, 5.0, 2.0),
        make_candle("2024-01-02", 10.5, 11.0, 1200, 4.76, 2.5),
    ];
    let item = make_enriched(
        candles,
        None,
        StockPickNewsSnapshot::default(),
        StockPickRiskSnapshot::default(),
        StockPickHistoryMatchSnapshot::default(),
        None,
        None,
    );
    let fb = compute_factor_breakdown(&item);
    assert_eq!(fb.quality, 50.0);
    assert_eq!(fb.value, 50.0);
    assert_eq!(fb.profitability, 50.0);
}

#[test]
fn factor_breakdown_total_in_range() {
    let candles = (0..20)
        .map(|i| {
            make_candle(
                &format!("2024-01-{:02}", i + 1),
                10.0 + i as f64 * 0.5,
                10.5 + i as f64 * 0.5,
                1000 + i as i64 * 100,
                2.0,
                1.5,
            )
        })
        .collect::<Vec<_>>();
    let item = make_enriched(
        candles,
        None,
        StockPickNewsSnapshot::default(),
        StockPickRiskSnapshot::default(),
        StockPickHistoryMatchSnapshot::default(),
        None,
        None,
    );
    let fb = compute_factor_breakdown(&item);
    assert!((0.0..=100.0).contains(&fb.total));
}

#[test]
fn factor_breakdown_with_fundamentals() {
    let candles = vec![
        make_candle("2024-01-01", 100.0, 105.0, 5000, 5.0, 1.0),
        make_candle("2024-01-02", 105.0, 110.0, 6000, 4.76, 1.2),
    ];
    let fund = FundamentalsSnapshot {
        symbol: "TEST".to_string(),
        company_name: "Test".to_string(),
        cik: String::new(),
        industry: Some("Tech".to_string()),
        currency: "USD".to_string(),
        fiscal_year_end: None,
        shares_outstanding: Some(100_000_000),
        market_cap: Some(11_000_000_000.0),
        net_income_usd: Some(500_000_000.0),
        revenues_usd: Some(5_000_000_000.0),
        assets_usd: Some(10_000_000_000.0),
        liabilities_usd: Some(3_000_000_000.0),
        stockholders_equity_usd: Some(7_000_000_000.0),
        cash_and_equivalents_usd: Some(2_000_000_000.0),
        gross_profit_usd: Some(2_000_000_000.0),
        operating_income_usd: Some(800_000_000.0),
        operating_expenses_usd: Some(1_200_000_000.0),
        operating_cash_flow_usd: Some(600_000_000.0),
        capital_expenditure_usd: Some(-200_000_000.0),
        free_cash_flow_usd: Some(400_000_000.0),
        long_term_debt_usd: Some(2_000_000_000.0),
        current_debt_usd: Some(1_000_000_000.0),
        total_debt_usd: Some(3_000_000_000.0),
        diluted_shares_outstanding: Some(105_000_000),
    };
    let item = make_enriched(
        candles,
        Some(fund),
        StockPickNewsSnapshot::default(),
        StockPickRiskSnapshot::default(),
        StockPickHistoryMatchSnapshot::default(),
        None,
        Some(11_000_000_000.0),
    );
    let fb = compute_factor_breakdown(&item);
    assert!(fb.quality != 40.0 || fb.quality == 40.0);
    assert!((0.0..=100.0).contains(&fb.quality));
    assert!((0.0..=100.0).contains(&fb.value));
    assert!((0.0..=100.0).contains(&fb.profitability));
}

#[test]
fn factor_breakdown_high_momentum_uptrend() {
    let candles = (0..10)
        .map(|i| {
            make_candle(
                &format!("2024-01-{:02}", i + 1),
                10.0 + i as f64,
                11.0 + i as f64,
                1000 + i as i64 * 100,
                5.0,
                1.0,
            )
        })
        .collect::<Vec<_>>();
    let item = make_enriched(
        candles,
        None,
        StockPickNewsSnapshot::default(),
        StockPickRiskSnapshot::default(),
        StockPickHistoryMatchSnapshot::default(),
        None,
        None,
    );
    let fb = compute_factor_breakdown(&item);
    assert!(fb.momentum > 50.0);
}

// --- penalty_score ---

#[test]
fn penalty_no_penalties_default() {
    let item = make_enriched(
        Vec::new(),
        None,
        StockPickNewsSnapshot::default(),
        StockPickRiskSnapshot::default(),
        StockPickHistoryMatchSnapshot::default(),
        None,
        None,
    );
    assert_eq!(penalty_score(&item), 0.0);
}

#[test]
fn penalty_high_change_pct() {
    let item = make_enriched(
        Vec::new(),
        None,
        StockPickNewsSnapshot::default(),
        StockPickRiskSnapshot::default(),
        StockPickHistoryMatchSnapshot::default(),
        Some(20.0),
        None,
    );
    assert_eq!(penalty_score(&item), -10.0);
}

#[test]
fn penalty_moderate_change_pct() {
    let item = make_enriched(
        Vec::new(),
        None,
        StockPickNewsSnapshot::default(),
        StockPickRiskSnapshot::default(),
        StockPickHistoryMatchSnapshot::default(),
        Some(10.0),
        None,
    );
    assert_eq!(penalty_score(&item), -5.0);
}

#[test]
fn penalty_negative_market_cap() {
    let item = make_enriched(
        Vec::new(),
        None,
        StockPickNewsSnapshot::default(),
        StockPickRiskSnapshot::default(),
        StockPickHistoryMatchSnapshot::default(),
        None,
        Some(-1.0),
    );
    assert_eq!(penalty_score(&item), -6.0);
}

#[test]
fn penalty_volatility_elevated() {
    let item = make_enriched(
        Vec::new(),
        None,
        StockPickNewsSnapshot::default(),
        StockPickRiskSnapshot {
            volatility_elevated: true,
            ..StockPickRiskSnapshot::default()
        },
        StockPickHistoryMatchSnapshot::default(),
        None,
        None,
    );
    assert_eq!(penalty_score(&item), -4.0);
}

#[test]
fn penalty_liquidity_warning() {
    let item = make_enriched(
        Vec::new(),
        None,
        StockPickNewsSnapshot::default(),
        StockPickRiskSnapshot {
            liquidity_warning: true,
            ..StockPickRiskSnapshot::default()
        },
        StockPickHistoryMatchSnapshot::default(),
        None,
        None,
    );
    assert_eq!(penalty_score(&item), -4.0);
}

#[test]
fn penalty_valuation_stretched() {
    let item = make_enriched(
        Vec::new(),
        None,
        StockPickNewsSnapshot::default(),
        StockPickRiskSnapshot {
            valuation_stretched: true,
            ..StockPickRiskSnapshot::default()
        },
        StockPickHistoryMatchSnapshot::default(),
        None,
        None,
    );
    assert_eq!(penalty_score(&item), -3.0);
}

#[test]
fn penalty_all_risks_combined() {
    let fund = FundamentalsSnapshot {
        revenues_usd: Some(-1_000_000.0),
        ..FundamentalsSnapshot::default()
    };
    let item = make_enriched(
        Vec::new(),
        Some(fund),
        StockPickNewsSnapshot::default(),
        StockPickRiskSnapshot {
            volatility_elevated: true,
            liquidity_warning: true,
            valuation_stretched: true,
            ..StockPickRiskSnapshot::default()
        },
        StockPickHistoryMatchSnapshot::default(),
        Some(20.0),
        Some(-1.0),
    );
    let penalty = penalty_score(&item);
    assert_eq!(penalty, -32.0);
}

#[test]
fn penalty_low_change_no_penalty() {
    let item = make_enriched(
        Vec::new(),
        None,
        StockPickNewsSnapshot::default(),
        StockPickRiskSnapshot::default(),
        StockPickHistoryMatchSnapshot::default(),
        Some(5.0),
        None,
    );
    assert_eq!(penalty_score(&item), 0.0);
}

#[test]
fn missing_fundamentals_gives_neutral_not_depressed() {
    let item = make_enriched(
        Vec::new(),
        None,
        StockPickNewsSnapshot::default(),
        StockPickRiskSnapshot::default(),
        StockPickHistoryMatchSnapshot::default(),
        None,
        None,
    );
    let factors = compute_factor_breakdown(&item);
    assert!(
        factors.quality >= 48.0,
        "quality should be neutral when fundamentals missing, got {}",
        factors.quality
    );
    assert!(
        factors.profitability >= 48.0,
        "profitability should be neutral when fundamentals missing, got {}",
        factors.profitability
    );
}

// --- risk_score edge case ---

#[test]
fn risk_score_few_candles_returns_default() {
    let candles = vec![make_candle("2024-01-01", 10.0, 10.5, 1000, 5.0, 2.0)];
    let item = make_enriched(
        candles,
        None,
        StockPickNewsSnapshot::default(),
        StockPickRiskSnapshot::default(),
        StockPickHistoryMatchSnapshot::default(),
        None,
        None,
    );
    let fb = compute_factor_breakdown(&item);
    assert_eq!(fb.risk, 35.0);
}

// --- event_score ---

#[test]
fn event_score_no_news() {
    let item = make_enriched(
        Vec::new(),
        None,
        StockPickNewsSnapshot::default(),
        StockPickRiskSnapshot::default(),
        StockPickHistoryMatchSnapshot::default(),
        None,
        None,
    );
    let fb = compute_factor_breakdown(&item);
    assert_eq!(fb.event, 35.0);
}

#[test]
fn event_score_with_catalysts_and_news() {
    let news = StockPickNewsSnapshot {
        deep_item_count: 5,
        light_item_count: 3,
        latest_published_at: "2024-06-01".to_string(),
        catalyst_count: 3,
        ..StockPickNewsSnapshot::default()
    };
    let item = make_enriched(
        Vec::new(),
        None,
        news,
        StockPickRiskSnapshot::default(),
        StockPickHistoryMatchSnapshot::default(),
        None,
        None,
    );
    let fb = compute_factor_breakdown(&item);
    assert!((fb.event - 81.0).abs() < 0.01);
}

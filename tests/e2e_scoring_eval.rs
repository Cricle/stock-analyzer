mod common;

use common::stocks::TEST_STOCKS;
use stock_analyzer::score::dimensions::{
    fundamental::{self, FundamentalInput},
    technical::{self, TechnicalInput},
};
use stock_analyzer::score::types::score_label;

/// Build a TechnicalInput from real market data.
async fn build_technical_input(
    client: &stock_analyzer::MarketDataClient,
    symbol: &str,
) -> TechnicalInput {
    let candles = client
        .fetch_candles(symbol, "qfq", 200)
        .await
        .unwrap_or_default();
    let quote = client.fetch_quote(symbol).await.ok();

    let current_price = quote.as_ref().map(|q| q.close);

    let closes: Vec<f64> = candles.iter().map(|c| c.close).collect();

    let rsi = compute_rsi(&closes, 14);
    let sma50 = compute_sma(&closes, 50);
    let sma200 = compute_sma(&closes, 200);
    let ema10 = compute_ema(&closes, 10);

    let volumes: Vec<f64> = candles.iter().map(|c| c.volume as f64).collect();
    let avg_vol = if volumes.len() >= 20 {
        volumes[volumes.len() - 20..].iter().sum::<f64>() / 20.0
    } else {
        0.0
    };
    let latest_vol = volumes.last().copied().unwrap_or(0.0);
    let volume_elevated = latest_vol > avg_vol * 1.2;

    let latest_positive = closes
        .last()
        .zip(closes.iter().nth_back(1))
        .map(|(cur, prev)| cur > prev)
        .unwrap_or(false);

    TechnicalInput {
        rsi,
        macd: None,
        macd_signal: None,
        macd_hist: None,
        adx: None,
        close_10_ema: ema10,
        close_50_sma: sma50,
        close_200_sma: sma200,
        obv: None,
        current_price,
        volume_elevated,
        latest_positive,
    }
}

fn compute_rsi(closes: &[f64], period: usize) -> Option<f64> {
    if closes.len() < period + 1 {
        return None;
    }
    let mut gains = 0.0;
    let mut losses = 0.0;
    for i in closes.len() - period..closes.len() {
        let change = closes[i] - closes[i - 1];
        if change > 0.0 {
            gains += change;
        } else {
            losses -= change;
        }
    }
    let avg_gain = gains / period as f64;
    let avg_loss = losses / period as f64;
    if avg_loss == 0.0 {
        return Some(100.0);
    }
    let rs = avg_gain / avg_loss;
    Some(100.0 - 100.0 / (1.0 + rs))
}

fn compute_sma(closes: &[f64], period: usize) -> Option<f64> {
    if closes.len() < period {
        return None;
    }
    let sum: f64 = closes[closes.len() - period..].iter().sum();
    Some(sum / period as f64)
}

fn compute_ema(closes: &[f64], period: usize) -> Option<f64> {
    if closes.len() < period {
        return None;
    }
    let k = 2.0 / (period as f64 + 1.0);
    let mut ema = closes[..period].iter().sum::<f64>() / period as f64;
    for &price in &closes[period..] {
        ema = price * k + ema * (1.0 - k);
    }
    Some(ema)
}

/// Build a FundamentalInput from real market data.
async fn build_fundamental_input(
    client: &stock_analyzer::MarketDataClient,
    symbol: &str,
) -> FundamentalInput {
    let fund = client.fetch_fundamentals(symbol).await.ok();

    FundamentalInput {
        pe_like: None,
        ps_like: None,
        roe: None,
        leverage: None,
        market_cap: fund.as_ref().and_then(|f| f.market_cap),
        revenues_usd: fund.as_ref().and_then(|f| f.revenues_usd),
        net_income_usd: fund.as_ref().and_then(|f| f.net_income_usd),
    }
}

#[tokio::test]
#[ignore]
async fn e2e_scoring_technical_dimension() {
    let client = stock_analyzer::MarketDataClient::new().await.unwrap();

    for stock in TEST_STOCKS {
        let input = build_technical_input(&client, stock.symbol).await;
        let result = technical::score_technical(&input);

        println!(
            "{}: technical_score={} reason={}",
            stock.symbol, result.score, result.reason
        );
        assert!(
            result.score <= 100,
            "Score should be <= 100, got {}",
            result.score
        );

        if input.rsi.is_some() || input.current_price.is_some() {
            assert!(
                result.score > 0 || !result.reason.is_empty(),
                "Expected non-trivial score for {}",
                stock.symbol
            );
        }
    }
}

#[tokio::test]
#[ignore]
async fn e2e_scoring_fundamental_dimension() {
    let client = stock_analyzer::MarketDataClient::new().await.unwrap();

    for stock in TEST_STOCKS {
        let input = build_fundamental_input(&client, stock.symbol).await;
        let result = fundamental::score_fundamental(&input);

        println!(
            "{}: fundamental_score={} reason={}",
            stock.symbol, result.score, result.reason
        );
        assert!(
            result.score <= 100,
            "Score should be <= 100, got {}",
            result.score
        );
    }
}

#[tokio::test]
async fn e2e_scoring_label_mapping() {
    assert_eq!(score_label(85), "strong_buy");
    assert_eq!(score_label(70), "buy");
    assert_eq!(score_label(55), "neutral");
    assert_eq!(score_label(35), "cautious");
    assert_eq!(score_label(20), "avoid");
}

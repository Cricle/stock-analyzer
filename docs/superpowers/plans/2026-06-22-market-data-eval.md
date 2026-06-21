# Market Data Evaluation — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build integration tests that evaluate data completeness, accuracy, and scoring quality across 3 markets (A-share, HK, US) using 6 representative stocks.

**Architecture:** Layered testing — real market data fetching (Layer 1), real-data scoring (Layer 2), and report structure validation (Layer 3). No LLM API key required. Uses `MarketDataClient::new()` for data, synchronous scoring functions for technical/fundamental dimensions.

**Tech Stack:** Rust, tokio, sa-data (MarketDataClient), sa-engine (score dimensions), sa-types (QuoteSnapshot, FundamentalsSnapshot, NewsItem, CandlePoint), Decimal (rust_decimal)

---

## File Map

| File | Purpose |
|------|---------|
| `tests/common/stocks.rs` | TestStock definition and 6 stock constants |
| `tests/common/eval.rs` | Completeness check helpers and summary table printer |
| `tests/common/mock_llm.rs` | Mock LLM client for sentiment/LLM-analysis scoring |
| `tests/e2e_market_data.rs` | Layer 1: real data fetch + validation for 6 stocks |
| `tests/e2e_scoring_eval.rs` | Layer 2: real data → ScoreablePick → scoring validation |

---

### Task 1: Test Stock Definitions

**Files:**
- Create: `tests/common/stocks.rs`
- Modify: `tests/common/mod.rs`

- [ ] **Step 1: Create the stocks module**

```rust
// tests/common/stocks.rs

use sa_data::MarketKind;

pub struct TestStock {
    pub symbol: &'static str,
    pub name: &'static str,
    pub market: &'static str,
    pub market_kind: MarketKind,
    pub is_famous: bool,
}

pub const TEST_STOCKS: &[TestStock] = &[
    // A-share
    TestStock {
        symbol: "600519",
        name: "贵州茅台",
        market: "A股",
        market_kind: MarketKind::AShare,
        is_famous: true,
    },
    TestStock {
        symbol: "688256",
        name: "寒武纪",
        market: "A股",
        market_kind: MarketKind::AShare,
        is_famous: false,
    },
    // HK
    TestStock {
        symbol: "00700",
        name: "腾讯控股",
        market: "港股",
        market_kind: MarketKind::HongKong,
        is_famous: true,
    },
    TestStock {
        symbol: "00020",
        name: "商汤科技",
        market: "港股",
        market_kind: MarketKind::HongKong,
        is_famous: false,
    },
    // US
    TestStock {
        symbol: "AAPL",
        name: "Apple",
        market: "美股",
        market_kind: MarketKind::UsEquity,
        is_famous: true,
    },
    TestStock {
        symbol: "PLTR",
        name: "Palantir",
        market: "美股",
        market_kind: MarketKind::UsEquity,
        is_famous: false,
    },
];

pub fn a_share_stocks() -> Vec<&'static TestStock> {
    TEST_STOCKS.iter().filter(|s| s.market_kind == MarketKind::AShare).collect()
}

pub fn hk_stocks() -> Vec<&'static TestStock> {
    TEST_STOCKS.iter().filter(|s| s.market_kind == MarketKind::HongKong).collect()
}

pub fn us_stocks() -> Vec<&'static TestStock> {
    TEST_STOCKS.iter().filter(|s| s.market_kind == MarketKind::UsEquity).collect()
}
```

- [ ] **Step 2: Add module declaration to common/mod.rs**

Add to `tests/common/mod.rs`:

```rust
pub mod stocks;
pub mod eval;
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo check --tests 2>&1 | tail -5`
Expected: no errors related to `stocks` module

- [ ] **Step 4: Commit**

```bash
git add tests/common/stocks.rs tests/common/mod.rs
git commit -m "test: add test stock definitions for 3 markets"
```

---

### Task 2: Evaluation Helpers

**Files:**
- Create: `tests/common/eval.rs`

- [ ] **Step 1: Create eval module with completeness tracking**

```rust
// tests/common/eval.rs

use sa_types::{QuoteSnapshot, FundamentalsSnapshot, NewsItem, CandlePoint};
use rust_decimal::prelude::ToPrimitive;

pub struct StockEvalResult {
    pub symbol: String,
    pub name: String,
    pub quote_ok: bool,
    pub fundamentals_ok: bool,
    pub fundamentals_partial: bool,
    pub news_ok: bool,
    pub news_count: usize,
    pub candles_ok: bool,
    pub candle_count: usize,
}

impl StockEvalResult {
    pub fn score_pct(&self) -> u32 {
        let mut total = 0;
        let mut max = 0;

        // Quote: 25 points
        max += 25;
        if self.quote_ok { total += 25; }

        // Fundamentals: 25 points
        max += 25;
        if self.fundamentals_ok { total += 25; }
        else if self.fundamentals_partial { total += 15; }

        // News: 25 points
        max += 25;
        if self.news_ok { total += 25; }
        else if self.news_count > 0 { total += 10; }

        // Candles: 25 points
        max += 25;
        if self.candles_ok { total += 25; }
        else if self.candle_count > 0 { total += 10; }

        (total * 100) / max
    }
}

pub fn assert_quote_valid(quote: &QuoteSnapshot) -> bool {
    let price_ok = quote.close > rust_decimal::Decimal::ZERO;
    let volume_ok = quote.volume > 0;
    price_ok && volume_ok
}

pub fn assert_fundamentals_valid(fund: &FundamentalsSnapshot) -> (bool, bool) {
    let has_metric = fund.net_income_usd.is_some()
        || fund.revenues_usd.is_some()
        || fund.stockholders_equity_usd.is_some();
    let has_market_cap = fund.market_cap.is_some()
        && fund.market_cap.unwrap() > rust_decimal::Decimal::ZERO;
    let is_complete = has_metric && has_market_cap && !fund.company_name.is_empty();
    let is_partial = has_metric || has_market_cap;
    (is_complete, is_partial)
}

pub fn assert_news_valid(news: &[NewsItem]) -> bool {
    if news.len() < 3 {
        return false;
    }
    news.iter().all(|n| !n.title.is_empty())
}

pub fn assert_candles_valid(candles: &[CandlePoint]) -> bool {
    if candles.len() < 60 {
        return false;
    }
    candles.iter().all(|c| {
        c.open > rust_decimal::Decimal::ZERO
            && c.close > rust_decimal::Decimal::ZERO
            && c.high >= c.low
            && c.volume > 0
    })
}

pub fn print_completeness_table(results: &[StockEvalResult]) {
    println!("\n{:<10} | {:<8} | {:<15} | {:<8} | {:<10} | {}",
        "Stock", "Quote", "Fundamentals", "News", "Candles", "Score");
    println!("{}", "-".repeat(75));
    for r in results {
        let quote = if r.quote_ok { "OK" } else { "MISSING" };
        let fund = if r.fundamentals_ok { "OK" }
            else if r.fundamentals_partial { "partial" }
            else { "MISSING" };
        let news = if r.news_ok { "OK" }
            else if r.news_count > 0 { &format!("{} items", r.news_count) }
            else { "MISSING" };
        let candles = if r.candles_ok { "OK" }
            else if r.candle_count > 0 { &format!("{} days", r.candle_count) }
            else { "MISSING" };
        println!("{:<10} | {:<8} | {:<15} | {:<8} | {:<10} | {}%",
            r.symbol, quote, fund, news, candles, r.score_pct());
    }
    let avg = results.iter().map(|r| r.score_pct() as f64).sum::<f64>() / results.len() as f64;
    println!("{}", "-".repeat(75));
    println!("{:<10} | {:<8} | {:<15} | {:<8} | {:<10} | {:.0}%",
        "AVERAGE", "", "", "", "", avg);
}
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo check --tests 2>&1 | tail -5`
Expected: no errors

- [ ] **Step 3: Commit**

```bash
git add tests/common/eval.rs
git commit -m "test: add evaluation helpers for market data completeness"
```

---

### Task 3: Layer 1 — Market Data Fetch Tests

**Files:**
- Create: `tests/e2e_market_data.rs`

- [ ] **Step 1: Write the market data fetch test**

```rust
// tests/e2e_market_data.rs

mod common;

use common::stocks::TEST_STOCKS;
use common::eval::{
    StockEvalResult, assert_quote_valid, assert_fundamentals_valid,
    assert_news_valid, assert_candles_valid, print_completeness_table,
};

#[tokio::test]
async fn e2e_fetch_all_market_data() {
    let client = sa_data::MarketDataClient::new().await;
    let mut results = Vec::new();

    for stock in TEST_STOCKS {
        let symbol = stock.symbol;
        println!("\n=== Fetching {} ({}) ===", stock.name, symbol);

        // Quote
        let quote_result = client.fetch_quote(symbol).await;
        let quote_ok = match &quote_result {
            Ok(q) => {
                let valid = assert_quote_valid(q);
                println!("  Quote: price={} volume={} change={:?} valid={}",
                    q.close, q.volume, q.change_pct, valid);
                valid
            }
            Err(e) => {
                println!("  Quote: ERROR - {}", e);
                false
            }
        };

        // Fundamentals
        let fund_result = client.fetch_fundamentals(symbol).await;
        let (fundamentals_ok, fundamentals_partial) = match &fund_result {
            Ok(f) => {
                let (complete, partial) = assert_fundamentals_valid(f);
                println!("  Fundamentals: name={} currency={} market_cap={:?} complete={} partial={}",
                    f.company_name, f.currency, f.market_cap, complete, partial);
                (complete, partial)
            }
            Err(e) => {
                println!("  Fundamentals: ERROR - {}", e);
                (false, false)
            }
        };

        // News (30-day window)
        let news_result = client.fetch_news(symbol, 20, None, None).await;
        let (news_ok, news_count) = match &news_result {
            Ok(items) => {
                let valid = assert_news_valid(items);
                println!("  News: {} items, valid={}", items.len(), valid);
                for (i, item) in items.iter().take(3).enumerate() {
                    println!("    [{}] {} - {}", i + 1, item.published_at, item.title);
                }
                (valid, items.len())
            }
            Err(e) => {
                println!("  News: ERROR - {}", e);
                (false, 0)
            }
        };

        // Candles (120 days to ensure enough data)
        let candles_result = client.fetch_candles(symbol, "qfq", 120).await;
        let (candles_ok, candle_count) = match &candles_result {
            Ok(candles) => {
                let valid = assert_candles_valid(candles);
                println!("  Candles: {} days, valid={}", candles.len(), valid);
                if let Some(last) = candles.last() {
                    println!("    Last: {} O={} H={} L={} C={} V={}",
                        last.trade_date, last.open, last.high, last.low, last.close, last.volume);
                }
                (valid, candles.len())
            }
            Err(e) => {
                println!("  Candles: ERROR - {}", e);
                (false, 0)
            }
        };

        results.push(StockEvalResult {
            symbol: symbol.to_string(),
            name: stock.name.to_string(),
            quote_ok,
            fundamentals_ok,
            fundamentals_partial,
            news_ok,
            news_count,
            candles_ok,
            candle_count,
        });
    }

    print_completeness_table(&results);

    // Assertions based on success criteria
    let quotes_ok = results.iter().filter(|r| r.quote_ok).count();
    assert!(quotes_ok >= 4, "Expected at least 4/6 stocks with valid quotes, got {}", quotes_ok);

    let funds_ok = results.iter().filter(|r| r.fundamentals_ok || r.fundamentals_partial).count();
    assert!(funds_ok >= 4, "Expected at least 4/6 stocks with fundamentals, got {}", funds_ok);

    let news_ok = results.iter().filter(|r| r.news_ok).count();
    assert!(news_ok >= 3, "Expected at least 3/6 stocks with valid news, got {}", news_ok);
}

#[tokio::test]
async fn e2e_market_detection() {
    let client = sa_data::MarketDataClient::new().await;

    for stock in TEST_STOCKS {
        let detected = client.detect_market(stock.symbol);
        assert_eq!(detected, stock.market_kind,
            "Market detection for {} should be {:?}, got {:?}",
            stock.symbol, stock.market_kind, detected);
    }
}
```

- [ ] **Step 2: Run the market data tests**

Run: `cargo test --test e2e_market_data -- --nocapture 2>&1 | tail -30`
Expected: Tests pass, completeness table printed

- [ ] **Step 3: Review the completeness output and note any failures**

If any stocks have MISSING data, document which stocks and which data types.

- [ ] **Step 4: Commit**

```bash
git add tests/e2e_market_data.rs
git commit -m "test: add market data completeness evaluation for 6 stocks across 3 markets"
```

---

### Task 4: Layer 2 — Scoring Evaluation Tests

**Files:**
- Create: `tests/e2e_scoring_eval.rs`

- [ ] **Step 1: Write the scoring evaluation test**

```rust
// tests/e2e_scoring_eval.rs

mod common;

use common::stocks::TEST_STOCKS;
use sa_engine::score::dimensions::{
    technical::{self, TechnicalInput},
    fundamental::{self, FundamentalInput},
};
use sa_engine::score::types::score_label;
use rust_decimal::prelude::ToPrimitive;

/// Build a TechnicalInput from real market data.
async fn build_technical_input(
    client: &sa_data::MarketDataClient,
    symbol: &str,
) -> TechnicalInput {
    let candles = client.fetch_candles(symbol, "qfq", 200).await.unwrap_or_default();
    let quote = client.fetch_quote(symbol).await.ok();

    let current_price = quote.as_ref().map(|q| q.close.to_f64().unwrap_or(0.0));

    // Compute simple indicators from candles
    let closes: Vec<f64> = candles.iter().map(|c| c.close.to_f64().unwrap_or(0.0)).collect();

    let rsi = compute_rsi(&closes, 14);
    let sma50 = compute_sma(&closes, 50);
    let sma200 = compute_sma(&closes, 200);
    let ema10 = compute_ema(&closes, 10);

    // Volume analysis
    let volumes: Vec<f64> = candles.iter().map(|c| c.volume as f64).collect();
    let avg_vol = if volumes.len() >= 20 {
        volumes[volumes.len()-20..].iter().sum::<f64>() / 20.0
    } else { 0.0 };
    let latest_vol = volumes.last().copied().unwrap_or(0.0);
    let volume_elevated = latest_vol > avg_vol * 1.2;

    // Price trend
    let latest_positive = closes.last().zip(closes.iter().nth_back(1))
        .map(|(cur, prev)| cur > prev)
        .unwrap_or(false);

    TechnicalInput {
        rsi,
        macd: None,      // Would need full MACD computation
        macd_signal: None,
        macd_hist: None,
        adx: None,        // Would need full ADX computation
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
    if closes.len() < period + 1 { return None; }
    let mut gains = 0.0;
    let mut losses = 0.0;
    for i in closes.len()-period..closes.len() {
        let change = closes[i] - closes[i-1];
        if change > 0.0 { gains += change; }
        else { losses -= change; }
    }
    let avg_gain = gains / period as f64;
    let avg_loss = losses / period as f64;
    if avg_loss == 0.0 { return Some(100.0); }
    let rs = avg_gain / avg_loss;
    Some(100.0 - 100.0 / (1.0 + rs))
}

fn compute_sma(closes: &[f64], period: usize) -> Option<f64> {
    if closes.len() < period { return None; }
    let sum: f64 = closes[closes.len()-period..].iter().sum();
    Some(sum / period as f64)
}

fn compute_ema(closes: &[f64], period: usize) -> Option<f64> {
    if closes.len() < period { return None; }
    let k = 2.0 / (period as f64 + 1.0);
    let mut ema = closes[..period].iter().sum::<f64>() / period as f64;
    for &price in &closes[period..] {
        ema = price * k + ema * (1.0 - k);
    }
    Some(ema)
}

/// Build a FundamentalInput from real market data.
async fn build_fundamental_input(
    client: &sa_data::MarketDataClient,
    symbol: &str,
) -> FundamentalInput {
    let fund = client.fetch_fundamentals(symbol).await.ok();

    FundamentalInput {
        pe_like: None,  // PE not directly in FundamentalsSnapshot; would need price/market_cap calc
        ps_like: None,
        roe: None,      // Would need net_income / equity calc
        leverage: None,
        market_cap: fund.as_ref().and_then(|f| f.market_cap.and_then(|mc| mc.to_f64())),
        revenues_usd: fund.as_ref().and_then(|f| f.revenues_usd.and_then(|r| r.to_f64())),
        net_income_usd: fund.as_ref().and_then(|f| f.net_income_usd.and_then(|n| n.to_f64())),
    }
}

#[tokio::test]
async fn e2e_scoring_technical_dimension() {
    let client = sa_data::MarketDataClient::new().await;

    for stock in TEST_STOCKS {
        let input = build_technical_input(&client, stock.symbol).await;
        let result = technical::score_technical(&input);

        println!("{}: technical_score={} reason={}", stock.symbol, result.score, result.reason);
        assert!(result.score <= 100, "Score should be <= 100, got {}", result.score);

        // With real data, we should get some score (not zero unless all indicators missing)
        if input.rsi.is_some() || input.current_price.is_some() {
            assert!(result.score > 0 || !result.reason.is_empty(),
                "Expected non-trivial score for {}", stock.symbol);
        }
    }
}

#[tokio::test]
async fn e2e_scoring_fundamental_dimension() {
    let client = sa_data::MarketDataClient::new().await;

    for stock in TEST_STOCKS {
        let input = build_fundamental_input(&client, stock.symbol).await;
        let result = fundamental::score_fundamental(&input);

        println!("{}: fundamental_score={} reason={}", stock.symbol, result.score, result.reason);
        assert!(result.score <= 100, "Score should be <= 100, got {}", result.score);
    }
}

#[tokio::test]
async fn e2e_scoring_label_mapping() {
    // Verify score labels are consistent
    assert_eq!(score_label(85), "strong_buy");
    assert_eq!(score_label(70), "buy");
    assert_eq!(score_label(55), "neutral");
    assert_eq!(score_label(35), "cautious");
    assert_eq!(score_label(20), "avoid");
}
```

- [ ] **Step 2: Run the scoring tests**

Run: `cargo test --test e2e_scoring_eval -- --nocapture 2>&1 | tail -30`
Expected: Tests pass, scores printed for each stock

- [ ] **Step 3: Review scoring output**

Check that:
- Scores are in valid range [0, 100]
- Famous companies don't score unreasonably low
- No panics from missing data (graceful handling with None)

- [ ] **Step 4: Commit**

```bash
git add tests/e2e_scoring_eval.rs
git commit -m "test: add scoring evaluation with real market data for 6 stocks"
```

---

### Task 5: Run All Tests and Generate Summary

**Files:**
- No new files

- [ ] **Step 1: Run all e2e tests together**

Run: `cargo test --test e2e_market_data --test e2e_scoring_eval -- --nocapture 2>&1 | tail -50`
Expected: All tests pass, completeness table and scores printed

- [ ] **Step 2: Document findings**

If any data gaps or scoring anomalies were found, add a summary comment in the test output or create a brief findings note.

- [ ] **Step 3: Final commit**

```bash
git add -A
git commit -m "test: complete market data evaluation suite for 3 markets"
```

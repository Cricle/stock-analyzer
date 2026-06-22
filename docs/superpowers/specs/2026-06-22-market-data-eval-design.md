# Market Data Evaluation — Integration Test Design

**Date**: 2026-06-22
**Branch**: feat/store-implementations
**Goal**: Evaluate data completeness, accuracy, and comprehensiveness across 3 markets (A-share, HK, US) using 6 representative stocks.

## Test Stocks

| Market | Famous | Emerging |
|--------|--------|----------|
| A-share | 600519 贵州茅台 | 688256 寒武纪 |
| HK | 00700 腾讯控股 | 00020 商汤科技 |
| US | AAPL Apple | PLTR Palantir |

## Architecture

Layered testing approach — no LLM API key required:

```
┌─────────────────────────────────────┐
│  Layer 3: Report Pipeline (mock LLM)│  e2e_report_eval.rs
├─────────────────────────────────────┤
│  Layer 2: Scoring (real data)       │  e2e_scoring_eval.rs
├─────────────────────────────────────┤
│  Layer 1: Market Data (real fetch)  │  e2e_market_data.rs
└─────────────────────────────────────┘
```

## Layer 1 — Market Data Fetch Tests

**File**: `tests/e2e_market_data.rs`

For each of the 6 stocks, fetch real data via `MarketDataClient` and validate:

### Quote Validation
- `current_price` is `Some` and > 0
- `volume` is `Some` and > 0
- `change_pct` is `Some` (not necessarily non-zero for weekends/holidays)
- `symbol` matches the requested symbol
- `name` is non-empty

### Fundamentals Validation
- At least one of `pe`, `pb`, `roe` is `Some`
- `market_cap` is `Some` and > 0 for famous companies
- `revenue` or `net_income` is `Some` for famous companies
- A-share stocks use CNY units, HK uses HKD, US uses USD

### News Validation
- At least 3 news items returned (for active stocks within 30-day window)
- Each news item has non-empty `title` and `url`
- News dates are within the expected range
- Language matches market (A-share: Chinese, US: English, HK: mixed)

### Candlestick Validation
- At least 60 trading days of candle data
- Each candle has `open`, `high`, `low`, `close` > 0
- `high >= low` invariant holds
- `volume` is present and > 0
- Dates are sequential (no large gaps > 5 business days)

### Cross-Market Consistency
- A-share symbols: 6-digit codes (600519, 688256)
- HK symbols: numeric codes (00700, 00020)
- US symbols: alphabetic tickers (AAPL, PLTR)
- Market detection (`detect_market`) returns correct `MarketKind`

### Data Completeness Report
After all fetches, print a summary table:

```
Stock       | Quote | Fundamentals | News | Candles | Score
600519      |  OK   |     OK       |  OK  |   OK    | 100%
688256      |  OK   |   partial    |  OK  |   OK    |  85%
00700       |  OK   |     OK       |  OK  |   OK    | 100%
...
```

## Layer 2 — Scoring Evaluation Tests

**File**: `tests/e2e_scoring_eval.rs`

Use real market data to construct `ScoreablePick` and validate scoring:

### Score Construction
- Fetch quote + fundamentals + news for each stock
- Map to `ScoreablePick` fields
- Verify no `None` for critical fields (price, volume)

### Scoring Validation
- Run `score_pick()` for each stock
- Verify each dimension score is in [0, 100]:
  - Technical score
  - Fundamental score
  - Sentiment score
  - LLM analysis score (use default/neutral since no LLM)
- Verify composite score is weighted average of dimensions
- Verify famous companies score higher on fundamentals than emerging ones (generally)

### Edge Cases
- Stock with missing PE ratio (common for pre-profit companies like 寒武纪)
- Stock with zero volume (holiday trading)
- Stock with very high PE (> 1000)

## Layer 3 — Report Pipeline Tests

**File**: `tests/e2e_report_eval.rs`

Test the full `TaskManager` pipeline with a mock LLM client:

### Mock LLM Design
- `MockLlmClient` implements the LLM call interface
- Returns pre-canned JSON responses matching expected LLM output format
- Validates that prompts contain expected market data context

### Pipeline Validation
- Create task via `TaskManager::create_task_and_run_blocking()`
- Verify task status transitions: Pending → Running → Completed
- Verify `AnalysisResult` structure:
  - `report.summary` is non-empty
  - `report.recommendation` is one of Buy/Sell/Hold
  - `report.risk_assessment` is present
  - `report.technical_analysis` references real indicator values
  - Token usage is tracked

### Report Completeness
- Check all expected report sections are populated
- Verify no empty strings in required fields
- Verify numeric fields are in valid ranges

## Test Helpers

### `tests/common/stocks.rs`
```rust
struct TestStock {
    symbol: &'static str,
    name: &'static str,
    market: &'static str,
    market_kind: MarketKind,
    is_famous: bool,
    expected_currency: &'static str,
}
```

### `tests/common/eval.rs`
```rust
fn print_completeness_table(results: &[StockEvalResult]);
fn assert_quote_valid(quote: &QuoteSnapshot, stock: &TestStock);
fn assert_fundamentals_valid(fund: &FundamentalsSnapshot, stock: &TestStock);
fn assert_news_valid(news: &[NewsItem], stock: &TestStock);
fn assert_candles_valid(candles: &[CandlePoint], stock: &TestStock);
```

## Success Criteria

1. All 6 stocks fetch quote data successfully
2. At least 4/6 stocks have complete fundamentals
3. All 6 stocks return at least 3 news items
4. All 6 stocks have at least 60 days of candle data
5. Scoring produces valid scores for all 6 stocks
6. Report pipeline completes without errors for all 6 stocks

## Out of Scope

- LLM output quality evaluation (requires real LLM)
- Performance benchmarking
- Load testing
- Storage persistence verification (uses in-memory stores)

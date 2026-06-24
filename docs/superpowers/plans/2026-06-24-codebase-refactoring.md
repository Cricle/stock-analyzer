# Codebase Refactoring Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 精简 stock-analyzer 仓库，专注分析能力；数据获取完全委托给 akshare-rs。同时拆分大文件，合并重复模块。

**Architecture:** 将数据获取代码（news_filter、tools）迁移到 akshare-rs，sa 通过 trait 接口调用数据层。合并 score/ 到 scoring/，拆分大文件为 200-400 行的小文件。

**Tech Stack:** Rust, async-trait, tokio, serde

---

## File Structure

### Files to Create in sa
- `crates/sa/src/data/traits.rs` — MarketDataProvider trait
- `crates/sa/src/data/mock.rs` — MockMarketProvider for tests
- `crates/sa/src/scoring/dimensions/mod.rs` — moved from score/dimensions/
- `crates/sa/src/scoring/dimensions/technical.rs` — moved from score/
- `crates/sa/src/scoring/dimensions/llm_analysis.rs` — moved from score/
- `crates/sa/src/scoring/dimensions/sentiment.rs` — moved from score/
- `crates/sa/src/scoring/dimensions/fundamental.rs` — moved from score/
- Split files (see Task 6-10)

### Files to Modify in sa
- `crates/sa/src/data/mod.rs` — add trait re-export
- `crates/sa/src/lib.rs` — remove `pub mod tools`
- `crates/sa/src/tools/mod.rs` — DELETE
- `crates/sa/src/score/mod.rs` — DELETE
- `crates/sa/src/store_impls.rs` — split into 4 files
- Various import updates

### Files to Create/Modify in akshare-rs
- `crates/akshare/src/provider/market_client/news_filter.rs` — moved from sa
- `crates/akshare/src/provider/market_client/tools/` — moved from sa
- `crates/akshare/src/provider/market_client/mod.rs` — add re-exports

---

## Task 1: Define MarketDataProvider Trait

**Files:**
- Create: `crates/sa/src/data/traits.rs`
- Modify: `crates/sa/src/data/mod.rs`

- [ ] **Step 1: Create trait file**

```rust
// crates/sa/src/data/traits.rs
use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;

use super::{CandlePoint, FundamentalsSnapshot, NewsItem, QuoteSnapshot};

#[async_trait]
pub trait MarketDataProvider: Send + Sync {
    // News
    async fn fetch_news(&self, symbol: &str, limit: usize) -> Result<Vec<NewsItem>>;
    async fn fetch_global_news(&self, market: &str, limit: usize) -> Result<Vec<NewsItem>>;

    // Market data
    async fn fetch_candles(&self, symbol: &str, days: usize) -> Result<Vec<CandlePoint>>;
    async fn fetch_quote(&self, symbol: &str) -> Result<QuoteSnapshot>;
    async fn fetch_fundamentals(&self, symbol: &str) -> Result<FundamentalsSnapshot>;

    // Financial statements
    async fn fetch_balance_sheet(&self, symbol: &str) -> Result<Value>;
    async fn fetch_cashflow(&self, symbol: &str) -> Result<Value>;
    async fn fetch_income_statement(&self, symbol: &str) -> Result<Value>;

    // Indicators
    async fn compute_indicators(&self, candles: &[CandlePoint], params: &Value) -> Result<Value>;

    // Insider transactions
    async fn fetch_insider_transactions(&self, symbol: &str) -> Result<Value>;
}
```

- [ ] **Step 2: Update mod.rs to export trait**

Add to `crates/sa/src/data/mod.rs`:
```rust
pub mod traits;
pub use traits::MarketDataProvider;
```

- [ ] **Step 3: Verify compilation**

Run: `cargo check`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add crates/sa/src/data/traits.rs crates/sa/src/data/mod.rs
git commit -m "feat: add MarketDataProvider trait definition"
```

---

## Task 2: Create MockMarketProvider

**Files:**
- Create: `crates/sa/src/data/mock.rs`
- Modify: `crates/sa/src/data/mod.rs`

- [ ] **Step 1: Create mock file**

```rust
// crates/sa/src/data/mock.rs
use anyhow::Result;
use async_trait::async_trait;
use serde_json::{Value, json};

use super::traits::MarketDataProvider;
use super::{CandlePoint, FundamentalsSnapshot, NewsItem, QuoteSnapshot};

pub struct MockMarketProvider {
    pub news: Vec<NewsItem>,
    pub global_news: Vec<NewsItem>,
    pub candles: Vec<CandlePoint>,
    pub quote: Option<QuoteSnapshot>,
    pub fundamentals: Option<FundamentalsSnapshot>,
    pub balance_sheet: Value,
    pub cashflow: Value,
    pub income_statement: Value,
    pub indicators: Value,
    pub insider_transactions: Value,
}

impl Default for MockMarketProvider {
    fn default() -> Self {
        Self {
            news: Vec::new(),
            global_news: Vec::new(),
            candles: Vec::new(),
            quote: None,
            fundamentals: None,
            balance_sheet: json!(null),
            cashflow: json!(null),
            income_statement: json!(null),
            indicators: json!(null),
            insider_transactions: json!(null),
        }
    }
}

#[async_trait]
impl MarketDataProvider for MockMarketProvider {
    async fn fetch_news(&self, _symbol: &str, limit: usize) -> Result<Vec<NewsItem>> {
        Ok(self.news.iter().take(limit).cloned().collect())
    }

    async fn fetch_global_news(&self, _market: &str, limit: usize) -> Result<Vec<NewsItem>> {
        Ok(self.global_news.iter().take(limit).cloned().collect())
    }

    async fn fetch_candles(&self, _symbol: &str, _days: usize) -> Result<Vec<CandlePoint>> {
        Ok(self.candles.clone())
    }

    async fn fetch_quote(&self, _symbol: &str) -> Result<QuoteSnapshot> {
        self.quote.clone().ok_or_else(|| anyhow::anyhow!("no quote data"))
    }

    async fn fetch_fundamentals(&self, _symbol: &str) -> Result<FundamentalsSnapshot> {
        self.fundamentals.clone().ok_or_else(|| anyhow::anyhow!("no fundamentals data"))
    }

    async fn fetch_balance_sheet(&self, _symbol: &str) -> Result<Value> {
        Ok(self.balance_sheet.clone())
    }

    async fn fetch_cashflow(&self, _symbol: &str) -> Result<Value> {
        Ok(self.cashflow.clone())
    }

    async fn fetch_income_statement(&self, _symbol: &str) -> Result<Value> {
        Ok(self.income_statement.clone())
    }

    async fn compute_indicators(&self, _candles: &[CandlePoint], _params: &Value) -> Result<Value> {
        Ok(self.indicators.clone())
    }

    async fn fetch_insider_transactions(&self, _symbol: &str) -> Result<Value> {
        Ok(self.insider_transactions.clone())
    }
}
```

- [ ] **Step 2: Update mod.rs**

Add to `crates/sa/src/data/mod.rs`:
```rust
pub mod mock;
```

- [ ] **Step 3: Verify compilation**

Run: `cargo check`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add crates/sa/src/data/mock.rs crates/sa/src/data/mod.rs
git commit -m "feat: add MockMarketProvider for testing"
```

---

## Task 3: Migrate news_filter.rs to akshare-rs

**Files:**
- Create: `crates/akshare/src/provider/market_client/news_filter.rs`
- Modify: `crates/akshare/src/provider/market_client/mod.rs`
- Delete: `crates/sa/src/data/news_filter.rs`

- [ ] **Step 1: Copy news_filter.rs to akshare-rs**

Copy the entire content of `crates/sa/src/data/news_filter.rs` to `crates/akshare/src/provider/market_client/news_filter.rs`.

- [ ] **Step 2: Update akshare-rs mod.rs**

Add to `crates/akshare/src/provider/market_client/mod.rs`:
```rust
pub mod news_filter;
pub use news_filter::*;
```

- [ ] **Step 3: Update sa re-exports**

Update `crates/sa/src/data/mod.rs` to re-export from akshare:
```rust
pub use akshare::provider::market_client::normalized_news_date;
```

- [ ] **Step 4: Delete old file from sa**

```bash
rm crates/sa/src/data/news_filter.rs
```

- [ ] **Step 5: Verify compilation**

Run: `cargo check`
Expected: PASS (fix any import errors)

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "refactor: migrate news_filter.rs to akshare-rs"
```

---

## Task 4: Migrate tools/ to akshare-rs

**Files:**
- Create: `crates/akshare/src/provider/market_client/tools/` (all files)
- Modify: `crates/akshare/src/provider/market_client/mod.rs`
- Delete: `crates/sa/src/tools/` (entire directory)

- [ ] **Step 1: Copy tools/ to akshare-rs**

Copy all files from `crates/sa/src/tools/` to `crates/akshare/src/provider/market_client/tools/`:
- `mod.rs`
- `summarize.rs`
- `news/fetch.rs`
- `news/global.rs`
- `news/prelude.rs`
- `news.rs`
- `market_data/stock_data.rs`
- `market_data/financial.rs`
- `market_data/prelude.rs`
- `market_data.rs`
- `indicators/compute.rs`
- `indicators/prelude.rs`
- `indicators.rs`

- [ ] **Step 2: Update imports in moved files**

Fix all `use crate::` imports to `use crate::provider::market_client::` or appropriate paths.

- [ ] **Step 3: Update akshare-rs mod.rs**

Add to `crates/akshare/src/provider/market_client/mod.rs`:
```rust
pub mod tools;
```

- [ ] **Step 4: Delete tools/ from sa**

```bash
rm -rf crates/sa/src/tools/
```

- [ ] **Step 5: Update sa lib.rs**

Remove `pub mod tools;` from `crates/sa/src/lib.rs`.

- [ ] **Step 6: Update all imports in sa**

Find and fix all references to `crate::tools::` in sa codebase. Replace with appropriate akshare imports.

- [ ] **Step 7: Verify compilation**

Run: `cargo check`
Expected: PASS (fix any import errors)

- [ ] **Step 8: Commit**

```bash
git add -A
git commit -m "refactor: migrate tools/ to akshare-rs"
```

---

## Task 5: Merge score/ into scoring/

**Files:**
- Create: `crates/sa/src/scoring/dimensions/` (all files from score/dimensions/)
- Modify: `crates/sa/src/scoring/mod.rs`
- Modify: `crates/sa/src/score/scorer.rs` — move to scoring/
- Modify: `crates/sa/src/score/config.rs` — move to scoring/
- Modify: `crates/sa/src/score/types.rs` — move to scoring/
- Modify: `crates/sa/src/score/history.rs` — move to scoring/
- Delete: `crates/sa/src/score/` (entire directory)

- [ ] **Step 1: Move dimensions/ to scoring/**

```bash
cp -r crates/sa/src/score/dimensions crates/sa/src/scoring/dimensions
```

- [ ] **Step 2: Move other score files to scoring/**

Move and rename:
- `score/scorer.rs` → `scoring/scorer.rs`
- `score/config.rs` → `scoring/config.rs`
- `score/types.rs` → `scoring/score_types.rs` (avoid conflict with existing types.rs)
- `score/history.rs` → `scoring/history.rs`

- [ ] **Step 3: Update scoring/mod.rs**

Add to `crates/sa/src/scoring/mod.rs`:
```rust
pub mod dimensions;
pub mod scorer;
pub mod config;
pub mod score_types;
pub mod history;
```

- [ ] **Step 4: Fix imports in moved files**

Change all `crate::score::` to `crate::scoring::` in the moved files.

- [ ] **Step 5: Update all imports in sa**

Find and replace all `use crate::score::` with `use crate::scoring::` across the codebase.

- [ ] **Step 6: Delete score/ module**

```bash
rm -rf crates/sa/src/score/
```

- [ ] **Step 7: Update lib.rs**

Remove `pub mod score;` from `crates/sa/src/lib.rs` if present.

- [ ] **Step 8: Verify compilation**

Run: `cargo check`
Expected: PASS

- [ ] **Step 9: Commit**

```bash
git add -A
git commit -m "refactor: merge score/ into scoring/"
```

---

## Task 6: Split store_impls.rs

**Files:**
- Create: `crates/sa/src/store/mod.rs`
- Create: `crates/sa/src/store/memory.rs`
- Create: `crates/sa/src/store/redis.rs`
- Create: `crates/sa/src/store/sqlite.rs`
- Delete: `crates/sa/src/store_impls.rs`

- [ ] **Step 1: Analyze store_impls.rs structure**

Read the file and identify the logical sections:
- InMemoryStore implementation
- RedisStore implementation (if exists)
- SQLiteStore implementation (if exists)
- Factory/builder functions

- [ ] **Step 2: Create store/ directory and mod.rs**

```rust
// crates/sa/src/store/mod.rs
mod memory;
// mod redis;
// mod sqlite;

pub use memory::InMemoryStore;
```

- [ ] **Step 3: Extract InMemoryStore to memory.rs**

Move InMemoryStore implementation to `crates/sa/src/store/memory.rs`.

- [ ] **Step 4: Update imports**

Update all files that import from `store_impls` to use `store::` instead.

- [ ] **Step 5: Delete store_impls.rs**

```bash
rm crates/sa/src/store_impls.rs
```

- [ ] **Step 6: Update lib.rs**

Replace `pub mod store_impls;` with `pub mod store;`.

- [ ] **Step 7: Verify compilation**

Run: `cargo check`
Expected: PASS

- [ ] **Step 8: Commit**

```bash
git add -A
git commit -m "refactor: split store_impls.rs into store/ module"
```

---

## Task 7: Split pick/scoring/mod.rs (1446 lines)

**Files:**
- Create: `crates/sa/src/pick/scoring/factors.rs`
- Create: `crates/sa/src/pick/scoring/weights.rs`
- Create: `crates/sa/src/pick/scoring/calc.rs`
- Modify: `crates/sa/src/pick/scoring/mod.rs`

- [ ] **Step 1: Analyze the file**

Identify function groups:
- Factor definitions and scoring functions (factors.rs)
- Weight calculation and normalization (weights.rs)
- Core calculation logic (calc.rs)

- [ ] **Step 2: Create factors.rs**

Move factor-related functions:
- `momentum_score()`
- `value_score()`
- `quality_score()`
- Other dimension scoring functions

- [ ] **Step 3: Create weights.rs**

Move weight-related functions:
- Weight normalization
- Portfolio constraint application

- [ ] **Step 4: Create calc.rs**

Move calculation functions:
- Technical indicator calculations
- RSI, MACD, Bollinger functions

- [ ] **Step 5: Update mod.rs**

```rust
mod factors;
mod weights;
mod calc;

pub use factors::*;
pub use weights::*;
pub use calc::*;
```

- [ ] **Step 6: Verify compilation**

Run: `cargo check`
Expected: PASS

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "refactor: split pick/scoring/mod.rs into submodules"
```

---

## Task 8: Split report/lifecycle/task_run.rs (1423 lines)

**Files:**
- Create: `crates/sa/src/report/lifecycle/fetch.rs`
- Create: `crates/sa/src/report/lifecycle/format.rs`
- Modify: `crates/sa/src/report/lifecycle/task_run.rs`

- [ ] **Step 1: Analyze the file**

Identify function groups:
- Data fetching functions (fetch.rs)
- Formatting/summary functions (format.rs)
- Task execution flow (keep in task_run.rs)

- [ ] **Step 2: Create fetch.rs**

Move:
- `fetch_core_market_data()`
- `hydrate_scenario_data()`

- [ ] **Step 3: Create format.rs**

Move:
- `format_fund_flow_summary()`
- `format_billboard_summary()`
- `format_margin_summary()`
- `format_hot_rank_summary()`
- `format_earnings_forecast_summary()`
- `format_limit_pool_summary()`

- [ ] **Step 4: Update task_run.rs**

Update imports to use the new modules.

- [ ] **Step 5: Verify compilation**

Run: `cargo check`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "refactor: split task_run.rs into fetch and format modules"
```

---

## Task 9: Split pick/pipeline/mod.rs (1381 lines)

**Files:**
- Create: `crates/sa/src/pick/pipeline/filter.rs`
- Create: `crates/sa/src/pick/pipeline/rank.rs`
- Create: `crates/sa/src/pick/pipeline/select.rs`
- Modify: `crates/sa/src/pick/pipeline/mod.rs`

- [ ] **Step 1: Analyze the file**

Identify pipeline stages:
- Candidate filtering (filter.rs)
- Candidate ranking (rank.rs)
- Final selection (select.rs)

- [ ] **Step 2: Create filter.rs**

Move filtering logic:
- Market cap filters
- Volume filters
- Basic quality filters

- [ ] **Step 3: Create rank.rs**

Move ranking logic:
- Multi-factor ranking
- Score aggregation

- [ ] **Step 4: Create select.rs**

Move selection logic:
- Top-N selection
- Portfolio constraints

- [ ] **Step 5: Update mod.rs**

```rust
mod filter;
mod rank;
mod select;

pub use filter::*;
pub use rank::*;
pub use select::*;
```

- [ ] **Step 6: Verify compilation**

Run: `cargo check`
Expected: PASS

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "refactor: split pick/pipeline/mod.rs into submodules"
```

---

## Task 10: Split remaining large files

**Files:**
- Split `pick/objective/mod.rs` (1166 lines) → `constraints.rs`, `optimize.rs`
- Split `report/result/stages.rs` (968 lines) → `prepare.rs`, `execute.rs`, `finalize.rs`
- Split `memory/core.rs` (944 lines) → `store.rs`, `retrieve.rs`, `index.rs`
- Split `report/diagnosis/consistency.rs` (914 lines) → `check.rs`, `validate.rs`
- Split `scoring/helpers/calc.rs` (798 lines) → `technical.rs`, `fundamental.rs`

- [ ] **Step 1: Split pick/objective/mod.rs**

Create `constraints.rs` and `optimize.rs`, move relevant functions.

- [ ] **Step 2: Split report/result/stages.rs**

Create `prepare.rs`, `execute.rs`, `finalize.rs`, move stage logic.

- [ ] **Step 3: Split memory/core.rs**

Create `store.rs`, `retrieve.rs`, `index.rs`, move memory operations.

- [ ] **Step 4: Split report/diagnosis/consistency.rs**

Create `check.rs`, `validate.rs`, move diagnostic logic.

- [ ] **Step 5: Split scoring/helpers/calc.rs**

Create `technical.rs`, `fundamental.rs`, move calculation functions.

- [ ] **Step 6: Verify compilation**

Run: `cargo check`
Expected: PASS

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "refactor: split remaining large files into submodules"
```

---

## Task 11: Update Tests

**Files:**
- Modify: All test files that use `crate::tools::` or `crate::score::`
- Update mock usage to use MockMarketProvider

- [ ] **Step 1: Find all test files**

```bash
grep -rn "#\[cfg(test)\]" crates/sa/src/ --include="*.rs" -l
```

- [ ] **Step 2: Update test imports**

Replace `crate::tools::` with appropriate akshare imports or mock usage.

- [ ] **Step 3: Update test data setup**

Use MockMarketProvider instead of direct data fetching in tests.

- [ ] **Step 4: Run all tests**

Run: `cargo test`
Expected: All tests pass

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "test: update tests for refactored codebase"
```

---

## Task 12: Final Verification

- [ ] **Step 1: Run full test suite**

```bash
cargo test --all-features
```

- [ ] **Step 2: Run clippy**

```bash
cargo clippy -- -D warnings
```

- [ ] **Step 3: Verify no dead code**

```bash
cargo build 2>&1 | grep "warning.*unused"
```

- [ ] **Step 4: Count lines**

```bash
find crates/sa/src -name "*.rs" | xargs wc -l | tail -1
```

Expected: Significantly less than 48,864 lines

- [ ] **Step 5: Final commit**

```bash
git add -A
git commit -m "chore: complete codebase refactoring"
```

# Codebase Refactoring Design Spec

**Date:** 2026-06-24
**Goal:** 精简 stock-analyzer 仓库，专注分析能力；数据获取完全委托给 akshare-rs。同时拆分大文件，合并重复模块。

## Current State

- **Total lines:** ~48,864 in sa crate
- **Data layer code:** ~3,195 行（news_filter 1563 + tools 1607）应属于 akshare-rs
- **Duplicate modules:** `score/` 和 `scoring/` 职责重叠
- **Large files:** 11 个文件超过 500 行，最大 1446 行

## Target Architecture

```
sa (analysis engine)
├── analysis/          — 指标计算、技术分析
├── report/            — 报告组装、生命周期
├── pick/              — 选股（独立模块）
├── scoring/           — 评分系统（合并 score/）
│   └── dimensions/    — 评分维度（从 score/ 移入）
├── memory/            — RAG、向量存储
├── llm/               — LLM 客户端、解析
├── guide/             — 市场日报
├── checkpoint/        — 检查点
├── data/
│   ├── mod.rs         — re-exports from akshare-rs
│   └── traits.rs      — MarketDataProvider trait
└── (无 tools/ 模块)

akshare-rs (data layer)
├── news_filter.rs     — 从 sa 迁入
├── tools/             — 从 sa 迁入
│   ├── news/
│   ├── market_data/
│   └── indicators/
└── ...existing code...
```

---

## Part 1: Code Migration to akshare-rs

### 1.1 Migrate `news_filter.rs` (1563 lines)

Functions to move:
- `within_date_window()` — date window filtering
- `news_search_dedup_key()` — dedup key generation
- `merge_ranked_news()` — merge and sort
- `normalized_news_date()` — date normalization (already re-exported)
- `build_dated_news_query()` — query builder
- All related tests

### 1.2 Migrate `tools/` module (1607 lines)

Files to move:
- `tools/news/fetch.rs` — news fetching
- `tools/market_data/stock_data.rs` — stock data fetching
- `tools/market_data/financial.rs` — financial data fetching
- `tools/indicators/compute.rs` — indicator computation
- `tools/summarize.rs` — data summarization
- All related tests

### 1.3 sa retains

- `data/mod.rs` — only re-exports from akshare-rs
- `data/traits.rs` — `MarketDataProvider` trait definition

---

## Part 2: Module Consolidation

### 2.1 Merge `score/` into `scoring/`

Current:
- `score/dimensions/` — scoring dimensions (technical.rs, llm_analysis.rs, etc.)
- `scoring/` — scoring system core (assessment, helpers, types)

Target:
```
scoring/
├── mod.rs
├── assessment/
│   └── core.rs
├── dimensions/          # moved from score/
│   ├── technical.rs
│   ├── llm_analysis.rs
│   └── ...
├── helpers/
│   └── calc.rs
└── types/
    ├── assessment/
    └── breakdown/
```

All references change from `score::dimensions::*` to `scoring::dimensions::*`.

---

## Part 3: Large File Splitting

Split by responsibility + data flow, each file to 200-400 lines:

| Original | Lines | Split Into |
|----------|-------|------------|
| `pick/scoring/mod.rs` | 1446 | `factors.rs`, `weights.rs`, `calc.rs`, `mod.rs` |
| `report/lifecycle/task_run.rs` | 1423 | `fetch.rs`, `analyze.rs`, `output.rs`, `mod.rs` |
| `pick/pipeline/mod.rs` | 1381 | `filter.rs`, `rank.rs`, `select.rs`, `mod.rs` |
| `pick/objective/mod.rs` | 1166 | `constraints.rs`, `optimize.rs`, `mod.rs` |
| `report/result/stages.rs` | 968 | `prepare.rs`, `execute.rs`, `finalize.rs`, `mod.rs` |
| `memory/core.rs` | 944 | `store.rs`, `retrieve.rs`, `index.rs`, `mod.rs` |
| `report/diagnosis/consistency.rs` | 914 | `check.rs`, `validate.rs`, `mod.rs` |
| `scoring/helpers/calc.rs` | 798 | `technical.rs`, `fundamental.rs`, `mod.rs` |
| `store_impls.rs` | 755 | `redis.rs`, `sqlite.rs`, `qdrant.rs`, `mod.rs` |

Splitting principles:
- Each file 200-400 lines
- Single responsibility
- Preserve existing pub interfaces

---

## Part 4: Testing Strategy

### 4.1 Define trait interface

```rust
// sa/src/data/traits.rs
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

### 4.2 Production code depends on trait

- `TradingToolbox` receives `Arc<dyn MarketDataProvider>`
- Analysis logic fetches data through trait

### 4.3 Tests use mock implementation

```rust
// sa/src/data/mock.rs
pub struct MockMarketProvider {
    pub news: Vec<NewsItem>,
    pub candles: Vec<CandlePoint>,
    pub quote: Option<QuoteSnapshot>,
    // ...
}
```

### 4.4 Integration tests

- Keep少量真实调用，标记 `#[ignore]`
- Manual trigger to verify akshare-rs integration

---

## Implementation Order

Execute simultaneously in one pass:

1. **Define `MarketDataProvider` trait** in sa
2. **Migrate `news_filter.rs`** to akshare-rs
3. **Migrate `tools/`** to akshare-rs
4. **Update sa** to use trait + mock
5. **Merge `score/` into `scoring/`**
6. **Split large files**
7. **Update all imports and references**
8. **Run tests, fix compilation errors**

---

## Success Criteria

- [ ] sa crate 无直接数据获取代码（只通过 trait 调用）
- [ ] `news_filter.rs` 和 `tools/` 完全迁移到 akshare-rs
- [ ] `score/` 模块删除，内容合并到 `scoring/`
- [ ] 所有大文件拆分到 200-400 行
- [ ] 所有测试通过（单元测试 + mock 测试）
- [ ] 编译通过，无 warning

---

## Risk Mitigation

- **Compilation errors**: 逐步修复，每步验证编译
- **Import breakage**: 使用 IDE 自动修复 + grep 验证
- **Test failures**: 先 mock，再修复逻辑
- **akshare-rs 接口不匹配**: 提前确认 akshare-rs 现有接口

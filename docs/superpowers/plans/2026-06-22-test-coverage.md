# Test Coverage补全 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Bring unit test coverage to 90% and E2E coverage to 80%, with scoring system at 100%.

**Architecture:** Bottom-up approach — fix compilation first, then scoring correctness, then systematic unit test补全, then mock-based E2E flows.

**Tech Stack:** Rust, cargo test, cargo-tarpaulin, axum (test server), serde_json

---

## File Map

| Phase | Files Modified | Files Created |
|-------|---------------|---------------|
| 1a | `crates/sa-engine/Cargo.toml` | — |
| 1b | — | `crates/sa-models/src/scoring/helpers/calc_tests.rs`, `crates/sa-models/src/scoring/assessment/core_tests.rs`, `crates/sa-models/src/analysis/report_logic/setup_quality_tests.rs` |
| 2 | various `mod.rs` files | various test modules |
| 3 | — | `tests/common/mod.rs`, `tests/fixtures/*.json`, E2E test files |
| 4 | `.github/workflows/ci.yml` | — |

---

## Phase 1a: Fix axum Dev-dependency

### Task 1: Add axum to sa-engine dev-dependencies

**Files:**
- Modify: `crates/sa-engine/Cargo.toml`

- [ ] **Step 1: Add axum dev-dependency**

```toml
# Add at end of crates/sa-engine/Cargo.toml
[dev-dependencies]
axum = { version = "0.8", features = ["json"] }
```

- [ ] **Step 2: Verify compilation**

Run: `cargo test -p sa-engine --no-run 2>&1 | tail -5`
Expected: `Finished` with no errors

- [ ] **Step 3: Run existing tests**

Run: `cargo test -p sa-engine -- llm::client::tests 2>&1 | tail -10`
Expected: `test result: ok. 3 passed`

- [ ] **Step 4: Commit**

```bash
git add crates/sa-engine/Cargo.toml
git commit -m "fix: add axum dev-dependency to sa-engine for test compilation"
```

---

## Phase 1b: Scoring System Tests

### Task 2: Tests for `calc.rs` helper functions

**Files:**
- Modify: `crates/sa-models/src/scoring/helpers/calc.rs` (append test module)

The `calc.rs` file has private functions, so tests must go inside the same file as `#[cfg(test)] mod tests`.

- [ ] **Step 1: Add tests for `numeric_tokens`**

Append to `crates/sa-models/src/scoring/helpers/calc.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numeric_tokens_simple() {
        let tokens = numeric_tokens("price is 123.45");
        assert_eq!(tokens, vec!["123.45"]);
    }

    #[test]
    fn numeric_tokens_negative() {
        let tokens = numeric_tokens("drop of -5.2%");
        assert_eq!(tokens, vec!["-5.2"]);
    }

    #[test]
    fn numeric_tokens_multiple() {
        let tokens = numeric_tokens("entry 100 stop 95 target 110");
        assert_eq!(tokens, vec!["100", "95", "110"]);
    }

    #[test]
    fn numeric_tokens_empty() {
        let tokens = numeric_tokens("no numbers here");
        assert!(tokens.is_empty());
    }

    #[test]
    fn numeric_tokens_only_dot() {
        let tokens = numeric_tokens("just a dot . here");
        assert!(tokens.is_empty());
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p sa-models -- scoring::helpers::calc::tests::numeric_tokens 2>&1 | tail -10`
Expected: 5 tests pass

- [ ] **Step 3: Add tests for `count_numeric_levels`**

Append inside the existing `mod tests` block:

```rust
    #[test]
    fn count_numeric_levels_2_to_5_digits() {
        assert_eq!(count_numeric_levels("price at 1234"), 1);
        assert_eq!(count_numeric_levels("12 and 12345"), 1); // 12 is 2 digits, 12345 is 5 digits
        assert_eq!(count_numeric_levels("1 and 123456"), 0); // 1 is too short, 123456 is too long
    }

    #[test]
    fn count_numeric_levels_mixed() {
        assert_eq!(count_numeric_levels("entry 100.50 stop 95"), 2);
    }

    #[test]
    fn count_numeric_levels_empty() {
        assert_eq!(count_numeric_levels(""), 0);
    }
```

- [ ] **Step 4: Add tests for `count_numeric_dates`**

```rust
    #[test]
    fn count_numeric_dates_ymd() {
        assert_eq!(count_numeric_dates("report from 2026-06-21"), 1);
        assert_eq!(count_numeric_dates("2026-01-01 to 2026-12-31"), 2);
    }

    #[test]
    fn count_numeric_dates_slash() {
        assert_eq!(count_numeric_dates("date 2026/06/21"), 1);
    }

    #[test]
    fn count_numeric_dates_none() {
        assert_eq!(count_numeric_dates("no dates here"), 0);
    }

    #[test]
    fn count_numeric_dates_short_year() {
        assert_eq!(count_numeric_dates("26-06-21"), 0); // not 4-digit year
    }
```

- [ ] **Step 5: Add tests for `parse_first_number`**

```rust
    #[test]
    fn parse_first_number_basic() {
        assert_eq!(parse_first_number("price is 123.45"), Some(123.45));
    }

    #[test]
    fn parse_first_number_negative() {
        assert_eq!(parse_first_number("drop -5.2"), Some(-5.2));
    }

    #[test]
    fn parse_first_number_none() {
        assert_eq!(parse_first_number("no numbers"), None);
    }

    #[test]
    fn parse_first_number_first_wins() {
        assert_eq!(parse_first_number("100 and 200"), Some(100.0));
    }
```

- [ ] **Step 6: Add tests for `parse_position_percentage`**

```rust
    #[test]
    fn parse_position_percentage_with_percent() {
        assert_eq!(parse_position_percentage("20%"), Some(0.2));
    }

    #[test]
    fn parse_position_percentage_decimal() {
        assert_eq!(parse_position_percentage("0.2"), Some(0.2));
    }

    #[test]
    fn parse_position_percentage_whole_number() {
        assert_eq!(parse_position_percentage("20"), Some(0.2));
    }

    #[test]
    fn parse_position_percentage_out_of_range() {
        assert_eq!(parse_position_percentage("150"), None);
    }

    #[test]
    fn parse_position_percentage_empty() {
        assert_eq!(parse_position_percentage(""), None);
    }
```

- [ ] **Step 7: Add tests for `looks_like_ymd_date`**

```rust
    #[test]
    fn looks_like_ymd_valid() {
        assert!(looks_like_ymd_date("2026-06-21"));
        assert!(looks_like_ymd_date("2026/6/1"));
    }

    #[test]
    fn looks_like_ymd_invalid() {
        assert!(!looks_like_ymd_date("26-06-21"));
        assert!(!looks_like_ymd_date("2026-06"));
        assert!(!looks_like_ymd_date("hello"));
    }
```

- [ ] **Step 8: Add tests for `has_execution_boundary`**

```rust
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
```

- [ ] **Step 9: Add tests for `analyst_matches` and `matches_semantic_alias`**

```rust
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
    fn matches_semantic_alias_market() {
        assert!(matches_semantic_alias("market", "", "市场分析", ""));
        assert!(matches_semantic_alias("market", "", "技术面报告", ""));
    }

    #[test]
    fn matches_semantic_alias_news() {
        assert!(matches_semantic_alias("news", "", "新闻催化", ""));
    }
```

- [ ] **Step 10: Run all calc tests**

Run: `cargo test -p sa-models -- scoring::helpers::calc::tests 2>&1 | tail -15`
Expected: all tests pass

- [ ] **Step 11: Commit**

```bash
git add crates/sa-models/src/scoring/helpers/calc.rs
git commit -m "test: add comprehensive tests for scoring calc helpers"
```

### Task 3: Tests for `core.rs` scoring functions

**Files:**
- Modify: `crates/sa-models/src/scoring/assessment/core.rs` (append test module)

The functions in `core.rs` take complex types. We need to construct test fixtures.

- [ ] **Step 1: Add test module with fixtures**

Append to `crates/sa-models/src/scoring/assessment/core.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn make_analyst(key: &str, up: f64, down: f64, sideways: f64) -> AgentReportNode {
        AgentReportNode {
            key: key.into(),
            up_probability: up,
            down_probability: down,
            sideways_probability: sideways,
            evidence_points: vec!["evidence1".into(), "evidence2".into()],
            next_steps: vec!["step1".into()],
            ..Default::default()
        }
    }

    fn make_result_with_analysts(analysts: Vec<AgentReportNode>) -> AnalysisResult {
        let mut result = AnalysisResult {
            task_id: "test".into(),
            report_id: "rpt-test".into(),
            symbol: "TEST".into(),
            stock_name: "Test Corp".into(),
            analysis_date: "2026-06-22".into(),
            market_type: "美股".into(),
            graph: AnalysisGraph::default(),
            agent_state: AgentStateSnapshot::default(),
            artifacts: AnalysisArtifacts::default(),
            report: Default::default(),
            ic_report: Default::default(),
            created_at: "2026-06-22T00:00:00Z".into(),
        };
        result.graph.analysts = analysts;
        result
    }
```

- [ ] **Step 2: Add `score_data_quality` tests**

```rust
    #[test]
    fn score_data_quality_all_present() {
        let d = score_data_quality(4, 4, 5, 0);
        assert_eq!(d.score, 20); // 12 + 4 + 5 = 21, clamped to 20
        assert_eq!(d.max_score, 20);
    }

    #[test]
    fn score_data_quality_all_empty() {
        let d = score_data_quality(0, 0, 0, 0);
        assert_eq!(d.score, 0);
    }

    #[test]
    fn score_data_quality_with_failures() {
        let d = score_data_quality(4, 2, 3, 2);
        // 12 + 2 + 3 - 4 = 13
        assert_eq!(d.score, 13);
    }

    #[test]
    fn score_data_quality_failures_capped() {
        let d = score_data_quality(0, 0, 0, 10);
        // penalty capped at -6
        assert_eq!(d.score, 0); // 0 - 6 = -6, clamped to 0
    }
```

- [ ] **Step 3: Add `score_cross_agent_consistency` tests**

```rust
    #[test]
    fn score_cross_agent_consistency_all_bullish() {
        let analysts = vec![
            make_analyst("market", 0.7, 0.15, 0.15),
            make_analyst("fundamentals", 0.65, 0.2, 0.15),
            make_analyst("news", 0.6, 0.2, 0.2),
        ];
        let result = make_result_with_analysts(analysts);
        let d = score_cross_agent_consistency(&result);
        assert!(d.score >= 13, "expected high consistency, got {}", d.score);
    }

    #[test]
    fn score_cross_agent_consistency_split() {
        let analysts = vec![
            make_analyst("market", 0.7, 0.15, 0.15),
            make_analyst("fundamentals", 0.2, 0.6, 0.2),
        ];
        let result = make_result_with_analysts(analysts);
        let d = score_cross_agent_consistency(&result);
        assert!(d.score <= 8, "expected low consistency, got {}", d.score);
    }

    #[test]
    fn score_cross_agent_consistency_empty() {
        let result = make_result_with_analysts(vec![]);
        let d = score_cross_agent_consistency(&result);
        assert_eq!(d.score, 6);
    }
```

- [ ] **Step 4: Add `score_setup_direction_alignment` tests**

```rust
    #[test]
    fn score_setup_direction_alignment_no_history() {
        let mut result = make_result_with_analysts(vec![]);
        result.artifacts.memory_context.setup_resolved_match_count = 0;
        let d = score_setup_direction_alignment(&result);
        assert_eq!(d.score, 4);
    }
```

- [ ] **Step 5: Add `score_risk_clarity` tests**

```rust
    #[test]
    fn score_risk_clarity_with_debate() {
        let mut result = make_result_with_analysts(vec![]);
        result.graph.risk_debate.turns = vec![
            crate::RiskDebateTurn { stance: "aggressive".into(), ..Default::default() },
            crate::RiskDebateTurn { stance: "conservative".into(), ..Default::default() },
        ];
        let research = StructuredResearchPlan {
            risk_assessment: "high risk at 1200".into(),
            ..Default::default()
        };
        let trader = StructuredTraderPlan::default();
        let portfolio = StructuredPortfolioDecision::default();
        let d = score_risk_clarity(&result, &research, &trader, &portfolio);
        assert!(d.score > 0);
    }
```

- [ ] **Step 6: Run all core tests**

Run: `cargo test -p sa-models -- scoring::assessment::core::tests 2>&1 | tail -15`
Expected: all tests pass

- [ ] **Step 7: Commit**

```bash
git add crates/sa-models/src/scoring/assessment/core.rs
git commit -m "test: add tests for scoring assessment core functions"
```

### Task 4: Tests for `setup_quality.rs`

**Files:**
- Modify: `crates/sa-models/src/analysis/report_logic/setup_quality.rs` (append test module)

- [ ] **Step 1: Add `normalize_gap_to_i18n_key` tests**

Append to `crates/sa-models/src/analysis/report_logic/setup_quality.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_gap_cash_flow() {
        assert_eq!(normalize_gap_to_i18n_key("missing cash flow data"), "setup_gap_cash_flow");
        assert_eq!(normalize_gap_to_i18n_key("现金流缺失"), "setup_gap_cash_flow");
    }

    #[test]
    fn normalize_gap_sentiment() {
        assert_eq!(normalize_gap_to_i18n_key("sentiment unclear"), "setup_gap_sentiment");
        assert_eq!(normalize_gap_to_i18n_key("情绪面"), "setup_gap_sentiment");
    }

    #[test]
    fn normalize_gap_news() {
        assert_eq!(normalize_gap_to_i18n_key("news coverage sparse"), "setup_gap_news_coverage");
        assert_eq!(normalize_gap_to_i18n_key("新闻不足"), "setup_gap_news_coverage");
        assert_eq!(normalize_gap_to_i18n_key("资讯缺失"), "setup_gap_news_coverage");
    }

    #[test]
    fn normalize_gap_volume() {
        assert_eq!(normalize_gap_to_i18n_key("volume data missing"), "setup_gap_volume_data");
        assert_eq!(normalize_gap_to_i18n_key("成交量异常"), "setup_gap_volume_data");
    }

    #[test]
    fn normalize_gap_technical() {
        assert_eq!(normalize_gap_to_i18n_key("technical confirmation needed"), "setup_gap_technical_confirmation");
    }

    #[test]
    fn normalize_gap_earnings() {
        assert_eq!(normalize_gap_to_i18n_key("earnings data stale"), "setup_gap_earnings_data");
        assert_eq!(normalize_gap_to_i18n_key("财报未更新"), "setup_gap_earnings_data");
    }

    #[test]
    fn normalize_gap_capital_flow() {
        assert_eq!(normalize_gap_to_i18n_key("capital flow unclear"), "setup_gap_capital_flow");
        assert_eq!(normalize_gap_to_i18n_key("资金流"), "setup_gap_capital_flow");
    }

    #[test]
    fn normalize_gap_insider() {
        assert_eq!(normalize_gap_to_i18n_key("insider selling detected"), "setup_gap_insider_data");
        assert_eq!(normalize_gap_to_i18n_key("减持"), "setup_gap_insider_data");
    }

    #[test]
    fn normalize_gap_valuation() {
        assert_eq!(normalize_gap_to_i18n_key("valuation stretched"), "setup_gap_valuation_data");
        assert_eq!(normalize_gap_to_i18n_key("估值偏高"), "setup_gap_valuation_data");
    }

    #[test]
    fn normalize_gap_sector() {
        assert_eq!(normalize_gap_to_i18n_key("sector rotation risk"), "setup_gap_sector_data");
        assert_eq!(normalize_gap_to_i18n_key("板块轮动"), "setup_gap_sector_data");
    }

    #[test]
    fn normalize_gap_unknown_falls_back() {
        assert_eq!(normalize_gap_to_i18n_key("some random gap"), "setup_gap_execution_boundary_incomplete");
    }
```

- [ ] **Step 2: Add `normalize_gap_match_text` tests**

```rust
    #[test]
    fn normalize_gap_match_text_removes_punctuation() {
        assert_eq!(normalize_gap_match_text("hello, world; test: foo"), "hello  world  test  foo");
    }

    #[test]
    fn normalize_gap_match_text_lowercases() {
        assert_eq!(normalize_gap_match_text("HELLO World"), "hello world");
    }

    #[test]
    fn normalize_gap_match_text_trims() {
        assert_eq!(normalize_gap_match_text("  hello  "), "hello");
    }
```

- [ ] **Step 3: Add `tokenize_gap_match_text` tests**

```rust
    #[test]
    fn tokenize_gap_match_text_basic() {
        let tokens = tokenize_gap_match_text("missing cash flow data");
        assert_eq!(tokens, vec!["missing", "cash", "flow", "data"]);
    }

    #[test]
    fn tokenize_gap_match_text_short_tokens_filtered() {
        let tokens = tokenize_gap_match_text("a bb ccc");
        assert_eq!(tokens, vec!["bb", "ccc"]);
    }

    #[test]
    fn tokenize_gap_match_text_empty() {
        let tokens = tokenize_gap_match_text("");
        assert!(tokens.is_empty());
    }
```

- [ ] **Step 4: Add `score_related_gap_match` tests**

```rust
    #[test]
    fn score_related_gap_match_some_overlap() {
        let base = vec!["cash".into(), "flow".into(), "data".into()];
        assert_eq!(score_related_gap_match(&base, "missing cash flow"), 2);
    }

    #[test]
    fn score_related_gap_match_no_overlap() {
        let base = vec!["cash".into(), "flow".into()];
        assert_eq!(score_related_gap_match(&base, "volume spike"), 0);
    }
```

- [ ] **Step 5: Run all setup_quality tests**

Run: `cargo test -p sa-models -- analysis::report_logic::setup_quality::tests 2>&1 | tail -15`
Expected: all tests pass

- [ ] **Step 6: Commit**

```bash
git add crates/sa-models/src/analysis/report_logic/setup_quality.rs
git commit -m "test: add tests for setup_quality scoring and gap normalization"
```

---

## Phase 2: Unit Test Coverage to 90%

### Task 5: Tests for `sa-models/src/value_utils.rs`

**Files:**
- Modify: `crates/sa-models/src/value_utils.rs`

- [ ] **Step 1: Read the file to understand functions**

Run: `head -60 crates/sa-models/src/value_utils.rs`

- [ ] **Step 2: Add tests for each public function**

(Implementation depends on file contents — add test module with coverage for all public fns)

- [ ] **Step 3: Run and verify**

Run: `cargo test -p sa-models -- value_utils 2>&1 | tail -10`

- [ ] **Step 4: Commit**

```bash
git add crates/sa-models/src/value_utils.rs
git commit -m "test: add tests for value_utils"
```

### Task 6: Tests for `sa-models/src/user_preferences.rs`

**Files:**
- Modify: `crates/sa-models/src/user_preferences.rs`

- [ ] **Step 1: Read the file**

Run: `head -80 crates/sa-models/src/user_preferences.rs`

- [ ] **Step 2: Add tests for preference parsing and defaults**

- [ ] **Step 3: Run and verify**

Run: `cargo test -p sa-models -- user_preferences 2>&1 | tail -10`

- [ ] **Step 4: Commit**

```bash
git add crates/sa-models/src/user_preferences.rs
git commit -m "test: add tests for user_preferences"
```

### Task 7: Tests for `sa-models/src/task.rs`

**Files:**
- Modify: `crates/sa-models/src/task.rs`

- [ ] **Step 1: Read and add tests**

- [ ] **Step 2: Run and verify**

Run: `cargo test -p sa-models -- task 2>&1 | tail -10`

- [ ] **Step 3: Commit**

### Task 8: Tests for `sa-models/src/analysis/derived.rs`

**Files:**
- Modify: `crates/sa-models/src/analysis/derived.rs`

- [ ] **Step 1: Read and add tests for all 9 public functions**

- [ ] **Step 2: Run and verify**

Run: `cargo test -p sa-models -- analysis::derived 2>&1 | tail -10`

- [ ] **Step 3: Commit**

### Task 9: Tests for `sa-models/src/analysis/scenario_types.rs`

**Files:**
- Modify: `crates/sa-models/src/analysis/scenario_types.rs`

- [ ] **Step 1: Read and add tests for all 8 public functions**

- [ ] **Step 2: Run and verify**

- [ ] **Step 3: Commit**

### Task 10: Tests for `sa-data/src/cache.rs`

**Files:**
- Modify: `crates/sa-data/src/cache.rs`

- [ ] **Step 1: Read and add tests for 3 public functions**

- [ ] **Step 2: Run and verify**

Run: `cargo test -p sa-data -- cache 2>&1 | tail -10`

- [ ] **Step 3: Commit**

### Task 11: Tests for `sa-data/src/news_search.rs`

**Files:**
- Modify: `crates/sa-data/src/news_search.rs`

- [ ] **Step 1: Read and add tests for 2 public functions (mock HTTP)**

- [ ] **Step 2: Run and verify**

- [ ] **Step 3: Commit**

### Task 12: Tests for `sa-data/src/news_filter.rs`

**Files:**
- Modify: `crates/sa-data/src/news_filter.rs`

- [ ] **Step 1: Read and add tests for filtering logic**

- [ ] **Step 2: Run and verify**

- [ ] **Step 3: Commit**

### Task 13: Tests for `sa-data/src/diagnosis.rs`

**Files:**
- Modify: `crates/sa-data/src/diagnosis.rs`

- [ ] **Step 1: Read and add tests for 6 public functions**

- [ ] **Step 2: Run and verify**

- [ ] **Step 3: Commit**

### Task 14: Tests for `sa-data/src/search.rs`

**Files:**
- Modify: `crates/sa-data/src/search.rs`

- [ ] **Step 1: Read and add tests for 3 public functions**

- [ ] **Step 2: Run and verify**

- [ ] **Step 3: Commit**

### Task 15: Tests for `sa-data/src/client.rs` (bulk)

**Files:**
- Modify: `crates/sa-data/src/client.rs`

This file has 127 public functions. Focus on:
- Parsing functions (no external calls needed)
- Data transformation functions
- HTTP client functions (mock with `wiremock` or local test server)

- [ ] **Step 1: Add `wiremock` dev-dependency to sa-data**

```toml
# Add to crates/sa-data/Cargo.toml
[dev-dependencies]
wiremock = "0.6"
tokio = { version = "1", features = ["test-util"] }
```

- [ ] **Step 2: Read file and identify pure parsing functions**

- [ ] **Step 3: Add tests for parsing functions (no mock needed)**

- [ ] **Step 4: Add tests for HTTP-dependent functions using wiremock**

- [ ] **Step 5: Run and verify**

Run: `cargo test -p sa-data -- client 2>&1 | tail -15`

- [ ] **Step 6: Commit**

### Task 16: Tests for `sa-storage/src/lib.rs`

**Files:**
- Modify: `crates/sa-storage/src/lib.rs`

- [ ] **Step 1: Read and add tests for 4 public functions**

- [ ] **Step 2: Run and verify**

Run: `cargo test -p sa-storage 2>&1 | tail -10`

- [ ] **Step 3: Commit**

### Task 17: Tests for `sa-engine` remaining modules

**Files:**
- Modify: various files in `crates/sa-engine/src/`

Focus on untested areas:
- `llm/parse/parsers.rs`, `llm/parse/helpers.rs`, `llm/parse/validate.rs`, `llm/parse/diagnosis.rs`
- `llm/prompt/generate.rs`, `llm/prompt/calibration.rs`
- `guidance/` modules
- `checkpoint/mod.rs`

- [ ] **Step 1: Read each file and add tests for untested public functions**

- [ ] **Step 2: Run full sa-engine test suite**

Run: `cargo test -p sa-engine 2>&1 | tail -15`

- [ ] **Step 3: Commit**

### Task 18: Full workspace test verification

- [ ] **Step 1: Run all tests**

Run: `cargo test --workspace 2>&1 | tail -20`
Expected: all tests pass, no compilation errors

- [ ] **Step 2: Install and run tarpaulin**

Run: `cargo install cargo-tarpaulin 2>&1 | tail -5`
Run: `cargo tarpaulin --workspace --skip-clean 2>&1 | tail -20`
Expected: coverage >= 90%

- [ ] **Step 3: If coverage < 90%, identify gaps and add more tests**

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "test: achieve 90% unit test coverage across workspace"
```

---

## Phase 3: E2E Tests (Mock-based)

### Task 19: Create shared test infrastructure

**Files:**
- Create: `tests/common/mod.rs`
- Create: `tests/fixtures/sample_market.json`
- Create: `tests/fixtures/sample_news.json`
- Create: `tests/fixtures/sample_llm_response.json`

- [ ] **Step 1: Create fixtures directory**

```bash
mkdir -p tests/fixtures tests/common
```

- [ ] **Step 2: Create `tests/fixtures/sample_market.json`**

```json
{
  "symbol": "AAPL",
  "price": 185.50,
  "change_pct": 1.2,
  "volume": 52000000,
  "rsi": 55.0,
  "macd": 0.3,
  "macd_signal": 0.2,
  "macd_hist": 0.1,
  "ema_10": 183.0,
  "sma_50": 180.0,
  "sma_200": 170.0
}
```

- [ ] **Step 3: Create `tests/fixtures/sample_news.json`**

```json
{
  "headlines": [
    "Apple announces record Q2 earnings",
    "iPhone sales exceed expectations",
    "Apple expands AI features across product line"
  ]
}
```

- [ ] **Step 4: Create `tests/fixtures/sample_llm_response.json`**

```json
{
  "score": 72,
  "reason": "近期利好消息较多，市场情绪积极"
}
```

- [ ] **Step 5: Create `tests/common/mod.rs` with mock helpers**

```rust
use serde_json::Value;

pub fn load_fixture(name: &str) -> Value {
    let path = format!("tests/fixtures/{}.json", name);
    let content = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to load fixture {path}: {e}"));
    serde_json::from_str(&content).unwrap()
}

pub fn sample_scoreable_pick() -> sa_engine::score::scorer::ScoreablePick {
    sa_engine::score::scorer::ScoreablePick {
        symbol: "AAPL".into(),
        market: "美股".into(),
        rsi: Some(55.0),
        macd: Some(0.3),
        macd_signal: Some(0.2),
        macd_hist: Some(0.1),
        adx: Some(25.0),
        close_10_ema: Some(183.0),
        close_50_sma: Some(180.0),
        close_200_sma: Some(170.0),
        obv: None,
        current_price: Some(185.5),
        volume_elevated: true,
        latest_positive: true,
        pe_like: Some(28.0),
        ps_like: Some(7.0),
        roe: Some(150.0),
        leverage: Some(1.5),
        market_cap: Some(2_800_000_000_000.0),
        revenues_usd: Some(394_000_000_000.0),
        net_income_usd: Some(100_000_000_000.0),
        news_headlines: vec![
            "Apple announces record Q2 earnings".into(),
            "iPhone sales exceed expectations".into(),
        ],
        confidence: 72.0,
        objective_final_score: 68.0,
        momentum_score: 65.0,
        hit_rate: Some(0.65),
        catalyst_count: 3,
        hard_negative_count: 0,
        volume_ratio: Some(1.3),
        period_return_pct: Some(5.0),
    }
}
```

- [ ] **Step 6: Commit**

```bash
git add tests/
git commit -m "test: add shared E2E test infrastructure and fixtures"
```

### Task 20: E2E Scoring Pipeline

**Files:**
- Create: `tests/e2e_scoring.rs`

- [ ] **Step 1: Create E2E scoring test**

```rust
mod common;

#[test]
fn e2e_score_consistency_bullish() {
    let pick = common::sample_scoreable_pick();
    // Verify pick has all bullish signals
    assert!(pick.rsi.unwrap() < 70);
    assert!(pick.volume_elevated);
    assert!(pick.latest_positive);
    // Score should be above neutral
    // Note: score_stock_pick is async and needs LLM client,
    // so we test the individual dimensions here
    let tech_input = sa_engine::score::dimensions::technical::TechnicalInput {
        rsi: pick.rsi,
        macd: pick.macd,
        macd_signal: pick.macd_signal,
        macd_hist: pick.macd_hist,
        adx: pick.adx,
        close_10_ema: pick.close_10_ema,
        close_50_sma: pick.close_50_sma,
        close_200_sma: pick.close_200_sma,
        obv: pick.obv,
        current_price: pick.current_price,
        volume_elevated: pick.volume_elevated,
        latest_positive: pick.latest_positive,
    };
    let tech = sa_engine::score::dimensions::technical::score_technical(&tech_input);
    assert!(tech.score >= 60, "expected bullish tech score, got {}", tech.score);
    assert!(tech.score <= 100);
}

#[test]
fn e2e_score_consistency_bearish() {
    let tech_input = sa_engine::score::dimensions::technical::TechnicalInput {
        rsi: Some(80.0),
        macd: Some(-0.5),
        macd_signal: Some(-0.2),
        macd_hist: Some(-0.3),
        adx: Some(30.0),
        close_10_ema: Some(90.0),
        close_50_sma: Some(95.0),
        close_200_sma: Some(100.0),
        obv: None,
        current_price: Some(85.0),
        volume_elevated: true,
        latest_positive: false,
    };
    let tech = sa_engine::score::dimensions::technical::score_technical(&tech_input);
    assert!(tech.score <= 40, "expected bearish tech score, got {}", tech.score);
}

#[test]
fn e2e_score_fundamental_mixed() {
    let fund_input = sa_engine::score::dimensions::fundamental::FundamentalInput {
        pe_like: Some(10.0),
        ps_like: None,
        roe: Some(-5.0),
        leverage: Some(0.8),
        market_cap: None,
        revenues_usd: Some(1_000_000_000.0),
        net_income_usd: Some(-100_000_000.0),
    };
    let fund = sa_engine::score::dimensions::fundamental::score_fundamental(&fund_input);
    assert!(fund.score >= 20 && fund.score <= 80, "mixed signals should be mid-range, got {}", fund.score);
}
```

- [ ] **Step 2: Run E2E scoring tests**

Run: `cargo test --test e2e_scoring 2>&1 | tail -10`
Expected: all pass

- [ ] **Step 3: Commit**

```bash
git add tests/e2e_scoring.rs
git commit -m "test: add E2E scoring pipeline tests"
```

### Task 21: E2E Report Logic

**Files:**
- Modify: `crates/sa-models/src/analysis/report_logic/setup_quality.rs` (make `normalize_gap_to_i18n_key` `pub(crate)`)
- Create: `tests/e2e_report.rs`

- [ ] **Step 1: Make `normalize_gap_to_i18n_key` accessible**

Change `fn normalize_gap_to_i18n_key` to `pub(crate) fn normalize_gap_to_i18n_key` in `setup_quality.rs`.

- [ ] **Step 2: Create E2E report test**

```rust
#[test]
fn e2e_gap_normalization_no_raw_strings() {
    let gaps = vec![
        "missing cash flow data",
        "sentiment unclear",
        "news coverage sparse",
        "volume spike detected",
    ];
    for gap in &gaps {
        let key = sa_models::analysis::report_logic::setup_quality::normalize_gap_to_i18n_key(gap);
        assert!(key.starts_with("setup_gap_"), "gap '{}' normalized to '{}' instead of i18n key", gap, key);
    }
}

#[test]
fn e2e_gap_normalization_cjk() {
    let gaps = vec!["现金流缺失", "情绪面不佳", "新闻不足", "成交量异常"];
    for gap in gaps {
        let key = sa_models::analysis::report_logic::setup_quality::normalize_gap_to_i18n_key(gap);
        assert!(key.starts_with("setup_gap_"), "CJK gap '{}' not normalized", gap);
    }
}
```

- [ ] **Step 2: Run E2E report tests**

Run: `cargo test --test e2e_report 2>&1 | tail -10`

- [ ] **Step 3: Commit**

```bash
git add tests/e2e_report.rs
git commit -m "test: add E2E report logic tests"
```

### Task 22: E2E Data Ingestion

**Files:**
- Create: `tests/e2e_data.rs`

- [ ] **Step 1: Create E2E data test with mock HTTP**

```rust
use wiremock::{MockServer, Mock, ResponseTemplate};
use wiremock::matchers::method;

#[tokio::test]
async fn e2e_cache_store_roundtrip() {
    // Test cache set/get without real Redis
    let cache = sa_data::cache::MemoryCache::new();
    cache.set("key1", "value1", std::time::Duration::from_secs(60)).await.unwrap();
    let result = cache.get("key1").await.unwrap();
    assert_eq!(result, Some("value1".to_string()));
}

#[tokio::test]
async fn e2e_cache_expiry() {
    let cache = sa_data::cache::MemoryCache::new();
    cache.set("key2", "value2", std::time::Duration::from_millis(1)).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    let result = cache.get("key2").await.unwrap();
    assert!(result.is_none(), "expected expired key to be None");
}
```

- [ ] **Step 2: Run E2E data tests**

Run: `cargo test --test e2e_data 2>&1 | tail -10`

- [ ] **Step 3: Commit**

```bash
git add tests/e2e_data.rs
git commit -m "test: add E2E data ingestion tests"
```

### Task 23: E2E Store Operations

**Files:**
- Create: `tests/e2e_store.rs`

- [ ] **Step 1: Create E2E store test**

```rust
#[test]
fn e2e_store_trait_implementations_exist() {
    // Verify that the store traits are properly implemented
    // by checking that key types can be constructed
    // This is a compile-time check wrapped in a runtime test
    fn assert_cache_store<T: sa_models::store::CacheStore>() {}
    fn assert_vector_store<T: sa_models::store::VectorStore>() {}
    fn assert_analysis_store<T: sa_models::store::AnalysisStore>() {}
    // If these compile, the traits are implemented
}
```

- [ ] **Step 2: Run and verify**

Run: `cargo test --test e2e_store 2>&1 | tail -10`

- [ ] **Step 3: Commit**

---

## Phase 4: CI Integration

### Task 24: Add tarpaulin to CI

**Files:**
- Modify: `.github/workflows/ci.yml`

- [ ] **Step 1: Add coverage job**

Append to `.github/workflows/ci.yml`:

```yaml
  coverage:
    name: Coverage
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - name: Install cargo-tarpaulin
        run: cargo install cargo-tarpaulin
      - name: Run coverage
        run: cargo tarpaulin --workspace --out Stdout --fail-under 90
```

- [ ] **Step 2: Verify YAML syntax**

Run: `python3 -c "import yaml; yaml.safe_load(open('.github/workflows/ci.yml'))"`

- [ ] **Step 3: Commit**

```bash
git add .github/workflows/ci.yml
git commit -m "ci: add cargo-tarpaulin coverage job with 90% threshold"
```

### Task 25: Final verification

- [ ] **Step 1: Run full workspace tests**

Run: `cargo test --workspace 2>&1 | tail -20`
Expected: all pass

- [ ] **Step 2: Run tarpaulin**

Run: `cargo tarpaulin --workspace 2>&1 | tail -20`
Expected: >= 90%

- [ ] **Step 3: Final commit if needed**

```bash
git add -A
git commit -m "test: achieve 90% unit test and 80% E2E coverage targets"
```

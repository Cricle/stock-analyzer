# Test Coverage补全 Design

**Date**: 2026-06-21
**Goal**: Unit test coverage → 90%, E2E coverage → 80%, scoring system 100% tested

## Current State

- 118 `#[test]` functions across 19 files
- 394 public functions across 81 files (~29% coverage)
- `sa-engine` tests blocked by missing `axum` dev-dependency
- Zero E2E tests
- Scoring system: `sa-engine/src/score/` well-tested, `sa-models/src/scoring/` largely untested

## Approach: Bottom-up

### Phase 1: Fix Compilation & Scoring Tests

**1a. Fix axum dev-dependency**

Add `axum` to `[dev-dependencies]` in `crates/sa-engine/Cargo.toml`. Verify 3 existing LLM client tests pass.

**1b. Scoring system test补全**

| File | Functions to test | Tests to add |
|------|------------------|--------------|
| `sa-models/src/scoring/helpers/calc.rs` | `numeric_tokens`, `count_numeric_levels`, `count_numeric_dates`, `parse_first_number`, `parse_position_percentage`, `looks_like_ymd_date`, `has_execution_boundary`, `analyst_probability_quality`, `select_analyst`, `analyst_matches`, `matches_semantic_alias` | ~20 |
| `sa-models/src/scoring/assessment/core.rs` | `score_data_quality`, `score_trend_confirmation`, `score_fundamentals`, `score_catalyst_quality`, `score_historical_transferability`, `score_setup_direction_alignment`, `score_cross_agent_consistency`, `score_risk_clarity` | ~25 |
| `sa-models/src/analysis/report_logic/setup_quality.rs` | `normalize_gap_to_i18n_key`, `derive_trade_setup_quality`, `collect_execution_blocking_gaps`, `normalize_gap_match_text`, `tokenize_gap_match_text`, `score_related_gap_match`, `related_gap_items`, `enrich_diagnostic_linkage`, `scenario_gap_messages`, `append_scenario_gap_narrative` | ~30 |

### Phase 2: Unit Test Coverage to 90%

~250 additional tests across all crates.

**sa-models** (~80 tests):
- `analysis/report_logic/` — report generation, diagnostics, trader plan logic
- `analysis/derived.rs` — derived calculations
- `analysis/scenario_types.rs` — scenario parsing
- `user_preferences.rs` — preference handling
- `value_utils.rs` — value utilities

**sa-data** (~120 tests):
- `client.rs` — 127 public fns, mock HTTP responses for AKShare API
- `news_search.rs` — news fetching logic
- `news_filter.rs` — filtering logic
- `cache.rs` — cache operations
- `diagnosis.rs` — diagnostic logic
- `search.rs` — search functionality

**sa-engine** (~40 tests):
- `llm/parse/` — JSON parsing, validation, diagnosis
- `llm/prompt/` — prompt generation
- `guidance/` — guidance store, embedding
- `checkpoint/` — checkpoint operations

**sa-storage** (~4 tests):
- All 4 public functions in `lib.rs`

### Phase 3: E2E Tests (Mock-based)

Create `tests/` integration test directories. Mock all external services (AKShare, LLM, Redis, Qdrant).

**E2E Flow 1: Full Analysis Pipeline** (`sa-engine/tests/e2e_analysis.rs`)
- Mock AKShare → sample market data
- Mock LLM → structured JSON responses
- Mock Qdrant → sample embeddings
- Validate: task creation → analysis run → scoring → report generation

**E2E Flow 2: Scoring Pipeline** (`sa-engine/tests/e2e_scoring.rs`)
- Feed known market data → verify score consistency
- Edge cases: all-bearish, all-bullish, mixed signals, missing data

**E2E Flow 3: Data Ingestion** (`sa-data/tests/e2e_data.rs`)
- Mock HTTP responses for AKShare endpoints
- Quote fetch → parse → cache → retrieve
- News fetch → filter → deduplicate

**E2E Flow 4: Report Logic** (`sa-models/tests/e2e_report.rs`)
- Construct sample `AnalysisResult` → generate full report
- Setup quality scoring, diagnostics enrichment, gap normalization

**E2E Flow 5: Store Operations** (`sa-storage/tests/e2e_store.rs`)
- Mock Redis/Qdrant backends
- CacheStore CRUD, VectorStore insert/search, AnalysisStore task lifecycle

**Mock infrastructure** (shared):
- `tests/common/mod.rs` — mock LLM client, mock HTTP server
- Sample JSON fixtures in `tests/fixtures/`

### Phase 4: CI Integration

**CI changes** (`.github/workflows/ci.yml`):
- Add `cargo install cargo-tarpaulin` step
- Run `cargo tarpaulin --workspace --out Stdout` after tests
- Fail CI if coverage drops below threshold

**Coverage targets**:
- Unit tests: 90% line coverage
- E2E: 80% of critical flows covered

## Estimated Effort

| Phase | Tests | Priority |
|-------|-------|----------|
| Phase 1: Fix + Scoring | ~75 | Highest — correctness |
| Phase 2: Unit tests | ~250 | High — coverage |
| Phase 3: E2E | ~20 cases | Medium — integration |
| Phase 4: CI | config | Low — automation |

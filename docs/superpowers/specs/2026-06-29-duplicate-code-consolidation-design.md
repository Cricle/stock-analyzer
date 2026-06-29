# Duplicate Code Consolidation Design

## Goal

Eliminate 4 duplicate code patterns found via `duplicate_code` scan, reducing code size and improving maintainability.

## Patterns

### 1. `normalize_inline_value` duplication

**Files:**
- `src/llm/parse/helpers.rs:49-68` — uses `"; "` separator
- `src/value_utils.rs:75-93` — uses `", "` separator

**Approach:** Parameterize separator in `value_utils.rs`, delete local copy in `helpers.rs`.

- Add `pub fn normalize_inline_value_with_sep(value: &Value, sep: &str) -> String` to `value_utils.rs`
- Keep existing `normalize_inline_value` in `value_utils.rs` calling the new function with `", "`
- Delete `normalize_inline_value` from `llm/parse/helpers.rs`
- In `helpers.rs`, replace calls with `crate::value_utils::normalize_inline_value_with_sep(value, "; ")`

### 2. `CandidateEvidenceRecord` / `StockPickEvidencePayload` duplication

**Files:**
- `src/pick/types.rs:37-49` — `CandidateEvidenceRecord`
- `src/pick/history/mod.rs:22-38` — `StockPickEvidencePayload`

**Approach:** Delete `StockPickEvidencePayload`, rename all 6 references to `CandidateEvidenceRecord`.

Files to update:
- `src/pick/history/mod.rs` — delete struct, change field type references
- `src/pick/mod.rs` — 1 reference
- `src/pick/pipeline/mod.rs` — 2 references

### 3. `missing_evidence_ladder` extraction duplication

**Files:**
- `src/llm/generated/debate.rs:89-102`
- `src/llm/generated/portfolio/helpers.rs:227-240`

**Approach:** Add `from_evidence_field` method to `GeneratedMissingEvidenceLadder` in `helpers.rs`.

```rust
pub(crate) fn from_evidence_field(
    field: impl Fn(&str) -> Option<Value>,
    risk_raw: Option<&Value>,
) -> Self {
    Self::from_value(
        meaningful_value(field("missing_evidence_ladder")).or_else(|| {
            extract_object_value(
                risk_raw,
                &[
                    "missing_evidence_ladder",
                    "missing_evidence",
                    "missing_evidence_classification",
                    "missing_evidence_severity_ladder",
                ],
            )
        }),
    )
}
```

Both call sites replace 14 lines with one call.

### 4. Test `AnalysisResult` construction

**Files:**
- `src/analysis/report_logic/trader_plan/tests/news_diagnostics.rs:4-12`
- `src/analysis/report_logic/trader_plan/tests/setup_news.rs:200-212`
- Other test files with similar patterns

**Approach:** Add `test_analysis_result()` to `tests/common/mod.rs` with all fields explicit (no `Default::default()`).

```rust
pub fn test_analysis_result() -> sa::AnalysisResult {
    sa::AnalysisResult {
        task_id: "task-1".to_string(),
        report_id: "report-1".to_string(),
        symbol: "TEST".to_string(),
        stock_name: "test".to_string(),
        analysis_date: "2026-05-21".to_string(),
        market_type: "US".to_string(),
        graph: Default::default(),
        agent_state: Default::default(),
        artifacts: Default::default(),
        report: Default::default(),
        ic_report: Default::default(),
        created_at: "2026-05-21T00:00:00Z".to_string(),
    }
}
```

Test files replace 10-line blocks with `test_analysis_result()`.

## Verification

1. `cargo clippy` — 0 warnings
2. `cargo test` — all existing tests pass
3. `duplicate_code --minimum-successive-lines 8` — the 4 patterns no longer appear

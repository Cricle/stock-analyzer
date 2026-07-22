# Directional Execution Boundary Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make one direction-aware execution boundary the canonical source for report trade levels and block active execution when the real setup cannot meet a 2.0 reward/risk requirement.

**Architecture:** Add a typed execution boundary to the report contract and construct it from numeric execution fields, current price, ATR, market availability, and direction. Synchronize legacy report consumers from that boundary, preserving compatibility while removing conflicting peer sources. The frontend renders typed state and prerequisites through its existing localization layer.

**Tech Stack:** Rust, Serde, existing report builder and consistency validator, TypeScript, Vitest, Cargo test.

---

### Task 1: Define The Typed Contract

**Files:**

- Modify: `src/analysis/report_types/decision.rs`
- Modify: `src/analysis/report_types/report_core.rs`
- Modify: `frontend/src/types/analysis.ts`
- Test: `tests/analysis_report_types_execution_boundary.rs`

- [ ] **Step 1: Write the failing serialization and defaults test**

```rust
#[test]
fn execution_boundary_serializes_typed_prerequisites_and_stages() {
    let boundary = ExecutionBoundary {
        direction: DecisionViewDirection::Bearish,
        confirmation_price: Some(44.50),
        entry_price: Some(44.28),
        stop_price: Some(48.71),
        stage_one_target: Some(38.90),
        final_target: Some(36.08),
        minimum_reward_risk: 2.0,
        actual_reward_risk: Some(2.0),
        active_execution_allowed: false,
        prerequisites: vec![ExecutionPrerequisite::BorrowQuantity],
        ..Default::default()
    };

    let value = serde_json::to_value(boundary).unwrap();
    assert_eq!(value["direction"], "bearish");
    assert_eq!(value["prerequisites"][0], "borrow_quantity");
}
```

- [ ] **Step 2: Run the focused test and verify it fails**

Run: `rtk cargo test --test analysis_report_types_execution_boundary execution_boundary_serializes_typed_prerequisites_and_stages`

Expected: FAIL because `ExecutionBoundary` and `ExecutionPrerequisite` do not exist.

- [ ] **Step 3: Add the contract**

```rust
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ExecutionBoundary {
    pub direction: DecisionViewDirection,
    pub confirmation_price: Option<f64>,
    pub entry_price: Option<f64>,
    pub stop_price: Option<f64>,
    pub stage_one_target: Option<f64>,
    pub final_target: Option<f64>,
    pub minimum_reward_risk: f64,
    pub actual_reward_risk: Option<f64>,
    pub active_execution_allowed: bool,
    pub confirmation_mode: ConfirmationMode,
    pub prerequisites: Vec<ExecutionPrerequisite>,
    pub cash_flow_substitute: CashFlowSubstituteEvidence,
}
```

Define `ConfirmationMode`, `ExecutionPrerequisite`, and `CashFlowSubstituteEvidence` with snake-case Serde names. Add `execution_boundary: ExecutionBoundary` to `StructuredReport`; mirror exact optional interfaces and enum unions in `frontend/src/types/analysis.ts`.

- [ ] **Step 4: Run the focused test and verify it passes**

Run: `rtk cargo test --test analysis_report_types_execution_boundary execution_boundary_serializes_typed_prerequisites_and_stages`

Expected: PASS.

- [ ] **Step 5: Commit the contract**

```bash
rtk git add src/analysis/report_types/decision.rs src/analysis/report_types/report_core.rs frontend/src/types/analysis.ts tests/analysis_report_types_execution_boundary.rs
rtk git commit -m "feat: add typed execution boundary contract"
```

### Task 2: Normalize Directional Levels Before Any View Is Rendered

**Files:**

- Create: `src/analysis/report_logic/execution_boundary.rs`
- Modify: `src/analysis/report_logic/core/report_builder.rs`
- Modify: `src/analysis/report_logic/mod.rs`
- Test: `tests/execution_boundary_test.rs`

- [ ] **Step 1: Write failing bearish and bullish invariant tests**

```rust
#[test]
fn bearish_equal_entry_and_confirmation_gets_a_buffered_entry() {
    let boundary = normalize_execution_boundary(input(DecisionViewDirection::Bearish, 50.25, 2.8071, 48.71, 48.71, 56.90, 38.90));
    assert!(boundary.entry_price.unwrap() < boundary.confirmation_price.unwrap());
    assert!(boundary.confirmation_price.unwrap() < boundary.stop_price.unwrap());
}

#[test]
fn bearish_confirmation_distance_is_nonzero_below_current_price() {
    let boundary = normalize_execution_boundary(input(DecisionViewDirection::Bearish, 50.25, 2.8071, 48.71, 48.71, 56.90, 38.90));
    assert!(boundary.confirmation_distance_pct.unwrap() > 3.0);
}
```

- [ ] **Step 2: Run the focused tests and verify they fail**

Run: `rtk cargo test --test execution_boundary_test bearish_`

Expected: FAIL because the normalizer does not exist.

- [ ] **Step 3: Implement numeric-only normalization**

```rust
let raw_buffer = atr * 0.1;
let buffer = raw_buffer.clamp(confirmation * 0.0025, confirmation * 0.005);
let entry = match direction {
    DecisionViewDirection::Bearish => confirmation - buffer,
    DecisionViewDirection::Bullish => confirmation + buffer,
    DecisionViewDirection::Neutral => raw_entry,
};
```

Use direction to require `final_target <= stage_one_target < entry < confirmation < stop` for bearish and the inverse for bullish. Reject invalid or incomplete structures by setting `active_execution_allowed` to false. Calculate confirmation distance as the absolute current-price delta divided by current price for both directions. Do not inspect LLM prose or localized strings.

Call the normalizer in `StructuredReport::from_result` before decision view, probability, action guides, IC discipline, and summary are built. Synchronize `trader_plan`, `portfolio_decision`, and `decision_view` from the normalized boundary after construction.

- [ ] **Step 4: Run focused tests and verify they pass**

Run: `rtk cargo test --test execution_boundary_test bearish_ bullish_`

Expected: PASS.

- [ ] **Step 5: Commit the normalizer**

```bash
rtk git add src/analysis/report_logic/execution_boundary.rs src/analysis/report_logic/mod.rs src/analysis/report_logic/core/report_builder.rs tests/execution_boundary_test.rs
rtk git commit -m "feat: normalize directional execution boundaries"
```

### Task 3: Preserve Real Targets And Enforce The Active-Execution Threshold

**Files:**

- Modify: `src/report/diagnosis/consistency/check.rs`
- Modify: `src/report/diagnosis/consistency/validate.rs`
- Modify: `src/analysis/report_logic/probability.rs`
- Test: `tests/consistency_check_test.rs`
- Test: `tests/consistency_validate_test.rs`

- [ ] **Step 1: Replace target-widening expectations with failing preservation tests**

```rust
#[test]
fn low_reward_risk_preserves_target_and_blocks_active_execution() {
    let mut result = bearish_result(entry: 48.47, stop: 56.90, target: 38.90);
    let issues = ConsistencyValidator::validate_and_fix(&mut result);
    assert_eq!(result.report.trader_plan.target_reference, "38.90");
    assert!(!result.report.execution_boundary.active_execution_allowed);
    assert!(issues.iter().any(|issue| issue.check_name == "block_low_reward_risk"));
}
```

- [ ] **Step 2: Run the focused tests and verify they fail**

Run: `rtk cargo test --test consistency_check_test low_reward_risk -- --nocapture`

Expected: FAIL because the current validator changes the target to a 1.5 ratio.

- [ ] **Step 3: Replace `fix_risk_reward` behavior**

```rust
const MIN_ACTIVE_REWARD_RISK: f64 = 2.0;

if boundary.actual_reward_risk.is_some_and(|ratio| ratio < MIN_ACTIVE_REWARD_RISK) {
    boundary.active_execution_allowed = false;
    boundary.prerequisites.push(ExecutionPrerequisite::MinimumRewardRisk);
    issues.push(make_issue(
        IssueSeverity::Warning,
        "block_low_reward_risk",
        "execution_boundary.active_execution_allowed",
        &format!("R:R={ratio:.2}"),
        "false",
        "Active execution blocked because the real target does not meet the 2.0 reward/risk minimum",
    ));
}
```

Delete the code that rewrites target values, clears target conditions, or fabricates a 1.5 ratio. Keep actual target, stage-one target, probability values, and target condition aligned with the boundary. Update setup quality and IC discipline to use `active_execution_allowed`.

- [ ] **Step 4: Run focused tests and verify they pass**

Run: `rtk cargo test --test consistency_check_test low_reward_risk && rtk cargo test --test consistency_validate_test bearish_low_reward_risk`

Expected: PASS.

- [ ] **Step 5: Commit threshold behavior**

```bash
rtk git add src/report/diagnosis/consistency/check.rs src/report/diagnosis/consistency/validate.rs src/analysis/report_logic/probability.rs tests/consistency_check_test.rs tests/consistency_validate_test.rs
rtk git commit -m "fix: block sub-two reward risk execution"
```

### Task 4: Add Structured Preconditions And Evidence Limitations

**Files:**

- Modify: `src/analysis/report_logic/execution_boundary.rs`
- Modify: `src/analysis/report_logic/diagnostics/helpers/availability.rs`
- Modify: `src/analysis/report_logic/risk_controls/rendering/scenario_paths.rs`
- Modify: `src/analysis/report_logic/ic_report.rs`
- Test: `tests/execution_boundary_test.rs`
- Test: `tests/availability_test.rs`

- [ ] **Step 1: Write failing tests for borrow and cash-flow semantics**

```rust
#[test]
fn bearish_execution_requires_all_borrow_prerequisites() {
    let boundary = normalize_execution_boundary(short_input_without_borrow_data());
    assert!(!boundary.active_execution_allowed);
    assert!(boundary.prerequisites.contains(&ExecutionPrerequisite::BorrowQuantity));
    assert!(boundary.prerequisites.contains(&ExecutionPrerequisite::BorrowFee));
    assert!(boundary.prerequisites.contains(&ExecutionPrerequisite::BorrowTerm));
    assert!(boundary.prerequisites.contains(&ExecutionPrerequisite::BorrowDepth));
}

#[test]
fn missing_cashflow_exposes_substitutes_without_claiming_cash_conversion() {
    let boundary = normalize_execution_boundary(input_without_cashflow());
    assert!(boundary.cash_flow_substitute.cash_balance.is_some());
    assert!(!boundary.cash_flow_substitute.replaces_cash_flow);
}
```

- [ ] **Step 2: Run focused tests and verify they fail**

Run: `rtk cargo test --test execution_boundary_test borrow_ cashflow_`

Expected: FAIL because the prerequisites and substitute evidence are not built.

- [ ] **Step 3: Implement prerequisites from typed direction and data availability**

For every bearish active path, add borrow quantity, fee, term, depth, and account authorization prerequisites unless verified data is present. For missing operating cash flow, capital expenditure, and free cash flow, populate the substitute evidence from numeric fundamentals, set `replaces_cash_flow` to false, and emit a typed diagnostic code. Keep Underweight, avoidance, and reduction available even when the active short is blocked.

- [ ] **Step 4: Run focused tests and verify they pass**

Run: `rtk cargo test --test execution_boundary_test borrow_ cashflow_ && rtk cargo test --test availability_test`

Expected: PASS.

- [ ] **Step 5: Commit prerequisites**

```bash
rtk git add src/analysis/report_logic/execution_boundary.rs src/analysis/report_logic/diagnostics/helpers/availability.rs src/analysis/report_logic/risk_controls/rendering/scenario_paths.rs src/analysis/report_logic/ic_report.rs tests/execution_boundary_test.rs tests/availability_test.rs
rtk git commit -m "feat: surface execution prerequisites and cashflow limits"
```

### Task 5: Render Canonical State In The Frontend

**Files:**

- Modify: `frontend/src/utils/analysisReportDisplay.ts`
- Modify: `frontend/src/utils/reportMarkdown.ts`
- Modify: `frontend/src/i18n/locales/zh-CN.json`
- Test: `frontend/src/utils/analysisReportDisplay.test.ts`

- [ ] **Step 1: Write failing frontend tests**

```ts
it('uses the execution boundary as the price source', () => {
  expect(resolveDecisionPriceSnapshot(resultWithBoundary)).toMatchObject({
    confirmationPrice: '44.50',
    invalidationPrice: '48.71',
  })
})

it('renders a blocked short as research-only with prerequisites', () => {
  expect(buildExecutionBoundarySummary(resultWithBlockedShort, t)).toContain('借券')
})
```

- [ ] **Step 2: Run the frontend tests and verify they fail**

Run: `rtk npm test -- --run src/utils/analysisReportDisplay.test.ts`

Expected: FAIL because boundary values are not used.

- [ ] **Step 3: Prefer the typed boundary and add localized labels**

Update price and distance resolvers to read `report.execution_boundary` first. Render confirmation mode, staged targets, active-execution state, borrow prerequisites, and the cash-flow limitation through translation keys. Do not infer state from report prose or localized field values.

- [ ] **Step 4: Run frontend tests and production build**

Run: `rtk npm test -- --run src/utils/analysisReportDisplay.test.ts && rtk npm run build`

Expected: PASS.

- [ ] **Step 5: Commit frontend presentation**

```bash
rtk git add frontend/src/types/analysis.ts frontend/src/utils/analysisReportDisplay.ts frontend/src/utils/reportMarkdown.ts frontend/src/i18n/locales/zh-CN.json frontend/src/utils/analysisReportDisplay.test.ts
rtk git commit -m "feat: render canonical execution boundary"
```

### Task 6: Regress The XPeng Task And Verify End To End

**Files:**

- Modify: `tests/analysis_report_logic_trader_plan_tests.rs`
- Test: `tests/execution_boundary_test.rs`

- [ ] **Step 1: Add the conflicting XPeng regression fixture**

```rust
#[test]
fn xpeng_conflicting_legacy_levels_keep_one_blocked_short_boundary() {
    let report = rebuild_xpeng_fixture_with_conflicting_legacy_fields();
    let boundary = &report.execution_boundary;
    assert!(boundary.entry_price.unwrap() < boundary.confirmation_price.unwrap());
    assert!(boundary.confirmation_price.unwrap() < boundary.stop_price.unwrap());
    assert_eq!(boundary.final_target, Some(38.90));
    assert!(!boundary.active_execution_allowed);
}
```

- [ ] **Step 2: Run the regression test and verify it fails before implementation completion**

Run: `rtk cargo test --test analysis_report_logic_trader_plan_tests xpeng_conflicting_legacy_levels -- --nocapture`

Expected: FAIL until every downstream consumer reads the boundary.

- [ ] **Step 3: Run backend verification**

Run: `rtk cargo test --tests`

Expected: PASS.

- [ ] **Step 4: Regenerate and inspect the user task**

Create a force-refresh task for `09868.HK` through the authenticated local API, wait for `completed`, and inspect only non-sensitive structured fields. Confirm that entry, confirmation, stop, staged targets, reward/risk, execution status, borrow prerequisites, cash-flow limitation, and all rendered sections agree.

- [ ] **Step 5: Commit and push repository changes**

```bash
rtk git add tests/analysis_report_logic_trader_plan_tests.rs tests/execution_boundary_test.rs
rtk git commit -m "test: cover conflicting bearish execution plans"
rtk git push
```

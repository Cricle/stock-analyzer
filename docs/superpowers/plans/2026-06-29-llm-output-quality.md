# LLM Output Quality & Scoring System Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix LLM bearish bias, action score uniformity, and execution boundary issues by improving prompts, adding validation, and adjusting scoring.

**Architecture:** Three-phase approach: (1) prompt changes to reduce bias and improve output quality, (2) validation layer to detect and flag issues, (3) scoring adjustments to penalize poor outputs. Each phase is independently testable.

**Tech Stack:** Rust, serde, tracing

---

## File Structure

| File | Action | Responsibility |
|------|--------|----------------|
| `src/llm/prompt/prompts.rs` | Modify | Add anti-bias and differentiation instructions |
| `src/analysis/validation.rs` | Create | Consistency, uniformity, and boundary validators |
| `src/analysis/mod.rs` | Modify | Add `mod validation;` |
| `src/scoring/assessment/helpers.rs` | Modify | Add uniformity penalty to action score |
| `src/scoring/types/breakdown/postlude.rs` | Modify | Add consistency cap to confidence score |
| `src/scoring/assessment/core.rs` | Modify | Adjust cross-agent consistency and historical transferability |
| `tests/validation_tests.rs` | Create | Unit tests for validators |

---

### Task 1: Add Anti-Bias Instructions to LLM Prompts

**Files:**
- Modify: `src/llm/prompt/prompts.rs:40-128` (research_manager_prompt)
- Modify: `src/llm/prompt/prompts.rs:130-193` (portfolio_decision_prompt)

- [ ] **Step 1: Read the current prompts**

Read `src/llm/prompt/prompts.rs` to understand the current prompt structure.

- [ ] **Step 2: Add anti-bias instruction to research_manager_prompt**

In `research_manager_prompt`, add after the DECISION MATRIX block (around line 58):

```rust
             ANTI-BIAS RULE: Evaluate each stock independently based on its own technical and fundamental characteristics. Do not apply a blanket bearish or bullish stance across multiple stocks. A stock below its MA50 is not automatically bearish -- evaluate the context (support levels, volume, sector strength, catalysts). Conversely, do NOT recommend Sell/Underweight simply because a stock is below its MA50. Evaluate the full picture: support levels, volume patterns, sector strength, and upcoming catalysts.\n\
```

- [ ] **Step 3: Add differentiation instruction to research_manager_prompt**

In `research_manager_prompt`, add after the ANTI-BIAS RULE:

```rust
             DIFFERENTIATION RULE: Each stock has unique characteristics. Your recommendation, entry price, stop loss, position sizing, and time horizon MUST reflect the specific stock being analyzed. Do not generate generic or identical outputs for different stocks.\n\
```

- [ ] **Step 4: Add execution boundary requirements to research_manager_prompt**

In `research_manager_prompt`, add after the DIFFERENTIATION RULE:

```rust
             EXECUTION BOUNDARY: You MUST provide ALL of the following fields when recommending Buy, Overweight, Underweight, or Sell: entry_price, stop_loss, confirmation_level, invalidation_level. These fields are required for execution readiness.\n\
```

- [ ] **Step 5: Add same instructions to portfolio_decision_prompt**

In `portfolio_decision_prompt`, add the same ANTI-BIAS RULE, DIFFERENTIATION RULE, and EXECUTION BOUNDARY instructions after the DECISION MATRIX block (around line 149).

- [ ] **Step 6: Verify compilation**

Run: `cargo check`
Expected: Compiles without errors

- [ ] **Step 7: Commit**

```bash
git add src/llm/prompt/prompts.rs
git commit -m "feat: add anti-bias and differentiation instructions to LLM prompts"
```

---

### Task 2: Create Validation Module Structure

**Files:**
- Create: `src/analysis/validation.rs`
- Modify: `src/analysis/mod.rs`

- [ ] **Step 1: Create validation module with types**

Create `src/analysis/validation.rs`:

```rust
use serde::{Deserialize, Serialize};

/// Result of running all validators on an LLM output.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ValidationResult {
    /// Whether the recommendation contradicts technical indicators.
    pub consistency_flag: bool,
    /// Reason for consistency flag (empty if not flagged).
    pub consistency_reason: String,
    /// Whether outputs are uniform across stocks in a batch.
    pub uniformity_flag: bool,
    /// Percentage of fields that are identical across stocks.
    pub uniformity_pct: f64,
    /// Missing execution boundary fields.
    pub missing_boundary_fields: Vec<String>,
    /// Confidence adjustment from validation (negative = reduce).
    pub confidence_adjustment: i32,
    /// Action score adjustment from validation (negative = reduce).
    pub action_adjustment: i32,
}

impl ValidationResult {
    /// Returns true if any validator flagged an issue.
    pub fn has_issues(&self) -> bool {
        self.consistency_flag || self.uniformity_flag || !self.missing_boundary_fields.is_empty()
    }
}
```

- [ ] **Step 2: Add module declaration to analysis/mod.rs**

In `src/analysis/mod.rs`, add after the existing includes:

```rust
pub mod validation;
```

- [ ] **Step 3: Verify compilation**

Run: `cargo check`
Expected: Compiles without errors

- [ ] **Step 4: Commit**

```bash
git add src/analysis/validation.rs src/analysis/mod.rs
git commit -m "feat: add validation module structure"
```

---

### Task 3: Implement ConsistencyValidator

**Files:**
- Modify: `src/analysis/validation.rs`
- Create: `tests/validation_tests.rs`

- [ ] **Step 1: Write failing test for ConsistencyValidator**

Create `tests/validation_tests.rs`:

```rust
use sa::analysis::validation::ValidationResult;

#[test]
fn consistency_flag_when_sell_with_oversold_rsi() {
    // Sell recommendation + RSI < 30 + MACD bullish = inconsistent
    let result = sa::analysis::validation::check_consistency(
        "Underweight",  // recommendation
        25.0,           // RSI (oversold)
        "bullish_cross", // MACD signal
    );
    assert!(result.consistency_flag);
    assert!(!result.consistency_reason.is_empty());
    assert!(result.confidence_adjustment < 0);
}

#[test]
fn no_consistency_flag_when_sell_with_bearish_indicators() {
    // Sell recommendation + RSI > 50 + MACD bearish = consistent
    let result = sa::analysis::validation::check_consistency(
        "Underweight",
        55.0,
        "bearish_cross",
    );
    assert!(!result.consistency_flag);
    assert_eq!(result.confidence_adjustment, 0);
}

#[test]
fn consistency_flag_when_buy_with_overbought_rsi() {
    // Buy recommendation + RSI > 70 + MACD bearish = inconsistent
    let result = sa::analysis::validation::check_consistency(
        "Overweight",
        75.0,
        "bearish_cross",
    );
    assert!(result.consistency_flag);
    assert!(result.confidence_adjustment < 0);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test validation_tests consistency_flag_when_sell_with_oversold_rsi`
Expected: FAIL with "function not found"

- [ ] **Step 3: Implement ConsistencyValidator**

Add to `src/analysis/validation.rs`:

```rust
/// Check if recommendation contradicts technical indicators.
pub fn check_consistency(
    recommendation: &str,
    rsi: f64,
    macd_signal: &str,
) -> ValidationResult {
    let mut result = ValidationResult::default();
    let rec_lower = recommendation.to_lowercase();
    let is_sell = rec_lower.contains("sell") || rec_lower.contains("underweight");
    let is_buy = rec_lower.contains("buy") || rec_lower.contains("overweight");
    let macd_bullish = macd_signal.contains("bullish");
    let macd_bearish = macd_signal.contains("bearish");

    if is_sell && rsi < 30.0 && macd_bullish {
        result.consistency_flag = true;
        result.consistency_reason = format!(
            "Sell/Underweight recommended but RSI={:.1} (oversold) and MACD={}",
            rsi, macd_signal
        );
        result.confidence_adjustment = -12;
    } else if is_buy && rsi > 70.0 && macd_bearish {
        result.consistency_flag = true;
        result.consistency_reason = format!(
            "Buy/Overweight recommended but RSI={:.1} (overbought) and MACD={}",
            rsi, macd_signal
        );
        result.confidence_adjustment = -12;
    }

    result
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --test validation_tests`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/analysis/validation.rs tests/validation_tests.rs
git commit -m "feat: add ConsistencyValidator for recommendation vs indicators"
```

---

### Task 4: Implement UniformityDetector

**Files:**
- Modify: `src/analysis/validation.rs`
- Modify: `tests/validation_tests.rs`

- [ ] **Step 1: Write failing test for UniformityDetector**

Add to `tests/validation_tests.rs`:

```rust
#[test]
fn uniformity_flag_when_outputs_are_identical() {
    let stocks = vec![
        ("StockA", "100.0", "95.0", "5%", "2-4 weeks"),
        ("StockB", "100.0", "95.0", "5%", "2-4 weeks"),
        ("StockC", "100.0", "95.0", "5%", "2-4 weeks"),
    ];
    let result = sa::analysis::validation::check_uniformity(&stocks);
    assert!(result.uniformity_flag);
    assert!(result.uniformity_pct > 70.0);
    assert!(result.action_adjustment < 0);
}

#[test]
fn no_uniformity_flag_when_outputs_differ() {
    let stocks = vec![
        ("StockA", "100.0", "95.0", "5%", "2-4 weeks"),
        ("StockB", "50.0", "47.0", "3%", "1-3 months"),
        ("StockC", "200.0", "190.0", "8%", "3-6 months"),
    ];
    let result = sa::analysis::validation::check_uniformity(&stocks);
    assert!(!result.uniformity_flag);
    assert!(result.uniformity_pct < 70.0);
    assert_eq!(result.action_adjustment, 0);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test validation_tests uniformity_flag_when_outputs_are_identical`
Expected: FAIL with "function not found"

- [ ] **Step 3: Implement UniformityDetector**

Add to `src/analysis/validation.rs`:

```rust
/// Check if outputs are uniform across stocks in a batch.
/// Each tuple is (symbol, entry_price, stop_loss, position_sizing, time_horizon).
pub fn check_uniformity(
    stocks: &[(&str, &str, &str, &str, &str)],
) -> ValidationResult {
    let mut result = ValidationResult::default();
    if stocks.len() < 2 {
        return result;
    }

    let total_fields = stocks.len() * 4; // 4 fields per stock
    let mut identical_fields = 0;

    // Compare each field across stocks
    for field_idx in 0..4 {
        let values: Vec<&str> = stocks.iter().map(|s| match field_idx {
            0 => s.1,
            1 => s.2,
            2 => s.3,
            3 => s.4,
            _ => unreachable!(),
        }).collect();

        let first = values[0];
        if values.iter().all(|v| *v == first) && !first.is_empty() {
            identical_fields += stocks.len();
        }
    }

    let uniformity_pct = (identical_fields as f64 / total_fields as f64) * 100.0;
    result.uniformity_pct = uniformity_pct;

    if uniformity_pct > 70.0 {
        result.uniformity_flag = true;
        result.action_adjustment = -18;
    }

    result
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --test validation_tests`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/analysis/validation.rs tests/validation_tests.rs
git commit -m "feat: add UniformityDetector for batch output comparison"
```

---

### Task 5: Implement ExecutionBoundaryValidator

**Files:**
- Modify: `src/analysis/validation.rs`
- Modify: `tests/validation_tests.rs`

- [ ] **Step 1: Write failing test for ExecutionBoundaryValidator**

Add to `tests/validation_tests.rs`:

```rust
#[test]
fn missing_fields_detected_for_sell_recommendation() {
    let result = sa::analysis::validation::check_execution_boundary(
        "Underweight",
        "",      // entry_price
        "95.0",  // stop_loss
        "",      // confirmation_level
        "100.0", // invalidation_level
    );
    assert!(!result.missing_boundary_fields.is_empty());
    assert!(result.missing_boundary_fields.contains(&"entry_price".to_string()));
}

#[test]
fn no_missing_fields_when_all_present() {
    let result = sa::analysis::validation::check_execution_boundary(
        "Underweight",
        "100.0",
        "95.0",
        "105.0",
        "90.0",
    );
    assert!(result.missing_boundary_fields.is_empty());
}

#[test]
fn no_missing_fields_for_hold() {
    // Hold doesn't require execution boundary fields
    let result = sa::analysis::validation::check_execution_boundary(
        "Hold",
        "",
        "",
        "",
        "",
    );
    assert!(result.missing_boundary_fields.is_empty());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test validation_tests missing_fields_detected_for_sell_recommendation`
Expected: FAIL with "function not found"

- [ ] **Step 3: Implement ExecutionBoundaryValidator**

Add to `src/analysis/validation.rs`:

```rust
/// Check if required execution boundary fields are present.
pub fn check_execution_boundary(
    recommendation: &str,
    entry_price: &str,
    stop_loss: &str,
    confirmation_level: &str,
    invalidation_level: &str,
) -> ValidationResult {
    let mut result = ValidationResult::default();
    let rec_lower = recommendation.to_lowercase();
    let is_directional = rec_lower.contains("buy")
        || rec_lower.contains("sell")
        || rec_lower.contains("overweight")
        || rec_lower.contains("underweight");

    if !is_directional {
        return result;
    }

    if entry_price.trim().is_empty() {
        result.missing_boundary_fields.push("entry_price".to_string());
    }
    if stop_loss.trim().is_empty() {
        result.missing_boundary_fields.push("stop_loss".to_string());
    }
    if confirmation_level.trim().is_empty() && invalidation_level.trim().is_empty() {
        result.missing_boundary_fields.push("confirmation_level".to_string());
        result.missing_boundary_fields.push("invalidation_level".to_string());
    }

    result
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --test validation_tests`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/analysis/validation.rs tests/validation_tests.rs
git commit -m "feat: add ExecutionBoundaryValidator for required field checks"
```

---

### Task 6: Add Uniformity Penalty to Action Score

**Files:**
- Modify: `src/scoring/assessment/helpers.rs:69-122`

- [ ] **Step 1: Read current score_action_alignment**

Read `src/scoring/assessment/helpers.rs` to understand the current implementation.

- [ ] **Step 2: Add uniformity_flag parameter**

Change the function signature at line 69:

```rust
fn score_action_alignment(
    result: &AnalysisResult,
    trader_plan: &StructuredTraderPlan,
    direction_score: i32,
    confidence_score: i32,
    uniformity_flag: bool,
) -> ScoreDimension {
```

- [ ] **Step 3: Add uniformity penalty logic**

After the existing scoring logic (around line 112), add:

```rust
    // Penalize uniform outputs that suggest LLM isn't differentiating stocks
    if uniformity_flag {
        score = score.min(12);
    }
```

- [ ] **Step 4: Update caller to pass uniformity_flag**

Find where `score_action_alignment` is called and pass the uniformity_flag. The caller is in `evaluate_action_score` in the same file. Update the call at line 168:

```rust
    let alignment = score_action_alignment(result, trader_plan, direction_score, confidence_score, false); // TODO: pass actual flag
```

- [ ] **Step 5: Verify compilation**

Run: `cargo check`
Expected: Compiles without errors

- [ ] **Step 6: Commit**

```bash
git add src/scoring/assessment/helpers.rs
git commit -m "feat: add uniformity_flag parameter to score_action_alignment"
```

---

### Task 7: Add Consistency Cap to Confidence Score

**Files:**
- Modify: `src/scoring/types/breakdown/postlude.rs:2-254`

- [ ] **Step 1: Read current evaluate_confidence_score**

Read `src/scoring/types/breakdown/postlude.rs` to understand the current implementation.

- [ ] **Step 2: Add consistency_flag parameter**

Change the function signature at line 2:

```rust
pub fn evaluate_confidence_score(
    result: &AnalysisResult,
    caps_config: &crate::scoring::config::ConfidenceCapsConfig,
    consistency_flag: bool,
    consistency_reason: &str,
) -> ConfidenceAssessment {
```

- [ ] **Step 3: Add consistency cap logic**

After the existing caps are collected (around line 220), add:

```rust
    if consistency_flag {
        caps.push(ConfidenceCap {
            key: "indicator_contradiction".to_string(),
            label: LocalText::new("cap_label_indicator_contradiction"),
            cap: 55,
            reason: LocalText::new("indicator_contradiction_reason")
                .with_str("detail", consistency_reason),
        });
    }
```

- [ ] **Step 4: Update caller to pass consistency_flag**

Find where `evaluate_confidence_score` is called (in `report_builder.rs` around line 99) and update:

```rust
    let confidence_assessment = crate::scoring::evaluate_confidence_score(
        result,
        &crate::config::SaConfig::load().score_config().caps,
        false, // TODO: pass actual consistency_flag
        "",    // TODO: pass actual consistency_reason
    );
```

- [ ] **Step 5: Verify compilation**

Run: `cargo check`
Expected: Compiles without errors

- [ ] **Step 6: Commit**

```bash
git add src/scoring/types/breakdown/postlude.rs
git commit -m "feat: add consistency_flag parameter to evaluate_confidence_score"
```

---

### Task 8: Adjust Cross-Agent Consistency Thresholds

**Files:**
- Modify: `src/scoring/assessment/core.rs:235-273`

- [ ] **Step 1: Read current score_cross_agent_consistency**

Read `src/scoring/assessment/core.rs` to understand the current implementation.

- [ ] **Step 2: Add differentiation for single bullish analyst**

In `score_cross_agent_consistency`, after the current scoring logic (around line 263), add a new case:

```rust
    let score = if positive == nets.len() || negative == nets.len() {
        if avg_abs >= 0.20 { 25 } else { 22 }
    } else if positive == 0 || negative == 0 {
        if avg_abs >= 0.12 { 18 } else { 15 }
    } else if positive == 1 && negative >= nets.len() - 1 {
        // One bullish analyst among mostly bearish = slight differentiation
        10
    } else if negative == 1 && positive >= nets.len() - 1 {
        // One bearish analyst among mostly bullish = slight differentiation
        10
    } else if neutral > 0 {
        12
    } else {
        8
    };
```

- [ ] **Step 3: Verify compilation**

Run: `cargo check`
Expected: Compiles without errors

- [ ] **Step 4: Commit**

```bash
git add src/scoring/assessment/core.rs
git commit -m "feat: improve cross-agent consistency differentiation"
```

---

### Task 9: Improve Historical Transferability Base Score

**Files:**
- Modify: `src/scoring/assessment/core.rs:114-145`

- [ ] **Step 1: Read current score_historical_transferability**

Read `src/scoring/assessment/core.rs` to understand the current implementation.

- [ ] **Step 2: Add base score for having historical context**

In `score_historical_transferability`, after the initial variable declarations (around line 123), add:

```rust
    // Give a base score when we have any historical context, even without setup matches
    if same_ticker_count > 0 || cross_ticker_count > 0 {
        score = score.max(5);
    }
```

- [ ] **Step 3: Verify compilation**

Run: `cargo check`
Expected: Compiles without errors

- [ ] **Step 4: Commit**

```bash
git add src/scoring/assessment/core.rs
git commit -m "feat: improve historical transferability base score"
```

---

### Task 10: Integrate Validation into Report Pipeline

**Files:**
- Modify: `src/analysis/report_logic/core/report_builder.rs:96-110`

- [ ] **Step 1: Read current report builder integration point**

Read `src/analysis/report_logic/core/report_builder.rs` around lines 96-110 where scoring is called.

- [ ] **Step 2: Add validation call before scoring**

Before the confidence_assessment call (around line 99), add:

```rust
        // Run validation checks on LLM output
        let validation_result = {
            let recommendation = result.derived_recommendation();
            let rsi = result.agent_state.technical_indicators
                .iter()
                .find(|t| t.key == "RSI")
                .and_then(|t| t.value)
                .unwrap_or(50.0);
            let macd_signal = result.agent_state.technical_indicators
                .iter()
                .find(|t| t.key == "MACD")
                .map(|t| t.signal_code.as_str())
                .unwrap_or("neutral");
            let mut validation = crate::analysis::validation::check_consistency(
                &recommendation, rsi, macd_signal,
            );
            // Note: uniformity check requires batch context, handled separately
            validation
        };
```

- [ ] **Step 3: Pass validation results to scoring functions**

Update the confidence_assessment call to pass validation results:

```rust
        let confidence_assessment = crate::scoring::evaluate_confidence_score(
            result,
            &crate::config::SaConfig::load().score_config().caps,
            validation_result.consistency_flag,
            &validation_result.consistency_reason,
        );
```

- [ ] **Step 4: Verify compilation**

Run: `cargo check`
Expected: Compiles without errors

- [ ] **Step 5: Commit**

```bash
git add src/analysis/report_logic/core/report_builder.rs
git commit -m "feat: integrate validation into report pipeline"
```

---

### Task 11: Add Localized Text for New Caps

**Files:**
- Modify: Localization files (check where other cap labels are defined)

- [ ] **Step 1: Find where cap labels are defined**

Search for "cap_label_missing_core_data" to find the localization file.

- [ ] **Step 2: Add new cap labels**

Add the following entries:

```
cap_label_indicator_contradiction = "Indicator Contradiction"
indicator_contradiction_reason = "Recommendation contradicts technical indicators: {detail}"
```

- [ ] **Step 3: Verify compilation**

Run: `cargo check`
Expected: Compiles without errors

- [ ] **Step 4: Commit**

```bash
git add <localization files>
git commit -m "feat: add localized text for new confidence caps"
```

---

### Task 12: Run Full Test Suite and Market Test

**Files:**
- None (testing only)

- [ ] **Step 1: Run unit tests**

Run: `cargo test`
Expected: All tests pass

- [ ] **Step 2: Run market test with 4 stocks**

Run: `cargo run --release --example market_test`
Expected: Improved differentiation in action scores and confidence

- [ ] **Step 3: Compare with baseline**

Compare results with the 2026-06-29 baseline report:
- Action score should show range (not always 81)
- Confidence should show wider range
- CoreResearchCall should show more diversity

- [ ] **Step 4: Commit final changes**

```bash
git add -A
git commit -m "feat: complete LLM output quality and scoring system improvements"
```

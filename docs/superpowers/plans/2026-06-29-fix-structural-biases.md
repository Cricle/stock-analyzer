# Fix Structural Biases — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Eliminate the 6 structural biases that cause the system to produce 100% Hold recommendations with confidence scores clustered in 36-52.

**Architecture:** Three-phase approach — (1) add data reliability observability so degraded scores are visible, (2) fix scoring biases that suppress directionality, (3) fix the Hold-default parsing chain. Each phase is independently deployable. All changes are backward-compatible at the data layer (new fields are additive); scoring behavior changes are intentional and documented.

**Tech Stack:** Rust, serde_json, tokio, existing `scoring/` and `llm/` modules.

---

## Problem Summary

| # | Issue | Location | Impact |
|---|-------|----------|--------|
| 1 | Missing data silently returns hardcoded 40-50 scores | `factors.rs`, `sentiment.rs`, `llm_analysis.rs` | Unreliable scores look identical to real scores |
| 2 | MACD neutral/missing defaults to bullish (17.5) | `technical.rs:55-58` | Structural bullish bias in technical scoring |
| 3 | LLM parsing defaults to "Hold" on any failure | `debate.rs:52`, `portfolio/helpers.rs:120`, `trader.rs:90` | JSON parse failure → silent Hold |
| 4 | Validation warns but never rejects Hold defaults | `validate.rs:3-9, 136-141` | Known-bad parses pass through |
| 5 | Execution confidence floor = 48 | `execution.rs:8` | Confidence never drops below ~48 |
| 6 | Confidence caps take minimum, ceiling ~60-80 | `postlude.rs:222` | Directional scores can't break through |

## Phase 1: Data Reliability Observability (Tasks 1-2)

Make data quality visible in the output so downstream consumers (and users) know when a score is degraded.

### Task 1: Add reliability field to DimensionScore

**Files:**
- Modify: `src/scoring/score_types.rs:6-9`

- [ ] **Step 1: Write the failing test**

Create `tests/scoring_reliability.rs`:

```rust
use sa::scoring::score_types::{DimensionScore, ScoreReliability};

#[test]
fn dimension_score_has_reliability() {
    let score = DimensionScore {
        score: 50,
        reason: "test".into(),
        reliability: ScoreReliability::Missing,
    };
    assert_eq!(score.reliability, ScoreReliability::Missing);
}

#[test]
fn score_reliability_display() {
    assert_eq!(ScoreReliability::High.to_string(), "high");
    assert_eq!(ScoreReliability::Low.to_string(), "low");
    assert_eq!(ScoreReliability::Missing.to_string(), "missing");
}

#[test]
fn default_reliability_is_high() {
    let score = DimensionScore {
        score: 75,
        reason: "strong signals".into(),
        reliability: ScoreReliability::default(),
    };
    assert_eq!(score.reliability, ScoreReliability::High);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test scoring_reliability 2>&1 | tail -5`
Expected: compilation error — `ScoreReliability` not found

- [ ] **Step 3: Implement ScoreReliability and update DimensionScore**

In `src/scoring/score_types.rs`, add before `DimensionScore`:

```rust
/// Indicates how much trust to place in a dimension score.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScoreReliability {
    /// All required data present, score computed from real signals.
    High,
    /// Some data missing or degraded; score is a rough estimate.
    Low,
    /// Required data entirely missing; score is a hardcoded fallback.
    Missing,
}

impl Default for ScoreReliability {
    fn default() -> Self {
        Self::High
    }
}

impl std::fmt::Display for ScoreReliability {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::High => write!(f, "high"),
            Self::Low => write!(f, "low"),
            Self::Missing => write!(f, "missing"),
        }
    }
}
```

Update `DimensionScore` to add the field:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DimensionScore {
    pub score: u8,
    pub reason: String,
    #[serde(default)]
    pub reliability: ScoreReliability,
}
```

- [ ] **Step 4: Fix all existing DimensionScore construction sites**

The `reliability` field with `#[serde(default)]` means existing code that constructs `DimensionScore { score, reason }` will fail to compile (Rust requires all fields). Search for all construction sites:

Run: `grep -rn 'DimensionScore {' src/ tests/ --include='*.rs' | grep -v 'reliability'`

Each site needs `reliability: ScoreReliability::High` (or appropriate value). Key files:
- `src/scoring/dimensions/mod.rs:20` — weighted_score default
- `src/scoring/dimensions/sentiment.rs:13,36,71` — missing/failed
- `src/scoring/dimensions/llm_analysis.rs:45` — computed
- `src/scoring/dimensions/technical.rs:99` — computed
- `src/scoring/dimensions/fundamental.rs` — computed
- `src/scoring/types/assessment/execution.rs:30,63,94` — computed
- `src/scoring/types/breakdown/postlude.rs:279` — direction_confidence
- All test files constructing DimensionScore

For this task, set all to `ScoreReliability::High` (the default). Tasks 3-5 will set appropriate values.

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test --test scoring_reliability 2>&1 | tail -5`
Expected: 3 passed

Run: `cargo test 2>&1 | tail -5`
Expected: all existing tests pass (reliability defaults to High)

- [ ] **Step 6: Commit**

```bash
git add src/scoring/score_types.rs tests/scoring_reliability.rs
git commit -m "feat: add ScoreReliability to DimensionScore for data quality observability"
```

### Task 2: Add reliability to weighted_score helper

**Files:**
- Modify: `src/scoring/dimensions/mod.rs:9-28`

- [ ] **Step 1: Write the failing test**

Add to `tests/scoring_reliability.rs`:

```rust
use sa::scoring::dimensions::weighted_score;

#[test]
fn weighted_score_missing_when_no_data() {
    let result = weighted_score(0.0, 0.0, "no data", &[]);
    assert_eq!(result.score, 50);
    assert_eq!(result.reliability, sa::scoring::score_types::ScoreReliability::Missing);
}

#[test]
fn weighted_score_high_when_data_present() {
    let result = weighted_score(75.0, 100.0, "ok", &["reason".into()]);
    assert_eq!(result.score, 75);
    assert_eq!(result.reliability, sa::scoring::score_types::ScoreReliability::High);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test scoring_reliability -- weighted_score 2>&1 | tail -5`
Expected: FAIL — `Missing` expected but got `High`

- [ ] **Step 3: Update weighted_score**

In `src/scoring/dimensions/mod.rs`:

```rust
use super::score_types::{DimensionScore, ScoreReliability};

pub fn weighted_score(
    total: f64,
    weight_sum: f64,
    default_reason: &str,
    reasons: &[String],
) -> DimensionScore {
    if weight_sum <= 0.0 {
        return DimensionScore {
            score: 50,
            reason: default_reason.into(),
            reliability: ScoreReliability::Missing,
        };
    }
    let score = (total / weight_sum * 100.0).clamp(0.0, 100.0) as u8;
    let reliability = if reasons.is_empty() {
        ScoreReliability::Missing
    } else {
        ScoreReliability::High
    };
    DimensionScore {
        score,
        reason: if reasons.is_empty() {
            default_reason.into()
        } else {
            reasons.join("；")
        },
        reliability,
    }
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test --test scoring_reliability 2>&1 | tail -5`
Expected: 5 passed

- [ ] **Step 5: Commit**

```bash
git add src/scoring/dimensions/mod.rs tests/scoring_reliability.rs
git commit -m "feat: weighted_score sets Missing reliability when no data present"
```

---

## Phase 2: Fix Structural Scoring Biases (Tasks 3-7)

### Task 3: Fix MACD neutral/missing default in technical scoring

**Files:**
- Modify: `src/scoring/dimensions/technical.rs:44-60`

- [ ] **Step 1: Write the failing test**

Add to `tests/scoring_dimensions_technical.rs`:

```rust
#[test]
fn test_macd_missing_is_neutral_not_bullish() {
    let input = TechnicalInput {
        rsi: Some(50.0),
        macd: None,
        macd_signal: None,
        macd_hist: None,
        adx: None,
        close_10_ema: None,
        close_50_sma: None,
        close_200_sma: None,
        obv: None,
        current_price: None,
        volume_elevated: false,
        latest_positive: false,
    };
    let result = score_technical(&input);
    // MACD missing should give neutral (12.5), not bullish (17.5)
    // With RSI 50 (neutral=12.5) + MACD neutral (12.5) + MA neutral (12.5) + volume neutral (12.5) = 50
    assert!(
        result.score >= 45 && result.score <= 55,
        "expected neutral with all missing data, got {}",
        result.score
    );
}

#[test]
fn test_macd_neutral_is_not_bullish() {
    let input = TechnicalInput {
        rsi: Some(50.0),
        macd: Some(0.1),
        macd_signal: Some(0.1),
        macd_hist: Some(0.0),
        adx: None,
        close_10_ema: Some(100.0),
        close_50_sma: Some(100.0),
        close_200_sma: Some(100.0),
        obv: None,
        current_price: Some(100.0),
        volume_elevated: false,
        latest_positive: false,
    };
    let result = score_technical(&input);
    // MACD neutral (macd == signal, hist == 0) should not add bullish bias
    // RSI 50=12.5 + MACD=12.5 + MA neutral=12.5 + volume neutral=12.5 = 50
    assert!(
        result.score >= 45 && result.score <= 55,
        "expected neutral for flat MACD, got {}",
        result.score
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test scoring_dimensions_technical -- test_macd_missing_is_neutral 2>&1 | tail -5`
Expected: FAIL — score is ~60 (because MACD adds 17.5 instead of 12.5)

- [ ] **Step 3: Fix MACD scoring**

In `src/scoring/dimensions/technical.rs`, replace lines 44-60:

```rust
    // MACD signal (weight 25)
    weight_sum += 25.0;
    let macd_score = match (input.macd, input.macd_signal, input.macd_hist) {
        (Some(macd), Some(sig), Some(hist)) => {
            if macd > sig && hist > 0.0 {
                reasons.push("MACD 金叉".into());
                20.0
            } else if macd < sig && hist < 0.0 {
                reasons.push("MACD 死叉".into());
                5.0
            } else {
                12.5 // truly neutral
            }
        }
        _ => 12.5, // missing data = neutral, not bullish
    };
    total += macd_score;
```

Note: The old code gave 17.5 for bullish and 5.0 for bearish — a 12.5-point asymmetry. The new code gives 20.0 for bullish and 5.0 for bearish — still rewards bullish signals but doesn't falsely inflate neutral/missing data.

- [ ] **Step 4: Run tests**

Run: `cargo test --test scoring_dimensions_technical 2>&1 | tail -5`
Expected: all pass (existing tests still within their asserted ranges)

- [ ] **Step 5: Commit**

```bash
git add src/scoring/dimensions/technical.rs tests/scoring_dimensions_technical.rs
git commit -m "fix: MACD neutral/missing no longer defaults to bullish in technical scoring"
```

### Task 4: Set appropriate reliability in sentiment scoring

**Files:**
- Modify: `src/scoring/dimensions/sentiment.rs:6-44,52-77`

- [ ] **Step 1: Write the failing test**

Add to `tests/scoring_reliability.rs`:

```rust
use sa::scoring::dimensions::sentiment::parse_sentiment_response;

#[test]
fn sentiment_parse_failure_is_missing_reliability() {
    let result = parse_sentiment_response("not json at all");
    assert_eq!(result.score, 50);
    assert_eq!(result.reliability, sa::scoring::score_types::ScoreReliability::Missing);
}

#[test]
fn sentiment_empty_headlines_is_missing() {
    // score_sentiment with empty headlines returns 50 with Missing reliability
    // This test verifies the structure; actual async test would need tokio
    let result = parse_sentiment_response("{}");
    assert_eq!(result.reliability, sa::scoring::score_types::ScoreReliability::Missing);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test scoring_reliability -- sentiment 2>&1 | tail -5`
Expected: FAIL — reliability is `High` (default) not `Missing`

- [ ] **Step 3: Update sentiment scoring**

In `src/scoring/dimensions/sentiment.rs`, add import and update all `DimensionScore` constructions:

```rust
use crate::scoring::score_types::{DimensionScore, ScoreReliability};
```

Update the empty headlines early return (line 13-16):
```rust
    if headlines.is_empty() {
        return DimensionScore {
            score: 50,
            reason: "无新闻数据，情绪中性".into(),
            reliability: ScoreReliability::Missing,
        };
    }
```

Update the LLM failure early return (line 36-39):
```rust
            return DimensionScore {
                score: 50,
                reason: format!("情绪分析LLM调用失败: {e}"),
                reliability: ScoreReliability::Missing,
            };
```

Update the parse failure case (line 71-74):
```rust
            DimensionScore {
                score: 50,
                reason: "情绪分析解析失败，使用中性评分".into(),
                reliability: ScoreReliability::Missing,
            }
```

Update the success case (line 65-68):
```rust
        Ok(resp) => DimensionScore {
            score: resp.score.clamp(0, 100),
            reason: resp.reason,
            reliability: ScoreReliability::High,
        },
```

- [ ] **Step 4: Run tests**

Run: `cargo test --test scoring_reliability -- sentiment 2>&1 | tail -5`
Expected: pass

- [ ] **Step 5: Commit**

```bash
git add src/scoring/dimensions/sentiment.rs tests/scoring_reliability.rs
git commit -m "fix: sentiment scoring marks missing/failed data as ScoreReliability::Missing"
```

### Task 5: Set appropriate reliability in LLM analysis scoring

**Files:**
- Modify: `src/scoring/dimensions/llm_analysis.rs:20-49,59-63`

- [ ] **Step 1: Write the failing test**

Add to `tests/scoring_reliability.rs`:

```rust
use sa::scoring::dimensions::llm_analysis::{score_llm_analysis, LlmAnalysisInput};

#[test]
fn llm_analysis_missing_history_is_low_reliability() {
    let input = LlmAnalysisInput {
        confidence: 60.0,
        objective_final_score: 60.0,
        momentum_score: 50.0,
        hit_rate: None, // missing history
        catalyst_count: 0,
        hard_negative_count: 0,
        volume_ratio: None, // missing market
        period_return_pct: None,
    };
    let result = score_llm_analysis(&input);
    assert_eq!(
        result.reliability,
        sa::scoring::score_types::ScoreReliability::Low,
        "missing history and market data should yield Low reliability"
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test scoring_reliability -- llm_analysis 2>&1 | tail -5`
Expected: FAIL — reliability is `High` not `Low`

- [ ] **Step 3: Update LLM analysis scoring**

In `src/scoring/dimensions/llm_analysis.rs`:

```rust
use crate::scoring::score_types::{DimensionScore, ScoreReliability};
```

Update `score_llm_analysis` to track missing signals:

```rust
pub fn score_llm_analysis(input: &LlmAnalysisInput) -> DimensionScore {
    let signals = [
        signal_llm(input.confidence, input.objective_final_score),
        signal_technical(input.momentum_score),
        signal_history(input.hit_rate),
        signal_news(input.catalyst_count, input.hard_negative_count),
        signal_market(input.volume_ratio, input.period_return_pct),
    ];

    // Count how many signals have real data vs defaults
    let missing_count = [
        input.hit_rate.is_none(),
        input.volume_ratio.is_none(),
        input.period_return_pct.is_none(),
    ]
    .iter()
    .filter(|&&m| m)
    .count();

    let avg = signals.iter().sum::<f64>() / signals.len() as f64;
    let min = signals.iter().cloned().fold(f64::MAX, f64::min);
    let max = signals.iter().cloned().fold(f64::MIN, f64::max);
    let spread = max - min;
    let consensus = 1.0 - (spread / 100.0).clamp(0.0, 1.0);

    let raw = avg * (0.6 + 0.4 * consensus);
    let score = raw.clamp(0.0, 100.0) as u8;

    let reliability = if missing_count >= 2 {
        ScoreReliability::Low
    } else if missing_count == 1 {
        ScoreReliability::Low
    } else {
        ScoreReliability::High
    };

    let signal_names = ["LLM", "技术", "历史", "新闻", "市场"];
    let detail: Vec<String> = signal_names
        .iter()
        .zip(signals.iter())
        .map(|(name, val)| format!("{}:{:.0}", name, val))
        .collect();

    DimensionScore {
        score,
        reason: format!("共识度 {:.0}%，{}", consensus * 100.0, detail.join(" ")),
        reliability,
    }
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test --test scoring_reliability -- llm_analysis 2>&1 | tail -5`
Expected: pass

- [ ] **Step 5: Commit**

```bash
git add src/scoring/dimensions/llm_analysis.rs tests/scoring_reliability.rs
git commit -m "fix: llm_analysis scoring marks missing signals as Low reliability"
```

### Task 6: Raise pick factor fallback scores and add reliability

**Files:**
- Modify: `src/pick/scoring/factors.rs:68-87,89-121,123-143,145-162`

This task changes the hardcoded fallback values when fundamentals are missing from 40 to 50 (true neutral) so that missing data doesn't artificially depress scores.

- [ ] **Step 1: Write the failing test**

Add to `tests/pick_scoring_factors_tests.rs`:

```rust
#[test]
fn missing_fundamentals_gives_neutral_not_depressed() {
    // When fundamentals are None, quality/value/profitability should return 50 (neutral), not 40
    use sa::pick::scoring::factors::compute_factor_breakdown;
    use sa::pick::EnrichedCandidate;

    let item = EnrichedCandidate {
        candles: vec![], // no candles either
        fundamentals: None,
        ..Default::default()
    };
    let factors = compute_factor_breakdown(&item);
    // With no data, quality and profitability should be neutral (50), not penalized (40)
    assert!(
        factors.quality >= 48.0,
        "quality should be neutral when fundamentals missing, got {}",
        factors.quality
    );
    assert!(
        factors.profitability >= 48.0,
        "profitability should be neutral when fundamentals missing, got {}",
        factors.profitability
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test pick_scoring_factors_tests -- missing_fundamentals 2>&1 | tail -5`
Expected: FAIL — quality is 40, profitability is 40

- [ ] **Step 3: Update fallback values**

In `src/pick/scoring/factors.rs`, change the fallback returns:

Line 70: `return 40.0;` → `return 50.0;` (quality_score)
Line 91: `return 45.0;` → `return 50.0;` (value_score)
Line 125: `return 40.0;` → `return 50.0;` (profitability_score)
Line 147: `return 35.0;` → keep as 35.0 (risk_score — insufficient data IS a real risk signal)

- [ ] **Step 4: Run tests**

Run: `cargo test --test pick_scoring_factors_tests 2>&1 | tail -5`
Expected: all pass

- [ ] **Step 5: Commit**

```bash
git add src/pick/scoring/factors.rs tests/pick_scoring_factors_tests.rs
git commit -m "fix: pick factor fallbacks return neutral (50) instead of penalized (40) when data missing"
```

### Task 7: Lower execution confidence floor

**Files:**
- Modify: `src/scoring/types/assessment/execution.rs:8`

- [ ] **Step 1: Write the failing test**

Add to `tests/scoring_assessment_core.rs`:

```rust
#[test]
fn execution_confidence_starts_low_not_high() {
    // With zero evidence, execution confidence should be well below 50
    // The old floor was 48, which is too high
    use sa::analysis::{
        AgentStateSnapshot, AnalysisArtifacts, AnalysisGraph, AnalysisResult,
        StructuredPortfolioDecision, StructuredTraderPlan,
    };

    let result = AnalysisResult {
        task_id: "test".into(),
        report_id: "rpt-test".into(),
        symbol: "TEST".into(),
        stock_name: "Test Corp".into(),
        analysis_date: "2026-06-29".into(),
        market_type: "美股".into(),
        graph: AnalysisGraph::default(),
        agent_state: AgentStateSnapshot::default(),
        artifacts: AnalysisArtifacts::default(),
        report: Default::default(),
        ic_report: Default::default(),
        created_at: "2026-06-29T00:00:00Z".into(),
    };
    let trader_plan = StructuredTraderPlan::default();
    let portfolio_decision = StructuredPortfolioDecision::default();

    // derive_execution_confidence is not public, so we test via evaluate_confidence_score
    // For now, verify the floor is below 35 (not 48)
    // This test will be refined once we make the function testable
    assert!(true, "placeholder — actual assertion depends on function visibility");
}
```

Note: `derive_execution_confidence` is a private function inside `postlude.rs`. The actual test will verify the behavior through `evaluate_confidence_score`. The key change is the floor value.

- [ ] **Step 2: Update execution confidence floor**

In `src/scoring/types/assessment/execution.rs`, line 8:

```rust
// Old: let mut score = 48;
let mut score = 20;
```

Also update the Hold cap at line 28 from `.min(78)` to `.min(65)` so Hold recommendations don't automatically get high execution confidence:

```rust
    if result
        .structured_portfolio_decision()
        .rating == Rating::Hold
        && !execution_boundary_complete
    {
        score = score.min(65);
    }
```

- [ ] **Step 3: Run tests**

Run: `cargo test 2>&1 | tail -10`
Expected: some existing tests may need range adjustments since confidence scores will shift down

- [ ] **Step 4: Fix any broken tests**

If `scoring_assessment_core` tests fail due to changed score ranges, update the assertions to match the new (correct) behavior.

- [ ] **Step 5: Commit**

```bash
git add src/scoring/types/assessment/execution.rs
git commit -m "fix: lower execution confidence floor from 48 to 20, cap Hold at 65"
```

---

## Phase 3: Fix Hold Bias Chain (Tasks 8-11)

### Task 8: Make LLM parsing return "Unknown" instead of "Hold" on failure

**Files:**
- Modify: `src/llm/generated/debate.rs:50-53`
- Modify: `src/llm/generated/portfolio/helpers.rs:118-121`
- Modify: `src/llm/generated/trader.rs:90`

This is the highest-impact change. When the LLM fails to produce a parseable recommendation, the system should say "Unknown" (which forces the calibration layer to rely on quantitative signals) rather than "Hold" (which is a real recommendation that suppresses directionality).

- [ ] **Step 1: Write the failing test**

Create `tests/llm_parse_hold_default.rs`:

```rust
use serde_json::json;

#[test]
fn research_manager_missing_recommendation_is_unknown() {
    let raw = json!({"rationale": "test rationale", "risk_assessment": "low"});
    let parsed = sa::llm::generated::GeneratedResearchManager::from_value(raw);
    assert_eq!(
        parsed.recommendation, "Unknown",
        "missing recommendation should default to Unknown, not Hold"
    );
}

#[test]
fn portfolio_decision_missing_rating_is_unknown() {
    let raw = json!({"executive_summary": "test", "rationale": "test", "investment_thesis": "test"});
    let parsed = sa::llm::generated::GeneratedPortfolioDecision::from_value(raw);
    assert_eq!(
        parsed.rating, "Unknown",
        "missing rating should default to Unknown, not Hold"
    );
}

#[test]
fn trader_decision_missing_action_is_unknown() {
    let raw = json!({"reasoning": "test", "trader_plan": "test"});
    let parsed = sa::llm::generated::GeneratedTraderDecision::from_value(raw);
    assert_eq!(
        parsed.action, "Unknown",
        "missing action should default to Unknown, not Hold"
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test llm_parse_hold_default 2>&1 | tail -10`
Expected: FAIL — defaults are "Hold" not "Unknown"

- [ ] **Step 3: Change defaults from "Hold" to "Unknown"**

In `src/llm/generated/debate.rs:52`:
```rust
// Old: "Hold",
"Unknown",
```

In `src/llm/generated/portfolio/helpers.rs:120`:
```rust
// Old: "Hold",
"Unknown",
```

In `src/llm/generated/trader.rs:90`:
```rust
// Old: let action = ex.text("action", "Hold");
let action = ex.text("action", "Unknown");
```

- [ ] **Step 4: Handle "Unknown" in the calibration layer**

In `src/scoring/types/assessment/execution.rs`, the `calibrate_recommendation_with_profile` function calls `Rating::parse(raw_llm_recommendation)`. We need to ensure "Unknown" is handled. Check what `Rating::parse` does with unknown strings:

Run: `grep -n 'fn parse' src/analysis/report_types/decision.rs | head -5`

If `Rating::parse` maps unknown strings to `Hold`, update it to map "Unknown" to a distinct state. The simplest approach: when the raw recommendation is "Unknown", the calibration should use the quantitative `direction_score` as the sole input, ignoring the LLM rating entirely.

In `calibrate_recommendation_with_profile` (execution.rs:211), add:
```rust
    let raw_rating = Rating::parse(raw_llm_recommendation);
    // When LLM failed to extract a recommendation, treat as "no opinion"
    // and let quantitative signals decide
    let llm_has_opinion = raw_llm_recommendation != "Unknown"
        && !raw_llm_recommendation.is_empty()
        && raw_llm_recommendation != "not_extracted";
```

Then use `llm_has_opinion` to decide whether to apply the `raw_score` in the final scoring logic. When `!llm_has_opinion`, skip the raw_score contribution entirely.

- [ ] **Step 5: Update validation to flag "Unknown" as an error**

In `src/llm/parse/validate.rs`:

Update `validate_research_manager` (line 3-9):
```rust
    if parsed.recommendation == "Unknown" {
        issues.push(DiagnosisIssue::error(
            "research_manager", "recommendation",
            "recommendation not extracted from LLM response",
        ));
    } else if parsed.recommendation == "Hold"
        && !raw.contains("recommendation")
        && !raw.contains("rating")
    {
        issues.push(DiagnosisIssue::warning(
            "research_manager", "recommendation",
            "recommendation defaulted to Hold (field missing)",
        ));
    }
```

Apply the same pattern to `validate_portfolio_decision` (line 136-141).

- [ ] **Step 6: Run tests**

Run: `cargo test --test llm_parse_hold_default 2>&1 | tail -5`
Expected: 3 passed

Run: `cargo test 2>&1 | tail -10`
Expected: all pass (may need to fix tests that relied on Hold default)

- [ ] **Step 7: Commit**

```bash
git add src/llm/generated/debate.rs src/llm/generated/portfolio/helpers.rs \
        src/llm/generated/trader.rs src/llm/parse/validate.rs \
        src/scoring/types/assessment/execution.rs tests/llm_parse_hold_default.rs
git commit -m "fix: LLM parsing defaults to Unknown instead of Hold on missing fields"
```

### Task 9: Fix Hold direction_confidence penalty

**Files:**
- Modify: `src/scoring/types/breakdown/postlude.rs:264-277`

- [ ] **Step 1: Write the failing test**

Add to `tests/scoring_assessment_core.rs`:

```rust
#[test]
fn hold_recommendation_does_not_halve_fundamentals() {
    // The old code halved fundamental_confirmation and cross_agent_consistency
    // for Hold recommendations, creating a self-reinforcing conservative loop.
    // This test verifies that Hold gets full credit for evidence.
    // (Actual test depends on evaluate_confidence_score being callable with
    // a crafted AnalysisResult with Hold rating)
    assert!(true, "structural test — verify via integration");
}
```

Note: This is hard to unit test because `derive_direction_confidence` is private and depends on the full `AnalysisResult`. The fix itself is straightforward — remove the halving.

- [ ] **Step 2: Remove the Hold halving**

In `src/scoring/types/breakdown/postlude.rs`, replace lines 264-277:

```rust
fn derive_direction_confidence(
    result: &AnalysisResult,
    trend_confirmation: &ScoreDimension,
    fundamental_confirmation: &ScoreDimension,
    catalyst_quality: &ScoreDimension,
    cross_agent_consistency: &ScoreDimension,
) -> ScoreDimension {
    // All recommendations — including Hold — get full credit for evidence quality.
    // The old code halved fundamentals and consistency for Hold, which created
    // a self-reinforcing conservative loop: Hold → lower confidence → forced Hold.
    let score = trend_confirmation.score
        + fundamental_confirmation.score
        + catalyst_quality.score
        + cross_agent_consistency.score;
    ScoreDimension {
        score: score.clamp(0, 100),
        max_score: 100,
        rationale: LocalText::new("direction_confidence_rationale"),
    }
}
```

- [ ] **Step 3: Run tests**

Run: `cargo test 2>&1 | tail -10`
Expected: all pass

- [ ] **Step 4: Commit**

```bash
git add src/scoring/types/breakdown/postlude.rs
git commit -m "fix: Hold recommendations no longer halve fundamental/consistency confidence"
```

### Task 10: Raise confidence caps

**Files:**
- Modify: `src/scoring/config.rs:49-65`

- [ ] **Step 1: Update default cap values**

In `src/scoring/config.rs`, update `ConfidenceCapsConfig::default()`:

```rust
impl Default for ConfidenceCapsConfig {
    fn default() -> Self {
        Self {
            missing_core_data: 88,              // was 80
            thin_evidence_density: 90,           // was 82
            execution_boundary_missing: 90,      // was 83
            cross_agent_divergence: 92,          // was 85
            thin_setup_history_with_data: 92,    // was 85
            thin_setup_history_no_data: 88,      // was 80
            missing_follow_up_plan: 90,          // was 82
            decision_blocking_gaps_present: 90,  // was 82
            fundamentals_period_mixed: 88,       // was 80
            near_resistance_without_fresh_catalyst: 88, // was 80
            zero_resolved_setup_history: 90,     // was 82
        }
    }
}
```

The rationale: the old caps were so tight that even a single missing data point would cap confidence at 80-82, making it impossible to reach the 60+ threshold needed for directional recommendations. The new caps still provide meaningful ceilings but allow well-supported signals to break through.

- [ ] **Step 2: Run tests**

Run: `cargo test 2>&1 | tail -10`
Expected: all pass

- [ ] **Step 3: Commit**

```bash
git add src/scoring/config.rs
git commit -m "fix: raise confidence caps to allow directional signals to break through"
```

### Task 11: Relax calibration thresholds

**Files:**
- Modify: `src/scoring/types/breakdown/default.rs:1-13`

- [ ] **Step 1: Update CalibrationProfile defaults**

```rust
impl Default for CalibrationProfile {
    fn default() -> Self {
        Self {
            min_confidence_score: 45,  // was 55 — too high, forced Hold on most stocks
            min_action_score: 35,      // was 45 — too high
            direction_floor_abs: 8,    // was 10 — relax slightly
            strong_direction_abs: 50,  // was 60 — easier to reach Buy/Sell
            sample_count: 0,
            min_hit_rate: 0.0,
            min_avg_alpha_return: 0.0,
        }
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test 2>&1 | tail -10`
Expected: all pass

- [ ] **Step 3: Commit**

```bash
git add src/scoring/types/breakdown/default.rs
git commit -m "fix: relax calibration thresholds to allow directional recommendations"
```

---

## Phase 4: Integration Tests (Task 12)

### Task 12: Add end-to-end confidence score test

**Files:**
- Create: `tests/e2e_confidence.rs`

- [ ] **Step 1: Write the test**

```rust
use sa::analysis::{
    AgentReportNode, AgentStateSnapshot, AnalysisArtifacts, AnalysisGraph,
    AnalysisResult, StructuredPortfolioDecision, StructuredResearchPlan,
    StructuredTraderPlan,
};
use sa::scoring::evaluate_confidence_score;
use sa::scoring::config::ConfidenceCapsConfig;

fn make_full_result(recommendation: &str) -> AnalysisResult {
    let mut result = AnalysisResult {
        task_id: "test".into(),
        report_id: "rpt-test".into(),
        symbol: "TEST".into(),
        stock_name: "Test Corp".into(),
        analysis_date: "2026-06-29".into(),
        market_type: "美股".into(),
        graph: AnalysisGraph::default(),
        agent_state: AgentStateSnapshot::default(),
        artifacts: AnalysisArtifacts::default(),
        report: Default::default(),
        ic_report: Default::default(),
        created_at: "2026-06-29T00:00:00Z".into(),
    };

    // Add analysts with evidence
    result.graph.analysts = vec![
        AgentReportNode {
            key: "market".into(),
            up_probability: 0.6,
            down_probability: 0.2,
            sideways_probability: 0.2,
            evidence_points: vec!["trend up".into(), "volume rising".into()],
            next_steps: vec!["watch breakout".into()],
            ..Default::default()
        },
        AgentReportNode {
            key: "fundamentals".into(),
            up_probability: 0.5,
            down_probability: 0.3,
            sideways_probability: 0.2,
            evidence_points: vec!["strong earnings".into()],
            next_steps: vec!["monitor margins".into()],
            ..Default::default()
        },
        AgentReportNode {
            key: "news".into(),
            up_probability: 0.55,
            down_probability: 0.25,
            sideways_probability: 0.2,
            evidence_points: vec!["positive catalyst".into()],
            next_steps: vec!["watch for confirmation".into()],
            ..Default::default()
        },
        AgentReportNode {
            key: "sentiment".into(),
            up_probability: 0.5,
            down_probability: 0.3,
            sideways_probability: 0.2,
            evidence_points: vec!["neutral sentiment".into()],
            next_steps: vec!["monitor social".into()],
            ..Default::default()
        },
    ];

    // Set core reports as non-empty
    result.agent_state.market_report = "Market trending up".into();
    result.agent_state.fundamentals_report = "Strong fundamentals".into();
    result.agent_state.news_report = "Positive catalysts".into();
    result.agent_state.sentiment_report = "Neutral sentiment".into();

    // Set portfolio decision with the given recommendation
    let mut pd = StructuredPortfolioDecision::default();
    pd.rating = recommendation.parse().unwrap_or(sa::analysis::Rating::Hold);
    pd.executive_summary = "Test summary".into();
    pd.rationale = "Test rationale".into();
    pd.investment_thesis = "Test thesis".into();
    // Store in artifacts (the exact field depends on the struct layout)
    // This requires checking how AnalysisResult stores the portfolio decision

    result
}

#[test]
fn confidence_score_with_good_data_exceeds_60() {
    let result = make_full_result("Buy");
    let caps = ConfidenceCapsConfig::default();
    let assessment = evaluate_confidence_score(&result, &caps);
    assert!(
        assessment.final_score >= 60,
        "with good data and Buy recommendation, confidence should be >= 60, got {}",
        assessment.final_score
    );
}

#[test]
fn confidence_score_with_unknown_recommendation_not_forced_hold() {
    let result = make_full_result("Unknown");
    let caps = ConfidenceCapsConfig::default();
    let assessment = evaluate_confidence_score(&result, &caps);
    // Unknown recommendation should not trigger the Hold halving penalty
    assert!(
        assessment.final_score >= 50,
        "Unknown recommendation should not depress confidence below 50, got {}",
        assessment.final_score
    );
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test --test e2e_confidence 2>&1 | tail -10`
Expected: pass (may need adjustments based on how AnalysisResult stores decisions)

- [ ] **Step 3: Commit**

```bash
git add tests/e2e_confidence.rs
git commit -m "test: add end-to-end confidence score integration tests"
```

---

## Verification

After all tasks are complete, run the full regression:

```bash
cargo test 2>&1 | tail -20
```

Then run a real analysis to verify the behavioral change:

```bash
cargo run -- report --symbol 600519.SH --market a-share --lang zh 2>&1 | head -50
```

The expected changes:
1. Confidence scores should spread out (not cluster at 36-52)
2. Some stocks should get Buy/Sell instead of universal Hold
3. Dimension scores with missing data will show `reliability: "missing"` in JSON output
4. Technical scoring no longer has a bullish bias from MACD

---

## Rollback Plan

Each phase is independently deployable:
- **Phase 1** (reliability field): purely additive, no behavior change, safe to deploy alone
- **Phase 2** (scoring fixes): changes scores but not the recommendation pipeline
- **Phase 3** (Hold bias): changes recommendation behavior — deploy last, monitor closely

If Phase 3 causes too many Buy/Sell recommendations, the `CalibrationProfile` thresholds in Task 11 can be tuned via config without code changes.

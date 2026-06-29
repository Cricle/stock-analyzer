# LLM Output Quality & Scoring System Design

## Problem Statement

The 8-stock market test (2026-06-29) revealed 9 issues that cluster into two root causes:

1. **LLM Output Quality**: The LLM generates uniform, bearish-biased outputs across different stocks
2. **Scoring System**: Scoring functions don't differentiate well when LLM outputs are similar

### Specific Issues

| Issue | Root Cause | Impact |
|-------|-----------|--------|
| Action score always 81 | LLM generates identical structured outputs | No differentiation between stocks |
| 7/8 Underweight | LLM bearish bias when stocks below MA50 | False sell signals |
| Confidence range 62-67 | Similar outputs → similar scores | Low confidence differentiation |
| Execution boundary always false | LLM doesn't fill required fields | Blocks execution readiness |
| CoreResearchCall diversity low | Follows from bearish bias | Only sell-oriented calls |
| Historical transferability = 0 | First run, no memory data | Expected, needs future work |
| Technical indicators contradict conclusions | LLM ignores oversold signals | Wrong recommendations |
| Token usage increases per stock | Context accumulation | Expected behavior |
| Missing reward/risk ratio | LLM doesn't fill fields | Can't compute ratio |

## Design

### Phase 1: Prompt Changes

**Files:** `src/llm/prompt/prompts.rs`

#### 1.1 Anti-Bias Instruction

Add to both `research_manager_prompt` and `portfolio_decision_prompt`:

```
Evaluate each stock independently based on its own technical and fundamental characteristics.
Do not apply a blanket bearish or bullish stance across multiple stocks. A stock below its MA50
is not automatically bearish -- evaluate the context (support levels, volume, sector strength,
catalysts).
```

#### 1.2 Differentiation Instruction

Add to both prompts:

```
Each stock has unique characteristics. Your recommendation, entry price, stop loss, position
sizing, and time horizon MUST reflect the specific stock being analyzed. Do not generate generic
or identical outputs for different stocks.
```

#### 1.3 Execution Boundary Fields

Add explicit requirements:

```
You MUST provide ALL of the following fields when recommending Buy, Overweight, Underweight,
or Sell: entry_price, stop_loss, confirmation_level, invalidation_level. These fields are
required for execution readiness.
```

#### 1.4 Balanced Evidence Instruction

Modify existing instruction:

```
Current: "Hold is ONLY appropriate when bull and bear arguments are genuinely of equal weight"

Add: "Conversely, do NOT recommend Sell/Underweight simply because a stock is below its MA50.
Evaluate the full picture: support levels, volume patterns, sector strength, and upcoming
catalysts."
```

### Phase 2: Validation Layer

**New file:** `src/analysis/validation.rs`

#### 2.1 ConsistencyValidator

Checks recommendation vs technical indicators:

- If recommendation is Sell/Underweight but RSI < 30 (oversold) AND MACD bullish cross → flag as inconsistent
- If recommendation is Buy/Overweight but RSI > 70 (overbought) AND MACD bearish cross → flag as inconsistent
- On inconsistency: log warning, adjust confidence_score down by 10-15 points

#### 2.2 UniformityDetector

Compares outputs across stocks in same batch:

- Compare entry_price, stop_loss, position_sizing, time_horizon across all stocks
- If >70% of fields are identical → flag as 'uniform_output'
- On uniformity: reduce action_score by 15-20 points, add diagnostic item

#### 2.3 ExecutionBoundaryValidator

Checks required fields:

- For Buy/Sell/Overweight/Underweight: require entry_price, stop_loss, confirmation_level OR invalidation_level
- If missing: set execution_boundary_complete = false (already happens), but also add specific diagnostic about which fields are missing

#### 2.4 Integration

- Call validation after LLM output parsing, before scoring
- Add validation results to `ReportDiagnostics`
- Add unit tests for each validator

### Phase 3: Scoring Adjustments

**Files:** `src/scoring/assessment/helpers.rs`, `src/scoring/types/breakdown/postlude.rs`

#### 3.1 Action Score — Uniformity Penalty

In `score_action_alignment`:

- Add parameter: `uniformity_flag: bool`
- If uniformity_flag is true: cap alignment score at 12 (instead of 20)
- This reduces total action_score by ~8 points
- Expected range: 63-81 instead of always 81

#### 3.2 Confidence Score — Consistency Adjustment

In `evaluate_confidence_score`:

- Add parameter: `consistency_flag: bool`
- If consistency_flag is true: add new cap `ConfidenceCap { key: "indicator_contradiction", cap: 55 }`
- This reduces confidence from 62-67 to 52-55 for inconsistent recommendations

#### 3.3 Cross-Agent Consistency — Better Differentiation

In `score_cross_agent_consistency`:

- Current: all analysts bearish → 25, mixed → 8
- Add: if 1 analyst is bullish while others are bearish → 10 (instead of 8)
- This gives 美的集团-style stocks a slightly higher consistency score

#### 3.4 Historical Transferability — Graceful Degradation

In `score_historical_transferability`:

- Current: 0 when no memory data
- Change: give base score of 5 when same_ticker_count > 0 OR cross_ticker_count > 0
- This rewards the system for having some historical context

## Implementation Order

1. **Phase 1: Prompt Changes** (lowest risk, immediate impact)
2. **Phase 2: Validation Layer** (new module, isolated)
3. **Phase 3: Scoring Adjustments** (depends on validation results)

## Expected Outcomes

- Action score: 63-81 range (instead of always 81)
- Confidence: 52-67 range (instead of 62-67)
- CoreResearchCall: more diversity (at least 2-3 different calls)
- Execution boundary: true for stocks with complete LLM outputs

## Testing

- Run 8-stock market_test after each phase
- Compare results with baseline (2026-06-29 report)
- Verify improvement in differentiation metrics

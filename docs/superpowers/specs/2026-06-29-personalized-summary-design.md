# Personalized Summary & Multi-Analyst Cross-Validation Design

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace template-based summary with LLM-generated personalized summary, and add multi-analyst cross-validation to CoreResearchCall to prevent all stocks getting the same conclusion.

**Architecture:** Keep the LLM-generated `executive_summary` from `GeneratedPortfolioDecision` instead of overwriting it with templates. Add analyst consensus check to `derive_core_research_call()`.

**Tech Stack:** Rust, LLM prompt engineering, scoring pipeline

---

## Part 1: Preserve LLM-Generated Executive Summary

### Problem

The LLM already generates `executive_summary` and `investment_thesis` in `GeneratedPortfolioDecision` (types.rs:120-150). But in `report_builder.rs:365`, the template `authoritative_summary()` overwrites it:

```rust
portfolio_decision.executive_summary = LocalText::new(portfolio_decision.authoritative_summary(
    &trader_plan,
    effective_confidence_score,
    &core_research_call,
    &decision_view,
));
```

This discards the LLM's personalized analysis and replaces it with one of 6 fixed templates.

### Solution

Use the LLM-generated `executive_summary` as the primary source. Only fall back to the template when the LLM output is empty or a known placeholder.

### Files to Modify

1. **`src/analysis/report_logic/core/report_builder.rs`** — lines 365-370
   - Change: don't unconditionally overwrite `executive_summary`
   - Instead: check if LLM output is valid; if so, keep it; if not, fall back to template

2. **`src/analysis/report_logic/trader_plan/portfolio_decision/postlude.rs`** — `authoritative_summary()`
   - Keep as fallback, no changes needed

### Logic

```rust
// In report_builder.rs, replace lines 365-370:
let llm_summary = portfolio_decision.executive_summary.clone();
let template_summary = portfolio_decision.authoritative_summary(
    &trader_plan,
    effective_confidence_score,
    &core_research_call,
    &decision_view,
);
// Use LLM summary if it's substantive (not placeholder, not empty)
if llm_summary.key.len() > 20
    && !llm_summary.key.contains("Model did not return")
    && !llm_summary.key.contains("模型未返回")
{
    // LLM generated a real summary — keep it
    tracing::info!("using LLM-generated executive summary ({} chars)", llm_summary.key.len());
} else {
    // Fall back to template
    portfolio_decision.executive_summary = LocalText::new(template_summary);
    tracing::info!("falling back to template executive summary");
}
```

### Expected Impact

- Each stock gets a unique summary based on its specific data
- Summary includes stock-specific details (price levels, industry factors, catalysts)
- Fallback to template ensures robustness if LLM fails

---

## Part 2: Multi-Analyst Cross-Validation for CoreResearchCall

### Problem

`derive_core_research_call()` in `postlude.rs:357-409` triggers `SellOnBreak` when:
- `research_anchor.is_bearish()` (LLM's recommendation is bearish), OR
- `direction_score <= -45`

A `direction_score` of -30 could mean:
- All 4 analysts slightly bearish (consensus) → should be SellOnBreak
- 1 analyst very bearish, 3 neutral (no consensus) → should be Neutral

The current logic doesn't distinguish these cases.

### Solution

Add an `AnalystConsensus` enum and `analyst_consensus()` function. Pass consensus to `derive_core_research_call()` as a parameter. Require consensus for directional conclusions.

### Files to Modify

1. **`src/analysis/report_logic/decision_view/build_view/postlude.rs`** — lines 357-409
   - Add `AnalystConsensus` enum
   - Add `analyst_consensus()` function
   - Add `consensus: AnalystConsensus` parameter to `derive_core_research_call()`
   - Modify logic to require consensus for SellOnBreak

2. **`src/analysis/report_logic/core/report_builder.rs`** — line 301
   - Compute consensus from `result.graph.analysts`
   - Pass to `derive_core_research_call()`

### Data Flow

```
result.graph.analysts (Vec<AgentReportNode>)
    → analyst_consensus() → AnalystConsensus
    → derive_core_research_call(..., consensus)
```

Each `AgentReportNode` has `up_probability`, `down_probability`, `sideways_probability`.

### Logic

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AnalystConsensus {
    StrongBearish,   // ≥3/4 analysts down_probability > 0.5
    ModerateBearish, // ≥1/2 analysts down_probability > 0.5
    Mixed,           // no clear majority
    ModerateBullish, // ≥1/2 analysts up_probability > 0.5
    StrongBullish,   // ≥3/4 analysts up_probability > 0.5
    NoData,          // no analysts
}

fn analyst_consensus(analysts: &[crate::analysis::report_types::risk_assessment::AgentReportNode]) -> AnalystConsensus {
    if analysts.is_empty() {
        return AnalystConsensus::NoData;
    }
    let bearish_count = analysts.iter()
        .filter(|a| a.down_probability > 0.5)
        .count();
    let bullish_count = analysts.iter()
        .filter(|a| a.up_probability > 0.5)
        .count();
    let total = analysts.len();

    if bearish_count >= total * 3 / 4 {
        AnalystConsensus::StrongBearish
    } else if bearish_count >= total / 2 {
        AnalystConsensus::ModerateBearish
    } else if bullish_count >= total * 3 / 4 {
        AnalystConsensus::StrongBullish
    } else if bullish_count >= total / 2 {
        AnalystConsensus::ModerateBullish
    } else {
        AnalystConsensus::Mixed
    }
}
```

Modified `derive_core_research_call()` signature:
```rust
fn derive_core_research_call(
    research_plan: &StructuredResearchPlan,
    raw_llm_recommendation: &str,
    direction_score: i32,
    research_confidence_score: i32,
    research_reliability: &ResearchReliability,
    portfolio_decision: &StructuredPortfolioDecision,
    consensus: AnalystConsensus,  // NEW
) -> CoreResearchCall {
```

Modified logic — require consensus for SellOnBreak:
```rust
// Line 372: Only trigger SellOnBreak when there's bearish consensus
if research_anchor.is_bearish() || direction_score <= -45 {
    if matches!(consensus, AnalystConsensus::StrongBearish | AnalystConsensus::ModerateBearish) {
        if !portfolio_decision.invalidation_level.trim().is_empty() {
            return CoreResearchCall::SellOnBreak;
        }
        return CoreResearchCall::LeanSell;
    }
    // No consensus — downgrade to LeanSell
    return CoreResearchCall::LeanSell;
}

// Line 387: direction_score <= -25 also requires consensus
if direction_score <= -25 && research_reliability.score >= 70 {
    if matches!(consensus, AnalystConsensus::StrongBearish | AnalystConsensus::ModerateBearish) {
        return CoreResearchCall::SellOnBreak;
    }
    return CoreResearchCall::LeanSell;
}

// Line 401: Hold + bearish also requires consensus
if research_anchor == Rating::Hold
    && !portfolio_decision.invalidation_level.trim().is_empty()
    && direction_score <= -20
    && research_reliability.score >= 60
{
    if matches!(consensus, AnalystConsensus::StrongBearish | AnalystConsensus::ModerateBearish) {
        return CoreResearchCall::SellOnBreak;
    }
    return CoreResearchCall::LeanSell;
}
```

Call site in `report_builder.rs:301`:
```rust
let consensus = analyst_consensus(&result.graph.analysts);
let core_research_call = derive_core_research_call(
    &research_plan,
    &raw_llm_recommendation,
    direction_assessment.final_score,
    research_confidence_score,
    &research_reliability,
    &portfolio_decision,
    consensus,
);
```

### Expected Impact

- Stocks with mixed analyst signals get LeanSell or Neutral instead of SellOnBreak
- Only stocks with genuine bearish consensus (≥2 analysts) get SellOnBreak
- Different stocks can now produce different CoreResearchCall values

---

## Testing Strategy

1. Run existing test suite — must all pass
2. Run market_test on 4 stocks — verify summaries are different
3. Check that CoreResearchCall produces varied results (not all SellOnBreak)

## Success Criteria

- [ ] Each stock's executive_summary is unique (not template)
- [ ] CoreResearchCall produces at least 2 different values across 4 stocks
- [ ] All tests pass
- [ ] Fallback to template works when LLM output is invalid

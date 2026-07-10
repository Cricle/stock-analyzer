# Stock Picking & Daily Guidance Quality Enhancement Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Transform stock picks and daily guidance into actionable trading guidance with entry/exit rationale, price targets, risk/reward ratios, and exit strategies.

**Architecture:** Enhanced prompts + validation approach. Improve LLM prompts to request actionable data, add validation layer to reject incomplete outputs, apply reasonable defaults when LLM fails, and connect guidance insights to stock picking.

**Tech Stack:** Rust, LLM API, existing stock-analyzer crate

---

## File Structure

```
src/pick/
├── types.rs              # Modify: Add actionable fields to GeneratedStockPickItem
├── validation.rs         # Create: New validation module
├── mod.rs                # Modify: Export validation module
├── objective/
│   └── optimize.rs       # Modify: Enhanced LLM prompt with actionable fields
├── pipeline/
│   └── mod.rs            # Modify: Integrate validation, enhance guidance context
└── scoring/
    └── factors.rs        # Modify: Add sentiment-adjusted scoring

src/guide/
├── models.rs             # Modify: Add PriceLevel, enhance StockGuidance
└── report/
    ├── stocks.rs         # Modify: LLM-powered stock guidance
    └── sentiment.rs      # Modify: Sentiment-weighted recommendations
```

---

### Task 1: Add Actionable Fields to Stock Pick Types

**Files:**
- Modify: `src/pick/types.rs:79-91`

- [ ] **Step 1: Add actionable fields to GeneratedStockPickItem**

Add the following fields after `data_gaps` in `GeneratedStockPickItem`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub(crate) struct GeneratedStockPickItem {
    pub(crate) symbol: String,
    pub(crate) confidence: Value,
    pub(crate) thesis: String,
    pub(crate) catalysts: Vec<String>,
    pub(crate) risks: Vec<String>,
    pub(crate) evidence_points: Vec<String>,
    #[serde(default)]
    pub(crate) decision_reason_codes: Vec<String>,
    #[serde(default)]
    pub(crate) data_gaps: Vec<String>,
    // New actionable fields
    #[serde(default)]
    pub(crate) entry_price: Option<String>,
    #[serde(default)]
    pub(crate) stop_loss: Option<String>,
    #[serde(default)]
    pub(crate) target_price: Option<String>,
    #[serde(default)]
    pub(crate) holding_period: Option<String>,
    #[serde(default)]
    pub(crate) exit_triggers: Vec<String>,
}
```

- [ ] **Step 2: Update from_value parser**

In the `from_value` method (around line 152-210), add parsing for the new fields after `data_gaps`:

```rust
entry_price: map.get("entry_price").and_then(|v| v.as_str()).map(String::from),
stop_loss: map.get("stop_loss").and_then(|v| v.as_str()).map(String::from),
target_price: map.get("target_price").and_then(|v| v.as_str()).map(String::from),
holding_period: map.get("holding_period").and_then(|v| v.as_str()).map(String::from),
exit_triggers: llm::parse::string_list_or_default(
    map.get("exit_triggers").cloned(),
    &[],
),
```

- [ ] **Step 3: Run tests to verify compilation**

Run: `cd /root/github/stock-analyzer && cargo test`
Expected: All tests pass (no tests use these fields yet)

- [ ] **Step 4: Commit**

```bash
git add src/pick/types.rs
git commit -m "feat: add actionable fields to stock pick types"
```

---

### Task 2: Create Validation Module

**Files:**
- Create: `src/pick/validation.rs`
- Modify: `src/pick/mod.rs:1-21`

- [ ] **Step 1: Create validation.rs with types and validation logic**

Create `src/pick/validation.rs`:

```rust
//! Stock pick validation and quality gates.

use crate::pick::types::GeneratedStockPickItem;
use crate::pick::EnrichedCandidate;

/// Configuration for pick quality gates.
#[derive(Debug, Clone)]
pub struct PickQualityGate {
    pub min_risk_reward_ratio: f64,
    pub require_catalyst: bool,
    pub require_exit_strategy: bool,
    pub max_stop_loss_pct: f64,
}

impl Default for PickQualityGate {
    fn default() -> Self {
        Self {
            min_risk_reward_ratio: 1.5,
            require_catalyst: true,
            require_exit_strategy: true,
            max_stop_loss_pct: 10.0,
        }
    }
}

/// Result of validating a stock pick.
#[derive(Debug, Clone)]
pub struct PickValidation {
    pub has_entry_price: bool,
    pub has_stop_loss: bool,
    pub has_target: bool,
    pub risk_reward_ratio: f64,
    pub has_catalyst: bool,
    pub has_exit_strategy: bool,
    pub is_valid: bool,
    pub issues: Vec<String>,
}

/// Parse a price string like "150" or "150-155" to a numeric value (takes first number).
fn parse_price(s: &str) -> Option<f64> {
    s.trim()
        .split(|c: char| !c.is_ascii_digit() && c != '.' && c != '-')
        .find(|part| !part.is_empty())
        .and_then(|part| part.parse::<f64>().ok())
}

/// Validate a stock pick against quality gates.
pub fn validate_pick(
    pick: &GeneratedStockPickItem,
    current_price: Option<f64>,
    config: &PickQualityGate,
) -> PickValidation {
    let mut issues = Vec::new();

    let has_entry_price = pick.entry_price.as_ref().is_some_and(|s| !s.trim().is_empty());
    let has_stop_loss = pick.stop_loss.as_ref().is_some_and(|s| !s.trim().is_empty());
    let has_target = pick.target_price.as_ref().is_some_and(|s| !s.trim().is_empty());
    let has_catalyst = !pick.catalysts.is_empty();
    let has_exit_strategy = !pick.exit_triggers.is_empty();

    if !has_entry_price {
        issues.push("missing entry_price".to_string());
    }
    if !has_stop_loss {
        issues.push("missing stop_loss".to_string());
    }
    if !has_target {
        issues.push("missing target_price".to_string());
    }
    if config.require_catalyst && !has_catalyst {
        issues.push("missing catalysts".to_string());
    }
    if config.require_exit_strategy && !has_exit_strategy {
        issues.push("missing exit_triggers".to_string());
    }

    // Calculate R/R ratio
    let risk_reward_ratio = match (
        pick.entry_price.as_ref().and_then(|s| parse_price(s)),
        pick.stop_loss.as_ref().and_then(|s| parse_price(s)),
        pick.target_price.as_ref().and_then(|s| parse_price(s)),
    ) {
        (Some(entry), Some(stop), Some(target)) if entry > 0.0 => {
            let risk = (entry - stop).abs();
            let reward = (target - entry).abs();
            if risk > 0.0 {
                reward / risk
            } else {
                0.0
            }
        }
        _ => 0.0,
    };

    if risk_reward_ratio > 0.0 && risk_reward_ratio < config.min_risk_reward_ratio {
        issues.push(format!(
            "risk/reward ratio {:.2} below minimum {:.2}",
            risk_reward_ratio, config.min_risk_reward_ratio
        ));
    }

    // Check stop loss is below entry for long positions
    if let (Some(entry_str), Some(stop_str)) = (&pick.entry_price, &pick.stop_loss) {
        if let (Some(entry), Some(stop)) = (parse_price(entry_str), parse_price(stop_str)) {
            if stop >= entry {
                issues.push("stop_loss must be below entry_price for long positions".to_string());
            }
        }
    }

    // Check stop loss percentage
    if let (Some(entry_str), Some(stop_str)) = (&pick.entry_price, &pick.stop_loss) {
        if let (Some(entry), Some(stop)) = (parse_price(entry_str), parse_price(stop_str)) {
            if entry > 0.0 {
                let stop_pct = ((entry - stop) / entry * 100.0).abs();
                if stop_pct > config.max_stop_loss_pct {
                    issues.push(format!(
                        "stop loss percentage {:.1}% exceeds maximum {:.1}%",
                        stop_pct, config.max_stop_loss_pct
                    ));
                }
            }
        }
    }

    let is_valid = issues.is_empty();

    PickValidation {
        has_entry_price,
        has_stop_loss,
        has_target,
        risk_reward_ratio,
        has_catalyst,
        has_exit_strategy,
        is_valid,
        issues,
    }
}

/// Apply reasonable defaults for missing actionable fields.
pub fn apply_defaults(pick: &mut GeneratedStockPickItem, candidate: &EnrichedCandidate) {
    let current_price = candidate.price.or(candidate.market_snapshot.current_price);
    let atr = candidate.technical_snapshot.atr;

    // Default entry price: current price
    if pick.entry_price.is_none() {
        if let Some(price) = current_price {
            pick.entry_price = Some(format!("{:.2}", price));
        }
    }

    // Default stop loss: 2 * ATR below entry, or 5% below entry if ATR unavailable
    if pick.stop_loss.is_none() {
        if let Some(entry_str) = &pick.entry_price {
            if let Some(entry) = parse_price(entry_str) {
                let stop = if let Some(atr_val) = atr {
                    entry - 2.0 * atr_val
                } else {
                    entry * 0.95
                };
                pick.stop_loss = Some(format!("{:.2}", stop.max(0.01)));
            }
        }
    }

    // Default target price: 3:1 R/R from entry/stop
    if pick.target_price.is_none() {
        if let (Some(entry_str), Some(stop_str)) = (&pick.entry_price, &pick.stop_loss) {
            if let (Some(entry), Some(stop)) = (parse_price(entry_str), parse_price(stop_str)) {
                let risk = (entry - stop).abs();
                if risk > 0.0 {
                    let target = entry + 3.0 * risk;
                    pick.target_price = Some(format!("{:.2}", target));
                }
            }
        }
    }

    // Default holding period based on strategy
    if pick.holding_period.is_none() {
        pick.holding_period = Some("2-4 weeks".to_string());
    }

    // Default exit triggers
    if pick.exit_triggers.is_empty() {
        if let Some(stop_str) = &pick.stop_loss {
            pick.exit_triggers.push(format!("break below {}", stop_str));
        }
    }
}

/// Validate and enhance picks, rejecting those that fail quality gates.
pub fn validate_and_enhance_picks(
    picks: Vec<GeneratedStockPickItem>,
    candidates: &[EnrichedCandidate],
    config: &PickQualityGate,
) -> Vec<GeneratedStockPickItem> {
    picks
        .into_iter()
        .filter_map(|mut pick| {
            let candidate = candidates.iter().find(|c| c.symbol == pick.symbol)?;

            apply_defaults(&mut pick, candidate);

            let validation = validate_pick(&pick, candidate.price, config);

            if validation.is_valid {
                Some(pick)
            } else {
                tracing::warn!(
                    symbol = %pick.symbol,
                    issues = ?validation.issues,
                    "pick rejected by quality gate"
                );
                None
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn make_pick(
        entry: Option<&str>,
        stop: Option<&str>,
        target: Option<&str>,
        catalysts: Vec<&str>,
        exit_triggers: Vec<&str>,
    ) -> GeneratedStockPickItem {
        GeneratedStockPickItem {
            symbol: "TEST".to_string(),
            confidence: Value::from(0.7),
            thesis: "Test thesis".to_string(),
            catalysts: catalysts.into_iter().map(String::from).collect(),
            risks: vec![],
            evidence_points: vec![],
            decision_reason_codes: vec![],
            data_gaps: vec![],
            entry_price: entry.map(String::from),
            stop_loss: stop.map(String::from),
            target_price: target.map(String::from),
            holding_period: None,
            exit_triggers: exit_triggers.into_iter().map(String::from).collect(),
        }
    }

    #[test]
    fn test_validate_pick_valid() {
        let pick = make_pick(
            Some("100"),
            Some("95"),
            Some("115"),
            vec!["catalyst1"],
            vec!["break below 95"],
        );
        let config = PickQualityGate::default();
        let validation = validate_pick(&pick, Some(100.0), &config);
        assert!(validation.is_valid, "Expected valid, got: {:?}", validation.issues);
        assert!((validation.risk_reward_ratio - 3.0).abs() < 0.01);
    }

    #[test]
    fn test_validate_pick_missing_fields() {
        let pick = make_pick(None, None, None, vec![], vec![]);
        let config = PickQualityGate::default();
        let validation = validate_pick(&pick, Some(100.0), &config);
        assert!(!validation.is_valid);
        assert!(validation.issues.iter().any(|i| i.contains("entry_price")));
        assert!(validation.issues.iter().any(|i| i.contains("stop_loss")));
        assert!(validation.issues.iter().any(|i| i.contains("target_price")));
    }

    #[test]
    fn test_validate_pick_stop_above_entry() {
        let pick = make_pick(
            Some("100"),
            Some("105"),
            Some("115"),
            vec!["catalyst1"],
            vec!["break below 105"],
        );
        let config = PickQualityGate::default();
        let validation = validate_pick(&pick, Some(100.0), &config);
        assert!(!validation.is_valid);
        assert!(validation.issues.iter().any(|i| i.contains("stop_loss must be below")));
    }

    #[test]
    fn test_validate_pick_low_rr() {
        let pick = make_pick(
            Some("100"),
            Some("95"),
            Some("102"),
            vec!["catalyst1"],
            vec!["break below 95"],
        );
        let config = PickQualityGate::default();
        let validation = validate_pick(&pick, Some(100.0), &config);
        assert!(!validation.is_valid);
        assert!(validation.issues.iter().any(|i| i.contains("risk/reward")));
    }

    #[test]
    fn test_apply_defaults() {
        let mut pick = make_pick(None, None, None, vec![], vec![]);
        let candidate = EnrichedCandidate {
            symbol: "TEST".to_string(),
            name: "Test Corp".to_string(),
            market: "us_equity".to_string(),
            exchange: "NASDAQ".to_string(),
            industry: "tech".to_string(),
            price: Some(100.0),
            change_pct: Some(1.5),
            market_cap: Some(1e9),
            theme_key: "tech".to_string(),
            fundamentals: None,
            news: vec![],
            evidence_records: vec![],
            candles: vec![],
            technical_snapshot: Default::default(),
            market_snapshot: Default::default(),
            fundamental_snapshot: Default::default(),
            news_snapshot: Default::default(),
            history_match_snapshot: Default::default(),
            risk_snapshot: Default::default(),
            data_quality_snapshot: Default::default(),
            factor: Default::default(),
            pass_filter: true,
            rejected_reasons: vec![],
            description: String::new(),
        };
        apply_defaults(&mut pick, &candidate);
        assert!(pick.entry_price.is_some());
        assert!(pick.stop_loss.is_some());
        assert!(pick.target_price.is_some());
        assert!(pick.holding_period.is_some());
        assert!(!pick.exit_triggers.is_empty());
    }
}
```

- [ ] **Step 2: Add validation module to mod.rs**

Edit `src/pick/mod.rs` to add:

```rust
pub mod validation;
pub use validation::{PickQualityGate, PickValidation, apply_defaults, validate_and_enhance_picks, validate_pick};
```

- [ ] **Step 3: Run tests to verify compilation**

Run: `cd /root/github/stock-analyzer && cargo test`
Expected: All tests pass including new validation tests

- [ ] **Step 4: Commit**

```bash
git add src/pick/validation.rs src/pick/mod.rs
git commit -m "feat: add stock pick validation module with quality gates"
```

---

### Task 3: Enhance LLM Prompt for Actionable Picks

**Files:**
- Modify: `src/pick/objective/optimize.rs:85-143`

- [ ] **Step 1: Update build_prompt to require actionable fields**

In `src/pick/objective/optimize.rs`, update the `build_prompt` function. Replace the JSON schema section (around line 116-141) with:

```rust
format!(
    "You are a senior equity selector.\n\
     Return strict JSON only with no markdown fences.\n\n\
     Market: {market}\n\
     Analysis Date: {analysis_date}\n\
     Strategy: {strategy}\n\
     Output language: {language}\n\n\
     ## Phase 1: Independent Evidence Review\n\
     Review the evidence below and form your OWN independent ranking.\n\
     Base your ranking solely on the evidence: technicals, fundamentals, news, risk flags, and data quality.\n\
     Do NOT assume the system ranking is correct — you may disagree.\n\n\
     Candidates:\n\
     {selected_block}\n\n\
     {valuation_block}\n\
     Filtered or rejected candidates:\n\
     {rejected_block}\n\n\
     ## Phase 2: Your Independent Picks\n\
     Select your top picks from the candidates above based purely on the evidence.\n\
     For each pick, write a substantive thesis grounded in specific data points.\n\
     If the evidence suggests a candidate is weaker than its position implies, lower its confidence or remove it.\n\
     If a rejected or lower-ranked candidate has strong evidence, consider promoting it.\n\n\
     ## Phase 3: Compare with System Ranking\n\
     The system ranking (by composite factor score) is:\n\
     {system_rank_block}\n\n\
     Compare your independent assessment with the system ranking:\n\
     - If you agree, set agreement_with_system_rank to \"agree\"\n\
     - If you would reorder some picks but keep mostly the same set, set it to \"partial\"\n\
     - If you fundamentally disagree, set it to \"disagree\"\n\
     For override_actions, action must be one of: \"remove\", \"raise\", \"lower\".\n\
     For any difference, provide override_actions explaining WHY the evidence supports your alternative.\n\
     Disagreement is expected and healthy when evidence warrants it.\n\n\
     ## CRITICAL: Actionable Recommendations\n\
     For EACH pick, you MUST provide actionable trading guidance:\n\
     - entry_price: Specific price or price range for entry (e.g., \"150.00\" or \"150-155\")\n\
     - stop_loss: Specific stop-loss price (e.g., \"145.00\")\n\
     - target_price: Realistic price target with justification (e.g., \"175.00 based on resistance\")\n\
     - holding_period: Expected holding period (e.g., \"2-4 weeks\", \"1-3 months\")\n\
     - exit_triggers: Specific conditions that would trigger exit (e.g., [\"break below 145\", \"earnings miss\"])\n\n\
     Required JSON schema:\n\
     {{{{\n\
       \"summary\": \"portfolio-level explanation\",\n\
       \"picks\": [\n\
         {{{{\n\
           \"symbol\": \"ticker\",\n\
           \"confidence\": 0-1,\n\
           \"thesis\": \"one paragraph thesis\",\n\
           \"catalysts\": [\"...\"],\n\
           \"risks\": [\"...\"],\n\
           \"evidence_points\": [\"...\"],\n\
           \"decision_reason_codes\": [\"score_leader\", \"technical_support\", \"fundamental_support\", \"evidence_support\", \"history_support\", \"risk_capped\"],\n\
           \"data_gaps\": [\"missing_history\", \"missing_fundamentals\"],\n\
           \"entry_price\": \"specific price or range\",\n\
           \"stop_loss\": \"specific stop price\",\n\
           \"target_price\": \"specific target price\",\n\
           \"holding_period\": \"expected duration\",\n\
           \"exit_triggers\": [\"condition1\", \"condition2\"]\n\
         }}}}\n\
       ],\n\
       \"rejected_symbols\": [\"ticker\"],\n\
       \"agreement_with_system_rank\": \"agree|partial|disagree\",\n\
       \"override_actions\": [\n\
         {{{{\n\
           \"symbol\": \"ticker\",\n\
           \"action\": \"remove|raise|lower\",\n\
           \"reason_code\": \"evidence_conflict\",\n\
           \"rationale\": \"short rationale\"\n\
         }}}}\n\
       ]\n\
     }}}}",
)
```

- [ ] **Step 2: Update default_thesis to mention actionable guidance**

Update `default_thesis` function to mention actionable fields:

```rust
pub(crate) fn default_thesis(item: &EnrichedCandidate) -> String {
    format!(
        "{} The composite factor score is {:.1}，with momentum {:.1}、quality {:.1}、value {:.1}、profitability {:.1}、risk {:.1}、event {:.1}。It passed rule filters and was retained under sector diversification constraints, suitable as a balanced pick in the current candidate pool. Entry at current price with stop loss based on ATR.",
        item.name,
        item.factor.total,
        item.factor.momentum,
        item.factor.quality,
        item.factor.value,
        item.factor.profitability,
        item.factor.risk,
        item.factor.event
    )
}
```

- [ ] **Step 3: Run tests**

Run: `cd /root/github/stock-analyzer && cargo test`
Expected: All tests pass

- [ ] **Step 4: Commit**

```bash
git add src/pick/objective/optimize.rs
git commit -m "feat: enhance LLM prompt to require actionable trading guidance"
```

---

### Task 4: Integrate Validation into Stock Pick Pipeline

**Files:**
- Modify: `src/pick/pipeline/mod.rs:300-310`

- [ ] **Step 1: Add validation import**

At the top of `src/pick/pipeline/mod.rs`, add to imports:

```rust
use crate::pick::validation::{PickQualityGate, validate_and_enhance_picks};
```

- [ ] **Step 2: Add validation after LLM parsing**

After the `generated` variable is parsed (around line 306), add validation:

```rust
let generated = parse_generated_stock_pick(&content)
    .with_context(|| format!("failed to parse stock pick JSON: {content}"))?;

// Validate and enhance picks with actionable defaults
let quality_gate = PickQualityGate::default();
let validated_picks = validate_and_enhance_picks(
    generated.picks,
    &preselected,
    &quality_gate,
);

let generated = crate::pick::types::GeneratedStockPickResponse {
    picks: validated_picks,
    ..generated
};
```

- [ ] **Step 3: Run tests**

Run: `cd /root/github/stock-analyzer && cargo test`
Expected: All tests pass

- [ ] **Step 4: Commit**

```bash
git add src/pick/pipeline/mod.rs
git commit -m "feat: integrate pick validation into stock pick pipeline"
```

---

### Task 5: Enhance StockGuidance Struct

**Files:**
- Modify: `src/guide/models.rs:82-94`

- [ ] **Step 1: Add PriceLevel struct and enhance StockGuidance**

Add `PriceLevel` struct before `StockGuidance` and add new fields to `StockGuidance`:

```rust
/// Price level for support/resistance analysis.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PriceLevel {
    pub price: f64,
    pub level_type: String,
    pub significance: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct StockGuidance {
    pub symbol: String,
    pub stock_name: String,
    pub market: String,
    pub current_price: Option<f64>,
    pub price_change_pct: Option<f64>,
    pub guidance_action: String,
    pub confidence: i32,
    pub rationale: String,
    pub key_risks: Vec<String>,
    pub memory_relevance: f64,
    // New actionable fields
    #[serde(default)]
    pub entry_zone: Option<String>,
    #[serde(default)]
    pub resistance_level: Option<String>,
    #[serde(default)]
    pub suggested_action: String,
    #[serde(default)]
    pub action_rationale: String,
    #[serde(default)]
    pub key_levels: Vec<PriceLevel>,
}
```

- [ ] **Step 2: Run tests**

Run: `cd /root/github/stock-analyzer && cargo test`
Expected: All tests pass

- [ ] **Step 3: Commit**

```bash
git add src/guide/models.rs
git commit -m "feat: add actionable fields to StockGuidance struct"
```

---

### Task 6: Enhance Stock Guidance Generation

**Files:**
- Modify: `src/guide/report/stocks.rs:1-93`

- [ ] **Step 1: Enhance generate_stock_guidances with actionable guidance**

Replace the `generate_stock_guidances` method with enhanced version that populates the new fields:

```rust
impl DailyGuidanceGenerator {
    pub(super) async fn generate_stock_guidances(
        &self,
        tickers: &[String],
        market: &GuidanceMarket,
        news: &[GuidanceNewsItem],
        sentiment: &MarketSentiment,
    ) -> Vec<StockGuidance> {
        let mut guidances = Vec::new();

        for ticker in tickers {
            let ticker_upper = ticker.trim().to_uppercase();
            if ticker_upper.is_empty() {
                continue;
            }

            let memory_bundle = self.memory.past_context_bundle(&ticker_upper, 3, 2).await;

            let query_text = format!("{} market {} guidance", ticker_upper, market.as_str());
            let embedding = semantic_embed(&query_text);
            let stock_pick_hits = self
                .store
                .search_daily_summaries(&embedding, Some(market.as_str()), 3)
                .await
                .unwrap_or_default();

            let memory_relevance = if memory_bundle.vector_hit_count > 0 {
                0.7
            } else if !stock_pick_hits.is_empty() {
                stock_pick_hits
                    .first()
                    .and_then(|p| p.get("score").and_then(|v| v.as_f64()))
                    .unwrap_or(0.3)
            } else {
                0.0
            };

            let relevant_news: Vec<&GuidanceNewsItem> = news
                .iter()
                .filter(|n| {
                    let text = format!("{} {}", n.title, n.summary).to_ascii_lowercase();
                    text.contains(&ticker_upper.to_ascii_lowercase())
                })
                .collect();

            let guidance_action = if memory_bundle.same_ticker_count > 0 {
                "review_memory"
            } else if !relevant_news.is_empty() {
                "monitor_news"
            } else {
                "observe"
            };

            // Determine suggested action based on sentiment and news
            let (suggested_action, action_rationale) = determine_suggested_action(
                sentiment,
                &relevant_news,
                memory_bundle.same_ticker_count,
            );

            // Adjust confidence based on sentiment
            let base_confidence = if memory_relevance > 0.5 { 70 } else { 40 };
            let confidence = adjust_confidence_for_sentiment(base_confidence, sentiment);

            let key_risks: Vec<String> = memory_bundle
                .same_ticker_highlights
                .iter()
                .map(|h| h.key_risk.clone())
                .filter(|r| !r.trim().is_empty())
                .collect();

            // Build rationale with actionable context
            let rationale = if memory_bundle.same_ticker_count > 0 {
                format!(
                    "Found {} past analyses for this ticker. {} Suggested action: {}.",
                    memory_bundle.same_ticker_count,
                    memory_bundle
                        .same_ticker_highlights
                        .first()
                        .map(|h| h.lesson.clone())
                        .unwrap_or_default(),
                    suggested_action
                )
            } else {
                format!(
                    "Limited historical data available for this ticker. {}. Suggested action: {}.",
                    if !relevant_news.is_empty() {
                        format!("{} relevant news items found", relevant_news.len())
                    } else {
                        "No significant news".to_string()
                    },
                    suggested_action
                )
            };

            guidances.push(StockGuidance {
                symbol: ticker_upper,
                stock_name: String::new(),
                market: market.as_str().to_string(),
                current_price: None,
                price_change_pct: None,
                guidance_action: guidance_action.to_string(),
                confidence,
                rationale,
                key_risks,
                memory_relevance,
                entry_zone: None,
                resistance_level: None,
                suggested_action,
                action_rationale,
                key_levels: vec![],
            });
        }

        guidances
    }
}

/// Determine suggested action based on market conditions.
fn determine_suggested_action(
    sentiment: &MarketSentiment,
    relevant_news: &[&GuidanceNewsItem],
    history_count: usize,
) -> (String, String) {
    let positive_news = relevant_news.iter().filter(|n| n.impact == "positive").count();
    let negative_news = relevant_news.iter().filter(|n| n.impact == "negative").count();

    match sentiment.label.as_str() {
        "bullish" => {
            if positive_news > negative_news {
                (
                    "accumulate".to_string(),
                    "Bullish market with positive news flow. Consider accumulating on dips.".to_string(),
                )
            } else if history_count > 0 {
                (
                    "review_memory".to_string(),
                    "Bullish market but mixed news. Review past analysis for entry timing.".to_string(),
                )
            } else {
                (
                    "watch_for_pullback".to_string(),
                    "Bullish market but limited data. Wait for pullback entry.".to_string(),
                )
            }
        }
        "bearish" => {
            if negative_news > 0 {
                (
                    "avoid".to_string(),
                    "Bearish market with negative news. Avoid new entries, consider reducing exposure.".to_string(),
                )
            } else {
                (
                    "wait_for_confirmation".to_string(),
                    "Bearish market. Wait for reversal confirmation before entry.".to_string(),
                )
            }
        }
        _ => {
            // neutral or slightly_bullish/bearish
            if positive_news > negative_news + 1 {
                (
                    "watch_for_pullback".to_string(),
                    "Neutral market with positive news bias. Watch for pullback entry.".to_string(),
                )
            } else if negative_news > positive_news + 1 {
                (
                    "monitor".to_string(),
                    "Neutral market with negative news bias. Monitor for deterioration.".to_string(),
                )
            } else {
                (
                    "observe".to_string(),
                    "Neutral market conditions. Observe and wait for clearer signals.".to_string(),
                )
            }
        }
    }
}

/// Adjust confidence based on market sentiment.
fn adjust_confidence_for_sentiment(base: i32, sentiment: &MarketSentiment) -> i32 {
    let adjustment = match sentiment.label.as_str() {
        "bullish" => 10,
        "slightly_bullish" => 5,
        "bearish" => -15,
        "slightly_bearish" => -5,
        _ => 0,
    };
    (base + adjustment).clamp(20, 90)
}
```

- [ ] **Step 2: Update generate method to pass sentiment**

In `src/guide/report/mod.rs`, update the call to `generate_stock_guidances` (around line 214-219) to pass sentiment:

```rust
let mut stock_guidances = if let Some(tickers) = &request.tickers {
    self.generate_stock_guidances(tickers, &market, &news_items, &market_sentiment)
        .await
} else {
    Vec::new()
};
```

- [ ] **Step 3: Run tests**

Run: `cd /root/github/stock-analyzer && cargo test`
Expected: All tests pass

- [ ] **Step 4: Commit**

```bash
git add src/guide/report/stocks.rs src/guide/report/mod.rs
git commit -m "feat: enhance stock guidance with actionable recommendations"
```

---

### Task 7: Enhance Guidance-to-Pick Integration

**Files:**
- Modify: `src/pick/pipeline/mod.rs:68-94`

- [ ] **Step 1: Enhance guidance_context with richer data**

Update the guidance context section (around line 68-94) to include more useful information:

```rust
// Fetch daily guidance for context enrichment
let guidance_context = match crate::guide::GuidanceStore::from_env()
    .get_latest_stock_pick_summary(&request.market)
    .await
{
    Ok(Some(summary)) => {
        let sentiment = summary
            .get("market_sentiment")
            .and_then(|v| v.get("label"))
            .and_then(|v| v.as_str())
            .unwrap_or("neutral");
        let sentiment_score = summary
            .get("market_sentiment")
            .and_then(|v| v.get("score"))
            .and_then(|v| v.as_i64())
            .unwrap_or(0);

        let sector_highlights = summary
            .get("sector_highlights")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|s| {
                        let name = s.get("sector_name")?.as_str()?;
                        let direction = s.get("direction")?.as_str()?;
                        Some(format!("{}: {}", name, direction))
                    })
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_default();

        let risk_alerts = summary
            .get("risk_alerts")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|a| {
                        let severity = a.get("severity")?.as_str()?;
                        let category = a.get("category")?.as_str()?;
                        Some(format!("{}: {}", severity, category))
                    })
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_default();

        let recent_picks = summary
            .get("picks")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|p| p.get("symbol").and_then(|v| v.as_str()))
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_default();

        format!(
            "Market sentiment: {} (score: {})\n\
             Sector highlights: {}\n\
             Risk alerts: {}\n\
             Recent picks: {}",
            sentiment, sentiment_score, sector_highlights, risk_alerts, recent_picks
        )
    }
    _ => String::new(),
};
```

- [ ] **Step 2: Add sentiment-adjusted scoring to factors.rs**

Add a new function at the end of `src/pick/scoring/factors.rs`:

```rust
/// Adjust factor breakdown based on market guidance sentiment.
pub fn adjust_for_guidance_sentiment(
    factor: &mut FactorBreakdown,
    guidance_sentiment: i32,
    risk_alert_count: usize,
) {
    if guidance_sentiment > 30 {
        // Bullish market: boost momentum slightly
        factor.momentum = (factor.momentum * 1.1).clamp(0.0, 100.0);
    } else if guidance_sentiment < -30 {
        // Bearish market: penalize risk more
        factor.risk = (factor.risk * 0.9).clamp(0.0, 100.0);
    }

    if risk_alert_count > 2 {
        // High alert environment: additional risk penalty
        factor.risk = (factor.risk * 0.85).clamp(0.0, 100.0);
    }

    // Recalculate total
    factor.total = (0.22 * factor.momentum
        + 0.16 * factor.quality
        + 0.12 * factor.value
        + 0.12 * factor.profitability
        + 0.10 * factor.risk
        + 0.10 * factor.event
        + 0.10 * factor.evidence
        + 0.08 * factor.history
        + factor.penalty)
        .clamp(0.0, 100.0);
}
```

- [ ] **Step 3: Run tests**

Run: `cd /root/github/stock-analyzer && cargo test`
Expected: All tests pass

- [ ] **Step 4: Commit**

```bash
git add src/pick/pipeline/mod.rs src/pick/scoring/factors.rs
git commit -m "feat: enhance guidance-to-pick integration with sentiment scoring"
```

---

### Task 8: Add Comprehensive Tests

**Files:**
- Create: `tests/pick_validation_tests.rs`
- Create: `tests/guidance_quality_tests.rs`

- [ ] **Step 1: Create pick validation tests**

Create `tests/pick_validation_tests.rs`:

```rust
use sa::pick::validation::{PickQualityGate, apply_defaults, validate_pick};
use sa::pick::{EnrichedCandidate, FactorBreakdown};

fn make_candidate(price: Option<f64>, atr: Option<f64>) -> EnrichedCandidate {
    let mut technical = sa::StockPickTechnicalSnapshot::default();
    technical.atr = atr;

    EnrichedCandidate {
        symbol: "TEST".to_string(),
        name: "Test Corp".to_string(),
        market: "us_equity".to_string(),
        exchange: "NASDAQ".to_string(),
        industry: "tech".to_string(),
        price,
        change_pct: Some(1.5),
        market_cap: Some(1e9),
        theme_key: "tech".to_string(),
        fundamentals: None,
        news: vec![],
        evidence_records: vec![],
        candles: vec![],
        technical_snapshot: technical,
        market_snapshot: Default::default(),
        fundamental_snapshot: Default::default(),
        news_snapshot: Default::default(),
        history_match_snapshot: Default::default(),
        risk_snapshot: Default::default(),
        data_quality_snapshot: Default::default(),
        factor: FactorBreakdown::default(),
        pass_filter: true,
        rejected_reasons: vec![],
        description: String::new(),
    }
}

fn make_pick(
    entry: Option<&str>,
    stop: Option<&str>,
    target: Option<&str>,
    catalysts: Vec<&str>,
    exit_triggers: Vec<&str>,
) -> sa::pick::types::GeneratedStockPickItem {
    use serde_json::Value;
    sa::pick::types::GeneratedStockPickItem {
        symbol: "TEST".to_string(),
        confidence: Value::from(0.7),
        thesis: "Test thesis".to_string(),
        catalysts: catalysts.into_iter().map(String::from).collect(),
        risks: vec![],
        evidence_points: vec![],
        decision_reason_codes: vec![],
        data_gaps: vec![],
        entry_price: entry.map(String::from),
        stop_loss: stop.map(String::from),
        target_price: target.map(String::from),
        holding_period: None,
        exit_triggers: exit_triggers.into_iter().map(String::from).collect(),
    }
}

#[test]
fn test_valid_pick_passes_validation() {
    let pick = make_pick(
        Some("100"),
        Some("95"),
        Some("115"),
        vec!["earnings beat"],
        vec!["break below 95"],
    );
    let config = PickQualityGate::default();
    let validation = validate_pick(&pick, Some(100.0), &config);
    assert!(validation.is_valid);
    assert!((validation.risk_reward_ratio - 3.0).abs() < 0.01);
}

#[test]
fn test_missing_fields_fails_validation() {
    let pick = make_pick(None, None, None, vec![], vec![]);
    let config = PickQualityGate::default();
    let validation = validate_pick(&pick, Some(100.0), &config);
    assert!(!validation.is_valid);
    assert!(validation.issues.len() >= 3);
}

#[test]
fn test_stop_above_entry_fails() {
    let pick = make_pick(
        Some("100"),
        Some("105"),
        Some("115"),
        vec!["catalyst"],
        vec!["break below 105"],
    );
    let config = PickQualityGate::default();
    let validation = validate_pick(&pick, Some(100.0), &config);
    assert!(!validation.is_valid);
}

#[test]
fn test_low_rr_fails() {
    let pick = make_pick(
        Some("100"),
        Some("95"),
        Some("102"),
        vec!["catalyst"],
        vec!["break below 95"],
    );
    let config = PickQualityGate::default();
    let validation = validate_pick(&pick, Some(100.0), &config);
    assert!(!validation.is_valid);
}

#[test]
fn test_defaults_applied() {
    let mut pick = make_pick(None, None, None, vec![], vec![]);
    let candidate = make_candidate(Some(100.0), Some(2.0));
    apply_defaults(&mut pick, &candidate);
    assert_eq!(pick.entry_price, Some("100.00".to_string()));
    assert_eq!(pick.stop_loss, Some("96.00".to_string())); // 100 - 2*2
    assert_eq!(pick.target_price, Some("112.00".to_string())); // 100 + 3*4
    assert!(!pick.exit_triggers.is_empty());
}

#[test]
fn test_custom_config() {
    let pick = make_pick(
        Some("100"),
        Some("95"),
        Some("110"),
        vec!["catalyst"],
        vec!["break below 95"],
    );
    let config = PickQualityGate {
        min_risk_reward_ratio: 2.0,
        require_catalyst: true,
        require_exit_strategy: true,
        max_stop_loss_pct: 10.0,
    };
    let validation = validate_pick(&pick, Some(100.0), &config);
    assert!(validation.is_valid);
}
```

- [ ] **Step 2: Create guidance quality tests**

Create `tests/guidance_quality_tests.rs`:

```rust
use sa::guide::models::*;
use sa::guide::report::sentiment::{sentiment_label, sentiment_score};

#[test]
fn test_sentiment_score_calculation() {
    assert_eq!(sentiment_score(5, 1, 10), 40); // (5-1)/10 * 100
    assert_eq!(sentiment_score(1, 5, 10), -40);
    assert_eq!(sentiment_score(5, 5, 10), 0);
    assert_eq!(sentiment_score(1, 0, 2), 0); // Too few samples
}

#[test]
fn test_sentiment_labels() {
    assert_eq!(sentiment_label(50), "bullish");
    assert_eq!(sentiment_label(20), "slightly_bullish");
    assert_eq!(sentiment_label(0), "neutral");
    assert_eq!(sentiment_label(-20), "slightly_bearish");
    assert_eq!(sentiment_label(-50), "bearish");
}

#[test]
fn test_price_level_defaults() {
    let level = PriceLevel {
        price: 150.0,
        level_type: "support".to_string(),
        significance: "strong".to_string(),
    };
    assert_eq!(level.price, 150.0);
    assert_eq!(level.level_type, "support");
}

#[test]
fn test_stock_guidance_defaults() {
    let guidance = StockGuidance::default();
    assert!(guidance.symbol.is_empty());
    assert!(guidance.suggested_action.is_empty());
    assert!(guidance.key_levels.is_empty());
}

#[test]
fn test_risk_alert_structure() {
    let alert = RiskAlert {
        severity: "high".to_string(),
        category: "market_sentiment".to_string(),
        description: "Bearish market".to_string(),
        mitigation: "Reduce exposure".to_string(),
        affected_markets: vec!["us_equity".to_string()],
    };
    assert_eq!(alert.severity, "high");
    assert!(!alert.mitigation.is_empty());
}
```

- [ ] **Step 3: Run all tests**

Run: `cd /root/github/stock-analyzer && cargo test`
Expected: All tests pass including new tests

- [ ] **Step 4: Commit**

```bash
git add tests/pick_validation_tests.rs tests/guidance_quality_tests.rs
git commit -m "test: add comprehensive tests for pick validation and guidance quality"
```

---

### Task 9: Final Verification

- [ ] **Step 1: Run cargo clippy**

Run: `cd /root/github/stock-analyzer && cargo clippy`
Expected: Clean (no warnings)

- [ ] **Step 2: Run cargo fmt**

Run: `cd /root/github/stock-analyzer && cargo fmt`
Expected: Clean (no changes)

- [ ] **Step 3: Run full test suite**

Run: `cd /root/github/stock-analyzer && cargo test`
Expected: All tests pass

- [ ] **Step 4: Final commit if needed**

If any formatting or clippy fixes were needed:

```bash
git add -A
git commit -m "fix: clippy and formatting fixes"
```

---

## Verification Checklist

After implementation, verify:

1. `cargo test` — all tests pass
2. `cargo clippy` — clean
3. `cargo fmt` — clean
4. Manual test: Run stock pick for US market, verify actionable fields present
5. Manual test: Generate daily guidance, verify enhanced stock guidances
6. Quality check: Verify R/R ratios are reasonable, stop losses are below entry prices

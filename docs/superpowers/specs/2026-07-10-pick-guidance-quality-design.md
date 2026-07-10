# Stock Picking & Daily Guidance Quality Enhancement

**Goal:** Transform stock picks and daily guidance from basic recommendations into actionable trading guidance with entry/exit rationale, price targets, risk/reward ratios, and exit strategies.

**Architecture:** Enhanced prompts + validation approach. Improve LLM prompts to request actionable data, add validation layer to reject incomplete outputs, apply reasonable defaults when LLM fails, and connect guidance insights to stock picking.

**Tech Stack:** Rust, LLM API, existing stock-analyzer crate

---

## Section 1: Stock Pick Actionability Enhancement

**Goal:** Every stock pick should include entry price, stop loss, target price, holding period, and exit triggers.

### Files Modified
- `src/pick/objective/optimize.rs` — Enhanced LLM prompt
- `src/pick/validation.rs` — New validation layer
- `src/pick/types.rs` — Add actionable fields to types

### Changes

#### 1.1 Enhanced LLM Prompt (`src/pick/objective/optimize.rs`)

Update `build_prompt()` to explicitly require actionable fields in the JSON schema:

```
For each pick, you MUST include:
- entry_price: specific price or price range for entry (e.g., "150-155" or "current price")
- stop_loss: specific stop-loss price (e.g., "145")
- target_price: realistic price target with justification (e.g., "175 based on resistance level")
- holding_period: expected holding period (e.g., "2-4 weeks", "1-3 months")
- exit_triggers: specific conditions that would trigger exit (e.g., "break below 145", "earnings miss")
```

Update JSON schema in prompt to include these fields.

#### 1.2 New Validation Layer (`src/pick/validation.rs`)

```rust
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

pub fn validate_pick(pick: &GeneratedStockPickItem, current_price: Option<f64>) -> PickValidation {
    // Validate required fields
    // Calculate R/R ratio
    // Flag issues
}

pub fn apply_defaults(pick: &mut GeneratedStockPickItem, candidate: &EnrichedCandidate) {
    // Apply reasonable defaults for missing fields
    // entry_price: current price
    // stop_loss: 2 * ATR below entry
    // target_price: 3:1 R/R from entry/stop
    // holding_period: infer from strategy
}
```

#### 1.3 Quality Gates

- Minimum R/R ratio: 1.5:1 (configurable via `SaConfig`)
- Required fields: entry, stop, target, at least 1 catalyst
- Reject picks where stop > entry for long positions
- Reject picks with no exit strategy (after default application)

#### 1.4 Enhanced Default Generators

```rust
fn default_entry_price(candidate: &EnrichedCandidate) -> String {
    // Use current price or nearest support level
}

fn default_stop_loss(candidate: &EnrichedCandidate, entry: f64) -> String {
    // Use 2 * ATR below entry, or 5% below entry if ATR unavailable
}

fn default_target_price(candidate: &EnrichedCandidate, entry: f64, stop: f64) -> String {
    // Use 3:1 R/R ratio from entry/stop
}

fn default_holding_period(strategy: &str) -> String {
    // "swing" -> "2-4 weeks", "position" -> "1-3 months", etc.
}
```

---

## Section 2: Daily Guidance Enhancement

**Goal:** Transform daily guidance from basic news aggregation to actionable stock-level analysis.

### Files Modified
- `src/guide/report/stocks.rs` — LLM-powered stock analysis
- `src/guide/models.rs` — Enhanced StockGuidance struct
- `src/guide/report/sentiment.rs` — Sentiment-weighted recommendations

### Changes

#### 2.1 Enhanced StockGuidance Struct (`src/guide/models.rs`)

```rust
pub struct StockGuidance {
    // Existing fields...
    pub entry_zone: Option<String>,        // "Support at 150-155"
    pub resistance_level: Option<String>,  // "Resistance at 170"
    pub suggested_action: String,          // "watch_for_pullback", "accumulate", "avoid"
    pub action_rationale: String,          // Why this action
    pub key_levels: Vec<PriceLevel>,       // Support/resistance levels
}

pub struct PriceLevel {
    pub price: f64,
    pub level_type: String,  // "support", "resistance", "stop", "target"
    pub significance: String, // "strong", "moderate", "weak"
}
```

#### 2.2 LLM-Powered Stock Analysis (`src/guide/report/stocks.rs`)

Update `generate_stock_guidances()` to use LLM for generating actionable guidance:

1. Gather context: price data, technical indicators, recent news, memory highlights
2. Build prompt requesting actionable analysis
3. Parse LLM response into structured StockGuidance
4. Validate and apply defaults

#### 2.3 Sentiment-Weighted Recommendations

```rust
fn adjust_guidance_for_sentiment(
    guidance: &mut StockGuidance,
    sentiment: &MarketSentiment,
) {
    match sentiment.label.as_str() {
        "bullish" => {
            // More aggressive entry suggestions
            // Tighter stops acceptable
        }
        "bearish" => {
            // Emphasize defense
            // Wider stops required
            // Suggest waiting for confirmation
        }
        _ => {} // neutral - use defaults
    }
}
```

---

## Section 3: Guidance-to-Pick Integration

**Goal:** Feed daily guidance insights into stock picking for better-informed recommendations.

### Files Modified
- `src/pick/pipeline/mod.rs` — Enhanced guidance context
- `src/pick/scoring/factors.rs` — Sentiment-adjusted scoring

### Changes

#### 3.1 Enhanced Guidance Context (`src/pick/pipeline/mod.rs`)

Already partially implemented. Enhance `guidance_context` to include:

```rust
let guidance_context = format!(
    "Market sentiment: {} (score: {})\n\
     Sector highlights: {}\n\
     Risk alerts: {}\n\
     Recent pick performance: {}",
    sentiment_label, sentiment_score,
    format_sector_highlights(&summary),
    format_risk_alerts(&summary),
    format_recent_performance(&summary),
);
```

#### 3.2 Sentiment-Adjusted Scoring (`src/pick/scoring/factors.rs`)

```rust
fn adjust_scores_for_guidance(
    factor: &mut FactorBreakdown,
    guidance_sentiment: i32,
    risk_alert_count: usize,
) {
    if guidance_sentiment > 30 {
        factor.momentum *= 1.1;  // Boost momentum in bullish market
    } else if guidance_sentiment < -30 {
        factor.risk *= 0.9;      // Penalize risk in bearish market
    }

    if risk_alert_count > 2 {
        factor.risk *= 0.85;     // Additional risk penalty for high alert environment
    }
}
```

#### 3.3 Risk Alert Filtering

- If guidance has high-severity risk alerts, increase filtering strictness
- Flag candidates in sectors with negative guidance
- Add warning to picks when market sentiment is bearish

---

## Section 4: Validation & Quality Gates

**Goal:** Ensure every recommendation meets minimum quality standards before output.

### Files Created
- `src/pick/validation.rs` — New validation module

### Changes

#### 4.1 Pick Validation Rules

```rust
pub struct PickQualityGate {
    pub min_risk_reward_ratio: f64,  // Default: 1.5
    pub require_catalyst: bool,      // Default: true
    pub require_exit_strategy: bool, // Default: true
    pub max_stop_loss_pct: f64,      // Default: 10%
}
```

#### 4.2 Validation Pipeline

```rust
pub fn validate_and_enhance_picks(
    picks: Vec<GeneratedStockPickItem>,
    candidates: &[EnrichedCandidate],
    config: &PickQualityGate,
) -> Vec<GeneratedStockPickItem> {
    picks.into_iter().filter_map(|mut pick| {
        // Find matching candidate
        let candidate = candidates.iter().find(|c| c.symbol == pick.symbol)?;

        // Apply defaults for missing fields
        apply_defaults(&mut pick, candidate);

        // Validate
        let validation = validate_pick(&pick, candidate.price);

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
    }).collect()
}
```

#### 4.3 Daily Guidance Validation

```rust
pub fn validate_guidance(guidance: &StockGuidance) -> bool {
    !guidance.suggested_action.is_empty()
        && !guidance.action_rationale.is_empty()
        && guidance.confidence >= 30
}
```

---

## Verification

1. `cargo test` — all existing tests pass
2. `cargo clippy` — clean
3. Manual testing: Run stock pick for US/A-share/HK markets, verify actionable fields present
4. Manual testing: Generate daily guidance, verify enhanced stock guidances
5. Quality check: Verify R/R ratios are reasonable, stop losses are below entry prices

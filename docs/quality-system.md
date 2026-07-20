# Stock Pick Quality System

## Implementation Status

**Implemented:**
- Layer 2: Data Provenance Tracking (full implementation with scoring)
- Layer 3: Enhanced Objective Assessment (8-dimension scoring, 0-100 scale)

**Future Work:**
- Layer 1: Pre-LLM Data Quality Gates (types and helpers present, integration pending)
- Layer 4: Enrichment Retry (EnrichmentAttempt struct present, retry logic pending)

## Overview

The quality system adds 4 layers to the stock pick pipeline:

1. **Pre-LLM Data Quality Gates** - Reject candidates missing critical data (FUTURE WORK)
2. **Data Provenance Tracking** - Track source, timestamp, confidence for all data (IMPLEMENTED)
3. **Enhanced Objective Assessment** - 8-dimension scoring (0-100 scale) (IMPLEMENTED)
4. **Quality Tiers & Enrichment** - Classify picks, retry for insufficient data (PARTIAL: classification implemented, retry is future work)

## Quality Tiers

- **ProductionReady** (score ≥80, no major violations): Ready for use
- **ReviewRequired** (score 60-79): Minor gaps, needs review
- **DataInsufficient** (score <60): Missing critical data, enrichment attempted

## Objective Assessment Dimensions

1. **data_completeness** (0-20): Price, market cap, fundamentals coverage
2. **market_validation** (0-20): Candles, news, price plausibility
3. **reasoning_structure** (0-20): Thesis quality, evidence strength
4. **risk_balance** (0-20): Risk/reward ratio, diversification
5. **evidence_density** (0-20): Evidence quality and quantity
6. **data_provenance** (0-20): Source quality, freshness, coverage
7. **reasoning_consistency** (0-20): LLM claims vs actual data
8. **critical_field_completeness** (0-20): Entry/stop/target with rationales

Total: 160 points → normalized to 100

## Provenance Fields

Each pick includes `provenance_snapshot` with:
- `market_data`: Source, timestamp, confidence for price/volume data
- `fundamentals`: Source, timestamp for financial statements
- `technicals`: Source, timestamp for computed indicators
- `news`: Source, timestamp for news articles

## Usage

Quality assessment is automatic. Access via:

```rust
let response = run_pick_pipeline(&market_data, &llm_client, &request).await?;

for pick in response.picks {
    println!("Score: {}", pick.objective_assessment.final_score);
    println!("Tier: {:?}", pick.quality_tier);
    println!("Gaps: {:?}", pick.objective_assessment.gaps);
    
    if let Some(prov) = &pick.provenance_snapshot.market_data {
        println!("Market data from: {} at {}", prov.source, prov.fetched_at);
    }
}
```

## Configuration

Quality gates use default thresholds:
- Minimum candles: 5
- Required: price > 0, market_cap > 0, any fundamental data

To customize, modify `pick/gates.rs::check_critical_fields()`.

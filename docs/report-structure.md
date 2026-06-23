# Report Structure

## AnalysisResult

The main output of an analysis report is `AnalysisResult`, which contains:

```json
{
  "task_id": "uuid",
  "report_id": "uuid",
  "symbol": "600519.SH",
  "stock_name": "贵州茅台",
  "analysis_date": "2026-06-23",
  "market_type": "A股",
  "graph": { ... },
  "agent_state": { ... },
  "artifacts": { ... },
  "report": { ... },
  "ic_report": { ... },
  "created_at": "2026-06-23T10:30:00Z"
}
```

## StructuredReport

The `report` field contains the main analysis:

```json
{
  "summary": "贵州茅台当前处于...",
  "recommendation": "Buy",
  "confidence_score": 75,
  "direction_score": 12,
  "action_score": 8,
  "setup_tags": ["trend_confirmed", "fundamental_quality"],
  "technical_analysis": { ... },
  "fundamental_analysis": { ... },
  "news_analysis": { ... },
  "risk_assessment": { ... },
  "trader_plan": { ... },
  "portfolio_decision": { ... },
  "research_plan": { ... },
  "calibration_summary": { ... },
  "execution_readiness": { ... }
}
```

## Key Fields

### Recommendation

The `recommendation` field uses these values:

| Value | Description |
|-------|-------------|
| `Buy` | Strong positive outlook |
| `Overweight` | Moderately positive |
| `Hold` | Neutral stance |
| `Underweight` | Moderately negative |
| `Sell` | Strong negative outlook |

### Confidence Score

Range: 0-100

- 80-100: High confidence
- 60-79: Medium confidence
- 0-59: Low confidence

### Setup Tags

Tags describing the analysis setup:

| Tag | Description |
|-----|-------------|
| `trend_confirmed` | Strong trend confirmation |
| `event_driven` | Catalyst-driven opportunity |
| `fundamental_quality` | Strong fundamentals |
| `valuation_sensitive` | Valuation concerns |
| `execution_ready` | Ready for execution |
| `watchlist_only` | Monitor only |

## Market Chart

```json
{
  "candles": [
    {
      "trade_date": "2026-06-20",
      "open": 1850.0,
      "high": 1860.0,
      "low": 1840.0,
      "close": 1855.0,
      "volume": 5000000
    }
  ],
  "overlays": [
    {
      "key": "current_price",
      "value": 1855.0,
      "emphasis": "primary"
    }
  ],
  "trend_lines": [
    {
      "key": "sma_50",
      "color": "#3b82f6",
      "points": [...]
    }
  ],
  "indicators": [...]
}
```

## Decision View

```json
{
  "view": "Bullish",
  "action": "Buy",
  "confidence_band": "High",
  "current_price": "1855.0",
  "confirmation_price": "1870.0",
  "invalidation_price": "1820.0",
  "target_reference": "1950.0",
  "primary_path": "突破前高后加速上涨",
  "early_probe_allowed": true,
  "next_upgrade_condition": { ... },
  "next_downgrade_condition": { ... }
}
```

## Evidence Cards

```json
{
  "key": "pe_ratio",
  "category": "fundamentals",
  "value": "25.3",
  "direction": "positive",
  "strength": "primary",
  "source": "fundamentals",
  "claim": "PE ratio 25.3 indicates reasonable valuation"
}
```

## News Insights

```json
{
  "title": "茅台提价公告",
  "published_at": "2026-06-22",
  "source": "eastmoney",
  "fact_summary": {
    "key": "news_timing_published",
    "params": { "summary": "茅台宣布提价..." }
  },
  "interpretation": {
    "key": "news_supports_active_monitoring"
  },
  "impact_direction": "positive",
  "impact_strength": "medium",
  "what_it_confirms": { ... },
  "what_to_watch_next": { ... },
  "published_before_analysis": true
}
```

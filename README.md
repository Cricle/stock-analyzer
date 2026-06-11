# stock-analyzer

Stock analysis engine for market research, portfolio decisions, and scoring.

## Crates

| Crate | Purpose |
|-------|---------|
| `sa-types` | Core market data types (quotes, fundamentals, news, candlestick, capital flow) |
| `sa-models` | Analysis result models, scoring types, and storage trait interfaces |
| `sa-data` | Market data fetching via AKShare, Qdrant vector search, optional Redis caching |
| `sa-engine` | Graph-based analysis engine with LLM integration, memory RAG, and multi-dimensional scoring |

## Quick Start

```rust
use sa_engine::{TaskManager, TaskRunParams};

let manager = TaskManager::new(/* config */).await?;
let result = manager.run(TaskRunParams { /* ... */ }).await?;
```

## Storage

All storage access goes through trait interfaces defined in `sa-models`:

- **AnalysisStore** — task CRUD and result persistence
- **CacheStore** — key-value cache for intermediate data
- **VectorStore** — semantic vector search (Qdrant)
- **CheckpointStore** — resumable workflow checkpoints
- **GuidanceStore** — guidance rule persistence

Implement these traits for your backend (Postgres, Redis, Qdrant, etc.) and inject them at startup.

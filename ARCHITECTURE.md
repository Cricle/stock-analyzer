# Architecture

## Workspace Structure

```
stock-analyzer/
├── crates/
│   ├── sa-types/      — Core market data types and shared models
│   ├── sa-models/     — Analysis models, scoring, report logic, storage traits
│   ├── sa-data/       — Market data fetching (A-share, HK, US, news)
│   ├── sa-engine/     — Analysis pipeline, LLM, guidance, stock pick, task management
│   └── sa-storage/    — Storage implementations (Redis, SQLite, Qdrant)
├── src/
│   ├── main.rs        — CLI binary
│   └── mcp.rs         — MCP server binary
└── tests/             — Integration and E2E tests
```

## Crate Dependency Graph

```
sa-types (foundation)
    ↑
sa-models (depends on sa-types)
    ↑
sa-data (depends on sa-types, sa-models)
    ↑
sa-engine (depends on all above)
    ↑
sa-storage (depends on sa-types, sa-models)
```

## Design Principles

- **Workspace crates** — separation of concerns across focused crates
- **Trait-based storage** — storage access through traits in sa-models, no direct DB dependencies in engine
- **Graph-based execution** — analysis pipeline uses adk-graph for node-based orchestration
- **i18n keys** — code returns translation keys, JSON files provide display text
- **Feature-gated backends** — Redis caching behind `redis-cache`, local embeddings behind `local-rag-embeddings`

## Data Flow

1. **Data ingestion** (sa-data) — fetches market quotes, fundamentals, news from various sources
2. **Analysis pipeline** (sa-engine) — runs market, fundamental, news, research, and portfolio decision steps
3. **Memory & guidance** — stores/retrieves vector embeddings for RAG context
4. **Scoring** — multi-dimensional scoring (technical, fundamental, sentiment, LLM-based)
5. **Output** — returns JSON with i18n keys, optionally resolved to display text

## Binary Architecture

**CLI (`sa`)**: Direct invocation, reads env vars, calls engine functions, outputs JSON.

**MCP Server (`sa mcp`)**: Long-running process, exposes tools via MCP protocol:
- `generate_guidance` — daily market guidance
- `stock_pick` — stock selection
- `generate_report` — analysis report for a symbol

Both binaries share initialization code via `bin_helpers`.

## Storage Traits

| Trait | Purpose |
|-------|---------|
| `AnalysisStore` | Task CRUD, result persistence |
| `CacheStore` | Key-value cache with TTL |
| `VectorStore` | Semantic vector search |
| `CheckpointStore` | Resumable workflow checkpoints |

## Crate Details

### sa-types
Core types shared across the workspace: `CandlePoint`, `QuoteSnapshot`, `NewsItem`, `MarketType`, `Rating`, `LocalText`, etc.

### sa-models
Analysis models and business logic:
- Scoring models (technical, fundamental, sentiment)
- Report generation logic (setup tags, calibration, chart computation)
- Storage trait definitions
- Configuration types

### sa-data
Market data fetching from multiple sources:
- A-share: Tencent, Eastmoney, Akshare
- Hong Kong: HKEX, Yahoo Finance
- US: Yahoo Finance, Finnhub
- News: SearXNG, GDELT, Baidu News, Uapis

### sa-engine
Core analysis engine:
- LLM client (OpenAI-compatible, Anthropic)
- Analysis pipeline with graph-based execution
- Task management and lifecycle
- Guidance generation
- Stock pick scoring and selection
- Memory and RAG context

### sa-storage
Storage implementations:
- Redis-based caching
- SQLite persistence
- Qdrant vector search

## Test Coverage

Unit tests cover pure logic functions across all crates. Integration tests verify end-to-end pipeline behavior. CI runs `cargo tarpaulin` with 90% coverage threshold.

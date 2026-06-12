# Architecture

## Module Structure

```
sa-engine (single crate)
├── types/       — Core market data types
├── models/      — Analysis models, scoring, storage traits
├── data/        — Market data fetching (AKShare, news)
├── engine/      — Analysis pipeline, LLM, guidance, stock pick
├── i18n/        — Internationalization
├── bin_helpers  — CLI/MCP shared initialization
└── bin/
    ├── sa-engine.rs      — CLI
    └── sa-engine-mcp.rs  — MCP server
```

## Design Principles

- **Single crate** — all logic in sa-engine, no workspace dependencies
- **Trait-based storage** — storage access through traits in models/, no direct DB dependencies in engine
- **Graph-based execution** — analysis pipeline uses adk-graph for node-based orchestration
- **i18n keys** — code returns translation keys, JSON files provide display text
- **Feature-gated backends** — Redis caching behind `redis-cache`, local embeddings behind `local-rag-embeddings`

## Data Flow

1. **Data ingestion** (data/) — fetches market quotes, fundamentals, news from AKShare
2. **Analysis pipeline** (engine/) — runs market, fundamental, news, research, and portfolio decision steps
3. **Memory & guidance** — stores/retrieves vector embeddings for RAG context
4. **Scoring** — multi-dimensional scoring (technical, fundamental, sentiment, LLM-based)
5. **Output** — returns JSON with i18n keys, optionally resolved to display text

## Binary Architecture

**CLI (sa-engine)**: Direct invocation, reads env vars, calls engine functions, outputs JSON.

**MCP Server (sa-engine-mcp)**: Long-running process, exposes 3 tools via MCP protocol:
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
| `GuidanceStore` | Guidance rule persistence |

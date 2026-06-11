# Architecture

## Crate Dependency Graph

```
sa-types
  └─► sa-models
        └─► sa-data
              └─► sa-engine
```

## Design Principles

- **Trait-based storage** — no direct Redis/Qdrant/NATS dependencies in the engine; all storage access goes through traits in `sa-models`
- **Graph-based execution** — analysis pipeline uses `adk-graph` for node-based orchestration
- **Feature-gated backends** — Redis caching is behind the `redis-cache` feature flag; local embeddings behind `local-rag-embeddings`

## Data Flow

1. **Data ingestion** (`sa-data`) — fetches market quotes, fundamentals, news from AKShare
2. **Analysis pipeline** (`sa-engine`) — runs market, fundamental, news, research, and portfolio decision steps via `adk-graph`
3. **Memory & guidance** — stores/retrieves vector embeddings for RAG context and cross-day guidance
4. **Scoring** — multi-dimensional scoring (technical, fundamental, sentiment, LLM-based)

## Storage Traits

| Trait | Methods | Purpose |
|-------|---------|---------|
| `AnalysisStore` | 11 | Task CRUD, result persistence, request logging |
| `CacheStore` | 5 | Key-value cache with TTL support |
| `VectorStore` | 3 | Semantic vector search (insert, search, delete) |
| `CheckpointStore` | 4 | Resumable workflow checkpoints |
| `GuidanceStore` | 4 | Guidance rule persistence |

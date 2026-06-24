# Architecture

## Workspace Structure

```
stock-analyzer/
├── crates/
│   └── sa/            — Unified analysis crate (report, guide, pick, score)
├── src/
│   ├── main.rs        — CLI binary
│   └── mcp.rs         — MCP server binary
├── tests/             — Integration and E2E tests
└── docs/              — Documentation
```

Data fetching is delegated to [`akshare-rs`](https://github.com/Cricle/akshare-rs) via path dependency.

## Module Structure

```
sa/src/
├── lib.rs             — Crate root, re-exports
├── analysis/          — Core analysis types, report logic, derived calculations
├── checkpoint/        — Resumable workflow checkpoints
├── data/              — MarketDataProvider trait + re-exports from akshare-rs
├── env_config/        — Environment variable configuration
├── guide/             — Daily market guidance (report, store, embedding)
├── llm/               — LLM client (OpenAI/Anthropic), prompts, parsing
├── llm_config/        — LLM provider configuration
├── memory/            — Vector-based historical memory (RAG)
├── pick/              — Stock picking (pipeline, scoring, objective, history)
├── report/            — Analysis pipeline (lifecycle, runtime, diagnosis, result)
├── scoring/           — Multi-dimensional scoring (technical, fundamental, sentiment)
├── shared/            — Shared utilities
├── store/             — Storage traits + in-memory implementations
├── task/              — Task status types
├── task_manager/      — Task lifecycle management
├── telemetry/         — OpenTelemetry integration
├── types.rs           — Type re-exports from akshare-rs
├── user_preferences/  — User watchlist and preferences
└── value_utils/       — Value utility functions
```

## Dependency Graph

```
akshare-rs (data layer — news, quotes, candles, fundamentals)
    ↑
    sa (analysis engine — all analysis, scoring, LLM, storage)
    ↑
    CLI / MCP server
```

## Design Principles

- **Single analysis crate** — `sa` owns all analysis logic; data fetching lives in `akshare-rs`
- **Trait-based data access** — `MarketDataProvider` trait enables mock testing without network calls
- **Trait-based storage** — `AnalysisStore`, `CacheStore`, `VectorStore`, `CheckpointStore` define storage contracts
- **Graph-based execution** — analysis pipeline uses `adk-graph` for node-based orchestration
- **i18n keys** — code returns translation keys, JSON files provide display text
- **Feature flags** — `report`, `guide`, `pick`, `score` control module inclusion; `local-rag-embeddings` enables vector embeddings

## Data Flow

1. **Data ingestion** (akshare-rs) — fetches market quotes, fundamentals, news from A-share, HK, US sources
2. **Analysis pipeline** (`report/`) — runs market, fundamental, news, research, and portfolio decision steps via graph execution
3. **Memory & guidance** (`memory/`, `guide/`) — stores/retrieves vector embeddings for RAG context; generates daily guidance
4. **Scoring** (`scoring/`) — multi-dimensional scoring (technical, fundamental, sentiment, LLM-based)
5. **Stock picking** (`pick/`) — candidate resolution, multi-factor scoring, LLM selection
6. **Output** — returns JSON with i18n keys, optionally resolved to display text

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
| `CheckpointStore` | Resumable workflow ch
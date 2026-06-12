# sa-engine

Stock analysis engine with CLI, MCP server, and i18n support.

## Features

- **Daily guidance** — market overview, sector highlights, risk alerts
- **Stock selection** — multi-factor scoring with LLM-based analysis
- **Analysis reports** — technical indicators, risk assessment, catalyst review, decision view
- **i18n** — returns translation keys, resolves via bundled zh.json/en.json
- **CLI** — `sa-engine` binary with clap subcommands
- **MCP server** — `sa-engine-mcp` binary with stdio transport

## Quick Start

```bash
# CLI — daily guidance
sa-engine guidance --market a-share --lang zh

# CLI — stock selection
sa-engine stock-pick --market hk --date 2026-06-12

# CLI — analysis report
sa-engine report --symbol 600519.SH --lang en

# MCP server (stdio)
sa-engine-mcp --transport stdio
```

## Environment Variables

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `LLM_BASE_URL` | Yes | — | LLM API base URL |
| `LLM_API_KEY` | Yes | — | LLM API key |
| `LLM_MODEL` | No | `claude-sonnet-4-20250514` | Model identifier |
| `LLM_PROVIDER` | No | `openai` | `openai` or `anthropic` |
| `LLM_TIMEOUT_SECS` | No | `120` | Request timeout |
| `TUSHARE_TOKEN` | No | — | Tushare data API token |
| `REDIS_URL` | No | — | Redis cache URL |

## Module Structure

```
sa-engine/src/
  types/       — MarketKind, QuoteSnapshot, FundamentalsSnapshot, etc.
  models/      — Analysis types, scoring, storage traits, LocalText
  data/        — MarketDataClient, AKShare, news fetching
  engine/      — Analysis pipeline, LLM, guidance, stock pick
  i18n/        — I18n struct, zh.json, en.json
  bin_helpers  — Shared CLI/MCP initialization
  bin/
    sa-engine.rs      — CLI binary
    sa-engine-mcp.rs  — MCP server binary
```

## Storage Traits

All storage access goes through trait interfaces:

- **AnalysisStore** — task CRUD and result persistence
- **CacheStore** — key-value cache with TTL
- **VectorStore** — semantic vector search (Qdrant)
- **CheckpointStore** — resumable workflow checkpoints
- **GuidanceStore** — guidance rule persistence

## Feature Flags

| Flag | Description |
|------|-------------|
| `redis-cache` | Enable Redis caching for market data |
| `local-rag-embeddings` | Enable local embedding model (fastembed) |

## License

MIT

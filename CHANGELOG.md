# Changelog

## v2.0.0

### Changed

- **Merged into monolith** — sa-types, sa-models, sa-data merged into sa-engine as modules
- **New module structure** — types/, models/, data/, engine/, i18n/

### Added

- **CLI binary** (`sa-engine`) — guidance, stock-pick, report subcommands with `--lang zh|en`
- **MCP server binary** (`sa-engine-mcp`) — stdio transport, 3 tools (generate_guidance, stock_pick, generate_report)
- **i18n system** — I18n struct with dot-separated key resolution, bundled zh.json and en.json
- **bin_helpers** — shared initialization for CLI/MCP (MarketDataClient, LlmClient, NoopMemory)

## v1.0.0

### Added

- Core market data types (MarketKind, QuoteSnapshot, FundamentalsSnapshot, NewsItem, CandlePoint, CapitalFlowPoint)
- Analysis result models, scoring types, and storage trait interfaces
- Market data fetching via AKShare, Qdrant vector search, optional Redis caching
- Analysis engine with graph-based execution, LLM integration, memory RAG, stock pick scoring, guidance reports

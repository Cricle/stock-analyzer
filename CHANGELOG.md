# Changelog

## v1.0.0

### Added

- **sa-types**: Core market data types (`MarketKind`, `QuoteSnapshot`, `FundamentalsSnapshot`, `NewsItem`, `CandlePoint`, `CapitalFlowPoint`)
- **sa-models**: Analysis result models, scoring types, and storage trait interfaces (`AnalysisStore`, `CacheStore`, `VectorStore`, `CheckpointStore`, `GuidanceStore`)
- **sa-data**: Market data fetching via AKShare, Qdrant vector search client, optional Redis caching
- **sa-engine**: Analysis engine with graph-based execution, LLM integration, memory RAG, stock pick scoring, guidance reports, and multi-dimensional scoring

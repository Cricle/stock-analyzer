# Stock Analyzer Documentation

## Overview

Stock Analyzer is a Rust-based analysis engine that provides market guidance, stock selection, and per-symbol analysis reports. It supports A-share, Hong Kong, and US markets.

## Documentation Index

- [Architecture](../ARCHITECTURE.md) — Workspace structure and design principles
- [Configuration](configuration.md) — Environment variables and config files
- [Report Structure](report-structure.md) — Analysis report output format
- [API Reference](api-reference.md) — MCP tools and CLI commands
- [Development](development.md) — Building, testing, and contributing

## Quick Start

```bash
# Daily guidance for A-share market
sa guidance --market a-share --lang zh

# Stock selection for Hong Kong
sa stock-pick --market hk --lang zh

# Per-symbol analysis report
sa report --symbol 600519.SH --market a-share --lang en

# MCP server
sa mcp --transport stdio
```

## Supported Markets

| Market | Code | Data Sources |
|--------|------|--------------|
| A-share | `a-share` | Tencent, Eastmoney, Akshare |
| Hong Kong | `hk` | HKEX, Yahoo Finance |
| US | `us` | Yahoo Finance, Finnhub |

## Output Format

All output is JSON with i18n keys. Use `--lang zh` or `--lang en` to resolve keys into display text.

## License

MIT

# sa

Stock analysis engine — CLI & MCP server for market guidance, stock selection, and analysis reports.

Data fetching is delegated to [akshare-rs](https://github.com/Cricle/akshare-rs); this crate focuses on analysis, scoring, and LLM-powered report generation.

## What it does

- **Guidance** — daily market overview: sentiment, sector highlights, risk alerts, key news
- **Stock Pick** — multi-factor candidate scoring with LLM selection; supports A-share, HK, US markets
- **Report** — per-symbol analysis combining guidance context and stock pick evaluation

Output is always JSON. Use `--lang zh` or `--lang en` to resolve i18n keys into display text.

## Quality System

Stock picks include comprehensive quality assessment with provenance and objective scoring:

- **Data provenance tracking** — source, timestamp, confidence for all data inputs
- **8-dimension objective scoring** — 0-100 scale assessment across data completeness, market validation, reasoning structure, risk balance, evidence density, provenance quality, consistency, and critical fields
- **Quality tier classification** — ProductionReady/ReviewRequired/DataInsufficient based on scores

**Planned enhancements:** Pre-LLM data quality gates and automatic enrichment retry for insufficient picks.

See `docs/quality-system.md` for details.

## Install

Download from [Releases](https://github.com/Cricle/stock-analyzer/releases), install from crates.io, or build from source:

```bash
cargo install stock-analyser
# or
cargo build --release
```

A single binary `sa` handles both CLI and MCP server.

## Quick Start

```bash
# Daily guidance for A-share market (Chinese output)
sa guidance --market a-share --lang zh

# Stock selection for Hong Kong
sa stock-pick --market hk --lang zh

# Per-symbol analysis report
sa report --symbol 600519.SH --market a-share --lang en

# Compact JSON (single line, for piping)
sa --json guidance --market us --lang en | jq '.market_sentiment'
```

## MCP Server

```bash
# stdio transport (for Claude Desktop, Cursor, etc.)
sa mcp --transport stdio

# HTTP+SSE transport (for remote clients)
sa mcp --transport http --port 3000
```

### HTTP Authentication

When `mcp_key` is set in config or `SA_MCP_KEY` env var, HTTP clients must send:

```
X-MCP-KEY: <your-key>
```

Leave unset to allow unauthenticated access.

### MCP Tools

`generate_guidance`, `stock_pick`, `generate_report`. All accept `market` (a-share/hk/us) and `lang` (zh/en).

## Configuration

### LLM (required)

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `LLM_BASE_URL` | Yes | — | OpenAI-compatible or Anthropic API base URL |
| `LLM_API_KEY` | Yes | — | API key |
| `LLM_MODEL` | No | `claude-sonnet-4-20250514` | Model identifier |
| `LLM_PROVIDER` | No | `openai` | `openai` or `anthropic` |
| `LLM_TIMEOUT_SECS` | No | `300` | Request timeout in seconds |

### API Keys

| Variable | Description |
|----------|-------------|
| `FINNHUB_API_KEY` | Finnhub API key for US stock news (comma-separated for rotation) |
| `SA_MCP_KEY` | MCP HTTP auth key (overrides config file) |

Config file: `~/.config/sa-engine/config.toml` (or set `SA_ENGINE_CONFIG`).
See `config.example.toml` for format.

### Optional

| Variable | Default | Description |
|----------|---------|-------------|
| `OUTBOUND_PROXY_URL` | — | HTTP proxy for outbound requests |
| `DATA_DIR` | `/data` | Data directory for memory/history storage |
| `REPORT_KLINE_LIMIT` | — | Override kline limit for reports |
| `SCORE_WEIGHT_TECHNICAL` | — | Override scoring weight |
| `SCORE_WEIGHT_FUNDAMENTAL` | — | Override scoring weight |
| `SCORE_WEIGHT_SENTIMENT` | — | Override scoring weight |
| `SCORE_WEIGHT_LLM_ANALYSIS` | — | Override scoring weight |

## CLI Reference

```
sa [--json] <command> [options]

Commands:
  guidance      Generate daily market guidance
    --market    a-share | hk | us (default: a-share)
    --lang      zh | en

  stock-pick    Run stock selection
    --market    a-share | hk | us (default: a-share)
    --date      YYYY-MM-DD (default: today)
    --candidate-symbols  Comma-separated symbols
    --lang      zh | en

  report        Generate analysis report for a stock
    --symbol    Stock symbol (required)
    --market    a-share | hk | us (default: a-share)
    --sections  Comma-separated sections
    --lang      zh | en

  mcp           Start MCP server
    --transport stdio | http (default: stdio)
    --port      HTTP port (default: 3000)
    --config    Path to config file

Global:
  --json        Output compact JSON (single line)
```

## License

MIT

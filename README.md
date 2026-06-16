# sa-engine

Stock analysis engine — CLI and MCP server for market guidance, stock selection, and analysis reports.

## What it does

- **Guidance** — daily market overview: sentiment, sector highlights, risk alerts, key news
- **Stock Pick** — multi-factor candidate scoring with LLM selection; supports A-share, HK, US markets
- **Report** — per-symbol analysis combining guidance context and stock pick evaluation

Output is always JSON. Use `--lang zh` or `--lang en` to resolve i18n keys into display text.

## Install

Download from [Releases](https://github.com/Cricle/stock-analyzer/releases), or build from source:

```bash
cargo build --release
```

Two binaries: `sa-engine` (CLI) and `sa-engine-mcp` (MCP server).

## Quick Start

```bash
# Daily guidance for A-share market (Chinese output)
sa-engine guidance --market a-share --lang zh

# Stock selection for Hong Kong
sa-engine stock-pick --market hk --lang zh

# Per-symbol analysis report
sa-engine report --symbol 600519.SH --market a-share --lang en

# Compact JSON (single line, for piping)
sa-engine --json guidance --market us --lang en | jq '.market_sentiment'
```

## MCP Server

```bash
# stdio transport (for Claude Desktop, Cursor, etc.)
sa-engine-mcp --transport stdio
```

Tools: `generate_guidance`, `stock_pick`, `generate_report`. All accept `market` and `lang` parameters.

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

Config file alternative: `~/.config/sa-engine/config.toml` (or set `SA_ENGINE_CONFIG`).
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
sa-engine [--json] <command> [options]

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

Global:
  --json        Output compact JSON (single line)
```

## License

MIT

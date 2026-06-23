# Configuration

## Environment Variables

### LLM Configuration (Required)

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

### Optional Configuration

| Variable | Default | Description |
|----------|---------|-------------|
| `OUTBOUND_PROXY_URL` | — | HTTP proxy for outbound requests |
| `DATA_DIR` | `/data` | Data directory for memory/history storage |
| `REPORT_KLINE_LIMIT` | — | Override kline limit for reports |
| `SCORE_WEIGHT_TECHNICAL` | — | Override scoring weight |
| `SCORE_WEIGHT_FUNDAMENTAL` | — | Override scoring weight |
| `SCORE_WEIGHT_SENTIMENT` | — | Override scoring weight |
| `SCORE_WEIGHT_LLM_ANALYSIS` | — | Override scoring weight |

## Config File

Config file location: `~/.config/sa-engine/config.toml`

Or set `SA_ENGINE_CONFIG` to specify a custom path.

See `config.example.toml` for format.

## MCP HTTP Authentication

When `mcp_key` is set in config or `SA_MCP_KEY` env var, HTTP clients must send:

```
X-MCP-KEY: <your-key>
```

Leave unset to allow unauthenticated access.

## Debug Mode

Set `ANALYSIS_DEBUG_QUICK_ONLY=1` to enable quick-only debug mode (fewer LLM calls).

Set `REPORT_KLINE_LIMIT=60` to limit candle data and avoid huge prompts.

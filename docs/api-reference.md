# API Reference

## CLI Commands

### guidance

Generate daily market guidance.

```bash
sa guidance --market <market> --lang <lang>
```

**Options:**
- `--market`: `a-share` | `hk` | `us` (default: `a-share`)
- `--lang`: `zh` | `en`

**Output:** JSON with market sentiment, sector highlights, risk alerts, key news.

### stock-pick

Run stock selection.

```bash
sa stock-pick --market <market> --date <date> --candidate-symbols <symbols> --lang <lang>
```

**Options:**
- `--market`: `a-share` | `hk` | `us` (default: `a-share`)
- `--date`: `YYYY-MM-DD` (default: today)
- `--candidate-symbols`: Comma-separated symbols
- `--lang`: `zh` | `en`

**Output:** JSON with ranked stock candidates.

### report

Generate analysis report for a stock.

```bash
sa report --symbol <symbol> --market <market> --sections <sections> --lang <lang>
```

**Options:**
- `--symbol`: Stock symbol (required)
- `--market`: `a-share` | `hk` | `us` (default: `a-share`)
- `--sections`: Comma-separated sections
- `--lang`: `zh` | `en`

**Output:** JSON with full analysis report.

### mcp

Start MCP server.

```bash
sa mcp --transport <transport> --port <port> --config <config>
```

**Options:**
- `--transport`: `stdio` | `http` (default: `stdio`)
- `--port`: HTTP port (default: `3000`)
- `--config`: Path to config file

## MCP Tools

### generate_guidance

Generate daily market guidance.

**Parameters:**
- `market`: `a-share` | `hk` | `us`
- `lang`: `zh` | `en`

**Returns:** Guidance JSON with sentiment, sectors, risks, news.

### stock_pick

Run stock selection.

**Parameters:**
- `market`: `a-share` | `hk` | `us`
- `lang`: `zh` | `en`
- `date`: `YYYY-MM-DD` (optional)
- `candidate_symbols`: Comma-separated symbols (optional)

**Returns:** Ranked stock candidates JSON.

### generate_report

Generate analysis report for a stock.

**Parameters:**
- `symbol`: Stock symbol
- `market`: `a-share` | `hk` | `us`
- `lang`: `zh` | `en`
- `sections`: Comma-separated sections (optional)

**Returns:** Full analysis report JSON.

## Global Options

- `--json`: Output compact JSON (single line)

## Examples

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

# Design: Monolith Merge + CLI + MCP + i18n

## Overview

Merge sa-types, sa-models, sa-data into sa-engine as a single crate. Add two binaries (CLI + MCP server) sharing a common library core. Introduce i18n system where code returns keys and JSON files provide translations.

## 1. Crate Merge

**Goal**: Single crate `sa-engine` containing all logic.

### Module Layout

```
sa-engine/
  Cargo.toml          ← all dependencies merged
  src/
    lib.rs            ← public API re-exports
    types/            ← from sa-types (MarketKind, QuoteSnapshot, FundamentalsSnapshot, etc.)
    models/           ← from sa-models (scoring, analysis types, LocalText, store traits)
    data/             ← from sa-data (MarketDataClient, akshare, news, cache)
    engine/           ← original sa-engine analysis logic (guidance, stock_pick, report, llm, etc.)
    i18n/             ← new: multilingual system
    bin/
      sa-engine.rs        ← CLI binary
      sa-engine-mcp.rs    ← MCP server binary
```

### Workspace

Cargo.toml workspace members reduced to `["crates/sa-engine"]` only. The old crates directories are removed after code migration.

### Dependency Merge

All dependencies from the 4 crates go into sa-engine/Cargo.toml. New additions:
- `clap` (CLI framework)
- `rmcp` (MCP SDK)

Feature flags consolidated:
- `local-rag-embeddings` (from sa-engine)
- `redis-cache` (from sa-data)

## 2. CLI Binary (sa-engine)

Framework: clap with derive macros.

### Subcommands

```
sa-engine guidance [--market <a-share|hk|us>] [--lang <zh|en>]
sa-engine stock-pick [--market <a-share|hk|us>] [--date YYYY-MM-DD] [--lang <zh|en>]
sa-engine report --symbol <SYMBOL> [--market <a-share|hk|us>] [--sections s1,s2,...] [--lang <zh|en>]

Default market: `a-share` when omitted. Invalid section IDs in `--sections` are skipped with a warning to stderr.
```

### Output

All subcommands output JSON to stdout. By default, text fields are `LocalText` objects (`{ "key": "...", "params": {...} }`). With `--lang`, text fields are resolved to the target language string.

### Error Handling

Non-zero exit code on failure. Error message to stderr as JSON:
```json
{ "error": { "code": "data_fetch_failed", "message": "..." } }
```

## 3. MCP Server Binary (sa-engine-mcp)

Framework: rmcp.

### Transport

```
sa-engine-mcp --transport stdio                # JSON-RPC over stdin/stdout
sa-engine-mcp --transport http --port 3000     # HTTP + SSE
```

### Tools

| Tool | Parameters | Returns |
|------|-----------|---------|
| `generate_guidance` | `market?: enum` (default: "a-share") | Daily market guidance JSON |
| `stock_pick` | `market?: enum` (default: "a-share"), `date?: string` | Stock selection results JSON |
| `generate_report` | `symbol: string, market?: enum` (default: "a-share"), `sections?: string[]` | Analysis report JSON, sections independently generated |

### Response Format

All tools return JSON with i18n keys. Text fields use `LocalText` structure. The MCP client is responsible for resolving keys to display text.

## 4. i18n System

### Principle

Code returns keys, never display text. JSON files map keys to localized strings.

### Core Type

Reuse existing `LocalText`:
```rust
pub struct LocalText {
    pub key: String,
    pub params: serde_json::Map<String, serde_json::Value>,
}
```

### Module Structure

```
src/i18n/
  mod.rs              ← I18n struct
  locales/
    zh.json           ← Chinese translations
    en.json           ← English translations
```

### I18n Struct

```rust
pub struct I18n {
    locales: HashMap<String, serde_json::Value>,  // lang -> nested JSON
}

impl I18n {
    pub fn new() -> Self;                          // load bundled zh.json + en.json
    pub fn resolve(&self, key: &str, lang: &str) -> Option<String>;
    pub fn resolve_local_text(&self, lt: &LocalText, lang: &str) -> String;
}
```

### Key Format

Dot-separated, maps to nested JSON:
- `report.summary.title` -> `{"report":{"summary":{"title":"..."}}}`
- `report.risk.level_high` -> `{"report":{"risk":{"level_high":"..."}}}`

### Parameter Interpolation

Keys with params use `{param_name}` placeholders in JSON values:
```json
{ "report": { "risk": { "level_with_value": "Risk score: {value}" } } }
```
Resolved: `resolve("report.risk.level_with_value", "zh")` with `params = { "value": 85 }` -> `"风险评分: 85"`

## 5. Report Section Generation

### Sections

| ID | Content | Source Module |
|----|---------|--------------|
| `summary` | Comprehensive summary | engine/analysis/report_logic/core |
| `technical` | Technical indicators (MACD, RSI, KDJ, etc.) | engine/analysis/report_logic/technical_indicators |
| `risk` | Risk assessment and controls | engine/analysis/report_logic/risk_controls |
| `catalyst` | Catalyst analysis | engine/analysis/report_logic/catalyst_review |
| `decision` | Decision view | engine/analysis/report_logic/decision_view |
| `diagnostics` | Diagnostic information | engine/analysis/report_logic/diagnostics |
| `probability` | Probability assessment | engine/analysis/report_logic/probability |
| `trader_plan` | Trading plan | engine/analysis/report_logic/trader_plan |

### API

```rust
pub struct ReportSection {
    pub section_id: String,
    pub content: LocalText,
    pub data: serde_json::Value,
}

pub async fn generate_report(
    symbol: &str,
    market: MarketKind,
    sections: Option<Vec<String>>,  // None = all sections
    ctx: &EngineContext,
) -> Result<Vec<ReportSection>>;
```

### Section Independence

Each section is generated independently. If one section fails, others continue. Failed sections return an error section:
```json
{
  "section_id": "technical",
  "content": { "key": "error.section_failed", "params": { "reason": "..." } },
  "data": null
}
```

## 6. Engine Context

Shared state passed to all operations:

```rust
pub struct EngineContext {
    pub data_client: MarketDataClient,
    pub llm_client: LlmClient,
    pub i18n: I18n,
    pub config: EngineConfig,
}
```

CLI creates EngineContext from env vars + args. MCP server creates it at startup and shares across tool calls.

## 7. File Changes Summary

### New Files
- `crates/sa-engine/src/main.rs` -> removed, replaced by bin/
- `crates/sa-engine/src/bin/sa-engine.rs`
- `crates/sa-engine/src/bin/sa-engine-mcp.rs`
- `crates/sa-engine/src/i18n/mod.rs`
- `crates/sa-engine/src/i18n/locales/zh.json`
- `crates/sa-engine/src/i18n/locales/en.json`

### Moved Files
- `crates/sa-types/src/*` -> `crates/sa-engine/src/types/`
- `crates/sa-models/src/*` -> `crates/sa-engine/src/models/`
- `crates/sa-data/src/*` -> `crates/sa-engine/src/data/`

### Modified Files
- `Cargo.toml` (workspace: single member)
- `crates/sa-engine/Cargo.toml` (merged deps + clap + rmcp)
- `crates/sa-engine/src/lib.rs` (new module structure)

### Deleted Files
- `crates/sa-types/` (after migration)
- `crates/sa-models/` (after migration)
- `crates/sa-data/` (after migration)
- `src/lib.rs` (workspace root lib, no longer needed)

## 8. Migration Strategy

1. Merge dependencies into sa-engine/Cargo.toml
2. Move sa-types code into sa-engine/src/types/, update imports
3. Move sa-models code into sa-engine/src/models/, update imports
4. Move sa-data code into sa-engine/src/data/, update imports
5. Update sa-engine/src/lib.rs with new module structure
6. Update workspace Cargo.toml
7. Verify compilation and tests pass
8. Add i18n module with zh.json and en.json
9. Add CLI binary (sa-engine.rs)
10. Add MCP binary (sa-engine-mcp.rs)
11. Remove old crate directories

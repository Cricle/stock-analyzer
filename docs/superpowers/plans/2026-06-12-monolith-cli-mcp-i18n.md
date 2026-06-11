# Monolith Merge + CLI + MCP + i18n Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Merge 4 crates into 1 monolith, add CLI + MCP server binaries, and introduce i18n key-based output.

**Architecture:** Single crate `sa-engine` with modules (types, models, data, engine) and two binaries (CLI via clap, MCP via rmcp). All text output uses i18n keys resolved by JSON locale files.

**Tech Stack:** Rust, clap, rmcp, serde_json, tokio

---

## File Structure

```
crates/sa-engine/
  Cargo.toml                          ← MODIFY: merge all deps + clap + rmcp
  src/
    lib.rs                            ← MODIFY: new module declarations
    types/                            ← MOVE from crates/sa-types/src/
      mod.rs                          ← was lib.rs
      (all other files unchanged)
    models/                           ← MOVE from crates/sa-models/src/
      mod.rs                          ← was lib.rs
      analysis/
      scoring/
      config.rs, market.rs, qlib.rs, store.rs, task.rs, user_preferences.rs, value_utils.rs
    data/                             ← MOVE from crates/sa-data/src/
      mod.rs                          ← was lib.rs
      a_share/, akshare_rust/, hk/, us/
      cache.rs, client.rs, diagnosis.rs, news_filter.rs, news_search.rs, qdrant.rs, search.rs, tushare.rs, wire.rs
    engine/                           ← MOVE current sa-engine/src/ analysis modules
      mod.rs                          ← new: re-exports from old sa-engine modules
      analysis/, checkpoint/, guidance/, llm/, memory/, qlib_import/, score/, shared/, stock_pick/, task_manager.rs, telemetry.rs, tools/
      config.rs
    i18n/                             ← NEW
      mod.rs
      locales/
        zh.json
        en.json
    bin/
      sa-engine.rs                    ← NEW: CLI binary
      sa-engine-mcp.rs                ← NEW: MCP server binary
```

Root `Cargo.toml`: workspace members = `["crates/sa-engine"]` only.

---

## Phase 1: Dependency Merge

### Task 1: Merge Cargo.toml dependencies

**Files:**
- Modify: `crates/sa-engine/Cargo.toml`
- Modify: `Cargo.toml` (workspace root)

- [ ] **Step 1: Read current sa-data and sa-models Cargo.toml to collect deps**

Read `crates/sa-data/Cargo.toml` and `crates/sa-models/Cargo.toml`. Identify dependencies not already in sa-engine:
- sa-data unique: `akshare`, `quick-xml`, `redis` (optional), `reqwest-tracing`
- sa-models unique: (none not already in sa-engine)

- [ ] **Step 2: Update sa-engine/Cargo.toml**

Add missing deps from sa-data. Keep all existing sa-engine deps. Add new deps:
```toml
clap = { version = "4", features = ["derive"] }
rmcp = { version = "0.1", features = ["server", "transport-io", "transport-streamable-http"] }
```

Merge feature flags:
```toml
[features]
default = []
local-rag-embeddings = ["fastembed"]
redis-cache = ["redis"]
```

Add `redis` as optional dependency (from sa-data).

- [ ] **Step 3: Update workspace Cargo.toml**

Change workspace members to single crate:
```toml
[workspace]
members = ["crates/sa-engine"]
resolver = "2"
```

- [ ] **Step 4: Verify deps resolve**

Run: `cargo check -p sa-engine 2>&1 | head -20`
Expected: Compiles (may have import errors, that's fine at this stage)

- [ ] **Step 5: Commit**

```bash
git add crates/sa-engine/Cargo.toml Cargo.toml
git commit -m "chore: merge all dependencies into sa-engine Cargo.toml"
```

---

## Phase 2: Merge sa-types into sa-engine

### Task 2: Move sa-types code

**Files:**
- Move: `crates/sa-types/src/*` -> `crates/sa-engine/src/types/`
- Modify: `crates/sa-engine/src/lib.rs`
- Modify: `crates/sa-engine/src/types/mod.rs` (was lib.rs)

- [ ] **Step 1: Create types directory and move files**

```bash
mkdir -p crates/sa-engine/src/types
cp crates/sa-types/src/lib.rs crates/sa-engine/src/types/mod.rs
# sa-types only has lib.rs, no other files
```

- [ ] **Step 2: Rename lib.rs to mod.rs and adjust**

The file `crates/sa-engine/src/types/mod.rs` is the same as sa-types/src/lib.rs. No changes needed — it already defines `MarketKind`, `QuoteSnapshot`, etc. with `pub` visibility.

- [ ] **Step 3: Add types module to sa-engine lib.rs**

In `crates/sa-engine/src/lib.rs`, add at the top:
```rust
pub mod types;
```

- [ ] **Step 4: Update sa-engine imports that reference sa_types**

In all files under `crates/sa-engine/src/`, replace:
- `use sa_types::` → `use crate::types::`
- `sa_types::` (in type paths) → `crate::types::`

Files to update (grep for `sa_types`):
```bash
grep -rn "sa_types" crates/sa-engine/src/ --include='*.rs'
```

For each match, replace `sa_types` with `crate::types`.

- [ ] **Step 5: Verify compilation**

Run: `cargo check -p sa-engine 2>&1 | head -30`
Expected: Types module compiles, other modules may still have errors from sa_models/sa_data references.

- [ ] **Step 6: Commit**

```bash
git add crates/sa-engine/src/types/ crates/sa-engine/src/lib.rs
grep -rl "sa_types" crates/sa-engine/src/ --include='*.rs' | xargs git add
git commit -m "refactor: merge sa-types into sa-engine/types module"
```

---

## Phase 3: Merge sa-models into sa-engine

### Task 3: Move sa-models code

**Files:**
- Move: `crates/sa-models/src/*` -> `crates/sa-engine/src/models/`
- Modify: `crates/sa-engine/src/lib.rs`
- Modify: `crates/sa-engine/src/models/mod.rs` (was lib.rs)
- Modify: ~2 files in sa-models that use `sa_types::`

- [ ] **Step 1: Move sa-models source tree**

```bash
cp -r crates/sa-models/src/* crates/sa-engine/src/models/
```

Rename `lib.rs` to `mod.rs`:
```bash
mv crates/sa-engine/src/models/lib.rs crates/sa-engine/src/models/mod.rs
```

- [ ] **Step 2: Update models/mod.rs imports**

In `crates/sa-engine/src/models/mod.rs`, this file was the sa-models lib.rs. It uses `pub use` to re-export types. No `sa_types` references here — it uses `use crate::` which will now resolve to `crate::models::` submodules.

Check: the `mod` declarations in this file (e.g., `pub mod analysis;`) stay the same since the directory structure is preserved.

- [ ] **Step 3: Fix sa_types references in models/**

In `crates/sa-engine/src/models/`, replace:
- `use sa_types::` → `use crate::types::`

Files to update:
- `crates/sa-engine/src/models/analysis/scenario_types.rs` (4 occurrences)
- `crates/sa-engine/src/models/analysis/report_logic/trader_plan/tests/part2.rs` (7 occurrences)

- [ ] **Step 4: Fix crate:: references in models/**

IMPORTANT: In the old sa-models crate, `use crate::X` referred to sa-models root. Now it must refer to `crate::models::X`.

In ALL files under `crates/sa-engine/src/models/`, replace:
- `use crate::` → `use crate::models::`

This affects files like:
- `crates/sa-engine/src/models/scoring/mod.rs`
- `crates/sa-engine/src/models/store.rs`
- `crates/sa-engine/src/models/task.rs`
- `crates/sa-engine/src/models/analysis/report_logic/trader_plan/tests/part1.rs`
- `crates/sa-engine/src/models/analysis/report_logic/probability.rs`

Use sed to batch-replace:
```bash
find crates/sa-engine/src/models -name '*.rs' -exec sed -i 's/use crate::/use crate::models::/g' {} +
```

- [ ] **Step 5: Add models module to lib.rs**

In `crates/sa-engine/src/lib.rs`, add:
```rust
pub mod models;
```

- [ ] **Step 6: Update sa-engine imports that reference sa_models**

In all files under `crates/sa-engine/src/engine/` (the original sa-engine code), replace:
- `use sa_models::` → `use crate::models::`
- `sa_models::` (in type paths) → `crate::models::`

Files (42 files, ~197 occurrences). Use sed:
```bash
find crates/sa-engine/src/engine -name '*.rs' -exec sed -i 's/sa_models/crate::models/g' {} +
```

Also update `crates/sa-engine/src/lib.rs` if it references `sa_models`.

- [ ] **Step 7: Verify compilation**

Run: `cargo check -p sa-engine 2>&1 | head -30`

- [ ] **Step 8: Commit**

```bash
git add crates/sa-engine/src/models/
grep -rl "sa_models" crates/sa-engine/src/ --include='*.rs' | xargs git add
git commit -m "refactor: merge sa-models into sa-engine/models module"
```

---

## Phase 4: Merge sa-data into sa-engine

### Task 4: Move sa-data code

**Files:**
- Move: `crates/sa-data/src/*` -> `crates/sa-engine/src/data/`
- Modify: `crates/sa-engine/src/lib.rs`
- Modify: `crates/sa-engine/src/data/mod.rs` (was lib.rs)

- [ ] **Step 1: Move sa-data source tree**

```bash
cp -r crates/sa-data/src/* crates/sa-engine/src/data/
```

Rename `lib.rs` to `mod.rs`:
```bash
mv crates/sa-engine/src/data/lib.rs crates/sa-engine/src/data/mod.rs
```

- [ ] **Step 2: Update data/mod.rs imports**

In `crates/sa-engine/src/data/mod.rs`, replace:
- `pub use sa_types::{` → `pub use crate::types::{`

This is the only `sa_types` reference in sa-data (1 occurrence in lib.rs line 7).

- [ ] **Step 3: Fix crate:: references in data/**

In the old sa-data crate, `use crate::X` referred to sa-data root. Now it must refer to `crate::data::X`.

In ALL files under `crates/sa-engine/src/data/`, replace:
- `use crate::` → `use crate::data::`

Use sed:
```bash
find crates/sa-engine/src/data -name '*.rs' -exec sed -i 's/use crate::/use crate::data::/g' {} +
```

- [ ] **Step 4: Add data module to lib.rs**

In `crates/sa-engine/src/lib.rs`, add:
```rust
pub mod data;
```

- [ ] **Step 5: Verify compilation**

Run: `cargo check -p sa-engine 2>&1 | head -30`

- [ ] **Step 6: Commit**

```bash
git add crates/sa-engine/src/data/
grep -rl "sa_data" crates/sa-engine/src/ --include='*.rs' | xargs git add
git commit -m "refactor: merge sa-data into sa-engine/data module"
```

---

## Phase 5: Restructure sa-engine internals into engine/ module

### Task 5: Move original sa-engine analysis code into engine/ submodule

**Files:**
- Move: `crates/sa-engine/src/analysis/` -> `crates/sa-engine/src/engine/analysis/`
- Move: `crates/sa-engine/src/checkpoint/` -> `crates/sa-engine/src/engine/checkpoint/`
- Move: `crates/sa-engine/src/guidance/` -> `crates/sa-engine/src/engine/guidance/`
- Move: `crates/sa-engine/src/llm/` -> `crates/sa-engine/src/engine/llm/`
- Move: `crates/sa-engine/src/memory/` -> `crates/sa-engine/src/engine/memory/`
- Move: `crates/sa-engine/src/qlib_import/` -> `crates/sa-engine/src/engine/qlib_import/`
- Move: `crates/sa-engine/src/score/` -> `crates/sa-engine/src/engine/score/`
- Move: `crates/sa-engine/src/shared.rs` -> `crates/sa-engine/src/engine/shared.rs`
- Move: `crates/sa-engine/src/stock_pick/` -> `crates/sa-engine/src/engine/stock_pick/`
- Move: `crates/sa-engine/src/task_manager.rs` -> `crates/sa-engine/src/engine/task_manager.rs`
- Move: `crates/sa-engine/src/telemetry.rs` -> `crates/sa-engine/src/engine/telemetry.rs`
- Move: `crates/sa-engine/src/tools/` -> `crates/sa-engine/src/engine/tools/`
- Move: `crates/sa-engine/src/config.rs` -> `crates/sa-engine/src/engine/config.rs`
- Create: `crates/sa-engine/src/engine/mod.rs`

- [ ] **Step 1: Create engine/ directory and move all analysis modules**

```bash
mkdir -p crates/sa-engine/src/engine
for dir in analysis checkpoint guidance llm memory qlib_import score stock_pick tools; do
  mv crates/sa-engine/src/$dir crates/sa-engine/src/engine/$dir
done
for f in shared.rs task_manager.rs telemetry.rs config.rs; do
  mv crates/sa-engine/src/$f crates/sa-engine/src/engine/$f
done
```

- [ ] **Step 2: Create engine/mod.rs**

Create `crates/sa-engine/src/engine/mod.rs` with all the module declarations that were in the old lib.rs:

```rust
//! sa-engine analysis modules.

pub mod config;
pub mod shared;
pub mod task_manager;
pub mod telemetry;

pub mod analysis;
pub mod memory;
pub mod checkpoint;
pub mod stock_pick;
pub mod qlib_import;
pub mod tools;
pub mod score;
pub mod guidance;
pub mod llm;

pub use task_manager::{TaskManager, TaskRunParams};
pub use task_manager::TASK_STEPS;
pub use telemetry::{SharedTelemetry, TelemetryState};
pub use stock_pick::run as run_stock_pick;
pub use score::scorer::score_stock_pick;
pub use qlib_import::{run_import as import_qlib, run_init_from_env as import_qlib_from_env};
pub use guidance::generate_prewarm_tasks;
pub use guidance::embedding::{semantic_embed, hash_embed, EMBEDDING_DIMENSION};
pub use telemetry::{init_telemetry, record_analysis_task_duration, record_llm_usage};
pub use config::{env_flag, env_flag_value};
pub use shared::{shared_http_client, safe_ticker_component};
```

- [ ] **Step 3: Fix crate:: references in engine/**

In the old sa-engine, `use crate::X` referred to sa-engine root (which had analysis/, guidance/, etc.). Now those modules are under `crate::engine::`, so internal references need updating.

In ALL files under `crates/sa-engine/src/engine/`, we need to handle TWO kinds of references:
1. References to other engine modules: `crate::memory::X` → `crate::engine::memory::X`
2. References to models/types: already updated in Phase 2/3

The tricky part: `use crate::X` in engine/ code that refers to engine submodules needs `crate::engine::` prefix. But references to `crate::models::X` or `crate::types::X` should NOT change.

Strategy: Only replace `use crate::` patterns that reference engine submodules. The engine submodules are: `analysis`, `checkpoint`, `guidance`, `llm`, `memory`, `qlib_import`, `score`, `shared`, `stock_pick`, `task_manager`, `telemetry`, `tools`, `config`.

```bash
for mod in analysis checkpoint guidance llm memory qlib_import score shared stock_pick task_manager telemetry tools config; do
  find crates/sa-engine/src/engine -name '*.rs' -exec sed -i "s/use crate::${mod}/use crate::engine::${mod}/g" {} +
  find crates/sa-engine/src/engine -name '*.rs' -exec sed -i "s/crate::${mod}::/crate::engine::${mod}::/g" {} +
done
```

Also fix `crate::TASK_STEPS`, `crate::TaskManager`, etc. to `crate::engine::TASK_STEPS`, `crate::engine::TaskManager`:
```bash
# These re-exports need updating in any engine/ file that uses them
find crates/sa-engine/src/engine -name '*.rs' -exec sed -i 's/crate::TaskManager/crate::engine::TaskManager/g' {} +
find crates/sa-engine/src/engine -name '*.rs' -exec sed -i 's/crate::TaskRunParams/crate::engine::TaskRunParams/g' {} +
find crates/sa-engine/src/engine -name '*.rs' -exec sed -i 's/crate::SharedTelemetry/crate::engine::SharedTelemetry/g' {} +
find crates/sa-engine/src/engine -name '*.rs' -exec sed -i 's/crate::TelemetryState/crate::engine::TelemetryState/g' {} +
```

- [ ] **Step 4: Update lib.rs**

Replace the old `crates/sa-engine/src/lib.rs` content with:

```rust
//! sa-engine — Monolithic stock analysis engine.

pub mod types;
pub mod models;
pub mod data;
pub mod engine;

// Convenience re-exports at crate root
pub use engine::{
    TaskManager, TaskRunParams, TASK_STEPS,
    SharedTelemetry, TelemetryState,
    run_stock_pick, score_stock_pick,
    import_qlib, import_qlib_from_env,
    generate_prewarm_tasks,
    semantic_embed, hash_embed, EMBEDDING_DIMENSION,
    init_telemetry, record_analysis_task_duration, record_llm_usage,
    env_flag, env_flag_value,
    shared_http_client, safe_ticker_component,
};
```

- [ ] **Step 5: Verify compilation**

Run: `cargo check -p sa-engine 2>&1 | head -50`

Fix any remaining import issues iteratively.

- [ ] **Step 6: Run all tests**

Run: `cargo test -p sa-engine 2>&1 | tail -20`
Expected: All existing tests pass.

- [ ] **Step 7: Commit**

```bash
git add crates/sa-engine/src/
git commit -m "refactor: move sa-engine analysis code into engine/ submodule"
```

---

## Phase 6: Cleanup old crates

### Task 6: Remove old crate directories

**Files:**
- Delete: `crates/sa-types/`
- Delete: `crates/sa-models/`
- Delete: `crates/sa-data/`
- Delete: `src/lib.rs` (workspace root lib)

- [ ] **Step 1: Remove old crate directories**

```bash
rm -rf crates/sa-types crates/sa-models crates/sa-data
rm -f src/lib.rs
rmdir src 2>/dev/null || true
```

- [ ] **Step 2: Verify workspace builds**

Run: `cargo check -p sa-engine 2>&1 | tail -10`
Expected: Clean compilation.

- [ ] **Step 3: Run full test suite**

Run: `cargo test -p sa-engine 2>&1 | tail -20`
Expected: All tests pass.

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "chore: remove old sa-types, sa-models, sa-data crate directories"
```

---

## Phase 7: i18n System

### Task 7: Create i18n module

**Files:**
- Create: `crates/sa-engine/src/i18n/mod.rs`
- Create: `crates/sa-engine/src/i18n/locales/zh.json`
- Create: `crates/sa-engine/src/i18n/locales/en.json`
- Modify: `crates/sa-engine/src/lib.rs`

- [ ] **Step 1: Write the failing test for i18n resolve**

Create `crates/sa-engine/src/i18n/mod.rs`:

```rust
use std::collections::HashMap;

pub struct I18n {
    locales: HashMap<String, serde_json::Value>,
}

impl I18n {
    pub fn new() -> Self {
        let mut locales = HashMap::new();
        let zh = include_str!("locales/zh.json");
        let en = include_str!("locales/en.json");
        locales.insert("zh".to_string(), serde_json::from_str(zh).unwrap());
        locales.insert("en".to_string(), serde_json::from_str(en).unwrap());
        Self { locales }
    }

    pub fn resolve(&self, key: &str, lang: &str) -> Option<String> {
        let root = self.locales.get(lang)?;
        let parts: Vec<&str> = key.split('.').collect();
        let mut current = root;
        for part in &parts {
            current = current.get(part)?;
        }
        current.as_str().map(|s| s.to_string())
    }

    pub fn resolve_with_params(
        &self,
        key: &str,
        lang: &str,
        params: &serde_json::Map<String, serde_json::Value>,
    ) -> Option<String> {
        let template = self.resolve(key, lang)?;
        let mut result = template;
        for (k, v) in params {
            let placeholder = format!("{{{}}}", k);
            let value = match v {
                serde_json::Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            result = result.replace(&placeholder, &value);
        }
        Some(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_zh_key() {
        let i18n = I18n::new();
        let result = i18n.resolve("report.summary.title", "zh");
        assert_eq!(result.as_deref(), Some("分析摘要"));
    }

    #[test]
    fn resolve_en_key() {
        let i18n = I18n::new();
        let result = i18n.resolve("report.summary.title", "en");
        assert_eq!(result.as_deref(), Some("Analysis Summary"));
    }

    #[test]
    fn resolve_missing_key() {
        let i18n = I18n::new();
        let result = i18n.resolve("nonexistent.key", "zh");
        assert!(result.is_none());
    }

    #[test]
    fn resolve_with_params() {
        let i18n = I18n::new();
        let mut params = serde_json::Map::new();
        params.insert("symbol".to_string(), serde_json::Value::String("AAPL".to_string()));
        let result = i18n.resolve_with_params("report.header.for_symbol", "zh", &params);
        assert_eq!(result.as_deref(), Some("AAPL 的分析报告"));
    }
}
```

- [ ] **Step 2: Create locale JSON files**

Create `crates/sa-engine/src/i18n/locales/zh.json`:
```json
{
  "report": {
    "summary": {
      "title": "分析摘要"
    },
    "technical": {
      "title": "技术指标"
    },
    "risk": {
      "title": "风险评估"
    },
    "catalyst": {
      "title": "催化剂分析"
    },
    "decision": {
      "title": "决策视图"
    },
    "diagnostics": {
      "title": "诊断信息"
    },
    "probability": {
      "title": "概率评估"
    },
    "trader_plan": {
      "title": "交易计划"
    },
    "header": {
      "for_symbol": "{symbol} 的分析报告"
    }
  },
  "guidance": {
    "title": "每日指引"
  },
  "stock_pick": {
    "title": "选股结果"
  },
  "error": {
    "section_failed": "板块生成失败: {reason}"
  }
}
```

Create `crates/sa-engine/src/i18n/locales/en.json`:
```json
{
  "report": {
    "summary": {
      "title": "Analysis Summary"
    },
    "technical": {
      "title": "Technical Indicators"
    },
    "risk": {
      "title": "Risk Assessment"
    },
    "catalyst": {
      "title": "Catalyst Analysis"
    },
    "decision": {
      "title": "Decision View"
    },
    "diagnostics": {
      "title": "Diagnostics"
    },
    "probability": {
      "title": "Probability Assessment"
    },
    "trader_plan": {
      "title": "Trading Plan"
    },
    "header": {
      "for_symbol": "Analysis Report for {symbol}"
    }
  },
  "guidance": {
    "title": "Daily Guidance"
  },
  "stock_pick": {
    "title": "Stock Selection"
  },
  "error": {
    "section_failed": "Section generation failed: {reason}"
  }
}
```

- [ ] **Step 3: Add i18n module to lib.rs**

In `crates/sa-engine/src/lib.rs`, add:
```rust
pub mod i18n;
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p sa-engine --lib i18n 2>&1`
Expected: All 4 i18n tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/sa-engine/src/i18n/ crates/sa-engine/src/lib.rs
git commit -m "feat: add i18n module with zh/en locale support"
```

---

## Phase 8: CLI Binary

### Task 8: Create CLI binary with clap

**Files:**
- Create: `crates/sa-engine/src/bin/sa-engine.rs`
- Modify: `crates/sa-engine/Cargo.toml` (add [[bin]] section)

- [ ] **Step 1: Write the CLI skeleton with clap**

Create `crates/sa-engine/src/bin/sa-engine.rs`:

```rust
use clap::{Parser, Subcommand, ValueEnum};
use serde::Serialize;

#[derive(Parser)]
#[command(name = "sa-engine", about = "Stock analysis engine CLI")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(ValueEnum, Clone, Debug)]
enum Market {
    #[value(name = "a-share")]
    AShare,
    Hk,
    Us,
}

impl Market {
    fn to_market_kind(&self) -> sa_engine::types::MarketKind {
        match self {
            Market::AShare => sa_engine::types::MarketKind::AShare,
            Market::Hk => sa_engine::types::MarketKind::HongKong,
            Market::Us => sa_engine::types::MarketKind::UsEquity,
        }
    }
}

#[derive(ValueEnum, Clone, Debug)]
enum Lang {
    Zh,
    En,
}

impl Lang {
    fn as_str(&self) -> &'static str {
        match self {
            Lang::Zh => "zh",
            Lang::En => "en",
        }
    }
}

#[derive(Subcommand)]
enum Command {
    /// Generate daily market guidance
    Guidance {
        /// Target market
        #[arg(long, value_enum, default_value_t = Market::AShare)]
        market: Market,
        /// Output language (if set, resolves i18n keys to text)
        #[arg(long, value_enum)]
        lang: Option<Lang>,
    },
    /// Run stock selection
    StockPick {
        /// Target market
        #[arg(long, value_enum, default_value_t = Market::AShare)]
        market: Market,
        /// Analysis date (YYYY-MM-DD, defaults to today)
        #[arg(long)]
        date: Option<String>,
        /// Output language
        #[arg(long, value_enum)]
        lang: Option<Lang>,
    },
    /// Generate analysis report
    Report {
        /// Stock symbol
        #[arg(long)]
        symbol: String,
        /// Target market
        #[arg(long, value_enum, default_value_t = Market::AShare)]
        market: Market,
        /// Comma-separated section IDs to generate (default: all)
        #[arg(long, value_delimiter = ',')]
        sections: Option<Vec<String>>,
        /// Output language
        #[arg(long, value_enum)]
        lang: Option<Lang>,
    },
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    match cli.command {
        Command::Guidance { market, lang } => {
            eprintln!("guidance: market={:?}, lang={:?}", market, lang);
            // TODO: call engine::guidance
            println!("{{}}");
        }
        Command::StockPick { market, date, lang } => {
            eprintln!("stock-pick: market={:?}, date={:?}, lang={:?}", market, date, lang);
            // TODO: call engine::stock_pick
            println!("{{}}");
        }
        Command::Report { symbol, market, sections, lang } => {
            eprintln!("report: symbol={}, market={:?}, sections={:?}, lang={:?}", symbol, market, sections, lang);
            // TODO: call engine::report
            println!("{{}}");
        }
    }
}
```

- [ ] **Step 2: Add [[bin]] to Cargo.toml**

In `crates/sa-engine/Cargo.toml`, add:
```toml
[[bin]]
name = "sa-engine"
path = "src/bin/sa-engine.rs"
```

- [ ] **Step 3: Verify it compiles and runs**

Run: `cargo build -p sa-engine --bin sa-engine 2>&1 | tail -5`
Expected: Binary compiles.

Run: `cargo run -p sa-engine --bin sa-engine -- --help`
Expected: Shows help text with guidance/stock-pick/report subcommands.

Run: `cargo run -p sa-engine --bin sa-engine -- guidance --help`
Expected: Shows guidance subcommand help.

- [ ] **Step 4: Commit**

```bash
git add crates/sa-engine/src/bin/sa-engine.rs crates/sa-engine/Cargo.toml
git commit -m "feat: add CLI binary skeleton with clap (guidance/stock-pick/report)"
```

---

## Phase 9: MCP Server Binary

### Task 9: Create MCP server binary with rmcp

**Files:**
- Create: `crates/sa-engine/src/bin/sa-engine-mcp.rs`
- Modify: `crates/sa-engine/Cargo.toml` (add [[bin]] section)

- [ ] **Step 1: Write the MCP server skeleton**

Create `crates/sa-engine/src/bin/sa-engine-mcp.rs`:

```rust
use clap::{Parser, ValueEnum};
use rmcp::{
    ServerHandler,
    model::{CallToolResult, Content, ServerInfo, Tool},
    tool,
};

#[derive(Parser)]
#[command(name = "sa-engine-mcp", about = "Stock analysis MCP server")]
struct Cli {
    /// Transport type
    #[arg(long, value_enum, default_value_t = Transport::Stdio)]
    transport: Transport,
    /// HTTP port (only used with http transport)
    #[arg(long, default_value_t = 3000)]
    port: u16,
}

#[derive(ValueEnum, Clone, Debug)]
enum Transport {
    Stdio,
    Http,
}

#[derive(Clone)]
struct StockAnalyzerServer;

impl StockAnalyzerServer {
    fn new() -> Self {
        Self
    }
}

#[tool]
impl StockAnalyzerServer {
    async fn generate_guidance(&self, market: String) -> Result<CallToolResult, rmcp::Error> {
        let result = serde_json::json!({
            "data": {},
            "i18n_keys": ["guidance.title"],
            "lang": "zh"
        });
        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&result).unwrap(),
        )]))
    }

    async fn stock_pick(&self, market: String, date: Option<String>) -> Result<CallToolResult, rmcp::Error> {
        let result = serde_json::json!({
            "data": {},
            "i18n_keys": ["stock_pick.title"],
            "lang": "zh"
        });
        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&result).unwrap(),
        )]))
    }

    async fn generate_report(
        &self,
        symbol: String,
        market: Option<String>,
        sections: Option<Vec<String>>,
    ) -> Result<CallToolResult, rmcp::Error> {
        let result = serde_json::json!({
            "data": {},
            "i18n_keys": ["report.summary.title"],
            "lang": "zh"
        });
        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&result).unwrap(),
        )]))
    }
}

impl ServerHandler for StockAnalyzerServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            name: Some("sa-engine".to_string()),
            version: Some("0.1.0".to_string()),
            ..Default::default()
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match cli.transport {
        Transport::Stdio => {
            let server = StockAnalyzerServer::new();
            let transport = rmcp::transport::io::stdio();
            server.run(transport).await?;
        }
        Transport::Http => {
            eprintln!("HTTP transport on port {} (not yet implemented)", cli.port);
            // TODO: implement HTTP transport
        }
    }

    Ok(())
}
```

- [ ] **Step 2: Add [[bin]] to Cargo.toml**

In `crates/sa-engine/Cargo.toml`, add:
```toml
[[bin]]
name = "sa-engine-mcp"
path = "src/bin/sa-engine-mcp.rs"
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo build -p sa-engine --bin sa-engine-mcp 2>&1 | tail -5`
Expected: Binary compiles.

- [ ] **Step 4: Commit**

```bash
git add crates/sa-engine/src/bin/sa-engine-mcp.rs crates/sa-engine/Cargo.toml
git commit -m "feat: add MCP server binary skeleton with rmcp (stdio/HTTP)"
```

---

## Phase 10: Wire up CLI to real engine

### Task 10: Connect CLI subcommands to engine functions

**Files:**
- Modify: `crates/sa-engine/src/bin/sa-engine.rs`

- [ ] **Step 1: Implement guidance subcommand**

Replace the guidance TODO in `sa-engine.rs` with actual engine call:

```rust
Command::Guidance { market, lang } => {
    let market_kind = market.to_market_kind();
    let ctx = sa_engine::engine::shared::build_engine_context().await;
    let result = sa_engine::engine::guidance::generate_daily_guidance(market_kind, &ctx).await;
    match result {
        Ok(data) => {
            let output = if let Some(l) = lang {
                let i18n = sa_engine::i18n::I18n::new();
                // resolve keys in output
                resolve_output_keys(&data, &i18n, l.as_str())
            } else {
                data
            };
            println!("{}", serde_json::to_string_pretty(&output).unwrap());
        }
        Err(e) => {
            eprintln!("{}", serde_json::json!({"error": {"code": "guidance_failed", "message": e.to_string()}}));
            std::process::exit(1);
        }
    }
}
```

Note: The exact function signatures for `generate_daily_guidance` and similar depend on the existing engine code. The implementer must check the actual function signatures in `engine/guidance/mod.rs`, `engine/stock_pick/mod.rs`, and `engine/analysis/` and wire them up accordingly.

- [ ] **Step 2: Implement stock-pick subcommand**

Similar pattern — call `sa_engine::engine::stock_pick::run()` or equivalent.

- [ ] **Step 3: Implement report subcommand**

Call `sa_engine::engine::analysis::` report generation. Use the `sections` parameter to filter which sections to generate.

- [ ] **Step 4: Implement resolve_output_keys helper**

Add a helper function that walks the JSON output and resolves any `LocalText` objects to their display text using `I18n::resolve_with_params`.

- [ ] **Step 5: Test CLI end-to-end**

Run: `cargo run -p sa-engine --bin sa-engine -- guidance --market a-share 2>&1 | head -20`
Expected: JSON output (or meaningful error if env vars not set).

- [ ] **Step 6: Commit**

```bash
git add crates/sa-engine/src/bin/sa-engine.rs
git commit -m "feat: wire CLI subcommands to engine functions"
```

---

## Phase 11: Wire up MCP to real engine

### Task 11: Connect MCP tools to engine functions

**Files:**
- Modify: `crates/sa-engine/src/bin/sa-engine-mcp.rs`

- [ ] **Step 1: Implement generate_guidance tool**

Replace the stub with actual engine call, same pattern as CLI.

- [ ] **Step 2: Implement stock_pick tool**

Wire to `sa_engine::engine::stock_pick::run()` or equivalent.

- [ ] **Step 3: Implement generate_report tool**

Wire to report generation with section filtering.

- [ ] **Step 4: Implement HTTP transport**

Add HTTP + SSE transport support using rmcp's HTTP transport feature.

- [ ] **Step 5: Test MCP server**

Run: `echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"capabilities":{}}}' | cargo run -p sa-engine --bin sa-engine-mcp -- --transport stdio 2>&1 | head -5`
Expected: JSON-RPC response.

- [ ] **Step 6: Commit**

```bash
git add crates/sa-engine/src/bin/sa-engine-mcp.rs
git commit -m "feat: wire MCP tools to engine functions"
```

---

## Phase 12: Final verification

### Task 12: Full build and test

- [ ] **Step 1: Run full compilation**

Run: `cargo build -p sa-engine 2>&1 | tail -10`
Expected: Clean build for both binaries.

- [ ] **Step 2: Run full test suite**

Run: `cargo test -p sa-engine 2>&1 | tail -30`
Expected: All tests pass.

- [ ] **Step 3: Verify CLI help**

Run: `cargo run -p sa-engine --bin sa-engine -- --help`
Run: `cargo run -p sa-engine --bin sa-engine -- guidance --help`
Run: `cargo run -p sa-engine --bin sa-engine -- report --help`

- [ ] **Step 4: Final commit**

```bash
git add -A
git commit -m "chore: final cleanup and verification"
```

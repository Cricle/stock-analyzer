# Code Optimization Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Eliminate dead code, fix clippy warnings, optimize memory allocations, and clean up TODO stubs while maintaining identical functionality.

**Architecture:** Incremental approach — fix one category at a time, verify compilation and tests after each phase. All changes are in Rust crates under `crates/` and the akshare-rs dependency.

**Tech Stack:** Rust, Cargo, Clippy, akshare-rs

---

## Phase 1: Fix Clippy Warnings

### Task 1: Remove unused imports in akshare-rs a_share module

**Files:**
- Modify: `/root/github/akshare-rs/crates/akshare/src/provider/market_client/a_share/mod.rs`

- [ ] **Step 1: Read the file to see current imports**

```bash
head -20 /root/github/akshare-rs/crates/akshare/src/provider/market_client/a_share/mod.rs
```

- [ ] **Step 2: Remove unused imports**

Remove these lines:
- Line 4: `use rust_decimal::Decimal;`
- Line 12: `f64_to_dec` and `opt_f64_to_dec` from the import

- [ ] **Step 3: Verify compilation**

```bash
cd /root/github/akshare-rs && cargo clippy --package akshare 2>&1 | grep "a_share"
```
Expected: No warnings for a_share module

- [ ] **Step 4: Commit**

```bash
cd /root/github/akshare-rs && git add crates/akshare/src/provider/market_client/a_share/mod.rs && git commit -m "fix: remove unused imports in a_share module"
```

---

### Task 2: Remove unused imports in akshare-rs hk module

**Files:**
- Modify: `/root/github/akshare-rs/crates/akshare/src/provider/market_client/hk/mod.rs`

- [ ] **Step 1: Read the file**

```bash
head -20 /root/github/akshare-rs/crates/akshare/src/provider/market_client/hk/mod.rs
```

- [ ] **Step 2: Remove unused imports**

Remove these lines:
- Line 13: `use rust_decimal::Decimal;`
- Line 7: `opt_f64_to_dec` from the import

- [ ] **Step 3: Verify compilation**

```bash
cd /root/github/akshare-rs && cargo clippy --package akshare 2>&1 | grep "hk"
```
Expected: No warnings for hk module

- [ ] **Step 4: Commit**

```bash
cd /root/github/akshare-rs && git add crates/akshare/src/provider/market_client/hk/mod.rs && git commit -m "fix: remove unused imports in hk module"
```

---

### Task 3: Remove unused imports in akshare-rs us module

**Files:**
- Modify: `/root/github/akshare-rs/crates/akshare/src/provider/market_client/us/mod.rs`

- [ ] **Step 1: Read the file**

```bash
head -20 /root/github/akshare-rs/crates/akshare/src/provider/market_client/us/mod.rs
```

- [ ] **Step 2: Remove unused imports**

Remove these lines:
- Line 7: `use rust_decimal::Decimal;`
- Line 17: `f64_to_dec` and `opt_f64_to_dec` from the import

- [ ] **Step 3: Verify compilation**

```bash
cd /root/github/akshare-rs && cargo clippy --package akshare 2>&1 | grep "us"
```
Expected: No warnings for us module

- [ ] **Step 4: Commit**

```bash
cd /root/github/akshare-rs && git add crates/akshare/src/provider/market_client/us/mod.rs && git commit -m "fix: remove unused imports in us module"
```

---

### Task 4: Remove unused functions in akshare-rs akshare_rust module

**Files:**
- Modify: `/root/github/akshare-rs/crates/akshare/src/provider/market_client/akshare_rust/mod.rs`
- Modify: `/root/github/akshare-rs/crates/akshare/src/provider/market_client/akshare_rust/a_share.rs`

- [ ] **Step 1: Read mod.rs**

```bash
head -120 /root/github/akshare-rs/crates/akshare/src/provider/market_client/akshare_rust/mod.rs
```

- [ ] **Step 2: Remove unused functions from mod.rs**

Remove these functions:
- `quote_source` (line 13)
- `fundamentals_source` (line 21)
- `news_source` (line 29)
- `candles_source` (line 37)
- `search_stocks` (line 101)

- [ ] **Step 3: Read a_share.rs**

```bash
head -30 /root/github/akshare-rs/crates/akshare/src/provider/market_client/akshare_rust/a_share.rs
```

- [ ] **Step 4: Remove unused function from a_share.rs**

Remove `search_stocks` function (line 18)

- [ ] **Step 5: Verify compilation**

```bash
cd /root/github/akshare-rs && cargo clippy --package akshare 2>&1 | grep "akshare_rust"
```
Expected: No warnings

- [ ] **Step 6: Commit**

```bash
cd /root/github/akshare-rs && git add crates/akshare/src/provider/market_client/akshare_rust/ && git commit -m "fix: remove unused functions in akshare_rust module"
```

---

### Task 5: Remove unused functions and imports in akshare-rs mod.rs

**Files:**
- Modify: `/root/github/akshare-rs/crates/akshare/src/provider/market_client/mod.rs`

- [ ] **Step 1: Read the file**

```bash
head -80 /root/github/akshare-rs/crates/akshare/src/provider/market_client/mod.rs
```

- [ ] **Step 2: Remove unused functions**

Remove these functions:
- `f64_to_dec` (line 67)
- `opt_f64_to_dec` (line 71)

- [ ] **Step 3: Verify compilation**

```bash
cd /root/github/akshare-rs && cargo clippy --package akshare 2>&1 | grep "mod.rs"
```
Expected: No warnings

- [ ] **Step 4: Commit**

```bash
cd /root/github/akshare-rs && git add crates/akshare/src/provider/market_client/mod.rs && git commit -m "fix: remove unused functions in market_client mod.rs"
```

---

### Task 6: Remove unused imports in akshare-rs client.rs

**Files:**
- Modify: `/root/github/akshare-rs/crates/akshare/src/provider/market_client/client.rs`

- [ ] **Step 1: Read the file**

```bash
head -25 /root/github/akshare-rs/crates/akshare/src/provider/market_client/client.rs
```

- [ ] **Step 2: Remove unused imports**

Remove these imports:
- Line 1: `use std::time::Duration;`
- Line 9: `DataConfig` and `SearchProviderConfig` from the import
- Line 18: `Singleflight` from the import

- [ ] **Step 3: Verify compilation**

```bash
cd /root/github/akshare-rs && cargo clippy --package akshare 2>&1 | grep "client.rs"
```
Expected: No warnings

- [ ] **Step 4: Commit**

```bash
cd /root/github/akshare-rs && git add crates/akshare/src/provider/market_client/client.rs && git commit -m "fix: remove unused imports in client.rs"
```

---

### Task 7: Remove unused imports in stock-analyzer sa-data

**Files:**
- Modify: `/root/github/stock-analyzer/crates/sa-data/src/lib.rs`

- [ ] **Step 1: Read the file**

```bash
head -60 /root/github/stock-analyzer/crates/sa-data/src/lib.rs
```

- [ ] **Step 2: Remove unused imports**

Remove line 54: `SingleflightResult` and `Singleflight` imports

- [ ] **Step 3: Verify compilation**

```bash
cd /root/github/stock-analyzer && cargo clippy --package sa-data 2>&1 | grep "lib.rs"
```
Expected: No warnings

- [ ] **Step 4: Commit**

```bash
cd /root/github/stock-analyzer && git add crates/sa-data/src/lib.rs && git commit -m "fix: remove unused imports in sa-data"
```

---

### Task 8: Verify all clippy warnings resolved

**Files:**
- None (verification only)

- [ ] **Step 1: Run clippy on akshare-rs**

```bash
cd /root/github/akshare-rs && cargo clippy --workspace 2>&1 | tail -5
```
Expected: 0 warnings

- [ ] **Step 2: Run clippy on stock-analyzer**

```bash
cd /root/github/stock-analyzer && cargo clippy --workspace 2>&1 | tail -5
```
Expected: 0 warnings

- [ ] **Step 3: Run tests to verify no regressions**

```bash
cd /root/github/stock-analyzer && cargo test --workspace --lib 2>&1 | tail -10
```
Expected: All tests pass

---

## Phase 2: Remove Dead Code

### Task 9: Remove unused re-exports from sa-data

**Files:**
- Modify: `/root/github/stock-analyzer/crates/sa-data/src/lib.rs`

- [ ] **Step 1: Read the file**

```bash
cat /root/github/stock-analyzer/crates/sa-data/src/lib.rs
```

- [ ] **Step 2: Remove unused re-exports**

Remove any re-exports that are not used by sa-engine or sa-models. Verify with:

```bash
grep -rn "use sa_data::" crates/sa-engine/src/ crates/sa-models/src/ | head -20
```

- [ ] **Step 3: Verify compilation**

```bash
cd /root/github/stock-analyzer && cargo build --workspace 2>&1 | tail -5
```
Expected: Build succeeds

- [ ] **Step 4: Commit**

```bash
cd /root/github/stock-analyzer && git add crates/sa-data/src/lib.rs && git commit -m "refactor: remove unused re-exports from sa-data"
```

---

### Task 10: Identify and remove dead code in sa-engine

**Files:**
- Various files in `crates/sa-engine/src/`

- [ ] **Step 1: Find unused functions**

```bash
cd /root/github/stock-analyzer && cargo clippy --workspace -- -W dead_code 2>&1 | grep "never used" | head -20
```

- [ ] **Step 2: For each unused function, verify it's truly dead**

```bash
grep -rn "function_name" crates/ --include="*.rs" | grep -v "fn function_name"
```

- [ ] **Step 3: Remove dead functions**

Remove each function that has no callers.

- [ ] **Step 4: Verify compilation**

```bash
cd /root/github/stock-analyzer && cargo build --workspace 2>&1 | tail -5
```
Expected: Build succeeds

- [ ] **Step 5: Commit**

```bash
cd /root/github/stock-analyzer && git add -A && git commit -m "refactor: remove dead code from sa-engine"
```

---

## Phase 3: Optimize Memory Allocations

### Task 11: Reduce clones in stock_pick/pipeline/mod.rs

**Files:**
- Modify: `/root/github/stock-analyzer/crates/sa-engine/src/stock_pick/pipeline/mod.rs`

- [ ] **Step 1: Read the file and identify clone hotspots**

```bash
grep -n "\.clone()" crates/sa-engine/src/stock_pick/pipeline/mod.rs | head -30
```

- [ ] **Step 2: Replace unnecessary clones with references**

Example patterns to fix:

**Pattern A: String clones only used for reading**
```rust
// Before
let symbol = pick.symbol.clone();
let name = pick.name.clone();
format!("{}: {}", symbol, name)

// After
format!("{}: {}", pick.symbol, pick.name)
```

**Pattern B: Vec clones only iterated**
```rust
// Before
let codes = pick.risk_snapshot.signal_codes.clone();
for code in codes { ... }

// After
for code in &pick.risk_snapshot.signal_codes { ... }
```

**Pattern C: Struct clones only accessed**
```rust
// Before
let snapshot = item.market_snapshot.clone();
let price = snapshot.current_price;

// After
let price = item.market_snapshot.current_price;
```

- [ ] **Step 3: Verify compilation**

```bash
cd /root/github/stock-analyzer && cargo build --package sa-engine 2>&1 | tail -5
```
Expected: Build succeeds

- [ ] **Step 4: Run tests**

```bash
cd /root/github/stock-analyzer && cargo test --package sa-engine --lib 2>&1 | tail -10
```
Expected: All tests pass

- [ ] **Step 5: Commit**

```bash
cd /root/github/stock-analyzer && git add crates/sa-engine/src/stock_pick/pipeline/mod.rs && git commit -m "perf: reduce clones in stock_pick pipeline"
```

---

### Task 12: Reduce clones in llm/client modules

**Files:**
- Modify: `/root/github/stock-analyzer/crates/sa-engine/src/llm/client/anthropic.rs`
- Modify: `/root/github/stock-analyzer/crates/sa-engine/src/llm/client/streaming.rs`
- Modify: `/root/github/stock-analyzer/crates/sa-engine/src/llm/client/generation.rs`

- [ ] **Step 1: Identify clone patterns**

```bash
grep -n "\.clone()" crates/sa-engine/src/llm/client/*.rs | head -20
```

- [ ] **Step 2: Optimize async move blocks**

Example pattern to fix:

```rust
// Before
let url = url.clone();
let model = self.model.clone();
let api_key = self.openai_api_key.clone();
let http = self.http.clone();
let tracker = self.usage_tracker.clone();
tokio::spawn(async move {
    // use url, model, api_key, http, tracker
})

// After - clone only what's needed
let url = url.clone();
let api_key = self.openai_api_key.clone();
let http = self.http.clone();
tokio::spawn(async move {
    // use url, api_key, http
    // pass self.model by reference if only read
})
```

- [ ] **Step 3: Verify compilation**

```bash
cd /root/github/stock-analyzer && cargo build --package sa-engine 2>&1 | tail -5
```
Expected: Build succeeds

- [ ] **Step 4: Run tests**

```bash
cd /root/github/stock-analyzer && cargo test --package sa-engine --lib 2>&1 | tail -10
```
Expected: All tests pass

- [ ] **Step 5: Commit**

```bash
cd /root/github/stock-analyzer && git add crates/sa-engine/src/llm/client/ && git commit -m "perf: reduce clones in llm client modules"
```

---

### Task 13: Add capacity pre-allocation

**Files:**
- Various files in `crates/sa-engine/src/`

- [ ] **Step 1: Find Vec::new() and String::new() in hot paths**

```bash
grep -rn "Vec::new()\|String::new()" crates/sa-engine/src/ --include="*.rs" | grep -v test | head -20
```

- [ ] **Step 2: Add with_capacity where size is known or estimable**

Example patterns to fix:

```rust
// Before
let mut codes = Vec::new();
codes.push("score_leader".to_string());
codes.push("technical_support".to_string());
// ... more pushes

// After
let mut codes = Vec::with_capacity(5);
codes.push("score_leader".to_string());
codes.push("technical_support".to_string());
// ... more pushes
```

```rust
// Before
let mut output = String::new();
output.push_str(&section1);
output.push_str(&section2);
// ... more pushes

// After
let mut output = String::with_capacity(1024);
output.push_str(&section1);
output.push_str(&section2);
// ... more pushes
```

- [ ] **Step 3: Verify compilation**

```bash
cd /root/github/stock-analyzer && cargo build --package sa-engine 2>&1 | tail -5
```
Expected: Build succeeds

- [ ] **Step 4: Run tests**

```bash
cd /root/github/stock-analyzer && cargo test --package sa-engine --lib 2>&1 | tail -10
```
Expected: All tests pass

- [ ] **Step 5: Commit**

```bash
cd /root/github/stock-analyzer && git add -A && git commit -m "perf: add capacity pre-allocation for Vec and String"
```

---

## Phase 4: Clean Up TODO Stubs

### Task 14: Evaluate and clean up storage-related TODOs

**Files:**
- Modify: `/root/github/stock-analyzer/crates/sa-engine/src/stock_pick/pipeline/mod.rs`
- Modify: `/root/github/stock-analyzer/crates/sa-engine/src/stock_pick/history/mod.rs`
- Modify: `/root/github/stock-analyzer/crates/sa-engine/src/memory/embedding.rs`
- Modify: `/root/github/stock-analyzer/crates/sa-engine/src/memory/vector_store.rs`
- Modify: `/root/github/stock-analyzer/crates/sa-engine/src/score/history.rs`
- Modify: `/root/github/stock-analyzer/crates/sa-engine/src/checkpoint/mod.rs`
- Modify: `/root/github/stock-analyzer/crates/sa-engine/src/guidance/store/*.rs`

- [ ] **Step 1: Trace call sites for each TODO**

For each TODO, determine if the code path is actually executed:

```bash
grep -rn "function_name" crates/ --include="*.rs" | grep -v "fn function_name" | grep -v "TODO"
```

- [ ] **Step 2: For unused code paths, remove them**

If a TODO stub is never called, remove the entire function or code block.

- [ ] **Step 3: For used code paths, verify trait-based implementation works**

If a TODO stub is called, verify the current implementation (using traits) works correctly. If it does, remove the TODO comment. If it doesn't, implement properly.

- [ ] **Step 4: Verify compilation**

```bash
cd /root/github/stock-analyzer && cargo build --workspace 2>&1 | tail -5
```
Expected: Build succeeds

- [ ] **Step 5: Run tests**

```bash
cd /root/github/stock-analyzer && cargo test --workspace --lib 2>&1 | tail -10
```
Expected: All tests pass

- [ ] **Step 6: Commit**

```bash
cd /root/github/stock-analyzer && git add -A && git commit -m "refactor: clean up storage-related TODO stubs"
```

---

### Task 15: Evaluate and clean up messaging-related TODOs

**Files:**
- Modify: `/root/github/stock-analyzer/crates/sa-engine/src/guidance/prewarm.rs`
- Modify: `/root/github/stock-analyzer/crates/sa-engine/src/task_manager.rs`

- [ ] **Step 1: Trace call sites**

```bash
grep -rn "publish_prewarm\|publish_event" crates/ --include="*.rs" | grep -v "fn " | grep -v "TODO"
```

- [ ] **Step 2: Determine if cross-instance messaging is needed**

If no cross-instance messaging is needed, remove the TODO stubs entirely.

- [ ] **Step 3: Verify compilation**

```bash
cd /root/github/stock-analyzer && cargo build --workspace 2>&1 | tail -5
```
Expected: Build succeeds

- [ ] **Step 4: Commit**

```bash
cd /root/github/stock-analyzer && git add -A && git commit -m "refactor: clean up messaging-related TODO stubs"
```

---

## Phase 5: Fix Bugs & Safety Issues

### Task 16: Verify unwrap() safety

**Files:**
- Various files in `crates/`

- [ ] **Step 1: Find all unwrap() in production code**

```bash
grep -rn "\.unwrap()" crates/ --include="*.rs" | grep -v test | grep -v doc | head -20
```

- [ ] **Step 2: For each unwrap(), determine if it's safe**

For each instance:
1. Is the value guaranteed to be Some/Ok?
2. If not, replace with `?` operator or proper error handling

- [ ] **Step 3: Verify compilation**

```bash
cd /root/github/stock-analyzer && cargo build --workspace 2>&1 | tail -5
```
Expected: Build succeeds

- [ ] **Step 4: Run tests**

```bash
cd /root/github/stock-analyzer && cargo test --workspace --lib 2>&1 | tail -10
```
Expected: All tests pass

- [ ] **Step 5: Commit**

```bash
cd /root/github/stock-analyzer && git add -A && git commit -m "fix: replace unsafe unwrap() with proper error handling"
```

---

### Task 17: Verify derived method safety

**Files:**
- Modify: `/root/github/stock-analyzer/crates/sa-models/src/analysis/derived.rs`

- [ ] **Step 1: Read the file**

```bash
cat /root/github/stock-analyzer/crates/sa-models/src/analysis/derived.rs
```

- [ ] **Step 2: Verify each derived method handles empty/missing data**

For each method:
1. Check if it can return empty string
2. Add fallback value if appropriate
3. Document behavior

- [ ] **Step 3: Verify compilation**

```bash
cd /root/github/stock-analyzer && cargo build --package sa-models 2>&1 | tail -5
```
Expected: Build succeeds

- [ ] **Step 4: Run tests**

```bash
cd /root/github/stock-analyzer && cargo test --package sa-models --lib 2>&1 | tail -10
```
Expected: All tests pass

- [ ] **Step 5: Commit**

```bash
cd /root/github/stock-analyzer && git add crates/sa-models/src/analysis/derived.rs && git commit -m "fix: add safety checks to derived methods"
```

---

## Final Verification

### Task 18: Run comprehensive tests

**Files:**
- None (verification only)

- [ ] **Step 1: Run all tests**

```bash
cd /root/github/stock-analyzer && cargo test --workspace 2>&1 | tail -20
```
Expected: All tests pass

- [ ] **Step 2: Run clippy**

```bash
cd /root/github/stock-analyzer && cargo clippy --workspace 2>&1 | tail -5
```
Expected: 0 warnings

- [ ] **Step 3: Check line count reduction**

```bash
find crates -name "*.rs" | xargs wc -l | tail -1
```
Expected: At least 2,000 lines reduced

- [ ] **Step 4: Verify no functional changes**

Run the analysis pipeline end-to-end to ensure identical behavior.

---

## Summary

- **Total tasks:** 18
- **Estimated time:** 2-3 hours
- **Risk:** Low (incremental changes with verification after each step)
- **Success criteria:** Zero clippy warnings, all tests pass, line count reduced by 2,000+

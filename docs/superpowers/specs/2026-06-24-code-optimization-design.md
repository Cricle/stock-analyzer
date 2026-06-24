# Code Optimization Design Spec

**Date:** 2026-06-24
**Goal:** Scan and delete dead code, merge duplicate code, optimize performance, reduce memory usage, fix bugs — while maintaining identical functionality.

## Current State

- **Total lines:** 46,788 (sa-engine: 27,493, sa-models: 17,060, sa-data: 1,617, sa-storage: 606, sa-types: 12)
- **Clippy warnings:** 18 (unused imports/functions in akshare-rs and stock-analyzer)
- **Clone calls:** 672 in sa-engine alone
- **to_string calls:** 1,361 across workspace
- **TODO stubs:** 17 indicating incomplete migrations
- **Capacity pre-allocations:** Only 2 instances

## Approach: Incremental

Fix one category at a time, verify compilation and tests after each phase.

---

## Phase 1: Fix Clippy Warnings

**Target:** 18 warnings across akshare-rs and stock-analyzer

### akshare-rs warnings:
1. `unused import: rust_decimal::Decimal` (3x)
   - `a_share/mod.rs:4`, `hk/mod.rs:13`, `us/mod.rs:7`
2. `function search_stocks is never used` (2x)
   - `akshare_rust/mod.rs:101`, `akshare_rust/a_share.rs:18`
3. `unused imports: f64_to_dec and opt_f64_to_dec` (3x)
   - `a_share/mod.rs:12`, `us/mod.rs:17`, `hk/mod.rs:7`
4. `function opt_f64_to_dec is never used` (1x)
   - `mod.rs:71`
5. `function news_source is never used` (1x)
   - `akshare_rust/mod.rs:29`
6. `function quote_source is never used` (1x)
   - `akshare_rust/mod.rs:13`
7. `function fundamentals_source is never used` (1x)
   - `akshare_rust/mod.rs:21`
8. `function candles_source is never used` (1x)
   - `akshare_rust/mod.rs:37`
9. `unused import: std::time::Duration` (1x)
   - `client.rs:1`
10. `function f64_to_dec is never used` (1x)
    - `mod.rs:67`
11. `unused imports: DataConfig and SearchProviderConfig` (1x)
    - `client.rs:9`
12. `unused import: Singleflight` (1x)
    - `client.rs:18`

### stock-analyzer warnings:
13. `unused imports: SingleflightResult and Singleflight` (1x)
    - `sa-data/src/lib.rs:54`

**Action:** Remove all unused imports and functions.

---

## Phase 2: Remove Dead Code & Unused Imports

### Unused re-exports:
- `sa-data/src/lib.rs`: `Singleflight`, `SingleflightResult` (not used by sa-engine)

### Dead code indicators:
- Functions that are defined but never called
- Modules that are imported but never used
- Type definitions that are never instantiated

**Action:** 
1. Remove unused re-exports from sa-data/src/lib.rs
2. Use `cargo clippy` and `cargo deadlinks` to identify dead code
3. Remove all provably dead code (no call sites)

---

## Phase 3: Optimize Memory Allocations

### High-impact targets:

#### 3.1 Reduce `.clone()` calls
**Hotspots:**
- `stock_pick/pipeline/mod.rs` — Many field-by-field clones when converting types
- `llm/client/anthropic.rs` — Clones in async move blocks
- `llm/generated/` — Clones when passing to parse functions

**Strategy:**
- Use references where ownership isn't needed (e.g., `&str` instead of `String`)
- Use `Cow<'_, str>` for conditional ownership (rarely modified strings)
- Implement `From`/`Into` for type conversions instead of manual clone+assign
- For async move blocks: clone only the specific fields needed, not entire structs

#### 3.2 Reduce `.to_string()` calls
**Hotspots:**
- `stock_pick/pipeline/mod.rs` — Many string literals converted to String
- `llm/` — Model names, provider types

**Strategy:**
- Use `&str` references where possible
- Use `String::from()` instead of `.to_string()` for literals (clearer intent)
- Consider `compact_str` or `smol_str` for small strings

#### 3.3 Add capacity pre-allocation
**Current:** Only 2 instances of `with_capacity`

**Strategy:**
- Pre-allocate Vecs when final size is known or estimable
- Pre-allocate Strings when content length is known
- Focus on hot paths (analysis pipeline, report generation)

#### 3.4 Avoid unnecessary collections
**Current:** 155 `.collect::<Vec>` calls

**Strategy:**
- Use iterator chains without collecting when possible
- Return `impl Iterator` instead of `Vec` where callers only iterate
- Use `collect::<Result<Vec<_>, _>>()` for error handling

---

## Phase 4: Clean Up TODO Stubs

### Category A: Storage-related (Redis/Qdrant/PostgreSQL)
- `stock_pick/pipeline/mod.rs:428,434` — Store recommendations
- `stock_pick/history/mod.rs:3,71` — Redis/Qdrant migration
- `memory/embedding.rs:34` — QdrantClient migration
- `memory/vector_store.rs:333` — Server-side filtering
- `score/history.rs:5` — PostgreSQL reconciliation
- `checkpoint/mod.rs:22,165` — Redis implementation
- `guidance/store/*` — VectorStore/CacheStore filtering

**Approach:** 
1. Trace each TODO's call site to determine if the code path is actually executed
2. If executed: verify the current trait-based implementation works correctly
3. If not executed: remove the dead code path entirely
4. For stubs that need implementation: implement using the trait-based approach

### Category B: Messaging (NATS/Redis pub/sub)
- `guidance/prewarm.rs:3` — NATS publishing
- `task_manager.rs:268,271` — Redis pub/sub

**Approach:** If no cross-instance messaging needed, remove. Otherwise, implement trait-based approach.

---

## Phase 5: Fix Bugs & Safety Issues

### 5.1 Verify unwrap() safety
- Scan all `unwrap()` calls in production code (non-test)
- Replace with `?` operator or proper error handling

### 5.2 Derived method safety
- `analysis/derived.rs` methods return String — verify they handle empty/missing data
- Add fallback values where appropriate

### 5.3 Trait implementation completeness
- Verify all `CacheStore`, `VectorStore`, `CheckpointStore` implementations handle edge cases
- Add proper error messages for unimplemented features

---

## Verification

After each phase:
```bash
cargo clippy --workspace 2>&1 | grep "warning\|error"
cargo test --workspace --lib
cargo build --workspace
```

Final verification:
```bash
# Ensure no regressions
cargo test --workspace
# Check line count reduction
find crates -name "*.rs" | xargs wc -l | tail -1
```

---

## Success Criteria

1. Zero clippy warnings
2. All tests pass
3. Line count reduced by at least 2,000 lines (dead code removal)
4. No functional changes — identical behavior
5. Measurable reduction in clone/allocation patterns

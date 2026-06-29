# Batch Tool Calls & Calibration Threshold Design

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Reduce single-stock execution time from ~1500s to ~800s by batching analyst tool calls, and restore calibration thresholds to appropriate levels now that all 4 analysts produce data.

**Architecture:** Change the analyst planner loop from one-tool-at-a-time to batch-tool mode, and adjust `strong_direction_abs` from 20→35 and `direction_floor_abs` from 8→12.

**Tech Stack:** Rust, adk-graph, LLM prompt engineering, scoring pipeline

---

## Part 1: Batch Tool Calls

### Problem

Each analyst currently cycles: `planner LLM → single tool → planner LLM → single tool → ... → finalize`. With 4 analysts and 3-7 tools each, this means 14-24 LLM calls per stock. At 15-30s each, LLM alone takes 210-720s.

### Solution

Allow the analyst planner to request multiple tools in a single LLM call. The tool node executes all requested tools in parallel, then returns to the planner.

### Flow Change

**Before:**
```
planner → tool_1 → planner → tool_2 → planner → tool_3 → planner → finalize
(4 LLM calls + 3 tool calls)
```

**After:**
```
planner → [tool_1, tool_2, tool_3] parallel → planner → finalize
(2 LLM calls + 3 tool calls in parallel)
```

### Files to Modify

1. **`src/llm/prompt/generate.rs`** — `generate_analyst_decision` prompt
   - Change "choose exactly one supported `tool_name`" to "request all needed tools as `tool_calls` array"
   - Update response schema: add `tool_calls: [{tool_name, tool_arguments}]` alongside existing `tool_name`/`tool_arguments` for backward compat

2. **`src/llm/types.rs`** — `GeneratedAnalystDecision`
   - Add `tool_calls: Vec<ToolCall>` field
   - Add `ToolCall` struct with `tool_name: String, tool_arguments: Value`

3. **`src/report/runtime/trading_graph/nodes/mod.rs`** — `analyst_planner_node` and `tool_node`
   - `analyst_planner_node`: when action=tool, populate `pending_tools` (Vec) instead of `pending_tool` (single)
   - `tool_node`: execute all pending tools in parallel using `futures::future::join_all`, collect observations, push all to tool_history

4. **`src/report/runtime/trading_graph/mod.rs`** — `analyst_route`
   - Check `pending_tools.is_some()` (non-empty vector) instead of `pending_tool.is_some()`

5. **`src/types.rs`** — `AnalystRuntimeState`
   - Change `pending_tool: Option<PendingToolCall>` → `pending_tools: Vec<PendingToolCall>`
   - Add `#[serde(default, alias = "pending_tool")]` for backward compat with checkpoint data
   - Remove old `pending_tool` field

### Backward Compatibility

- Keep `tool_name`/`tool_arguments` fields in the LLM response schema as fallback
- If `tool_calls` is empty but `tool_name` is present, treat as single-element batch
- Validation logic in `validate_analyst_decision` handles both formats

### Expected Impact

- Market analyst (5 tools): 5 LLM calls → 1-2 LLM calls
- Fundamentals analyst (7 tools): 7 LLM calls → 2-3 LLM calls
- Total LLM calls: ~24 → ~12-14
- Estimated time: ~1500s → ~800-900s

---

## Part 2: Calibration Threshold Adjustment

### Problem

`strong_direction_abs` was lowered from 50 to 20 as a workaround when analyst data was empty (only market analyst survived due to the Overwrite reducer bug). Now all 4 analysts produce data, direction scores are meaningful, and 20 is too low — moderate signals trigger the strong direction path.

### Solution

Restore `strong_direction_abs` to 35 and `direction_floor_abs` to 12.

### Files to Modify

1. **`src/scoring/types/breakdown/default.rs`** — `CalibrationProfile::default()`
   - `strong_direction_abs: 20` → `strong_direction_abs: 35`
   - `direction_floor_abs: 8` → `direction_floor_abs: 12`

### Threshold Rationale

| Parameter | Old (broken) | Current (workaround) | New (proposed) | Why |
|-----------|-------------|---------------------|----------------|-----|
| `strong_direction_abs` | 50 | 20 | 35 | 20 too loose; 35 requires ≥2 analyst signals |
| `direction_floor_abs` | 12 | 8 | 12 | Restore original; 8 has no gating effect |
| `min_confidence_score` | 45 | 45 | 45 | Unchanged |
| `min_action_score` | 35 | 35 | 35 | Unchanged |

### Direction Score Typical Range

With 4 analysts contributing (market 40%, fundamentals 25%, news 20%, sentiment 15%, risk ±15):
- All moderate bearish: ~-30 to -40
- Only market bearish: ~-10 to -15
- Mixed signals: ~-10 to +10
- All moderate bullish: ~+30 to +40

With `strong_direction_abs=35`: needs ≥2 analysts aligned to trigger strong path. Single analyst can still influence via `direction_floor_abs=12` path.

### What We Keep

- `Rating::Unknown` variant — correct semantic for unextractable recommendations
- Analyst probability fallback in `evaluate_direction_score` — correct defense
- `calibrate_recommendation_with_profile` logic — all branches remain valid

---

## Testing Strategy

1. Run existing test suite (1340 tests) — must all pass
2. Run `market_test` example on 贵州茅台 — verify all 4 analysts present, time < 1000s
3. Run on 2-3 more stocks to verify no direction bias

## Success Criteria

- [ ] Single-stock execution time < 1000s (down from ~1500s)
- [ ] All 4 analysts present in result
- [ ] All 1340 tests pass
- [ ] No false Hold bias (direction scores > 35 produce directional recommendations)

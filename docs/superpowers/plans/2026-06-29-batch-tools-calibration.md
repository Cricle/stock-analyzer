# Batch Tool Calls & Calibration Threshold Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Reduce single-stock execution time from ~1500s to ~800s by batching analyst tool calls, and restore calibration thresholds to appropriate levels.

**Architecture:** Change the analyst planner loop from one-tool-at-a-time to batch-tool mode (LLM requests multiple tools per call, tool node executes them in parallel), and adjust `strong_direction_abs` from 20→35 and `direction_floor_abs` from 8→12.

**Tech Stack:** Rust, adk-graph, serde_json, futures, LLM prompt engineering

---

## File Structure

| File | Responsibility |
|------|---------------|
| `src/llm/generated/types.rs` | `GeneratedAnalystDecision` struct — add `tool_calls` field |
| `src/llm/generated/role_report.rs` | `from_value` parser — parse `tool_calls` array |
| `src/llm/parse/validate.rs` | Validation — handle batch tool calls |
| `src/llm/parse/parsers.rs` | Parser entry point — backward compat for batch |
| `src/llm/prompt/generate.rs` | LLM prompt — update to allow batch tool requests |
| `src/analysis/report_types/risk_assessment.rs` | `AnalystRuntimeState` — `pending_tool` → `pending_tools` |
| `src/report/runtime/trading_graph/nodes/mod.rs` | `analyst_planner_node` + `tool_node` — batch execution |
| `src/report/runtime/trading_graph/mod.rs` | `analyst_route` — check `pending_tools` |
| `src/scoring/types/breakdown/default.rs` | `CalibrationProfile` — threshold adjustment |
| `tests/llm_parse_tests.rs` | Tests for batch parsing |

---

### Task 1: Add `ToolCall` type and `tool_calls` field to `GeneratedAnalystDecision`

**Files:**
- Modify: `src/llm/generated/types.rs:38-45`
- Modify: `src/llm/generated/role_report.rs:48-61`
- Test: `tests/llm_parse_tests.rs`

- [ ] **Step 1: Add `ToolCall` struct and `tool_calls` field**

In `src/llm/generated/types.rs`, add the `ToolCall` struct before `GeneratedAnalystDecision`, and add the `tool_calls` field:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ToolCall {
    pub tool_name: String,
    #[serde(default)]
    pub tool_arguments: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GeneratedAnalystDecision {
    pub action: String,
    pub reasoning: String,
    pub final_report: Option<GeneratedRoleReport>,
    pub tool_name: Option<String>,
    pub tool_arguments: Option<Value>,
    #[serde(default)]
    pub tool_calls: Vec<ToolCall>,
}
```

- [ ] **Step 2: Update `from_value` parser**

In `src/llm/generated/role_report.rs`, update `GeneratedAnalystDecision::from_value` to parse `tool_calls`:

```rust
impl GeneratedAnalystDecision {
    pub(crate) fn from_value(raw: Value) -> Self {
        let object = raw.as_object();
        let field = |key: &str| object.and_then(|map| map.get(key)).cloned();
        let tool_calls = field("tool_calls")
            .and_then(|v| {
                if let Value::Array(arr) = v {
                    Some(arr.into_iter().filter_map(|item| {
                        let obj = item.as_object()?;
                        Some(ToolCall {
                            tool_name: obj.get("tool_name")?.as_str()?.to_string(),
                            tool_arguments: obj.get("tool_arguments").cloned().unwrap_or(Value::Object(Default::default())),
                        })
                    }).collect())
                } else {
                    None
                }
            })
            .unwrap_or_default();
        Self {
            action: parse::text_or_default(field("action"), "finalize"),
            reasoning: parse::text_or_default(field("reasoning"), "模型未返回分析师动作原因。"),
            final_report: field("final_report").map(GeneratedRoleReport::from_value),
            tool_name: field("tool_name")
                .map(|value| parse::normalize_value(&value))
                .filter(|value| !value.is_empty()),
            tool_arguments: field("tool_arguments"),
            tool_calls,
        }
    }
}
```

- [ ] **Step 3: Update `parse_generated_analyst_decision` for backward compat**

In `src/llm/parse/parsers.rs`, update the fallback path to populate `tool_calls`:

```rust
pub fn parse_generated_analyst_decision(
    content: &str,
) -> anyhow::Result<GeneratedAnalystDecision> {
    let parsed = parse_object_candidates_value(content, GeneratedAnalystDecision::from_value)?;
    if parsed.action.eq_ignore_ascii_case("finalize") && parsed.final_report.is_none() {
        let report = parse_object_candidates_value(content, GeneratedRoleReport::from_value)?;
        return Ok(GeneratedAnalystDecision {
            action: "finalize".to_string(),
            reasoning: "normalized legacy role-report response into analyst finalize decision"
                .to_string(),
            final_report: Some(report),
            tool_name: None,
            tool_arguments: None,
            tool_calls: vec![],
        });
    }
    validate_analyst_decision(&parsed, content);
    Ok(parsed)
}
```

- [ ] **Step 4: Write test for batch tool_calls parsing**

In `tests/llm_parse_tests.rs`, add:

```rust
#[test]
fn parse_analyst_decision_batch_tool_calls() {
    let content = r#"{
        "action": "tool",
        "reasoning": "need multiple data points",
        "tool_calls": [
            {"tool_name": "get_stock_data", "tool_arguments": {}},
            {"tool_name": "get_indicators", "tool_arguments": {}}
        ]
    }"#;
    let result = parse_generated_analyst_decision(content);
    assert!(result.is_ok());
    let decision = result.unwrap();
    assert_eq!(decision.action, "tool");
    assert_eq!(decision.tool_calls.len(), 2);
    assert_eq!(decision.tool_calls[0].tool_name, "get_stock_data");
    assert_eq!(decision.tool_calls[1].tool_name, "get_indicators");
}

#[test]
fn parse_analyst_decision_single_tool_backward_compat() {
    let content = r#"{"action":"tool","reasoning":"need data","tool_name":"get_stock_data","tool_arguments":"{}"}"#;
    let result = parse_generated_analyst_decision(content);
    assert!(result.is_ok());
    let decision = result.unwrap();
    assert_eq!(decision.tool_name.as_deref(), Some("get_stock_data"));
    assert!(decision.tool_calls.is_empty());
}
```

- [ ] **Step 5: Run tests**

Run: `cargo test llm_parse -- --nocapture 2>&1 | tail -20`
Expected: All parse tests pass including new batch tests.

- [ ] **Step 6: Commit**

```bash
git add src/llm/generated/types.rs src/llm/generated/role_report.rs src/llm/parse/parsers.rs tests/llm_parse_tests.rs
git commit -m "feat: add ToolCall type and tool_calls field to GeneratedAnalystDecision"
```

---

### Task 2: Update validation for batch tool calls

**Files:**
- Modify: `src/llm/parse/validate.rs:39-70`

- [ ] **Step 1: Update `validate_analyst_decision`**

In `src/llm/parse/validate.rs`, change the tool validation to accept either `tool_name` or `tool_calls`:

```rust
pub fn validate_analyst_decision(parsed: &super::GeneratedAnalystDecision, raw: &str) -> Vec<DiagnosisIssue> {
    let mut issues = Vec::new();
    if is_default_text(&parsed.reasoning) {
        issues.push(DiagnosisIssue::error(
            "analyst_decision", "reasoning",
            "reasoning is default placeholder",
        ));
    }
    if parsed.action == "finalize" && parsed.final_report.is_none() {
        issues.push(DiagnosisIssue::error(
            "analyst_decision", "final_report",
            "finalize action but no final_report",
        ));
    }
    if parsed.action == "tool"
        && parsed.tool_name.is_none()
        && parsed.tool_name.as_deref() == Some("")
        && parsed.tool_calls.is_empty()
    {
        issues.push(DiagnosisIssue::error(
            "analyst_decision", "tool_name",
            "tool action but no tool_name or tool_calls",
        ));
    }
    if !issues.is_empty() {
        tracing::warn!(
            issues = %issues.iter().map(|i| i.message.as_str()).collect::<Vec<_>>().join(", "),
            action = %parsed.action,
            raw_len = raw.len(),
            "LLM output schema validation: parsed analyst decision has quality issues"
        );
    }
    issues
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test -- --nocapture 2>&1 | tail -10`
Expected: All tests pass.

- [ ] **Step 3: Commit**

```bash
git add src/llm/parse/validate.rs
git commit -m "feat: update validation to accept batch tool_calls"
```

---

### Task 3: Update LLM prompt for batch tool requests

**Files:**
- Modify: `src/llm/prompt/generate.rs:123-168`

- [ ] **Step 1: Update the analyst decision prompt**

In `src/llm/prompt/generate.rs`, change the prompt in `generate_analyst_decision`. Replace the lines:

```
If you need a tool, set `action` to `tool`, choose exactly one supported `tool_name`, and provide `tool_arguments` as a JSON object.
```

with:

```
If you need tools, set `action` to `tool` and provide `tool_calls` as an array of {tool_name, tool_arguments} objects. Request ALL needed tools at once — do not request one at a time. Each tool_name must be one of: {available_tools}.
```

And replace:

```
Required top-level JSON fields only:
action, reasoning, final_report, tool_name, tool_arguments.
`action` must be exactly `tool` or `finalize`.
`reasoning` must explain why another tool call is needed or why evidence is sufficient.
When `action=tool`, `tool_name` must be one of: {available_tools}.
```

with:

```
Required top-level JSON fields only:
action, reasoning, final_report, tool_calls.
`action` must be exactly `tool` or `finalize`.
`reasoning` must explain why tool calls are needed or why evidence is sufficient.
When `action=tool`, `tool_calls` must be an array of objects with `tool_name` (one of: {available_tools}) and `tool_arguments` (JSON object).
```

- [ ] **Step 2: Run tests**

Run: `cargo test -- --nocapture 2>&1 | tail -10`
Expected: All tests pass (prompt change doesn't affect parsing tests).

- [ ] **Step 3: Commit**

```bash
git add src/llm/prompt/generate.rs
git commit -m "feat: update analyst prompt to request batch tool_calls"
```

---

### Task 4: Change `pending_tool` to `pending_tools` in `AnalystRuntimeState`

**Files:**
- Modify: `src/analysis/report_types/risk_assessment.rs:54-65`

- [ ] **Step 1: Update `AnalystRuntimeState` struct**

In `src/analysis/report_types/risk_assessment.rs`:

```rust
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AnalystRuntimeState {
    pub key: String,
    #[serde(default, alias = "pending_tool")]
    pub pending_tools: Vec<PendingToolCall>,
    #[serde(default)]
    pub tool_history: Vec<ToolObservation>,
    #[serde(default)]
    pub final_messages: Vec<String>,
    #[serde(default)]
    pub cleared: bool,
}
```

- [ ] **Step 2: Fix all compilation errors**

The compiler will report errors in files that reference `pending_tool`. These will be fixed in Tasks 5 and 6. For now, just verify the type change compiles with: `cargo check 2>&1 | head -30`

Expected: Compilation errors in `nodes/mod.rs` and `routing.rs` (expected, will fix next).

- [ ] **Step 3: Commit**

```bash
git add src/analysis/report_types/risk_assessment.rs
git commit -m "feat: change pending_tool to pending_tools Vec in AnalystRuntimeState"
```

---

### Task 5: Update `analyst_planner_node` for batch tool calls

**Files:**
- Modify: `src/report/runtime/trading_graph/nodes/mod.rs:138-163`

- [ ] **Step 1: Update planner node to populate `pending_tools`**

In `src/report/runtime/trading_graph/nodes/mod.rs`, replace the tool handling block (lines ~138-163):

```rust
            if decision.action.eq_ignore_ascii_case("tool") {
                let task_id = result.task_id.clone();
                let symbol = result.symbol.clone();
                let runtime = result.analyst_runtime_state_mut(analyst_key);
                // Support batch tool_calls or single tool_name fallback
                let tools: Vec<PendingToolCall> = if !decision.tool_calls.is_empty() {
                    decision.tool_calls.into_iter().map(|tc| PendingToolCall {
                        tool_name: tc.tool_name,
                        arguments: tc.tool_arguments,
                        reason: decision.reasoning.clone(),
                    }).collect()
                } else if let Some(name) = decision.tool_name {
                    vec![PendingToolCall {
                        tool_name: name,
                        arguments: decision.tool_arguments.unwrap_or_else(|| json!({})),
                        reason: decision.reasoning.clone(),
                    }]
                } else {
                    vec![]
                };
                runtime.pending_tools = tools;
                tracing::info!(
                    task_id = %task_id,
                    symbol = %symbol,
                    analyst = analyst_key,
                    pending_tools = ?runtime.pending_tools,
                    "stored pending analyst tool calls"
                );
                result.artifacts.llm_token_usage = llm.usage_summary().await;
                manager
                    .persist_runtime_stage(
                        &result,
                        &format!("analyst:{analyst_key}:tool_request"),
                        analyst_node_name(analyst_key),
                    )
                    .await
                    .map_err(graph_error)?;
                return result_output(result);
            }
```

- [ ] **Step 2: Update the clear at end of `apply_analyst_report`**

In the same file, at line ~546, change:
```rust
    runtime.pending_tool = None;
```
to:
```rust
    runtime.pending_tools.clear();
```

- [ ] **Step 3: Commit**

```bash
git add src/report/runtime/trading_graph/nodes/mod.rs
git commit -m "feat: update analyst_planner_node to populate pending_tools Vec"
```

---

### Task 6: Update `tool_node` for parallel batch execution

**Files:**
- Modify: `src/report/runtime/trading_graph/nodes/mod.rs:219-270`

- [ ] **Step 1: Rewrite `tool_node` for batch execution**

Replace the `tool_node` function (lines ~219-270):

```rust
pub(super) fn tool_node(
    manager: TaskManager,
    analyst_key: &'static str,
) -> impl Fn(NodeContext) -> futures::future::BoxFuture<'static, GraphResult<NodeOutput>>
+ Send
+ Sync
+ 'static {
    move |ctx| {
        let manager = manager.clone();
        Box::pin(async move {
            let mut result = load_result(&ctx)?;
            let pending_runtime = result.analyst_runtime_state(analyst_key).cloned().unwrap_or_default();
            let pending_tools = pending_runtime.pending_tools.clone();
            if pending_tools.is_empty() {
                return Err(GraphError::NodeExecutionFailed {
                    node: tool_node_name(analyst_key).to_string(),
                    message: "pending tool calls missing".to_string(),
                });
            }
            tracing::info!(
                task_id = %result.task_id,
                symbol = %result.symbol,
                analyst = analyst_key,
                tool_count = pending_tools.len(),
                "execute analyst batch tools"
            );
            let scenario = result.artifacts.scenario_data.to_scenario_data();
            // Execute all tools in parallel
            let futures: Vec<_> = pending_tools.iter().map(|pending| {
                let mgr = manager.clone();
                let sym = result.symbol.clone();
                let mkt = result.market_type.clone();
                let sc = scenario.clone();
                let p = pending.clone();
                async move {
                    mgr.toolbox.execute(&sym, &mkt, Some(&sc), &p).await
                }
            }).collect();
            let observations = futures::future::join_all(futures).await;
            let runtime = result.analyst_runtime_state_mut(analyst_key);
            for obs in observations {
                runtime.tool_history.push(obs);
            }
            runtime.pending_tools.clear();
            manager
                .persist_runtime_stage(
                    &result,
                    &format!("analyst:{analyst_key}:tool_result"),
                    tool_node_name(analyst_key),
                )
                .await
                .map_err(graph_error)?;
            result_output(result)
        })
    }
}
```

- [ ] **Step 2: Update `analyst_route` in routing.rs**

In `src/report/runtime/trading_graph/routing.rs`, change line ~22:

```rust
    if runtime
        .and_then(|item| item.pending_tools.first())
        .is_some()
    {
        return tool_node_name(analyst_key).to_string();
    }
```

- [ ] **Step 3: Run tests**

Run: `cargo test -- --nocapture 2>&1 | tail -10`
Expected: All tests pass.

- [ ] **Step 4: Commit**

```bash
git add src/report/runtime/trading_graph/nodes/mod.rs src/report/runtime/trading_graph/routing.rs
git commit -m "feat: tool_node executes batch tools in parallel"
```

---

### Task 7: Adjust calibration thresholds

**Files:**
- Modify: `src/scoring/types/breakdown/default.rs:1-17`

- [ ] **Step 1: Update `CalibrationProfile::default()`**

In `src/scoring/types/breakdown/default.rs`:

```rust
impl Default for CalibrationProfile {
    fn default() -> Self {
        Self {
            min_confidence_score: 45,
            min_action_score: 35,
            direction_floor_abs: 12,
            strong_direction_abs: 35,
            sample_count: 0,
            min_hit_rate: 0.0,
            min_avg_alpha_return: 0.0,
        }
    }
}
```

- [ ] **Step 2: Run all tests**

Run: `cargo test -- --nocapture 2>&1 | tail -10`
Expected: All tests pass (some scoring tests may need adjustment if they assert specific threshold values).

- [ ] **Step 3: Commit**

```bash
git add src/scoring/types/breakdown/default.rs
git commit -m "feat: restore calibration thresholds strong_direction_abs=35, direction_floor_abs=12"
```

---

### Task 8: Integration verification

**Files:**
- Test: `examples/market_test.rs`

- [ ] **Step 1: Build and run market_test**

Run: `cargo build --release --example market_test 2>&1 | tail -5`
Then: `RECURSION_LIMIT=100 cargo run --release --example market_test 2>&1`

Expected:
- All 4 analysts present in result
- Execution time < 1000s (down from ~1500s)
- Recommendation is directional (not always Hold)

- [ ] **Step 2: Run full test suite**

Run: `cargo test 2>&1 | tail -5`
Expected: All tests pass.

- [ ] **Step 3: Run fmt and clippy**

Run: `cargo fmt && cargo clippy -- -D warnings 2>&1 | tail -5`
Expected: No issues.

- [ ] **Step 4: Final commit if needed**

```bash
git add -A
git commit -m "chore: fmt and clippy cleanup"
```

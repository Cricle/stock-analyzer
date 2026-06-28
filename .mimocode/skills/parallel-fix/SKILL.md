---
name: parallel-fix
description: Identify all issues in a category, dispatch parallel subagents to fix each, verify all pass, then commit.
---

# Parallel Fix

When multiple independent issues need fixing, dispatch parallel subagents for maximum speed.

## When to use

- When user says "全部并行修复", "全部修复，并行修复", "并行修"
- When there are 3+ independent issues across different modules/files
- When speed matters more than sequential safety

## Procedure

### Phase 1: Triage
1. Run `cargo clippy --all -- -D warnings 2>&1` to collect all warnings
2. Run `cargo test --workspace 2>&1` to collect all test failures
3. Group issues by module/file — independent groups can be parallelized

### Phase 2: Dispatch
For each independent group, spawn a subagent with:
- Clear list of specific issues to fix in specific files
- Instruction to fix only those issues, not refactor
- Instruction to verify compilation after fixes: `cargo check -p <crate>`

### Phase 3: Collect and verify
1. Wait for all subagents to complete
2. Run full verification: `cargo test --workspace` + `cargo clippy --all -- -D warnings`
3. If any subagent's fixes conflict, resolve manually

### Phase 4: Commit
1. Single commit with all fixes: `git add -A && git commit -m "fix: parallel fix for <category>"`
2. Push with proxy fallback

## Subagent template

```
Fix the following issues in <module>:

1. <file>:<line> — <description of warning/error>
2. <file>:<line> — <description of warning/error>

Rules:
- Fix ONLY these specific issues
- Do not refactor unrelated code
- After fixing, run: cargo check -p <crate>
- Report: what you fixed and any issues encountered
```

## Stopping condition

- All subagents complete, full test suite passes, clippy clean
- If a subagent fails, fix its issues manually before committing

## Notes

- Maximum 4-5 parallel subagents to avoid rate limits
- Each subagent should have a unique file scope to avoid merge conflicts
- This project uses `Arc<RwLock<>>` for thread-safe collectors — parallel fixes to concurrent code need extra care

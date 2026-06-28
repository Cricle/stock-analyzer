---
name: dead-code-cleanup
description: Scan for dead code, unused imports, duplicate code patterns, and text-matching antipatterns. Fix all findings.
---

# Dead Code Cleanup

Systematically find and remove dead code, unused imports, duplicate patterns, and prohibited antipatterns.

## When to use

- When user says "清理无用代码", "扫描死代码", "合并重复代码", "使用duplicate_code扫描并且修复"
- Before major releases or after large refactoring sessions
- When codebase feels bloated or inconsistent

## Procedure

### Phase 1: Cargo warnings scan
1. `cargo clippy --all -- -D warnings 2>&1` — catches unused imports, dead code, unnecessary clones
2. Fix all findings

### Phase 2: Dead code patterns
Search for:
- `#[allow(dead_code)]` annotations — verify if the code is truly planned for future use or should be removed
- Functions/methods with zero callers (use `cargo +nightly udeps` if available, or grep for usage)
- Structs/enums defined but never instantiated
- `use` statements importing items not used in the file

### Phase 3: Duplicate code detection
- Search for repeated code blocks (>5 lines) across files
- Look for copy-pasted test patterns that could use macros
- Identify similar function signatures that could share a generic implementation

### Phase 4: Prohibited antipatterns (project-specific)
This project has a strict rule: **禁止匹配llm做逻辑** (no text-matching for LLM output logic).

Scan for:
- Hardcoded Chinese keyword arrays used for text matching (e.g., `"芯片", "半导体", "AI", "人工智能"`)
- Regex patterns that extract data from LLM text output
- `helpers.rs` functions like `derive_*_from_text`, `extract_*_from_texts`
- Any `str.contains()` or `str.matches()` used to make business logic decisions

The correct pattern is **tool-calling** (function calling) — LLM calls tools to set structured data, not output text that gets parsed.

### Phase 5: Verify and commit
1. `cargo test --workspace` — all tests pass
2. `cargo clippy --all -- -D warnings` — clean
3. Commit with descriptive message explaining what was removed and why

## Stopping condition

- Zero clippy warnings, all tests pass, all identified dead code removed
- Report: count of removed items (imports, functions, structs, duplicate blocks)

## Notes

- Some `#[allow(dead_code)]` in `src/pick/tracking.rs` is intentional (planned structs) — skip those
- `FieldExtractor` pattern in `src/llm/generated/helpers.rs` replaces 18 raw closure occurrences — migrate if cleanup pass reaches those files

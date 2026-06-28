---
name: rust-lint-commit
description: Run cargo fmt + clippy, fix all warnings, then commit and push. Handles git proxy fallback automatically.
---

# Rust Lint & Commit

Run the full Rust quality pipeline: format, lint, fix, verify, commit, and push.

## When to use

- After completing a feature or fix, before pushing
- When user says "提交推送", "clippy和fmt后提交推送", "清理warn，提交推送"
- As a final step before handing off work

## Procedure

1. **Format**: `cargo fmt --all`
2. **Clippy with auto-fix**: `cargo clippy --all --fix --allow-dirty --allow-staged 2>&1`
3. **Clippy strict check**: `cargo clippy --all -- -D warnings 2>&1`
   - If errors remain, fix them one by one
4. **Test**: `cargo test --workspace 2>&1 | tail -30`
   - Verify all tests pass
5. **Commit**: `git add -A && git commit -m "<descriptive message>"`
   - Message should describe the why, not just the what
6. **Push with proxy fallback**:
   ```bash
   git push 2>&1 || (echo "Trying with proxy..." && https_proxy=http://127.0.0.1:7890 http_proxy=http://127.0.0.1:7890 git push 2>&1)
   ```

## Stopping condition

- All of: fmt clean, clippy clean (0 warnings), tests pass, commits pushed
- If clippy or tests fail after 3 fix attempts, stop and report the remaining issues

## Notes

- Git proxy `127.0.0.1:7890` is required in this environment
- Use `--no-verify` only when pre-commit hook blocks due to missing local Rust toolchain (CI will verify)
- For library workspaces, `Cargo.lock` should not be committed (in .gitignore)

# Development

## Building

```bash
# Debug build
cargo build

# Release build
cargo build --release

# Build the sa crate
cargo build -p sa
```

## Testing

```bash
# Run all tests
cargo test --workspace

# Run sa crate tests
cargo test -p sa

# Run specific test
cargo test test_name

# Run tests with output
cargo test -- --nocapture

# Run doc tests
cargo test -p sa --doc
```

## Linting

```bash
# Clippy
cargo clippy -p sa

# Format
cargo fmt -p sa
```

## Test Coverage

CI runs `cargo tarpaulin` with 40% coverage threshold:

```bash
cargo tarpaulin --workspace --out Stdout --fail-under 90
```

## Project Structure

```
stock-analyzer/
├── crates/
│   └── sa/            — Unified analysis crate
├── src/
│   ├── main.rs        — CLI binary
│   └── mcp.rs         — MCP server binary
├── tests/             — Integration tests
└── docs/              — Documentation
```

Data fetching is delegated to `akshare-rs` (path dependency).

## Module Map

```
sa/src/
├── analysis/          — Core analysis types, report logic
├── checkpoint/        — Resumable workflow checkpoints
├── data/              — MarketDataProvider trait + akshare-rs re-exports
├── guide/             — Daily market guidance
├── llm/               — LLM client, prompts, parsing
├── memory/            — Vector-based historical memory (RAG)
├── pick/              — Stock picking pipeline
├── report/            — Analysis pipeline lifecycle + runtime
├── scoring/           — Multi-dimensional scoring
├── store/             — Storage traits + in-memory implementations
├── task_manager/      — Task lifecycle management
└── types.rs           — Type re-exports from akshare-rs
```

## Adding Tests

### Unit Tests

Add unit tests in the same file as the code:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_function() {
        assert_eq!(function(), expected);
    }
}
```

### Mock Testing

Use `MarketDataProvider` trait with `MockMarketProvider` for data-layer tests:

```rust
use sa::data::{MarketDataProvider, mock::MockMarketProvider};

#[tokio::test]
async fn test_with_mock_data() {
    let provider = MockMarketProvider::default();
    // Set up test data on provider fields
    // Call analysis functions with &provider
}
```

### Integration Tests

Add integration tests in `tests/` directory:

```rust
#[tokio::test]
async fn test_feature() {
    // Test implementation
}
```

## Code Style

- Use `cargo fmt` for formatting
- Use `cargo clippy` for linting
- Follow Rust naming conventions
- Add doc comments for public APIs

## Debugging

### Environment Variables

- `RUST_LOG=debug`: Enable debug logging
- `ANALYSIS_DEBUG_QUICK_ONLY=1`: Quick-only debug mode
- `REPORT_KLINE_LIMIT=60`: Limit candle data

### Logging

Logs are written to stderr. Use `RUST_LOG` to control verbosity:

```bash
RUST_LOG=debug sa guidance --market a-share
```

## Contributing

1. Fork the repository
2. Create a feature branch
3. Add tests for new functionality
4. Ensure all tests pass
5. Submit a pull request

## License

MIT

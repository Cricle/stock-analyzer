# Development

## Building

```bash
# Debug build
cargo build

# Release build
cargo build --release

# Build specific crate
cargo build -p sa-engine
```

## Testing

```bash
# Run all tests
cargo test --workspace

# Run specific test
cargo test test_name

# Run tests with output
cargo test -- --nocapture

# Run integration tests
cargo test --test e2e_full_report
```

## Test Coverage

CI runs `cargo tarpaulin` with 90% coverage threshold:

```bash
cargo tarpaulin --workspace --out Stdout --fail-under 90
```

## Project Structure

```
stock-analyzer/
├── crates/
│   ├── sa-types/      — Core types
│   ├── sa-models/     — Analysis models
│   ├── sa-data/       — Market data
│   ├── sa-engine/     — Engine
│   └── sa-storage/    — Storage
├── src/
│   ├── main.rs        — CLI
│   └── mcp.rs         — MCP server
├── tests/             — Integration tests
└── docs/              — Documentation
```

## Crate Dependencies

```
sa-types (foundation)
    ↑
sa-models (depends on sa-types)
    ↑
sa-data (depends on sa-types, sa-models)
    ↑
sa-engine (depends on all above)
    ↑
sa-storage (depends on sa-types, sa-models)
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

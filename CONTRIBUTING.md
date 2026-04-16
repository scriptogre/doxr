# Contributing to drefs

## Prerequisites

- [Rust toolchain](https://rustup.rs/) (stable)
- [uv](https://docs.astral.sh/uv/) for Python package management
- JDK 21+ (only for PyCharm plugin development)

## Development Setup

```bash
git clone https://github.com/scriptogre/drefs.git
cd drefs
cargo build
```

## Running Tests

```bash
# All tests (unit + integration)
cargo test

# Unit tests only
cargo test --bin drefs

# Integration tests only
cargo test --test integration

# A specific test
cargo test native_syntax
```

## Linting

```bash
cargo fmt --check
cargo clippy
```

## Project Structure

```
src/
  main.rs          # CLI entry point (clap)
  config.rs        # pyproject.toml config loading
  discover.rs      # .py file discovery and module path mapping
  parse.rs         # tree-sitter Python AST parsing
  extract.rs       # Cross-reference pattern extraction (regex)
  graph.rs         # Symbol graph data structures and resolution
  diagnostic.rs    # Reference validation and error reporting
  inventory.rs     # Sphinx objects.inv parsing
  lsp.rs           # Language Server Protocol support
tests/
  integration.rs   # Integration tests
  fixtures/        # Test project fixtures
editors/
  pycharm/         # PyCharm/IntelliJ plugin (Kotlin)
```

## PyCharm Plugin

```bash
cd editors/pycharm
./gradlew buildPlugin
```

The built plugin zip will be at `build/distributions/drefs-pycharm-*.zip`.

## Test Fixtures

Test fixtures are minimal Python projects in `tests/fixtures/`. Each has a `pyproject.toml` and a `src/pkg/` directory. Integration tests run `drefs` against these fixtures and assert on the output.

To add a new test case, create a fixture directory and add tests in `tests/integration.rs`.

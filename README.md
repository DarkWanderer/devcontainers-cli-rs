# devcontainers-cli-rs

A fully vibe-coded replacement for https://github.com/devcontainers/cli. Attemtps to present same CLI interface and functionality

## Installation

```bash
cargo install --git https://github.com/DarkWanderer/devcontainers-cli-rs.git
```

## Development

### Running Tests

```bash
cargo test --workspace
```

### Code Coverage

This project uses [cargo-llvm-cov](https://github.com/taiki-e/cargo-llvm-cov) for code coverage.

#### Prerequisites

Install cargo-llvm-cov:
```bash
cargo install cargo-llvm-cov
```

#### Generate Coverage Report

Generate HTML coverage report:
```bash
cargo llvm-cov --workspace --all-features --html
```

Generate and open HTML report in browser:
```bash
cargo llvm-cov --workspace --all-features --open
```

Generate LCOV report (for editor integration):
```bash
cargo llvm-cov --workspace --all-features --lcov --output-path lcov.info
```

Generate codecov JSON format (used in CI):
```bash
cargo llvm-cov --workspace --all-features --codecov --output-path codecov.json
```

#### Clean Coverage Artifacts

```bash
cargo llvm-cov clean
```

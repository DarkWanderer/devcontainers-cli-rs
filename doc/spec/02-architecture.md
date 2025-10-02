# 02 – Architecture Specification

## Project Structure
- Cargo workspace
  - `crates/cli`: end-user binary; argument parsing, command dispatch, logging
  - `crates/core`: core domain logic (config resolution, lifecycle engine, provider abstraction)
  - `crates/providers/*`: isolated provider implementations (e.g., `docker`, `podman`)
  - `crates/integration-tests`: acceptance tests powered by `cargo nextest` or `cargo test`
- Shared devcontainer fixtures placed under `fixtures/`
- Documentation and spec under `doc/`

## Module Boundaries
- `cli::app`: Clap-based argument parser, global options, telemetry bootstrap
- `cli::commands::*`: thin command handlers converting CLI input into `core` operations
- `core::config`: parse/validate `devcontainer.json`, compose overrides, support schema evolution
- `core::lifecycle`: orchestrate phases (`resolve -> build -> create -> start -> exec hooks`)
- `core::features`: manage feature registry resolution, download, install, test
- `core::templates`: apply/publish template collections
- `core::providers`: trait-based API for container runtime interactions
- `core::logging`: structured logging facade returning typed events

## Key Abstractions
- `Provider` trait encapsulating container operations: build image, create container, start/stop, exec, volume/network management
- `Resolver` struct building normalized configuration from workspace, `.devcontainer` folders, and CLI overrides
- `LifecyclePlan` representing resolved steps; supports dry-run rendering
- `Event` enum describing log events for text/JSON reporters

## Data Flow
1. CLI parses args into `CommandContext`
2. `Resolver` loads config, merges overrides, fetches features/templates
3. `LifecyclePlan` generated based on command and provider capabilities
4. `Executor` streams plan steps to provider, emitting events
5. Results returned to CLI for formatting, exit code computed

## Error Handling
- Use `thiserror` for domain errors; map to user-facing diagnostics via `miette`
- Distinguish recoverable errors (e.g., config warnings) vs fatal (build failure)
- Support `--verbose` stack traces and optional JSON diagnostics

## Concurrency & Performance
- Async runtime (`tokio`) for IO-bound tasks (pulling images, network calls)
- CPU-bound tasks kept synchronous
- Caching layer for feature downloads using content-addressable store under user cache dir

## External Integrations
- Docker/Podman: communicate via CLI process invocation initially; upgrade to API clients later
- OCI Registries: interact through `oci-distribution` crate for features/templates publish
- Logging: integrate with `tracing` for structured output; text formatter for human readability

## Configuration Sources
- Discover `.devcontainer` folder, fallback to `devcontainer.json`
- Support `--config` pointing to alternate file
- Optional workspace metadata (Git) for substitution tokens
- Provide schema validation via `schemars` + JSON schema bundle

## Testing Strategy
- Unit tests at crate level for config parsing, plan generation, provider contracts
- Mock provider for deterministic behaviour in integration tests
- Golden-file tests for CLI output (`insta` snapshots) with JSON mode
- End-to-end smoke tests using Docker on CI (gated by feature flag)

## Build & Release Pipeline
- `cargo fmt`, `clippy`, `nextest` enforced in CI
- Cross-compilation via `cross` or `cargo zigbuild` for Linux/macOS/Windows
- Package binary releases and container image for CLI (optional)

## Future Extensions
- Provider plugins dynamically loaded via WASI or dynamic libs
- Long-running API server for editor integrations
- Telemetry pipeline (OpenTelemetry) with opt-in consent

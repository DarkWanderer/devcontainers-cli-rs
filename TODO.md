# Devcontainer CLI (Rust) – TODO

## Near-Term
- Add unit tests for config parsing, lifecycle planning, and provider abstraction; stand up an integration test crate using a mock provider.
- Wire lifecycle hook execution to provider `exec` once hook definitions are available in the resolved configuration.
- Add GitHub Actions CI workflows to build & test branches & PRs with coverage
- Implement CLI workflows for `run-user-commands`, `features`, `templates`, and `inspect` commands.
- Honor lifecycle command flags (`--no-cache`, `--push`, `--id-label`, `--remove-unknown`) with provider support.

## Mid-Term
- Support additional providers (Podman/remote) behind capability detection and feature flags.
- Implement feature registry operations (download/install/test/publish) with caching and OCI interactions.
- Provide template management workflows (apply/publish/list) and template metadata handling.
- Expand telemetry: structured logging configuration, JSON event schema, optional OpenTelemetry export.
- Document developer workflows (setup, testing matrix, release process) in `doc/`.
- Improve coverage for lowest-covered file

# Devcontainer CLI (Rust) – TODO

## Near-Term
- Add test coverage
- Add tests for current functionality using coverage as reference
- Flesh out `LifecyclePlan` generation for `build`, `create`, `start`, and hook execution phases; capture structured events per phase.
- Introduce a Docker provider implementing the `Provider` trait (image build, container create/start/exec, volume/network management).
- Replace CLI stub handlers with real flows wired to lifecycle execution and provider operations.
- Add unit tests for config parsing, lifecycle planning, and provider abstraction; stand up an integration test crate using a mock provider.

## Mid-Term
- Support additional providers (Podman/remote) behind capability detection and feature flags.
- Implement feature registry operations (download/install/test/publish) with caching and OCI interactions.
- Provide template management workflows (apply/publish/list) and template metadata handling.
- Expand telemetry: structured logging configuration, JSON event schema, optional OpenTelemetry export.
- Document developer workflows (setup, testing matrix, release process) in `doc/`.

# 00 – Product Overview

## Context
- Target parity with the Node-based Devcontainer CLI while embracing Rust ergonomics
- Primary users: developers automating devcontainer lifecycle, CI/CD pipelines, VS Code plugins, platform maintainers
- Execution environment: Linux/macOS/Windows hosts with Docker-compatible container runtime installed

## Goals
- Provide first-class `devcontainer` experience implemented in Rust
- Maintain compatibility with `devcontainer.json` schema and features
- Deliver predictable, observable CLI behaviour with composable subcommands
- Offer modular codebase that supports future extension (plugins, registries, providers)

## Non-Goals (Initial Release)
- Implement VS Code integration or GUI features
- Support bespoke container runtimes beyond the OCI/Docker ecosystem
- Re-implement all experimental features from upstream without validation
- Manage remote provisioning beyond what upstream CLI offers

## Assumptions & Constraints
- Rust 1.76+ stable is available in build environments
- Users may rely on existing `.devcontainer` directories and metadata
- Network access may be constrained; commands must degrade gracefully
- CLI should not require persistent daemon; rely on Docker/Podman APIs

## High-Level Requirements
- Mirror upstream command surface (`up`, `down`, `build`, `exec`, `run-user-commands`, etc.)
- Provide structured logging, verbosity control, and machine-readable output (JSON)
- Integrate with devcontainer features: features spec, lifecycle scripts, docker-compose, dockerfile contexts
- Support configuration injection via flags, env vars, workspace settings
- Offer extension points for provider abstraction (Docker, Podman, remote OCI)

## Success Metrics
- Can execute `devcontainer up` on common sample repositories with parity behaviour
- CLI round-trip times comparable or faster than Node CLI for baseline scenarios
- Production-quality documentation and test coverage for core workflows
- Positive developer feedback on ergonomics and diagnosability

## Milestones
1. Command-line scaffolding & configuration loading (MVP)
2. Container provisioning pipeline (Docker provider)
3. Feature management & lifecycle hooks
4. Extensibility & provider abstraction (Podman/Remote)
5. Telemetry, analytics, and integrations (optional)

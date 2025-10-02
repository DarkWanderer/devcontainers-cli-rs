# 01 – Command Surface Specification

## Shared Behaviour
- Binary name: `devcontainer`
- Global flags: `--help`, `--version`, `--verbose/-v` (repeatable), `--log-format <auto|text|json>`, `--project-root <path>`, `--workspace-folder <path>`, `--config <path>`, `--no-color`
- Exit codes: `0` success, `>=1` failure. Specific non-zero codes documented per command
- Output: default human-readable text; `--log-format json` for structured events
- Environment: respects `DEVCONTAINER_*` (e.g., `DEVCONTAINER_CONFIG`, `DEVCONTAINER_LOG_LEVEL`)
- Configuration hierarchy: CLI flags > env vars > `.devcontainer/devcontainer.json`

## `devcontainer up`
- Purpose: Build (if needed) and start devcontainer, optionally attaching
- Inputs:
  - Target: workspace folder, `--workspace-folder`, or `--id-label`
  - Provider selection flags: `--docker-path`, `--podman-path`, `--remote-user`
  - Lifecycle options: `--skip-post-create`, `--skip-post-attach`, `--skip-feature-auto-mount`
- Outputs: container instance metadata (ID, name, state). With `--log-format json`, emit structured events per phase (resolve config, build, create, start, run hooks)
- Exit codes: `0` success, `10` no matching containers, `20` configuration error, `30` provider failure
- Side effects: caches resolved configuration, ensures container is running

## `devcontainer down`
- Purpose: Stop and optionally remove devcontainer resources for workspace or id label
- Flags: `--workspace-folder`, `--id-label`, `--remove-unknown`, `--remove-volumes`
- Behaviour: stops containers, cleans networks/volumes when requested, idempotent

## `devcontainer build`
- Purpose: Resolve configuration and build container image only
- Flags: `--no-cache`, `--skip-feature-install`, `--cache-from <reference>`, `--push`
- Output: final image reference, build logs
- Integrates with OCI builder (Docker BuildKit) and optional Podman

## `devcontainer exec`
- Purpose: Run a command in an existing devcontainer
- Flags: `--workspace-folder`, `--id-label`, `--user`, `--cwd`, `--env VAR=VALUE`, `--tty/--no-tty`
- Behaviour: attaches streams, returns exit code of inner command

## `devcontainer run-user-commands`
- Purpose: Execute lifecycle commands defined in `devcontainer.json`
- Subcommands: `init`, `post-create`, `post-attach`. All share flags `--skip` and `--force`
- Behaviour: reuses resolved configuration and ensures container is running

## `devcontainer read-configuration`
- Purpose: Resolve and output normalized `devcontainer.json`
- Flags: `--workspace-folder`, `--config`, `--log-format`
- Output: JSON document containing full resolved configuration, features baked in

## `devcontainer features`
- Subcommands: `test`, `publish`, `package`, mirroring upstream CLI
- Shared flags: `--features-root`, `--registry`, `--version`, `--log-format`
- `test`: run feature test matrix; requires container runtime
- `publish`: push feature metadata and tarball to registry
- `package`: create local tarball for distribution

## `devcontainer templates`
- Subcommands: `apply`, `publish`, `list`
- `apply`: scaffold template into target workspace. Flags: `--template-id`, `--target`, `--force`
- `publish`: package & push template collection
- `list`: enumerate templates available locally or remote registries

## `devcontainer inspect`
- Purpose: Inspect container status, show metadata similar to `docker inspect`
- Flags: `--workspace-folder`, `--id-label`, `--log-format`
- Output: normalized status: container IDs, ports, lifecycle state, features, env

## `devcontainer version`
- Purpose: Print CLI version information
- Output: version, git commit, build metadata, provider plugin versions

## Hidden / Internal Commands (phase 2+)
- `devcontainer internal resolve`: CLI-to-internal module entrypoint without user ergonomics. Accepts JSON payload, returns JSON. Enables API reuse across frontends.
- `devcontainer internal daemon`: optional long-running service for IDE integration (future consideration). Not part of MVP.

## UX Guarantees
- Commands accept `--dry-run` where reasonable to preview actions
- Deterministic logging: phases (`resolve`, `build`, `create`, `start`, `postCreate`, etc.) carry unique event IDs
- Clear error messages with remediation hints, plus verbose stack traces at `--verbose 2`
- Config resolution output is stable for diff-based testing

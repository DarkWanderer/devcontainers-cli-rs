# DevContainers CLI Integration Tests

This directory contains integration tests for the DevContainers CLI, covering various features and configurations from the [devcontainer.json schema specification](https://containers.dev/implementors/json_schema/).

## Test Structure

Each subdirectory represents a specific test case with its own `.devcontainer/devcontainer.json` configuration and supporting files. The tests are automatically discovered and executed by the GitHub Actions workflow.

## Test Cases

### 1. `basic-image`
**Purpose**: Test basic image-based container configuration
**Features Tested**:
- Simple image reference (`alpine:3.19`)
- Basic `postCreateCommand` execution
- Minimal configuration with name and image

**Schema Coverage**: `image`, `name`, `postCreateCommand`

---

### 2. `minimal-config`
**Purpose**: Test absolute minimum required configuration
**Features Tested**:
- Only `image` property specified
- No optional properties
- Validates minimum viable devcontainer.json

**Schema Coverage**: `image` (required)

---

### 3. `dockerfile-build`
**Purpose**: Test Dockerfile-based container builds
**Features Tested**:
- Custom Dockerfile with build context
- Build arguments (`args`)
- Multi-layer Docker build
- Custom workspace folder
- Installing packages during build (git, curl, bash)

**Schema Coverage**: `build.dockerfile`, `build.context`, `build.args`, `workspaceFolder`

---

### 4. `lifecycle-commands`
**Purpose**: Test all lifecycle command hooks
**Features Tested**:
- `initializeCommand` - runs on host before container creation
- `onCreateCommand` - runs during container creation
- `updateContentCommand` - runs during content updates
- `postCreateCommand` - array form with shell command
- `postStartCommand` - object form with multiple named commands
- `postAttachCommand` - runs when attaching to container
- `waitFor` - specifies which command to wait for

**Schema Coverage**: Complete lifecycle command chain

**Verification**: Creates marker files in `/tmp/` for each lifecycle stage

---

### 5. `features`
**Purpose**: Test devcontainer features installation
**Features Tested**:
- Installing multiple features from ghcr.io registry
- Feature configuration with options:
  - `common-utils` - Zsh and Oh My Zsh
  - `git` - Version control
  - `node` - Node.js runtime (v20)
- Feature verification via postCreateCommand

**Schema Coverage**: `features` with configuration objects

---

### 6. `port-forwarding`
**Purpose**: Test port forwarding configuration
**Features Tested**:
- Forward multiple ports (`forwardPorts`)
- Port-specific attributes (`portsAttributes`):
  - Custom labels
  - Protocol specification (http/https)
  - Auto-forward behavior (notify, openBrowser, silent)
- Default port attributes (`otherPortsAttributes`)
- Application ports (`appPort`)

**Schema Coverage**: `forwardPorts`, `portsAttributes`, `otherPortsAttributes`, `appPort`

---

### 7. `environment-variables`
**Purpose**: Test environment variable configuration
**Features Tested**:
- Container environment variables (`containerEnv`)
- Remote environment variables (`remoteEnv`)
- Variable substitution:
  - `${containerEnv:PATH}` - reference other container env
  - `${containerWorkspaceFolder}` - workspace path in container
  - `${localWorkspaceFolderBasename}` - local workspace name
  - `${localWorkspaceFolder}` - local workspace path

**Schema Coverage**: `containerEnv`, `remoteEnv`, variable substitution syntax

**Verification**: Outputs sorted environment variables

---

### 8. `mounts`
**Purpose**: Test volume and bind mount configuration
**Features Tested**:
- Bind mounts with workspace folder substitution
- Named volume mounts
- Host file mounts (e.g., `.gitconfig`)
- Multiple mount types in single configuration

**Schema Coverage**: `mounts` with `type`, `source`, `target`

**Mount Types Tested**:
- `bind` mount: workspace data directory
- `volume` mount: cache volume
- `bind` mount: host configuration file

---

### 9. `user-configuration`
**Purpose**: Test user and permission configuration
**Features Tested**:
- Container user specification (`containerUser`)
- Remote user specification (`remoteUser`)
- UID/GID synchronization (`updateRemoteUserUID`)
- Environment probe mode (`userEnvProbe`)

**Schema Coverage**: `containerUser`, `remoteUser`, `updateRemoteUserUID`, `userEnvProbe`

**Verification**: Captures user identity and permissions

---

### 10. `docker-compose`
**Purpose**: Test Docker Compose integration
**Features Tested**:
- Multi-service composition
- Service selection
- Service dependencies
- Named networks
- Persistent volumes
- Shutdown action configuration

**Schema Coverage**: `dockerComposeFile`, `service`, `shutdownAction`

**Services**:
- `app` - Alpine-based development container
- `db` - PostgreSQL 16 database

---

### 11. `workspace-folder`
**Purpose**: Test custom workspace folder configuration
**Features Tested**:
- Custom workspace path in container
- Custom workspace mount configuration
- Workspace folder substitution in commands

**Schema Coverage**: `workspaceFolder`, `workspaceMount`

---

### 12. `run-args`
**Purpose**: Test custom Docker run arguments
**Features Tested**:
- Custom hostname
- Additional host entries
- Network configuration
- DNS settings
- Memory limits
- CPU limits

**Schema Coverage**: `runArgs`

**Verification**: Checks hostname and hosts file configuration

---

### 13. `privileged-mode`
**Purpose**: Test privileged containers and capabilities
**Features Tested**:
- Privileged mode (`privileged: true`)
- Init process (`init: true`)
- Capability additions (`capAdd`)
- Security options (`securityOpt`)

**Schema Coverage**: `privileged`, `init`, `capAdd`, `securityOpt`

**Capabilities Tested**: `SYS_PTRACE`, `NET_ADMIN`

**Verification**: Inspects process capabilities

---

### 14. `multi-stage-dockerfile`
**Purpose**: Test multi-stage Docker builds
**Features Tested**:
- Multi-stage Dockerfile with named stages
- Build stage targeting (`target`)
- COPY between stages
- Stage-specific configurations

**Schema Coverage**: `build.dockerfile`, `build.target`

**Stages**:
- `builder` - Build artifacts
- `development` - Development environment (targeted)
- `production` - Production environment (not used)

---

### 15. `override-command`
**Purpose**: Test command override behavior
**Features Tested**:
- Disable command override (`overrideCommand: false`)
- Preserve image's default CMD/ENTRYPOINT

**Schema Coverage**: `overrideCommand`

---

## Running Tests

### Locally
```bash
# Build the CLI
cargo build --release

# Run a specific test
cd tests/basic-image
../../target/release/devcontainer up --workspace-folder .
../../target/release/devcontainer exec --workspace-folder . echo "Test"
../../target/release/devcontainer down --workspace-folder .
```

### CI/CD
Tests are automatically run via GitHub Actions on:
- Push to `main`, `develop`, or `claude/**` branches
- Pull requests to `main` or `develop`
- Manual workflow dispatch

The workflow:
1. Discovers all test directories
2. Builds the devcontainer CLI
3. Runs each test in parallel as a separate job
4. Executes: `up` → `exec` → verification → `down`
5. Provides a summary of all test results

## Adding New Tests

To add a new test case:

1. Create a new directory under `tests/`
2. Add `.devcontainer/devcontainer.json` with your configuration
3. Add any supporting files (Dockerfile, scripts, etc.)
4. The workflow will automatically discover and run the new test
5. Optionally add verification logic in the workflow's "Verify test-specific requirements" step

## Schema Coverage

These tests cover the following major devcontainer.json features:

- ✅ Container source: `image`, `build`, `dockerComposeFile`
- ✅ Build configuration: `dockerfile`, `context`, `args`, `target`
- ✅ Lifecycle commands: All 7 command hooks
- ✅ Features: Installation and configuration
- ✅ Environment: `containerEnv`, `remoteEnv`, variable substitution
- ✅ Mounts: `mounts`, `workspaceMount`, bind and volume types
- ✅ Ports: `forwardPorts`, `portsAttributes`, `appPort`
- ✅ User configuration: `containerUser`, `remoteUser`, `updateRemoteUserUID`
- ✅ Docker options: `runArgs`, `privileged`, `init`, `capAdd`, `securityOpt`
- ✅ Workspace: `workspaceFolder`, custom paths
- ✅ Compose: Multi-service, dependencies, networks, volumes
- ✅ Advanced: `overrideCommand`, `shutdownAction`, `userEnvProbe`

## Not Yet Covered

Features that could be added in future tests:

- GPU requirements (`gpu`)
- Host requirements (`hostRequirements`)
- Customizations for specific tools (`customizations`)
- Secrets recommendations (`secrets`)
- Additional compose scenarios
- Feature install order override (`overrideFeatureInstallOrder`)
- More complex variable substitution patterns
- Network mode variations
- Init hooks with multiple commands

## Test Maintenance

- Keep test cases focused on single features when possible
- Add verification commands to validate feature behavior
- Update this README when adding new test cases
- Ensure test names are descriptive and match directory names
- Include comments in complex configurations

use std::{
    convert::TryFrom,
    fs,
    path::{Path, PathBuf},
};

use jsonschema::JSONSchema;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::{errors::DevcontainerError, Result};

static DEVCONTAINER_SCHEMA: Lazy<JSONSchema> = Lazy::new(|| {
    let schema_json: Value =
        serde_json::from_str(include_str!("../../schemas/devcontainer.schema.json"))
            .expect("Bundled devcontainer schema must be valid JSON");
    JSONSchema::compile(&schema_json).expect("Bundled devcontainer schema must compile")
});

fn validate_against_schema(document: &Value) -> Result<()> {
    if let Err(errors) = DEVCONTAINER_SCHEMA.validate(document) {
        let messages: Vec<String> = errors.map(|err| err.to_string()).collect();
        let message = if messages.is_empty() {
            "Unknown validation error".to_string()
        } else {
            messages.join("; ")
        };
        return Err(DevcontainerError::Configuration(format!(
            "Invalid devcontainer.json: {message}"
        )));
    }

    Ok(())
}

/// Raw devcontainer configuration as read from `devcontainer.json`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DevcontainerConfig {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub image: Option<String>,
    #[serde(default, rename = "dockerFile")]
    pub docker_file: Option<String>,
    #[serde(default, rename = "workspaceFolder")]
    pub workspace_folder: Option<String>,
    #[serde(default)]
    pub features: Map<String, Value>,
    #[serde(default, rename = "forwardPorts")]
    pub forward_ports: Vec<ForwardPortDefinition>,
    #[serde(default, rename = "postCreateCommand")]
    pub post_create_command: Option<CommandDefinition>,
    #[serde(default, rename = "postAttachCommand")]
    pub post_attach_command: Option<CommandDefinition>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ForwardPortDefinition {
    Number(u16),
    NumberString(String),
    Detailed {
        #[serde(rename = "localPort")]
        local_port: u16,
        #[serde(rename = "containerPort")]
        container_port: u16,
        #[serde(default)]
        protocol: PortProtocol,
    },
}

impl TryFrom<ForwardPortDefinition> for ForwardPort {
    type Error = DevcontainerError;

    fn try_from(value: ForwardPortDefinition) -> std::result::Result<Self, Self::Error> {
        match value {
            ForwardPortDefinition::Number(port) => Ok(Self {
                local_port: port,
                container_port: port,
                protocol: PortProtocol::Tcp,
            }),
            ForwardPortDefinition::NumberString(port_str) => {
                let port = port_str.parse::<u16>().map_err(|err| {
                    DevcontainerError::Configuration(format!(
                        "Invalid forward port value '{port_str}': {err}"
                    ))
                })?;
                Ok(Self {
                    local_port: port,
                    container_port: port,
                    protocol: PortProtocol::Tcp,
                })
            }
            ForwardPortDefinition::Detailed {
                local_port,
                container_port,
                protocol,
            } => Ok(Self {
                local_port,
                container_port,
                protocol,
            }),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum CommandDefinition {
    String(String),
    Array(Vec<String>),
}

impl CommandDefinition {
    pub fn to_exec_args(&self) -> Vec<String> {
        match self {
            CommandDefinition::String(command) => {
                vec!["/bin/sh".to_string(), "-c".to_string(), command.clone()]
            }
            CommandDefinition::Array(args) => args.clone(),
        }
    }
}

/// Normalized configuration after resolving overrides and defaults.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ResolvedConfig {
    pub project_name: String,
    pub workspace_folder: PathBuf,
    pub config_path: PathBuf,
    #[serde(default)]
    pub image_reference: Option<String>,
    #[serde(default)]
    pub dockerfile: Option<PathBuf>,
    #[serde(default)]
    pub features: Map<String, Value>,
    #[serde(default)]
    pub forward_ports: Vec<ForwardPort>,
    #[serde(default)]
    pub post_create_command: Option<CommandDefinition>,
    #[serde(default)]
    pub post_attach_command: Option<CommandDefinition>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ForwardPort {
    pub local_port: u16,
    pub container_port: u16,
    #[serde(default)]
    pub protocol: PortProtocol,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PortProtocol {
    #[default]
    Tcp,
    Udp,
}

/// Resolves a `DevcontainerConfig` into a `ResolvedConfig`.
pub struct ConfigResolver {
    source: ConfigSource,
    overrides: ConfigOverrides,
}

impl ConfigResolver {
    pub fn new(source: ConfigSource) -> Self {
        Self {
            source,
            overrides: ConfigOverrides::default(),
        }
    }

    pub fn with_overrides(mut self, overrides: ConfigOverrides) -> Self {
        self.overrides = overrides;
        self
    }

    pub fn resolve(&self) -> Result<ResolvedConfig> {
        tracing::debug!(?self.source, "Resolving devcontainer configuration");

        let config_path = self.source.resolve_path()?;
        let raw_document = fs::read_to_string(&config_path).map_err(|err| {
            DevcontainerError::Configuration(format!(
                "Failed to read {}: {err}",
                config_path.display()
            ))
        })?;

        let document: Value = serde_json::from_str(&raw_document).map_err(|err| {
            DevcontainerError::Configuration(format!(
                "{} is not valid JSON: {err}",
                config_path.display()
            ))
        })?;

        validate_against_schema(&document)?;

        let config: DevcontainerConfig = serde_json::from_value(document).map_err(|err| {
            DevcontainerError::Configuration(format!(
                "{} does not match expected structure: {err}",
                config_path.display()
            ))
        })?;

        let DevcontainerConfig {
            name,
            image,
            docker_file,
            workspace_folder: config_workspace_folder,
            features,
            forward_ports: raw_forward_ports,
            post_create_command,
            post_attach_command,
        } = config;

        let forward_ports: Vec<ForwardPort> = raw_forward_ports
            .into_iter()
            .map(ForwardPort::try_from)
            .collect::<std::result::Result<_, _>>()?;

        let config_dir = config_path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));

        let dockerfile = docker_file.map(|path| {
            let path = PathBuf::from(path);
            if path.is_absolute() {
                path
            } else {
                config_dir.join(path)
            }
        });

        let workspace_root = match &self.source {
            ConfigSource::Workspace(path) => path.clone(),
            ConfigSource::ExplicitFile(_) => config_dir.clone(),
        };

        let config_workspace_folder = config_workspace_folder.map(|folder| {
            let path = PathBuf::from(folder);
            if path.is_absolute() {
                path
            } else {
                workspace_root.join(path)
            }
        });

        let workspace_folder = self
            .overrides
            .workspace_folder
            .clone()
            .or(config_workspace_folder)
            .unwrap_or_else(|| workspace_root.clone());

        let project_name = self
            .overrides
            .project_name
            .clone()
            .or(name)
            .or_else(|| {
                workspace_root
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
            })
            .unwrap_or_else(|| "devcontainer".to_string());

        let image_reference = self.overrides.image_reference.clone().or(image);

        Ok(ResolvedConfig {
            project_name,
            workspace_folder,
            config_path,
            image_reference,
            dockerfile,
            features,
            forward_ports,
            post_create_command,
            post_attach_command,
        })
    }
}

/// Source of configuration data.
#[derive(Debug, Clone)]
pub enum ConfigSource {
    Workspace(PathBuf),
    ExplicitFile(PathBuf),
}

impl ConfigSource {
    fn resolve_path(&self) -> Result<PathBuf> {
        match self {
            ConfigSource::Workspace(path) => {
                let candidate = path.join(".devcontainer").join("devcontainer.json");
                if candidate.exists() {
                    return Ok(candidate);
                }

                let fallback = path.join("devcontainer.json");
                if fallback.exists() {
                    return Ok(fallback);
                }

                Err(DevcontainerError::Configuration(format!(
                    "Failed to locate devcontainer.json under {path:?}"
                )))
            }
            ConfigSource::ExplicitFile(path) => {
                if path.exists() {
                    Ok(path.clone())
                } else {
                    Err(DevcontainerError::Configuration(format!(
                        "Configuration file {path:?} does not exist"
                    )))
                }
            }
        }
    }
}

/// Overrides applied on top of the configuration source.
#[derive(Debug, Clone, Default)]
pub struct ConfigOverrides {
    pub project_name: Option<String>,
    pub workspace_folder: Option<PathBuf>,
    pub image_reference: Option<String>,
    pub env: Map<String, Value>,
}

impl ConfigOverrides {
    pub fn with_workspace_folder(mut self, path: PathBuf) -> Self {
        self.workspace_folder = Some(path);
        self
    }

    pub fn with_project_name(mut self, name: impl Into<String>) -> Self {
        self.project_name = Some(name.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn resolve_reads_workspace_configuration() {
        let workspace = tempdir().expect("tempdir");
        let workspace_path = workspace.path();
        let devcontainer_dir = workspace_path.join(".devcontainer");
        fs::create_dir_all(&devcontainer_dir).expect("create devcontainer dir");
        let config_path = devcontainer_dir.join("devcontainer.json");

        let config = json!({
            "name": "sample",
            "image": "mcr.microsoft.com/devcontainers/base:latest",
            "forwardPorts": [
                3000,
                {"localPort": 9229, "containerPort": 9229, "protocol": "udp"}
            ],
            "postCreateCommand": "echo post create",
            "postAttachCommand": ["echo", "post-attach"],
            "features": {
                "ghcr.io/devcontainers/features/node:1": {
                    "version": "18"
                }
            }
        });
        fs::write(&config_path, serde_json::to_string_pretty(&config).unwrap())
            .expect("write config");

        let resolver = ConfigResolver::new(ConfigSource::Workspace(workspace_path.to_path_buf()));
        let resolved = resolver.resolve().expect("resolve config");

        assert_eq!(resolved.project_name, "sample");
        assert_eq!(resolved.workspace_folder, workspace_path.to_path_buf());
        assert_eq!(resolved.config_path, config_path);
        assert_eq!(
            resolved.image_reference.as_deref(),
            Some("mcr.microsoft.com/devcontainers/base:latest")
        );
        assert_eq!(resolved.forward_ports.len(), 2);
        assert_eq!(resolved.forward_ports[0].local_port, 3000);
        assert_eq!(resolved.forward_ports[0].container_port, 3000);
        assert_eq!(resolved.forward_ports[0].protocol, PortProtocol::Tcp);
        assert_eq!(resolved.forward_ports[1].protocol, PortProtocol::Udp);
        assert!(resolved
            .features
            .contains_key("ghcr.io/devcontainers/features/node:1"));
        assert_eq!(
            resolved.post_create_command,
            Some(CommandDefinition::String("echo post create".to_string()))
        );
        assert_eq!(
            resolved.post_attach_command,
            Some(CommandDefinition::Array(vec![
                "echo".to_string(),
                "post-attach".to_string()
            ]))
        );
    }

    #[test]
    fn workspace_folder_from_config_is_relative_to_workspace_root() {
        let workspace = tempdir().expect("tempdir");
        let workspace_path = workspace.path();
        let devcontainer_dir = workspace_path.join(".devcontainer");
        fs::create_dir_all(&devcontainer_dir).expect("create devcontainer dir");
        fs::create_dir_all(workspace_path.join("nested/project"))
            .expect("create nested workspace folder");
        let config_path = devcontainer_dir.join("devcontainer.json");

        let config = json!({
            "name": "nested",
            "workspaceFolder": "nested/project",
            "forwardPorts": []
        });
        fs::write(&config_path, serde_json::to_string_pretty(&config).unwrap())
            .expect("write config");

        let resolver = ConfigResolver::new(ConfigSource::Workspace(workspace_path.to_path_buf()));
        let resolved = resolver.resolve().expect("resolve config");

        assert_eq!(resolved.project_name, "nested");
        assert_eq!(
            resolved.workspace_folder,
            workspace_path.join("nested/project")
        );
        assert!(resolved.dockerfile.is_none());
    }

    #[test]
    fn overrides_take_precedence() {
        let workspace = tempdir().expect("tempdir");
        let workspace_path = workspace.path();
        let devcontainer_dir = workspace_path.join(".devcontainer");
        fs::create_dir_all(&devcontainer_dir).expect("create devcontainer dir");
        let config_path = devcontainer_dir.join("devcontainer.json");

        let config = json!({
            "name": "original",
            "image": "example:image",
            "forwardPorts": []
        });
        fs::write(&config_path, serde_json::to_string_pretty(&config).unwrap())
            .expect("write config");

        let workspace_override = workspace_path.join("workspace-src");
        fs::create_dir_all(&workspace_override).expect("create override dir");

        let mut overrides = ConfigOverrides::default()
            .with_workspace_folder(workspace_override.clone())
            .with_project_name("override");
        overrides.image_reference = Some("override:image".into());

        let resolver = ConfigResolver::new(ConfigSource::Workspace(workspace_path.to_path_buf()))
            .with_overrides(overrides);
        let resolved = resolver.resolve().expect("resolve config");

        assert_eq!(resolved.project_name, "override");
        assert_eq!(resolved.workspace_folder, workspace_override);
        assert_eq!(resolved.image_reference.as_deref(), Some("override:image"));
        assert!(resolved.dockerfile.is_none());
    }

    #[test]
    fn invalid_configuration_reports_schema_error() {
        let workspace = tempdir().expect("tempdir");
        let workspace_path = workspace.path();
        let devcontainer_dir = workspace_path.join(".devcontainer");
        fs::create_dir_all(&devcontainer_dir).expect("create devcontainer dir");
        let config_path = devcontainer_dir.join("devcontainer.json");

        let config = json!({
            "name": "broken",
            "forwardPorts": ["not-a-number"],
            "features": {}
        });
        fs::write(&config_path, serde_json::to_string_pretty(&config).unwrap())
            .expect("write config");

        let resolver = ConfigResolver::new(ConfigSource::Workspace(workspace_path.to_path_buf()));
        let err = resolver.resolve().expect_err("expect schema error");

        match err {
            DevcontainerError::Configuration(message) => {
                assert!(message.contains("Invalid devcontainer.json"));
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn resolve_resolves_dockerfile_path_relative_to_config() {
        let workspace = tempdir().expect("tempdir");
        let workspace_path = workspace.path();
        let devcontainer_dir = workspace_path.join(".devcontainer");
        fs::create_dir_all(&devcontainer_dir).expect("create devcontainer dir");
        let config_path = devcontainer_dir.join("devcontainer.json");
        let dockerfile_path = devcontainer_dir.join("Dockerfile");

        let config = json!({
            "name": "dockerfile",
            "dockerFile": "Dockerfile",
            "forwardPorts": []
        });
        fs::write(&config_path, serde_json::to_string_pretty(&config).unwrap())
            .expect("write config");
        fs::write(&dockerfile_path, "FROM scratch\n").expect("write dockerfile");

        let resolver = ConfigResolver::new(ConfigSource::Workspace(workspace_path.to_path_buf()));
        let resolved = resolver.resolve().expect("resolve config");

        assert_eq!(
            resolved.dockerfile.as_ref().expect("expected dockerfile"),
            &dockerfile_path
        );
    }
}

use std::{
    path::{Path, PathBuf},
    process::{ExitStatus, Stdio},
};

use async_trait::async_trait;
use devcontainer_core::{
    config::ResolvedConfig,
    provider::{
        ExecResult, Provider, ProviderBuildContext, ProviderCleanupOptions, ProviderImage,
        ProviderKind, ProviderPreparation, RunningContainer, VolumeSpec,
    },
    DevcontainerError, Result,
};
use tokio::process::Command;
use tracing::{debug, info, warn};

#[derive(Debug, Clone)]
pub struct DockerProvider {
    docker_path: PathBuf,
}

impl DockerProvider {
    pub fn new() -> Self {
        Self::from_path(PathBuf::from("docker"))
    }

    pub fn from_path(path: impl Into<PathBuf>) -> Self {
        Self {
            docker_path: path.into(),
        }
    }

    fn cli(&self) -> Result<DockerCli> {
        DockerCli::new(&self.docker_path)
    }
}

impl Default for DockerProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Provider for DockerProvider {
    fn kind(&self) -> ProviderKind {
        ProviderKind::Docker
    }

    async fn prepare(&self, config: &ResolvedConfig) -> Result<ProviderPreparation> {
        let cli = self.cli()?;
        cli.verify_binary().await?;

        if !config.workspace_folder.exists() {
            return Err(DevcontainerError::Configuration(format!(
                "Workspace folder {} does not exist",
                config.workspace_folder.display()
            )));
        }

        let project_slug = sanitize_name(&config.project_name);
        let container_name = format!("devcontainer-{project_slug}");
        let workspace_mount_path = PathBuf::from(format!("/workspaces/{project_slug}"));
        let network_name = format!("devcontainer-{project_slug}-network");
        let volume_name = format!("devcontainer-{project_slug}-data");

        let image = if let Some(reference) = &config.image_reference {
            ProviderImage::Reference(reference.clone())
        } else if let Some(dockerfile) = &config.dockerfile {
            if !dockerfile.exists() {
                return Err(DevcontainerError::Configuration(format!(
                    "Dockerfile {} does not exist",
                    dockerfile.display()
                )));
            }

            let build_context = dockerfile
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| config.workspace_folder.clone());
            let tag = format!("devcontainer-{project_slug}:latest");
            ProviderImage::Build(ProviderBuildContext {
                dockerfile: dockerfile.clone(),
                build_context,
                tag,
            })
        } else {
            return Err(DevcontainerError::Configuration(
                "devcontainer.json must define either `image` or `dockerFile`".into(),
            ));
        };

        Ok(ProviderPreparation {
            image,
            container_name,
            project_slug,
            networks: vec![network_name],
            volumes: vec![VolumeSpec {
                name: volume_name,
                mount_path: PathBuf::from("/workspaces/.devcontainer"),
            }],
            workspace_mount_path,
        })
    }

    async fn ensure_networks(
        &self,
        _config: &ResolvedConfig,
        preparation: &ProviderPreparation,
    ) -> Result<()> {
        let cli = self.cli()?;

        for network in &preparation.networks {
            let inspect = cli
                .run(vec![
                    "network".to_string(),
                    "inspect".to_string(),
                    network.clone(),
                ])
                .await?;
            if inspect.status.success() {
                debug!(network = %network, "Docker network already exists");
                continue;
            }

            if inspect.stderr.contains("No such network") {
                info!(network = %network, "Creating docker network");
                cli.run_expect_success(vec![
                    "network".to_string(),
                    "create".to_string(),
                    network.clone(),
                ])
                .await?;
            } else {
                return Err(DevcontainerError::Provider(format!(
                    "Failed to inspect docker network {network}: {}",
                    inspect.stderr.trim()
                )));
            }
        }

        Ok(())
    }

    async fn ensure_volumes(
        &self,
        _config: &ResolvedConfig,
        preparation: &ProviderPreparation,
    ) -> Result<()> {
        let cli = self.cli()?;

        for volume in &preparation.volumes {
            let inspect = cli
                .run(vec![
                    "volume".to_string(),
                    "inspect".to_string(),
                    volume.name.clone(),
                ])
                .await?;
            if inspect.status.success() {
                debug!(volume = %volume.name, "Docker volume already exists");
                continue;
            }

            if inspect.stderr.contains("No such volume") {
                info!(volume = %volume.name, "Creating docker volume");
                cli.run_expect_success(vec![
                    "volume".to_string(),
                    "create".to_string(),
                    volume.name.clone(),
                ])
                .await?;
            } else {
                return Err(DevcontainerError::Provider(format!(
                    "Failed to inspect docker volume {}: {}",
                    volume.name,
                    inspect.stderr.trim()
                )));
            }
        }

        Ok(())
    }

    async fn build_image(
        &self,
        _config: &ResolvedConfig,
        preparation: &ProviderPreparation,
    ) -> Result<String> {
        let cli = self.cli()?;

        match &preparation.image {
            ProviderImage::Reference(reference) => {
                let inspect = cli
                    .run(vec![
                        "image".to_string(),
                        "inspect".to_string(),
                        reference.clone(),
                    ])
                    .await?;
                if inspect.status.success() {
                    debug!(image = %reference, "Using locally available image");
                    return Ok(reference.clone());
                }

                info!(image = %reference, "Pulling image via docker pull");
                cli.run_expect_success(vec!["pull".to_string(), reference.clone()])
                    .await?;
                Ok(reference.clone())
            }
            ProviderImage::Build(build) => {
                info!(
                    dockerfile = %build.dockerfile.display(),
                    context = %build.build_context.display(),
                    "Building devcontainer image"
                );

                let dockerfile = path_to_string(&build.dockerfile)?;
                let context = path_to_string(&build.build_context)?;

                cli.run_expect_success(vec![
                    "build".to_string(),
                    "-f".to_string(),
                    dockerfile,
                    "-t".to_string(),
                    build.tag.clone(),
                    context,
                ])
                .await?;

                Ok(build.tag.clone())
            }
        }
    }

    async fn create_container(
        &self,
        config: &ResolvedConfig,
        preparation: &ProviderPreparation,
        image_reference: &str,
    ) -> Result<RunningContainer> {
        let cli = self.cli()?;

        let identifier = &preparation.container_name;
        let remove = cli
            .run(vec![
                "container".to_string(),
                "rm".to_string(),
                "--force".to_string(),
                identifier.clone(),
            ])
            .await?;
        if !remove.status.success()
            && !remove.stderr.contains("No such container")
            && !remove.stderr.is_empty()
        {
            warn!(
                container = %identifier,
                stderr = %remove.stderr.trim(),
                "Failed to remove existing container before create"
            );
        }

        let workspace_src = path_to_string(&config.workspace_folder)?;
        let workspace_dst = path_to_string(&preparation.workspace_mount_path)?;

        let mut args = vec![
            "create".to_string(),
            "--name".to_string(),
            identifier.clone(),
            "--hostname".to_string(),
            identifier.clone(),
        ];

        if let Some(network) = preparation.networks.first() {
            args.push("--network".to_string());
            args.push(network.clone());
        }

        args.push("--label".to_string());
        args.push(format!("devcontainer.project={}", config.project_name));

        args.push("--workdir".to_string());
        args.push(workspace_dst.clone());

        args.push("--mount".to_string());
        args.push(format!("type=bind,src={workspace_src},dst={workspace_dst}"));

        for volume in &preparation.volumes {
            let mount_path = path_to_string(&volume.mount_path)?;
            args.push("--mount".to_string());
            args.push(format!("type=volume,src={},dst={mount_path}", volume.name));
        }

        args.push(image_reference.to_string());
        args.push("sleep".to_string());
        args.push("infinity".to_string());

        let output = cli.run_expect_success(args).await?;
        let id = output.stdout.trim().to_string();

        Ok(RunningContainer {
            id: if id.is_empty() { None } else { Some(id) },
            name: Some(identifier.clone()),
        })
    }

    async fn start_container(&self, container: &RunningContainer) -> Result<()> {
        let cli = self.cli()?;
        let identifier = container
            .name
            .as_ref()
            .or(container.id.as_ref())
            .ok_or_else(|| DevcontainerError::Provider("Container has no identifier".into()))?;

        cli.run_expect_success(vec!["start".to_string(), identifier.clone()])
            .await?;
        Ok(())
    }

    async fn exec(&self, container: &RunningContainer, command: &[String]) -> Result<ExecResult> {
        if command.is_empty() {
            return Ok(ExecResult::default());
        }

        let cli = self.cli()?;
        let identifier = container
            .name
            .as_ref()
            .or(container.id.as_ref())
            .ok_or_else(|| DevcontainerError::Provider("Container has no identifier".into()))?;

        let mut args = Vec::with_capacity(2 + command.len());
        args.push("exec".to_string());
        args.push(identifier.clone());
        args.extend(command.iter().cloned());

        let output = cli.run(args).await?;
        let exit_code = output.status.code().unwrap_or(-1);

        Ok(ExecResult {
            exit_code,
            stdout: output.stdout,
            stderr: output.stderr,
        })
    }

    async fn stop_container(
        &self,
        _config: &ResolvedConfig,
        preparation: &ProviderPreparation,
        container: &RunningContainer,
    ) -> Result<()> {
        let cli = self.cli()?;

        let identifier = container
            .name
            .as_ref()
            .or(container.id.as_ref())
            .cloned()
            .unwrap_or_else(|| preparation.container_name.clone());

        let output = cli
            .run(vec![
                "container".to_string(),
                "stop".to_string(),
                identifier.clone(),
            ])
            .await?;

        if output.status.success() {
            info!(container = %identifier, "Stopped container");
            return Ok(());
        }

        if output.stderr.contains("No such container") || output.stderr.contains("is not running") {
            debug!(container = %identifier, stderr = %output.stderr.trim(), "Container already stopped or missing");
            return Ok(());
        }

        Err(DevcontainerError::Provider(format!(
            "Failed to stop container {identifier}: {}",
            output.stderr.trim()
        )))
    }

    async fn cleanup(
        &self,
        _config: &ResolvedConfig,
        preparation: &ProviderPreparation,
        options: &ProviderCleanupOptions,
    ) -> Result<()> {
        let cli = self.cli()?;
        let mut args = vec![
            "container".to_string(),
            "rm".to_string(),
            "--force".to_string(),
        ];

        if options.remove_volumes {
            args.push("--volumes".to_string());
        }

        args.push(preparation.container_name.clone());

        let remove_container = cli.run(args).await?;
        if !remove_container.status.success()
            && !remove_container.stderr.contains("No such container")
        {
            return Err(DevcontainerError::Provider(format!(
                "Failed to remove container {}: {}",
                preparation.container_name,
                remove_container.stderr.trim()
            )));
        }

        for network in &preparation.networks {
            let output = cli
                .run(vec![
                    "network".to_string(),
                    "rm".to_string(),
                    network.clone(),
                ])
                .await?;
            if output.status.success() {
                info!(network = %network, "Removed docker network");
            } else if output.stderr.contains("No such network") {
                debug!(network = %network, "Docker network already absent");
            } else {
                return Err(DevcontainerError::Provider(format!(
                    "Failed to remove docker network {network}: {}",
                    output.stderr.trim()
                )));
            }
        }

        if options.remove_volumes {
            for volume in &preparation.volumes {
                let output = cli
                    .run(vec![
                        "volume".to_string(),
                        "rm".to_string(),
                        volume.name.clone(),
                    ])
                    .await?;
                if output.status.success() {
                    info!(volume = %volume.name, "Removed docker volume");
                } else if output.stderr.contains("No such volume") {
                    debug!(volume = %volume.name, "Docker volume already absent");
                } else {
                    return Err(DevcontainerError::Provider(format!(
                        "Failed to remove docker volume {}: {}",
                        volume.name,
                        output.stderr.trim()
                    )));
                }
            }
        }

        if options.remove_unknown {
            warn!("remove-unknown cleanup not implemented for docker provider");
        }

        Ok(())
    }
}

#[derive(Debug, Clone)]
struct DockerCli {
    program: PathBuf,
}

impl DockerCli {
    fn new(path: &Path) -> Result<Self> {
        let resolved = if path.components().count() == 1 {
            which::which(path).map_err(|err| {
                DevcontainerError::Provider(format!(
                    "Failed to locate docker binary '{}': {err}",
                    path.display()
                ))
            })?
        } else {
            PathBuf::from(path)
        };

        Ok(Self { program: resolved })
    }

    async fn verify_binary(&self) -> Result<()> {
        let output = self
            .run(vec![
                "version".to_string(),
                "--format".to_string(),
                "{{.Server.Version}}".to_string(),
            ])
            .await?;

        if output.status.success() {
            debug!(
                docker = %self.program.display(),
                version = %output.stdout.trim(),
                "Docker CLI reachable"
            );
            Ok(())
        } else {
            Err(DevcontainerError::Provider(format!(
                "Failed to execute '{} --version': {}",
                self.program.display(),
                output.stderr.trim()
            )))
        }
    }

    async fn run(&self, args: Vec<String>) -> Result<CommandOutput> {
        let mut command = Command::new(&self.program);
        command.args(&args);
        command.stdin(Stdio::null());
        let output = command.output().await.map_err(|err| {
            DevcontainerError::Provider(format!(
                "Failed to spawn '{}': {err}",
                format_command(&self.program, &args)
            ))
        })?;

        Ok(CommandOutput::new(
            format_command(&self.program, &args),
            output.status,
            output.stdout,
            output.stderr,
        ))
    }

    async fn run_expect_success(&self, args: Vec<String>) -> Result<CommandOutput> {
        let output = self.run(args).await?;
        output.ensure_success()
    }
}

#[derive(Debug, Clone)]
struct CommandOutput {
    command: String,
    status: ExitStatus,
    stdout: String,
    stderr: String,
}

impl CommandOutput {
    fn new(command: String, status: ExitStatus, stdout: Vec<u8>, stderr: Vec<u8>) -> Self {
        Self {
            command,
            status,
            stdout: String::from_utf8_lossy(&stdout).into_owned(),
            stderr: String::from_utf8_lossy(&stderr).into_owned(),
        }
    }

    fn ensure_success(self) -> Result<Self> {
        if self.status.success() {
            Ok(self)
        } else {
            let code = self.status.code().unwrap_or(-1);
            Err(DevcontainerError::Provider(format!(
                "Command '{}' exited with code {code}. stdout: {} stderr: {}",
                self.command,
                self.stdout.trim(),
                self.stderr.trim()
            )))
        }
    }
}

fn sanitize_name(input: &str) -> String {
    let mut result = String::new();

    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() {
            result.push(ch.to_ascii_lowercase());
        } else if matches!(ch, '-' | '_' | '.') {
            result.push(ch);
        } else {
            result.push('-');
        }
    }

    while result.starts_with('-') {
        result.remove(0);
    }

    while result.ends_with('-') {
        result.pop();
    }

    if result.is_empty() {
        "devcontainer".into()
    } else {
        result
    }
}

fn format_command(program: &Path, args: &[String]) -> String {
    let mut command = program.display().to_string();
    for arg in args {
        command.push(' ');
        command.push_str(arg);
    }
    command
}

fn path_to_string(path: &Path) -> Result<String> {
    if let Some(value) = path.to_str() {
        return Ok(value.to_string());
    }

    if let Ok(cap) = path.canonicalize() {
        if let Some(value) = cap.to_str() {
            return Ok(value.to_string());
        }
    }

    Err(DevcontainerError::Provider(format!(
        "Unable to represent path {} as UTF-8",
        path.display()
    )))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use devcontainer_core::config::ResolvedConfig;
    use tempfile::tempdir;

    use super::*;

    #[tokio::test]
    async fn prepare_builds_expected_metadata_for_reference_image() {
        let temp = tempdir().expect("temp workspace");
        let workspace = temp.path().join("workspace");
        fs::create_dir_all(&workspace).expect("workspace directory");
        let config_path = temp.path().join("devcontainer.json");
        fs::write(&config_path, "{}").expect("write config stub");

        let provider = DockerProvider::from_path("/bin/echo");
        let config = ResolvedConfig {
            project_name: "Sample Project".into(),
            workspace_folder: workspace,
            config_path,
            image_reference: Some("ghcr.io/devcontainers/base:latest".into()),
            dockerfile: None,
            features: Default::default(),
            forward_ports: vec![],
            post_create_command: None,
            post_attach_command: None,
        };

        let preparation = provider.prepare(&config).await.unwrap();
        assert_eq!(preparation.container_name, "devcontainer-sample-project");
        assert_eq!(preparation.project_slug, "sample-project");
        assert_eq!(
            preparation.workspace_mount_path,
            PathBuf::from("/workspaces/sample-project")
        );
        assert_eq!(
            preparation.networks,
            vec!["devcontainer-sample-project-network".to_string()]
        );
        assert_eq!(preparation.volumes.len(), 1);
        assert!(matches!(preparation.image, ProviderImage::Reference(_)));
    }
}

use std::path::PathBuf;

use async_trait::async_trait;

use crate::{config::ResolvedConfig, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderKind {
    Docker,
    Podman,
    Mock,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ProviderCapabilities {
    pub supports_features: bool,
    pub supports_templates: bool,
    pub supports_attach: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderBuildContext {
    pub dockerfile: PathBuf,
    pub build_context: PathBuf,
    pub tag: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderImage {
    Reference(String),
    Build(ProviderBuildContext),
}

impl ProviderImage {
    pub fn reference(&self) -> &str {
        match self {
            ProviderImage::Reference(value) => value,
            ProviderImage::Build(build) => &build.tag,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VolumeSpec {
    pub name: String,
    pub mount_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderPreparation {
    pub image: ProviderImage,
    pub container_name: String,
    pub project_slug: String,
    pub networks: Vec<String>,
    pub volumes: Vec<VolumeSpec>,
    pub workspace_mount_path: PathBuf,
}

#[derive(Debug, Clone, Default)]
pub struct RunningContainer {
    pub id: Option<String>,
    pub name: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExecResult {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProviderCleanupOptions {
    pub remove_volumes: bool,
    pub remove_unknown: bool,
}

#[async_trait]
pub trait Provider: Send + Sync {
    fn kind(&self) -> ProviderKind;

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities::default()
    }

    async fn prepare(&self, config: &ResolvedConfig) -> Result<ProviderPreparation>;

    async fn ensure_networks(
        &self,
        config: &ResolvedConfig,
        preparation: &ProviderPreparation,
    ) -> Result<()>;

    async fn ensure_volumes(
        &self,
        config: &ResolvedConfig,
        preparation: &ProviderPreparation,
    ) -> Result<()>;

    async fn build_image(
        &self,
        config: &ResolvedConfig,
        preparation: &ProviderPreparation,
    ) -> Result<String>;

    async fn create_container(
        &self,
        config: &ResolvedConfig,
        preparation: &ProviderPreparation,
        image_reference: &str,
    ) -> Result<RunningContainer>;

    async fn start_container(&self, container: &RunningContainer) -> Result<()>;

    async fn exec(&self, container: &RunningContainer, command: &[String]) -> Result<ExecResult>;

    async fn stop_container(
        &self,
        config: &ResolvedConfig,
        preparation: &ProviderPreparation,
        container: &RunningContainer,
    ) -> Result<()>;

    async fn cleanup(
        &self,
        config: &ResolvedConfig,
        preparation: &ProviderPreparation,
        options: &ProviderCleanupOptions,
    ) -> Result<()>;
}

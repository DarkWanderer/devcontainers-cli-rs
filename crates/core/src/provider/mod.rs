use async_trait::async_trait;

use crate::{config::ResolvedConfig, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderKind {
    Docker,
    Podman,
    Mock,
}

#[derive(Debug, Clone, Default)]
pub struct ProviderCapabilities {
    pub supports_features: bool,
    pub supports_templates: bool,
    pub supports_attach: bool,
}

#[derive(Debug, Clone, Default)]
pub struct ProviderPreparation {
    pub image_reference: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct RunningContainer {
    pub id: Option<String>,
    pub name: Option<String>,
}

#[async_trait]
pub trait Provider: Send + Sync {
    fn kind(&self) -> ProviderKind;

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities::default()
    }

    async fn prepare(&self, _config: &ResolvedConfig) -> Result<ProviderPreparation> {
        Ok(ProviderPreparation::default())
    }

    async fn start(&self, _preparation: ProviderPreparation) -> Result<RunningContainer> {
        Ok(RunningContainer::default())
    }
}

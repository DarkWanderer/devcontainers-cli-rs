use std::fmt::Display;

use crate::{
    config::ResolvedConfig,
    provider::{Provider, ProviderPreparation, RunningContainer},
    Result,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifecyclePhase {
    Resolve,
    Build,
    Create,
    Start,
    PostCreate,
    PostAttach,
}

impl Display for LifecyclePhase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                LifecyclePhase::Resolve => "resolve",
                LifecyclePhase::Build => "build",
                LifecyclePhase::Create => "create",
                LifecyclePhase::Start => "start",
                LifecyclePhase::PostCreate => "postCreate",
                LifecyclePhase::PostAttach => "postAttach",
            }
        )
    }
}

#[derive(Debug, Clone)]
pub struct LifecycleStep {
    pub phase: LifecyclePhase,
    pub description: String,
}

#[derive(Debug, Default, Clone)]
pub struct LifecyclePlan {
    pub steps: Vec<LifecycleStep>,
}

impl LifecyclePlan {
    pub fn new() -> Self {
        Self { steps: Vec::new() }
    }

    pub fn push(&mut self, phase: LifecyclePhase, description: impl Into<String>) {
        self.steps.push(LifecycleStep {
            phase,
            description: description.into(),
        });
    }
}

#[derive(Debug, Default, Clone)]
pub struct LifecycleOutcome {
    pub container: RunningContainer,
    pub executed_phases: Vec<LifecyclePhase>,
}

pub struct LifecycleExecutor<P: Provider> {
    provider: P,
}

impl<P: Provider> LifecycleExecutor<P> {
    pub fn new(provider: P) -> Self {
        Self { provider }
    }

    pub async fn execute(
        &self,
        config: &ResolvedConfig,
        plan: &LifecyclePlan,
    ) -> Result<LifecycleOutcome> {
        tracing::info!("Starting lifecycle execution");
        let mut executed_phases = Vec::new();

        tracing::debug!(
            ?config,
            step_count = plan.steps.len(),
            "Prepared lifecycle plan"
        );

        let preparation: ProviderPreparation = self.provider.prepare(config).await?;
        executed_phases.push(LifecyclePhase::Resolve);

        let container = self.provider.start(preparation).await?;
        executed_phases.push(LifecyclePhase::Start);

        Ok(LifecycleOutcome {
            container,
            executed_phases,
        })
    }
}

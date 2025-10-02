use std::{fmt::Display, path::PathBuf};

use crate::{
    config::ResolvedConfig,
    provider::{Provider, RunningContainer},
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LifecycleEvent {
    pub code: &'static str,
    pub message: String,
    pub detail: LifecycleEventDetail,
}

impl LifecycleEvent {
    pub fn new(
        code: &'static str,
        message: impl Into<String>,
        detail: LifecycleEventDetail,
    ) -> Self {
        Self {
            code,
            message: message.into(),
            detail,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LifecycleEventDetail {
    ResolveConfig {
        config_path: PathBuf,
    },
    BuildImage {
        image_reference: Option<String>,
    },
    CreateContainer {
        project_name: String,
        workspace_folder: PathBuf,
    },
    StartContainer {
        project_name: String,
    },
    Hook {
        hook: LifecycleHook,
        action: HookAction,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifecycleHook {
    PostCreate,
    PostAttach,
}

impl Display for LifecycleHook {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            LifecycleHook::PostCreate => "postCreate",
            LifecycleHook::PostAttach => "postAttach",
        };
        write!(f, "{}", name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HookAction {
    Execute,
    Skip { reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LifecycleStep {
    pub phase: LifecyclePhase,
    pub event: LifecycleEvent,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct LifecyclePlan {
    pub steps: Vec<LifecycleStep>,
}

impl LifecyclePlan {
    pub fn new() -> Self {
        Self { steps: Vec::new() }
    }

    pub fn push(&mut self, phase: LifecyclePhase, event: LifecycleEvent) {
        self.steps.push(LifecycleStep { phase, event });
    }

    pub fn step_for_phase(&self, phase: LifecyclePhase) -> Option<&LifecycleStep> {
        self.steps.iter().find(|step| step.phase == phase)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LifecyclePlanOptions {
    pub skip_post_create: Option<String>,
    pub skip_post_attach: Option<String>,
}

impl LifecyclePlan {
    pub fn for_up(config: &ResolvedConfig, options: LifecyclePlanOptions) -> Self {
        let mut plan = LifecyclePlan::new();

        plan.push(
            LifecyclePhase::Resolve,
            LifecycleEvent::new(
                "lifecycle.resolve.config",
                format!(
                    "Resolve devcontainer configuration from {}",
                    config.config_path.display()
                ),
                LifecycleEventDetail::ResolveConfig {
                    config_path: config.config_path.clone(),
                },
            ),
        );

        let build_message = match &config.image_reference {
            Some(image) => format!("Ensure devcontainer image {image} is available"),
            None => "Build devcontainer image from workspace configuration".to_string(),
        };

        plan.push(
            LifecyclePhase::Build,
            LifecycleEvent::new(
                "lifecycle.build.image",
                build_message,
                LifecycleEventDetail::BuildImage {
                    image_reference: config.image_reference.clone(),
                },
            ),
        );

        plan.push(
            LifecyclePhase::Create,
            LifecycleEvent::new(
                "lifecycle.create.container",
                format!("Create container for project {}", config.project_name),
                LifecycleEventDetail::CreateContainer {
                    project_name: config.project_name.clone(),
                    workspace_folder: config.workspace_folder.clone(),
                },
            ),
        );

        plan.push(
            LifecyclePhase::Start,
            LifecycleEvent::new(
                "lifecycle.start.container",
                format!("Start container for project {}", config.project_name),
                LifecycleEventDetail::StartContainer {
                    project_name: config.project_name.clone(),
                },
            ),
        );

        let LifecyclePlanOptions {
            skip_post_create,
            skip_post_attach,
        } = options;

        let post_create_action = match skip_post_create {
            Some(reason) => HookAction::Skip { reason },
            None => HookAction::Execute,
        };

        let post_create_code = match &post_create_action {
            HookAction::Execute => "lifecycle.hook.postCreate",
            HookAction::Skip { .. } => "lifecycle.hook.postCreate.skip",
        };

        let post_create_message = match &post_create_action {
            HookAction::Execute => "Run postCreate lifecycle hook".to_string(),
            HookAction::Skip { reason } => {
                format!("Skip postCreate lifecycle hook ({reason})")
            }
        };

        plan.push(
            LifecyclePhase::PostCreate,
            LifecycleEvent::new(
                post_create_code,
                post_create_message,
                LifecycleEventDetail::Hook {
                    hook: LifecycleHook::PostCreate,
                    action: post_create_action.clone(),
                },
            ),
        );

        let post_attach_action = match skip_post_attach {
            Some(reason) => HookAction::Skip { reason },
            None => HookAction::Execute,
        };

        let post_attach_code = match &post_attach_action {
            HookAction::Execute => "lifecycle.hook.postAttach",
            HookAction::Skip { .. } => "lifecycle.hook.postAttach.skip",
        };

        let post_attach_message = match &post_attach_action {
            HookAction::Execute => "Run postAttach lifecycle hook".to_string(),
            HookAction::Skip { reason } => {
                format!("Skip postAttach lifecycle hook ({reason})")
            }
        };

        plan.push(
            LifecyclePhase::PostAttach,
            LifecycleEvent::new(
                post_attach_code,
                post_attach_message,
                LifecycleEventDetail::Hook {
                    hook: LifecycleHook::PostAttach,
                    action: post_attach_action,
                },
            ),
        );

        plan
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

        for (index, step) in plan.steps.iter().enumerate() {
            tracing::info!(
                phase = %step.phase,
                code = step.event.code,
                message = %step.event.message,
                detail = ?step.event.detail,
                step_index = index,
                "Lifecycle step planned"
            );
        }

        if let Some(step) = plan.step_for_phase(LifecyclePhase::Resolve) {
            tracing::info!(
                phase = %step.phase,
                code = step.event.code,
                message = %step.event.message,
                "Executing lifecycle phase"
            );
        }
        let preparation = self.provider.prepare(config).await?;
        executed_phases.push(LifecyclePhase::Resolve);

        tracing::debug!(
            container_name = %preparation.container_name,
            project_slug = %preparation.project_slug,
            networks = ?preparation.networks,
            volumes = ?preparation.volumes,
            workspace_mount = %preparation.workspace_mount_path.display(),
            image = %preparation.image.reference(),
            "Provider preparation complete"
        );

        self.provider.ensure_networks(config, &preparation).await?;
        self.provider.ensure_volumes(config, &preparation).await?;

        if let Some(step) = plan.step_for_phase(LifecyclePhase::Build) {
            tracing::info!(
                phase = %step.phase,
                code = step.event.code,
                message = %step.event.message,
                "Executing lifecycle phase"
            );
        }
        let image_reference = self.provider.build_image(config, &preparation).await?;
        executed_phases.push(LifecyclePhase::Build);

        if let Some(step) = plan.step_for_phase(LifecyclePhase::Create) {
            tracing::info!(
                phase = %step.phase,
                code = step.event.code,
                message = %step.event.message,
                "Executing lifecycle phase"
            );
        }
        let container = self
            .provider
            .create_container(config, &preparation, &image_reference)
            .await?;
        executed_phases.push(LifecyclePhase::Create);

        if let Some(step) = plan.step_for_phase(LifecyclePhase::Start) {
            tracing::info!(
                phase = %step.phase,
                code = step.event.code,
                message = %step.event.message,
                "Executing lifecycle phase"
            );
        }

        self.provider.start_container(&container).await?;
        executed_phases.push(LifecyclePhase::Start);

        if plan.step_for_phase(LifecyclePhase::PostCreate).is_some() {
            executed_phases.push(LifecyclePhase::PostCreate);
        }

        if plan.step_for_phase(LifecyclePhase::PostAttach).is_some() {
            executed_phases.push(LifecyclePhase::PostAttach);
        }

        Ok(LifecycleOutcome {
            container,
            executed_phases,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_config() -> ResolvedConfig {
        ResolvedConfig {
            project_name: "demo".to_string(),
            workspace_folder: PathBuf::from("/workspace"),
            config_path: PathBuf::from("/workspace/.devcontainer/devcontainer.json"),
            image_reference: Some("example:image".to_string()),
            dockerfile: None,
            features: Default::default(),
            forward_ports: vec![],
        }
    }

    #[test]
    fn plan_for_up_contains_all_phases() {
        let config = sample_config();
        let plan = LifecyclePlan::for_up(&config, LifecyclePlanOptions::default());

        let phases: Vec<_> = plan.steps.iter().map(|step| step.phase).collect();
        assert_eq!(
            phases,
            vec![
                LifecyclePhase::Resolve,
                LifecyclePhase::Build,
                LifecyclePhase::Create,
                LifecyclePhase::Start,
                LifecyclePhase::PostCreate,
                LifecyclePhase::PostAttach,
            ]
        );

        assert!(matches!(
            plan.steps[1].event.detail,
            LifecycleEventDetail::BuildImage {
                image_reference: Some(ref image)
            } if image == "example:image"
        ));

        assert!(matches!(
            plan.steps[4].event.detail,
            LifecycleEventDetail::Hook {
                hook: LifecycleHook::PostCreate,
                action: HookAction::Execute
            }
        ));

        assert!(matches!(
            plan.steps[5].event.detail,
            LifecycleEventDetail::Hook {
                hook: LifecycleHook::PostAttach,
                action: HookAction::Execute
            }
        ));
    }

    #[test]
    fn plan_for_up_respects_skip_flags() {
        let mut config = sample_config();
        config.image_reference = None;

        let plan = LifecyclePlan::for_up(
            &config,
            LifecyclePlanOptions {
                skip_post_create: Some("--skip-post-create flag set".to_string()),
                skip_post_attach: Some("--skip-post-attach flag set".to_string()),
            },
        );

        assert!(matches!(
            plan.steps[1].event.detail,
            LifecycleEventDetail::BuildImage {
                image_reference: None
            }
        ));

        assert!(matches!(
            plan.steps[4].event.detail,
            LifecycleEventDetail::Hook {
                hook: LifecycleHook::PostCreate,
                action: HookAction::Skip { ref reason }
            } if reason == "--skip-post-create flag set"
        ));

        assert!(matches!(
            plan.steps[5].event.detail,
            LifecycleEventDetail::Hook {
                hook: LifecycleHook::PostAttach,
                action: HookAction::Skip { ref reason }
            } if reason == "--skip-post-attach flag set"
        ));
    }
}

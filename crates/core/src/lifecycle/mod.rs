use std::{fmt::Display, path::PathBuf};

use crate::{
    config::{CommandArgs, CommandDefinition, ResolvedConfig},
    provider::{Provider, RunningContainer},
    DevcontainerError, Result,
};

const NO_POST_CREATE_COMMAND_REASON: &str = "No postCreate command defined in configuration";
const NO_POST_ATTACH_COMMAND_REASON: &str = "No postAttach command defined in configuration";

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
        write!(f, "{name}")
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
            mut skip_post_create,
            mut skip_post_attach,
        } = options;

        let post_create_action = if let Some(reason) = skip_post_create.take() {
            HookAction::Skip { reason }
        } else if config.post_create_command.is_none() {
            HookAction::Skip {
                reason: NO_POST_CREATE_COMMAND_REASON.to_string(),
            }
        } else {
            HookAction::Execute
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

        let post_attach_action = if let Some(reason) = skip_post_attach.take() {
            HookAction::Skip { reason }
        } else if config.post_attach_command.is_none() {
            HookAction::Skip {
                reason: NO_POST_ATTACH_COMMAND_REASON.to_string(),
            }
        } else {
            HookAction::Execute
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

    pub fn provider(&self) -> &P {
        &self.provider
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

        if let Some(step) = plan.step_for_phase(LifecyclePhase::PostCreate) {
            tracing::info!(
                phase = %step.phase,
                code = step.event.code,
                message = %step.event.message,
                "Executing lifecycle phase"
            );

            if let LifecycleEventDetail::Hook {
                hook: LifecycleHook::PostCreate,
                action,
            } = &step.event.detail
            {
                self.handle_hook(
                    LifecycleHook::PostCreate,
                    action,
                    config.post_create_command.as_ref(),
                    &container,
                )
                .await?;
            }

            executed_phases.push(LifecyclePhase::PostCreate);
        }

        if let Some(step) = plan.step_for_phase(LifecyclePhase::PostAttach) {
            tracing::info!(
                phase = %step.phase,
                code = step.event.code,
                message = %step.event.message,
                "Executing lifecycle phase"
            );

            if let LifecycleEventDetail::Hook {
                hook: LifecycleHook::PostAttach,
                action,
            } = &step.event.detail
            {
                self.handle_hook(
                    LifecycleHook::PostAttach,
                    action,
                    config.post_attach_command.as_ref(),
                    &container,
                )
                .await?;
            }

            executed_phases.push(LifecyclePhase::PostAttach);
        }

        Ok(LifecycleOutcome {
            container,
            executed_phases,
        })
    }

    async fn handle_hook(
        &self,
        hook: LifecycleHook,
        action: &HookAction,
        command: Option<&CommandDefinition>,
        container: &RunningContainer,
    ) -> Result<()> {
        match action {
            HookAction::Execute => {
                if let Some(command) = command {
                    self.run_hook(container, hook, command).await
                } else {
                    tracing::warn!(
                        hook = %hook,
                        "Hook marked for execution without a resolved command"
                    );
                    Ok(())
                }
            }
            HookAction::Skip { reason } => {
                tracing::info!(hook = %hook, reason = %reason, "Skipping lifecycle hook");
                Ok(())
            }
        }
    }

    async fn run_hook(
        &self,
        container: &RunningContainer,
        hook: LifecycleHook,
        command: &CommandDefinition,
    ) -> Result<()> {
        match command {
            CommandDefinition::Single(cmd) => {
                self.run_hook_command(container, hook, None, cmd).await
            }
            CommandDefinition::Parallel(commands) => {
                for (name, cmd) in commands {
                    self.run_hook_command(container, hook, Some(name.as_str()), cmd)
                        .await?;
                }
                Ok(())
            }
        }
    }

    async fn run_hook_command(
        &self,
        container: &RunningContainer,
        hook: LifecycleHook,
        command_name: Option<&str>,
        command: &CommandArgs,
    ) -> Result<()> {
        let args = command.to_exec_args();
        if let Some(name) = command_name {
            tracing::debug!(
                hook = %hook,
                command_name = name,
                command = ?args,
                "Executing lifecycle hook command"
            );
        } else {
            tracing::debug!(hook = %hook, command = ?args, "Executing lifecycle hook command");
        }

        let result = self.provider.exec(container, &args).await?;
        if let Some(name) = command_name {
            tracing::debug!(
                hook = %hook,
                command_name = name,
                exit_code = result.exit_code,
                "Lifecycle hook completed"
            );
        } else {
            tracing::debug!(
                hook = %hook,
                exit_code = result.exit_code,
                "Lifecycle hook completed"
            );
        }

        let stdout = result.stdout.trim();
        if !stdout.is_empty() {
            if let Some(name) = command_name {
                tracing::info!(
                    hook = %hook,
                    command_name = name,
                    stdout = %stdout,
                    "Lifecycle hook stdout"
                );
            } else {
                tracing::info!(hook = %hook, stdout = %stdout, "Lifecycle hook stdout");
            }
        }

        let stderr = result.stderr.trim();
        if !stderr.is_empty() {
            if let Some(name) = command_name {
                tracing::warn!(
                    hook = %hook,
                    command_name = name,
                    stderr = %stderr,
                    "Lifecycle hook stderr"
                );
            } else {
                tracing::warn!(hook = %hook, stderr = %stderr, "Lifecycle hook stderr");
            }
        }

        if result.exit_code != 0 {
            let mut message = if let Some(name) = command_name {
                format!(
                    "{hook} command '{name}' failed with exit code {}",
                    result.exit_code
                )
            } else {
                format!("{hook} command failed with exit code {}", result.exit_code)
            };

            if !stderr.is_empty() {
                message.push_str(&format!(" ({stderr})"));
            }

            return Err(DevcontainerError::Provider(message));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::{
        ExecResult, Provider, ProviderCleanupOptions, ProviderImage, ProviderKind,
        ProviderPreparation, VolumeSpec,
    };
    use async_trait::async_trait;
    use std::sync::{Arc, Mutex};

    fn sample_config() -> ResolvedConfig {
        ResolvedConfig {
            project_name: "demo".to_string(),
            workspace_folder: PathBuf::from("/workspace"),
            container_workspace_folder: Some(PathBuf::from("/workspace")),
            config_path: PathBuf::from("/workspace/.devcontainer/devcontainer.json"),
            image_reference: Some("example:image".to_string()),
            dockerfile: None,
            features: Default::default(),
            forward_ports: vec![],
            post_create_command: Some(CommandDefinition::from_string("echo post create")),
            post_attach_command: Some(CommandDefinition::from_array(vec![
                "echo".to_string(),
                "post-attach".to_string(),
            ])),
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

    #[test]
    fn plan_for_up_skips_hooks_without_commands() {
        let mut config = sample_config();
        config.post_create_command = None;
        config.post_attach_command = None;

        let plan = LifecyclePlan::for_up(&config, LifecyclePlanOptions::default());

        assert!(matches!(
            plan.steps[4].event.detail,
            LifecycleEventDetail::Hook {
                hook: LifecycleHook::PostCreate,
                action: HookAction::Skip { ref reason }
            } if reason == super::NO_POST_CREATE_COMMAND_REASON
        ));

        assert!(matches!(
            plan.steps[5].event.detail,
            LifecycleEventDetail::Hook {
                hook: LifecycleHook::PostAttach,
                action: HookAction::Skip { ref reason }
            } if reason == super::NO_POST_ATTACH_COMMAND_REASON
        ));
    }

    #[derive(Clone)]
    struct TestProvider {
        state: Arc<Mutex<TestProviderState>>,
    }

    struct TestProviderState {
        exec_calls: Vec<Vec<String>>,
        exec_result: ExecResult,
    }

    impl TestProvider {
        fn new(exec_result: ExecResult) -> Self {
            Self {
                state: Arc::new(Mutex::new(TestProviderState {
                    exec_calls: Vec::new(),
                    exec_result,
                })),
            }
        }

        fn exec_calls(&self) -> Vec<Vec<String>> {
            let state = self.state.lock().expect("state lock");
            state.exec_calls.clone()
        }
    }

    #[async_trait]
    impl Provider for TestProvider {
        fn kind(&self) -> ProviderKind {
            ProviderKind::Mock
        }

        async fn prepare(&self, _config: &ResolvedConfig) -> Result<ProviderPreparation> {
            Ok(ProviderPreparation {
                image: ProviderImage::Reference("example:image".to_string()),
                container_name: "test-container".to_string(),
                project_slug: "demo".to_string(),
                networks: vec!["test-network".to_string()],
                volumes: vec![VolumeSpec {
                    name: "test-volume".to_string(),
                    mount_path: PathBuf::from("/data"),
                }],
                workspace_mount_path: PathBuf::from("/workspace"),
            })
        }

        async fn ensure_networks(
            &self,
            _config: &ResolvedConfig,
            _preparation: &ProviderPreparation,
        ) -> Result<()> {
            Ok(())
        }

        async fn ensure_volumes(
            &self,
            _config: &ResolvedConfig,
            _preparation: &ProviderPreparation,
        ) -> Result<()> {
            Ok(())
        }

        async fn build_image(
            &self,
            _config: &ResolvedConfig,
            _preparation: &ProviderPreparation,
        ) -> Result<String> {
            Ok("example:image:latest".to_string())
        }

        async fn create_container(
            &self,
            _config: &ResolvedConfig,
            _preparation: &ProviderPreparation,
            _image_reference: &str,
        ) -> Result<RunningContainer> {
            Ok(RunningContainer {
                id: Some("container-id".to_string()),
                name: Some("test-container".to_string()),
            })
        }

        async fn start_container(&self, _container: &RunningContainer) -> Result<()> {
            Ok(())
        }

        async fn exec(
            &self,
            _container: &RunningContainer,
            command: &[String],
        ) -> Result<ExecResult> {
            let result = {
                let mut state = self.state.lock().expect("state lock");
                state.exec_calls.push(command.to_vec());
                state.exec_result.clone()
            };
            Ok(result)
        }

        async fn stop_container(
            &self,
            _config: &ResolvedConfig,
            _preparation: &ProviderPreparation,
            _container: &RunningContainer,
        ) -> Result<()> {
            Ok(())
        }

        async fn cleanup(
            &self,
            _config: &ResolvedConfig,
            _preparation: &ProviderPreparation,
            _options: &ProviderCleanupOptions,
        ) -> Result<()> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn executor_runs_hooks_via_provider_exec() {
        let config = sample_config();
        let plan = LifecyclePlan::for_up(&config, LifecyclePlanOptions::default());
        let provider = TestProvider::new(ExecResult::default());
        let executor = LifecycleExecutor::new(provider.clone());

        let outcome = executor
            .execute(&config, &plan)
            .await
            .expect("lifecycle execution succeeds");

        assert_eq!(
            outcome.executed_phases,
            vec![
                LifecyclePhase::Resolve,
                LifecyclePhase::Build,
                LifecyclePhase::Create,
                LifecyclePhase::Start,
                LifecyclePhase::PostCreate,
                LifecyclePhase::PostAttach,
            ]
        );

        let calls = provider.exec_calls();
        assert_eq!(calls.len(), 2);
        assert_eq!(
            calls[0],
            vec![
                "/bin/sh".to_string(),
                "-c".to_string(),
                "echo post create".to_string(),
            ]
        );
        assert_eq!(
            calls[1],
            vec!["echo".to_string(), "post-attach".to_string()]
        );
    }

    #[tokio::test]
    async fn executor_fails_when_hook_returns_non_zero() {
        let config = sample_config();
        let plan = LifecyclePlan::for_up(&config, LifecyclePlanOptions::default());
        let provider = TestProvider::new(ExecResult {
            exit_code: 5,
            stdout: String::new(),
            stderr: "boom".to_string(),
        });
        let executor = LifecycleExecutor::new(provider.clone());

        let err = executor
            .execute(&config, &plan)
            .await
            .expect_err("postCreate failure propagates");

        match err {
            DevcontainerError::Provider(message) => {
                assert!(message.contains("postCreate"));
                assert!(message.contains("5"));
                assert!(message.contains("boom"));
            }
            other => panic!("Unexpected error: {other:?}"),
        }

        let calls = provider.exec_calls();
        assert_eq!(calls.len(), 1);
        assert_eq!(
            calls[0],
            vec![
                "/bin/sh".to_string(),
                "-c".to_string(),
                "echo post create".to_string(),
            ]
        );
    }

    #[tokio::test]
    async fn executor_skips_hooks_when_commands_absent() {
        let mut config = sample_config();
        config.post_create_command = None;
        config.post_attach_command = None;

        let plan = LifecyclePlan::for_up(&config, LifecyclePlanOptions::default());
        let provider = TestProvider::new(ExecResult::default());
        let executor = LifecycleExecutor::new(provider.clone());

        let outcome = executor
            .execute(&config, &plan)
            .await
            .expect("lifecycle execution succeeds without hooks");

        assert!(provider.exec_calls().is_empty());
        assert_eq!(
            outcome.executed_phases,
            vec![
                LifecyclePhase::Resolve,
                LifecyclePhase::Build,
                LifecyclePhase::Create,
                LifecyclePhase::Start,
                LifecyclePhase::PostCreate,
                LifecyclePhase::PostAttach,
            ]
        );
    }
}

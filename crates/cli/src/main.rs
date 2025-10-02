use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};
use devcontainer_core::{
    config::{ConfigOverrides, ConfigResolver, ConfigSource},
    lifecycle::{LifecycleExecutor, LifecyclePlan, LifecyclePlanOptions},
    provider::{Provider, ProviderCleanupOptions, RunningContainer},
    telemetry::{self, LogFormat},
    DevcontainerError, Result,
};
use devcontainer_provider_docker::DockerProvider;

#[derive(Parser, Debug)]
#[command(
    name = "devcontainer",
    author,
    version,
    about = "Rust implementation of the Devcontainer CLI",
    long_about = None
)]
struct Cli {
    #[arg(short = 'v', long = "verbose", global = true, action = clap::ArgAction::Count)]
    verbose: u8,
    #[arg(long = "log-format", global = true, value_enum, default_value_t = OutputFormat::Auto)]
    log_format: OutputFormat,
    #[arg(long = "project-root", global = true)]
    project_root: Option<PathBuf>,
    #[arg(long = "workspace-folder", global = true)]
    workspace_folder: Option<PathBuf>,
    #[arg(long = "config", global = true)]
    config: Option<PathBuf>,
    #[arg(long = "docker-path", global = true)]
    docker_path: Option<PathBuf>,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Clone, ValueEnum)]
enum OutputFormat {
    Auto,
    Text,
    Json,
}

impl OutputFormat {
    fn to_core(&self) -> LogFormat {
        match self {
            OutputFormat::Auto => LogFormat::Auto,
            OutputFormat::Text => LogFormat::Text,
            OutputFormat::Json => LogFormat::Json,
        }
    }
}

#[derive(Subcommand, Debug)]
enum Commands {
    Up(UpArgs),
    Down(DownArgs),
    Build(BuildArgs),
    Exec(ExecArgs),
    #[command(name = "run-user-commands")]
    RunUser(RunUserCommandsArgs),
    ReadConfiguration(ReadConfigurationArgs),
    Features(FeaturesArgs),
    Templates(TemplatesArgs),
    Inspect(InspectArgs),
    Version,
}

#[derive(Debug, Args)]
struct UpArgs {
    #[arg(long)]
    attach: bool,
    #[arg(long = "skip-post-create")]
    skip_post_create: bool,
    #[arg(long = "skip-post-attach")]
    skip_post_attach: bool,
}

impl UpArgs {
    async fn run(&self, ctx: &CommandContext) -> Result<()> {
        let source = ctx.config_source();
        let resolver = ConfigResolver::new(source).with_overrides(ctx.config_overrides());
        let resolved = resolver.resolve()?;

        let plan = LifecyclePlan::for_up(
            &resolved,
            LifecyclePlanOptions {
                skip_post_create: self
                    .skip_post_create
                    .then(|| "--skip-post-create flag set".to_string()),
                skip_post_attach: self
                    .skip_post_attach
                    .then(|| "--skip-post-attach flag set".to_string()),
            },
        );

        let provider = ctx.provider();
        let executor = LifecycleExecutor::new(provider);
        let outcome = executor.execute(&resolved, &plan).await?;

        tracing::info!(?outcome.container, "Devcontainer is ready");

        if self.attach {
            tracing::info!("Attach requested, placeholder handler invoked");
        }

        if self.skip_post_create || self.skip_post_attach {
            tracing::debug!(
                skip_post_create = self.skip_post_create,
                skip_post_attach = self.skip_post_attach,
                "Lifecycle hooks skipped",
            );
        }

        Ok(())
    }
}

#[derive(Debug, Args)]
struct DownArgs {
    #[arg(long = "remove-volumes")]
    remove_volumes: bool,
    #[arg(long = "remove-unknown")]
    remove_unknown: bool,
}

impl DownArgs {
    async fn run(&self, ctx: &CommandContext) -> Result<()> {
        let source = ctx.config_source();
        let resolver = ConfigResolver::new(source).with_overrides(ctx.config_overrides());
        let resolved = resolver.resolve()?;

        let provider = ctx.provider();
        let preparation = provider.prepare(&resolved).await?;
        let container = RunningContainer {
            id: None,
            name: Some(preparation.container_name.clone()),
        };

        provider
            .stop_container(&resolved, &preparation, &container)
            .await?;

        let options = ProviderCleanupOptions {
            remove_volumes: self.remove_volumes,
            remove_unknown: self.remove_unknown,
        };

        provider.cleanup(&resolved, &preparation, &options).await?;

        tracing::info!(
            remove_volumes = self.remove_volumes,
            remove_unknown = self.remove_unknown,
            "Devcontainer resources cleaned up",
        );

        Ok(())
    }
}

#[derive(Debug, Args)]
struct BuildArgs {
    #[arg(long = "no-cache")]
    no_cache: bool,
    #[arg(long = "push")]
    push: bool,
}

impl BuildArgs {
    async fn run(&self, ctx: &CommandContext) -> Result<()> {
        let source = ctx.config_source();
        let resolver = ConfigResolver::new(source).with_overrides(ctx.config_overrides());
        let resolved = resolver.resolve()?;

        if self.no_cache {
            tracing::warn!("--no-cache flag is not yet implemented; proceeding with cached build");
        }
        if self.push {
            tracing::warn!("--push flag is not yet implemented; build output will remain local");
        }

        let provider = ctx.provider();
        let preparation = provider.prepare(&resolved).await?;
        let image_reference = provider.build_image(&resolved, &preparation).await?;

        tracing::info!(image = %image_reference, "Devcontainer image ready");
        println!("{image_reference}");
        Ok(())
    }
}

#[derive(Debug, Args)]
struct ExecArgs {
    #[arg(long = "id-label")]
    id_label: Option<String>,
    #[arg(last = true)]
    command: Vec<String>,
}

impl ExecArgs {
    async fn run(&self, ctx: &CommandContext) -> Result<()> {
        if self.command.is_empty() {
            return Err(DevcontainerError::Configuration(
                "devcontainer exec requires a command to run".into(),
            ));
        }

        if self.id_label.is_some() {
            tracing::warn!("--id-label is not yet implemented; using workspace resolution");
        }

        let source = ctx.config_source();
        let resolver = ConfigResolver::new(source).with_overrides(ctx.config_overrides());
        let resolved = resolver.resolve()?;

        let plan = LifecyclePlan::for_up(
            &resolved,
            LifecyclePlanOptions {
                skip_post_create: Some("exec command requested".to_string()),
                skip_post_attach: Some("exec command requested".to_string()),
            },
        );

        let provider = ctx.provider();
        let executor = LifecycleExecutor::new(provider);
        let outcome = executor.execute(&resolved, &plan).await?;

        let result = executor
            .provider()
            .exec(&outcome.container, &self.command)
            .await?;

        if !result.stdout.is_empty() {
            print!("{}", result.stdout);
        }
        if !result.stderr.is_empty() {
            eprint!("{}", result.stderr);
        }

        if result.exit_code != 0 {
            return Err(DevcontainerError::Provider(format!(
                "Command exited with status {}",
                result.exit_code
            )));
        }

        Ok(())
    }
}

#[derive(Debug, Args)]
struct RunUserCommandsArgs {
    #[arg(
        long = "trigger",
        value_parser = clap::builder::PossibleValuesParser::new(["init", "post-create", "post-attach"])
    )]
    trigger: String,
}

impl RunUserCommandsArgs {
    async fn run(&self, _ctx: &CommandContext) -> Result<()> {
        tracing::info!(trigger = %self.trigger, "Running user command (stub)");
        Ok(())
    }
}

#[derive(Debug, Args)]
struct ReadConfigurationArgs;

impl ReadConfigurationArgs {
    async fn run(&self, ctx: &CommandContext) -> Result<()> {
        let source = ctx.config_source();
        let resolver = ConfigResolver::new(source).with_overrides(ctx.config_overrides());
        let resolved = resolver.resolve()?;
        let output = serde_json::to_string_pretty(&resolved)
            .map_err(|err| DevcontainerError::Other(err.into()))?;
        println!("{output}");
        Ok(())
    }
}

#[derive(Debug, Args)]
struct FeaturesArgs {
    #[command(subcommand)]
    command: FeaturesSubcommand,
}

#[derive(Debug, Subcommand)]
enum FeaturesSubcommand {
    Test,
    Publish,
    Package,
}

impl FeaturesArgs {
    async fn run(&self, _ctx: &CommandContext) -> Result<()> {
        tracing::info!(subcommand = ?self.command, "Features command invoked (stub)");
        Ok(())
    }
}

#[derive(Debug, Args)]
struct TemplatesArgs {
    #[command(subcommand)]
    command: TemplatesSubcommand,
}

#[derive(Debug, Subcommand)]
enum TemplatesSubcommand {
    Apply,
    Publish,
    List,
}

impl TemplatesArgs {
    async fn run(&self, _ctx: &CommandContext) -> Result<()> {
        tracing::info!(subcommand = ?self.command, "Templates command invoked (stub)");
        Ok(())
    }
}

#[derive(Debug, Args)]
struct InspectArgs;

impl InspectArgs {
    async fn run(&self, _ctx: &CommandContext) -> Result<()> {
        tracing::info!("Inspect command invoked (stub)");
        Ok(())
    }
}

struct CommandContext {
    project_root: PathBuf,
    workspace_folder: Option<PathBuf>,
    config_path: Option<PathBuf>,
    docker_path: Option<PathBuf>,
}

impl CommandContext {
    fn new(cli: &Cli) -> Result<Self> {
        let project_root = if let Some(root) = &cli.project_root {
            root.clone()
        } else {
            std::env::current_dir().map_err(|err| DevcontainerError::Other(err.into()))?
        };

        Ok(Self {
            project_root,
            workspace_folder: cli.workspace_folder.clone(),
            config_path: cli.config.clone(),
            docker_path: cli.docker_path.clone(),
        })
    }

    fn config_source(&self) -> ConfigSource {
        if let Some(config) = &self.config_path {
            ConfigSource::ExplicitFile(config.clone())
        } else {
            let workspace = self
                .workspace_folder
                .clone()
                .unwrap_or_else(|| self.project_root.clone());
            ConfigSource::Workspace(workspace)
        }
    }

    fn config_overrides(&self) -> ConfigOverrides {
        let mut overrides = ConfigOverrides::default();
        if let Some(workspace) = &self.workspace_folder {
            overrides = overrides.with_workspace_folder(workspace.clone());
        }
        overrides
    }

    fn provider(&self) -> DockerProvider {
        match &self.docker_path {
            Some(path) => DockerProvider::from_path(path.clone()),
            None => DockerProvider::new(),
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let log_format = cli.log_format.to_core();
    let verbosity = match cli.verbose {
        0 => "info",
        1 => "debug",
        _ => "trace",
    };

    telemetry::init(verbosity, log_format)?;

    let ctx = CommandContext::new(&cli)?;

    match cli.command {
        Commands::Up(args) => args.run(&ctx).await?,
        Commands::Down(args) => args.run(&ctx).await?,
        Commands::Build(args) => args.run(&ctx).await?,
        Commands::Exec(args) => args.run(&ctx).await?,
        Commands::RunUser(args) => args.run(&ctx).await?,
        Commands::ReadConfiguration(args) => args.run(&ctx).await?,
        Commands::Features(args) => args.run(&ctx).await?,
        Commands::Templates(args) => args.run(&ctx).await?,
        Commands::Inspect(args) => args.run(&ctx).await?,
        Commands::Version => {
            println!("{}", env!("CARGO_PKG_VERSION"));
        }
    }

    Ok(())
}

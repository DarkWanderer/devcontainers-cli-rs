use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};
use devcontainer_core::{
    config::{ConfigOverrides, ConfigResolver, ConfigSource},
    lifecycle::{LifecycleExecutor, LifecyclePhase, LifecyclePlan},
    telemetry::{self, LogFormat},
    DevcontainerError, Result,
};

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
    fn to_core(self) -> LogFormat {
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
    RunUserCommands(RunUserCommandsArgs),
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

        let mut plan = LifecyclePlan::new();
        plan.push(
            LifecyclePhase::Resolve,
            "Resolve devcontainer configuration",
        );
        plan.push(LifecyclePhase::Start, "Start devcontainer runtime");

        let executor = LifecycleExecutor::new(NullProvider);
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
    async fn run(&self, _ctx: &CommandContext) -> Result<()> {
        tracing::info!(
            remove_volumes = self.remove_volumes,
            remove_unknown = self.remove_unknown,
            "Stopping devcontainer (stub)",
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

        tracing::info!(
            ?resolved,
            no_cache = self.no_cache,
            push = self.push,
            "Build command invoked (stub)",
        );
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
    async fn run(&self, _ctx: &CommandContext) -> Result<()> {
        tracing::info!(?self.command, "Executing command in container (stub)");
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
        println!("{}", output);
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
}

struct NullProvider;

impl devcontainer_core::provider::Provider for NullProvider {
    fn kind(&self) -> devcontainer_core::provider::ProviderKind {
        devcontainer_core::provider::ProviderKind::Mock
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let log_format = cli.log_format.clone().to_core();
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
        Commands::RunUserCommands(args) => args.run(&ctx).await?,
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

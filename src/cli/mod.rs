use clap::Parser;
use clap_verbosity_flag::Verbosity;
use miette::IntoDiagnostic;
use tracing_subscriber::{
    filter::LevelFilter, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter,
};

mod completions;
mod default;
mod install;
mod pin;
mod run;
mod show;
mod update;
mod which;

#[derive(Debug, Parser)]
#[command(
    version,
    about = "
Moonup is a tool to manage multiple MoonBit installations.

If you find any bugs or have a feature request, please open an issue on
GitHub: https://github.com/chawyehsu/moonup/issues"
)]
#[clap(arg_required_else_help = true)]
pub struct Cli {
    #[command(subcommand)]
    command: Command,

    /// The verbosity level
    #[command(flatten)]
    verbose: Verbosity,
}

#[derive(Debug, Parser)]
pub enum Command {
    Completions(completions::Args),

    Default(default::Args),

    #[clap(visible_alias = "i")]
    Install(install::Args),

    Pin(pin::Args),

    Run(run::Args),

    #[clap(alias = "list", alias = "ls")]
    Show(show::Args),

    #[clap(visible_alias = "u")]
    Update(update::Args),

    Which(which::Args),
}

/// CLI entry point
pub async fn start() -> miette::Result<()> {
    let args = Cli::parse();
    setup_logger(args.verbose.tracing_level_filter())?;

    match args.command {
        Command::Completions(args) => completions::execute(args).await?,
        Command::Default(args) => default::execute(args).await?,
        Command::Install(args) => install::execute(args).await?,
        Command::Pin(args) => pin::execute(args).await?,
        Command::Run(args) => run::execute(args).await?,
        Command::Show(args) => show::execute(args).await?,
        Command::Update(args) => update::execute(args).await?,
        Command::Which(args) => which::execute(args).await?,
    }
    Ok(())
}

fn setup_logger(level_filter: LevelFilter) -> miette::Result<()> {
    // filter for low-level/depedency logs
    let low_level_filter = match level_filter {
        LevelFilter::OFF => LevelFilter::OFF,
        LevelFilter::ERROR => LevelFilter::ERROR,
        LevelFilter::WARN => LevelFilter::WARN,
        LevelFilter::INFO => LevelFilter::WARN,
        LevelFilter::DEBUG => LevelFilter::INFO,
        LevelFilter::TRACE => LevelFilter::TRACE,
    };

    let mut layer_env_filter = EnvFilter::builder()
        .with_default_directive(level_filter.into())
        .from_env()
        .into_diagnostic()?;

    layer_env_filter = layer_env_filter
        // add low-level filter for hyper_util/reqwest
        .add_directive(
            format!("hyper_util={}", low_level_filter)
                .parse()
                .into_diagnostic()?,
        )
        .add_directive(
            format!("reqwest={}", low_level_filter)
                .parse()
                .into_diagnostic()?,
        );

    let layer_fmt = tracing_subscriber::fmt::layer().without_time();

    tracing_subscriber::registry()
        .with(layer_env_filter)
        .with(layer_fmt)
        .init();

    Ok(())
}

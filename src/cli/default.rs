use clap::Parser;
use miette::IntoDiagnostic;

/// Set the default toolchain
#[derive(Parser, Debug)]
#[clap(arg_required_else_help = true)]
pub struct Args {
    /// Toolchain name, can be 'latest' or a specific version number
    toolchain: String,
}

pub async fn execute(args: Args) -> miette::Result<()> {
    let version = args.toolchain.as_str();

    // TODO: validate only installed toolchains can be set as default
    let deafult_file = crate::moonup_home().join("default");
    tokio::fs::write(&deafult_file, format!("{}\n", version))
        .await
        .into_diagnostic()?;

    println!(
        "{}Default toolchain set to version '{}'",
        console::style(console::Emoji("✔ ", "")).green(),
        version
    );

    Ok(())
}

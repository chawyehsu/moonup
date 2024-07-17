use std::env;

use clap::Parser;
use miette::IntoDiagnostic;

use crate::utils::detect_toolchain_file;

/// Pin the MoonBit toolchain to a specific version
#[derive(Parser, Debug)]
#[clap(arg_required_else_help = true)]
pub struct Args {
    /// Toolchain name, can be 'latest' or a specific version number
    toolchain: String,
}

pub async fn execute(args: Args) -> miette::Result<()> {
    let version = args.toolchain.as_str();

    let toolchain_file = detect_toolchain_file().unwrap_or_else(|| {
        let current_dir = env::current_dir().expect("can't access current directory");
        current_dir.join(crate::constant::TOOLCHAIN_FILE)
    });

    tokio::fs::write(&toolchain_file, format!("{}\n", version))
        .await
        .into_diagnostic()?;

    println!(
        "{}Pinned toolchain to version '{}'",
        console::style(console::Emoji("✔ ", "")).green(),
        version
    );
    println!("Toolchain file: {}", toolchain_file.display());

    Ok(())
}

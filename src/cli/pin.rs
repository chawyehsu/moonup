use clap::{CommandFactory, Parser};
use dialoguer::theme::ColorfulTheme;
use miette::IntoDiagnostic;
use std::env;

use crate::{
    constant,
    toolchain::{resolve::resolve_toolchain_file, ToolchainSpec},
};

use super::ToolchainSpecValueParser;

/// Pin the MoonBit toolchain to a specific version
#[derive(Parser, Debug)]
pub struct Args {
    /// Toolchain version tag or channel name [latest, nightly, bleeding]
    #[clap(value_parser = ToolchainSpecValueParser::new())]
    toolchain: Option<ToolchainSpec>,
}

pub async fn execute(args: Args) -> miette::Result<()> {
    let version = args.toolchain.or_else(|| {
        if let Ok(toolchains) = crate::toolchain::installed_toolchains() {
            let selections = toolchains
                .iter()
                .map(|t| t.name.to_owned())
                .rev()
                .collect::<Vec<_>>();

            if !selections.is_empty() {
                let selection = dialoguer::Select::with_theme(&ColorfulTheme::default())
                    .with_prompt("Pick a installed version")
                    .items(&selections)
                    .max_length(constant::MAX_SELECT_ITEMS)
                    .default(0)
                    .interact()
                    .into_diagnostic()
                    .expect("can't select a toolchain version");

                return Some(selections[selection].clone());
            }
        }

        None
    });

    let version = match version {
        Some(v) => v,
        None => {
            let mut cmd = Args::command();
            cmd.print_help().into_diagnostic()?;
            std::process::exit(2);
        }
    };

    let toolchain_file = resolve_toolchain_file().unwrap_or_else(|| {
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

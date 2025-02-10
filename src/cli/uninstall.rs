use std::path::PathBuf;

use clap::{CommandFactory, Parser};
use dialoguer::{theme, MultiSelect};
use miette::IntoDiagnostic;
use tracing::instrument;

use crate::{
    constant,
    toolchain::{installed_toolchains, ToolchainSpec},
};

/// Uninstall a MoonBit toolchain.
#[derive(Debug, Parser)]
pub struct Args {
    /// The toolchain(s) to uninstall.
    #[clap(value_parser = super::ToolchainSpecValueParser::new())]
    toolchain: Vec<ToolchainSpec>,
}

#[instrument(name = "uninstall", skip(args), fields(toolchain = ?args.toolchain))]
pub async fn execute(args: Args) -> miette::Result<()> {
    let toolchains = if args.toolchain.is_empty() {
        let installed = installed_toolchains()?;
        if installed.is_empty() {
            let mut cmd = Args::command();
            cmd.print_help().into_diagnostic()?;
            std::process::exit(2);
        }

        let selections = installed
            .iter()
            .map(|t| t.name.to_owned())
            .collect::<Vec<_>>();

        let selected = MultiSelect::with_theme(&theme::ColorfulTheme::default())
            .with_prompt("Select toolchains to uninstall")
            .items(&selections)
            .max_length(constant::MAX_SELECT_ITEMS)
            .interact()
            .into_diagnostic()?;

        selected
            .iter()
            .map(|&i| ToolchainSpec::from(selections[i].clone()))
            .collect()
    } else {
        args.toolchain
    };

    if toolchains.is_empty() {
        eprintln!("No toolchain provided to uninstall");
        std::process::exit(1);
    }

    for toolchain in toolchains {
        let toolchain_dir = toolchain.install_path();
        if !toolchain_dir.exists() {
            tracing::warn!("toolchain {} is not installed", toolchain);
            continue;
        }

        tracing::info!("uninstalling toolchain {}", toolchain);
        remove_toolchain(&toolchain_dir).await?;

        println!(
            "{} Uninstalled toolchain {}",
            console::style(console::Emoji("✔ ", "")).green(),
            console::style(&toolchain).yellow().bright()
        );
    }

    Ok(())
}

async fn remove_toolchain(toolchain_dir: &PathBuf) -> miette::Result<()> {
    tracing::debug!("removing toolchain from {}", toolchain_dir.display());
    tokio::fs::remove_dir_all(toolchain_dir)
        .await
        .into_diagnostic()?;
    Ok(())
}

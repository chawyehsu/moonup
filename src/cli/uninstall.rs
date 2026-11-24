use clap::{CommandFactory, Parser};
use dialoguer::{MultiSelect, theme};
use miette::{Context, IntoDiagnostic};
use tracing::instrument;

use crate::{
    constant,
    toolchain::{InstalledToolchain, ToolchainSpec, atomic, installed_toolchains},
};

/// Uninstall a MoonBit toolchain.
#[derive(Debug, Parser)]
pub struct Args {
    /// The toolchain(s) to uninstall.
    #[clap(value_parser = super::ToolchainSpecValueParser::new())]
    toolchain: Vec<ToolchainSpec>,

    /// Invalidate and remove all cached downloads.
    #[clap(long)]
    clear: bool,

    /// Keep the cached downloads of the toolchain.
    ///
    /// Cached downloads of the toolchain will be removed as well by default.
    /// Use this flag to keep the cached downloads.
    #[clap(long)]
    keep_cache: bool,
}

#[instrument(name = "uninstall", skip(args), fields(toolchain = ?args.toolchain))]
pub async fn execute(args: Args) -> miette::Result<()> {
    if args.clear {
        let download_dir = crate::moonup_home().join("downloads");
        tracing::info!("removing all cached downloads {}", download_dir.display());
        crate::fs::empty_dir(&download_dir)
            .into_diagnostic()
            .wrap_err("failed to clear cached downloads")?;
        println!(
            "{} Cleared all cached downloads",
            console::style(console::Emoji("✔ ", "")).green()
        );
        return Ok(());
    }

    let toolchains = if args.toolchain.is_empty() {
        let installed = installed_toolchains()?;
        if installed.is_empty() {
            let mut cmd = Args::command();
            cmd.print_help().into_diagnostic()?;
            return Err(crate::status::exit_with(2));
        }

        let selections = installed
            .iter()
            .map(|t| t.spec.to_owned())
            .collect::<Vec<_>>();

        let selected = MultiSelect::with_theme(&theme::ColorfulTheme::default())
            .with_prompt("Select toolchains to uninstall")
            .items(&selections)
            .max_length(constant::MAX_SELECT_ITEMS)
            .interact()
            .into_diagnostic()?;

        selected.iter().map(|&i| selections[i].clone()).collect()
    } else {
        args.toolchain
    };

    if toolchains.is_empty() {
        eprintln!("No toolchain provided to uninstall");
        return Err(crate::status::exit_with(1));
    }

    // Serialize toolchain mutations across processes, after the interactive
    // selection so the lock is not held during the dialog.
    let _lock = crate::lock::ToolchainLock::acquire()?;

    for toolchain in toolchains {
        let toolchain_dir = toolchain.install_path();
        if !toolchain_dir.exists() {
            atomic::sweep_staging(&toolchain);
            tracing::warn!("toolchain {} is not installed", toolchain);
            continue;
        }

        tracing::info!("uninstalling toolchain {}", toolchain);

        if !args.keep_cache {
            if let Ok(installed) = InstalledToolchain::from_path(&toolchain_dir) {
                let mut download_dir = crate::moonup_home().join("downloads");
                download_dir.push(toolchain.channel().as_str());
                download_dir.push(installed.key());

                tracing::info!("removing cached downloads {}", download_dir.display());
                let _ = crate::fs::remove_dir_all(download_dir).inspect_err(|e| {
                    tracing::warn!("failed to remove cached downloads: {}", e);
                });
            } else {
                tracing::warn!(
                    "failed to read installed toolchain from path: {}",
                    toolchain_dir.display()
                );
            }
        }

        tracing::debug!("removing toolchain from {}", toolchain_dir.display());
        crate::fs::remove_dir_all(toolchain_dir).into_diagnostic()?;
        atomic::sweep_staging(&toolchain);

        println!(
            "{} Uninstalled toolchain {}",
            console::style(console::Emoji("✔ ", "")).green(),
            console::style(&toolchain).yellow().bright()
        );
    }

    Ok(())
}

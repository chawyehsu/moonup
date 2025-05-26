use clap::{CommandFactory, Parser};
use dialoguer::{theme, MultiSelect};
use miette::{Context, IntoDiagnostic};
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
        let _ = crate::fs::empty_dir(&download_dir)
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

        if !args.keep_cache {
            let mut download_dir = crate::moonup_home().join("downloads");

            match &toolchain {
                ToolchainSpec::Bleeding => download_dir.push("bleeding"),
                ToolchainSpec::Version(v) => {
                    if v.starts_with("nightly") {
                        download_dir.push("nightly");
                        let date = v
                            .split_once('-')
                            .expect("should split nightly version str")
                            .1;
                        download_dir.push(date);
                    } else {
                        download_dir.push("latest");
                        download_dir.push(v);
                    }
                }
                _ => {
                    if toolchain.is_nightly() {
                        download_dir.push("nightly");
                    } else {
                        download_dir.push("latest");
                    }

                    let version_file = toolchain_dir.join("version");
                    let version = tokio::fs::read_to_string(version_file)
                        .await
                        .into_diagnostic()?;
                    download_dir.push(version.trim());
                }
            }

            tracing::info!("removing cached downloads {}", download_dir.display());
            let _ = crate::fs::remove_dir_all(download_dir).inspect_err(|e| {
                tracing::warn!("failed to remove cached downloads: {}", e);
            });
        }

        tracing::debug!("removing toolchain from {}", toolchain_dir.display());
        crate::fs::remove_dir_all(toolchain_dir).into_diagnostic()?;

        println!(
            "{} Uninstalled toolchain {}",
            console::style(console::Emoji("✔ ", "")).green(),
            console::style(&toolchain).yellow().bright()
        );
    }

    Ok(())
}

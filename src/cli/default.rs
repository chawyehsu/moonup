use clap::Parser;
use dialoguer::theme::ColorfulTheme;
use miette::IntoDiagnostic;

use crate::{constant, status, toolchain::ToolchainSpec};

use super::ToolchainSpecValueParser;

/// Set the default toolchain
#[derive(Parser, Debug)]
pub struct Args {
    /// Toolchain version tag or channel name [latest, nightly, bleeding]
    #[clap(value_parser = ToolchainSpecValueParser::new())]
    toolchain: Option<ToolchainSpec>,
}

pub async fn execute(args: Args) -> miette::Result<()> {
    let version = match args.toolchain {
        Some(version) => version,
        None => {
            let selections = match crate::toolchain::installed_toolchains() {
                Ok(toolchains) => toolchains
                    .iter()
                    .map(|t| t.name.to_owned())
                    .rev()
                    .collect::<Vec<_>>(),
                Err(_) => Vec::new(),
            };

            if selections.is_empty() {
                eprintln!("Please provide a toolchain version to set as default");
                return Err(status::exit_with(1));
            }

            let selection = dialoguer::Select::with_theme(&ColorfulTheme::default())
                .with_prompt("Pick a installed version")
                .items(&selections)
                .max_length(constant::MAX_SELECT_ITEMS)
                .default(0)
                .interact()
                .into_diagnostic()
                .expect("can't select a toolchain version");

            selections[selection].clone()
        }
    };

    let default_file = crate::moonup_home().join("default");
    std::fs::create_dir_all(default_file.parent().unwrap()).into_diagnostic()?;
    tokio::fs::write(&default_file, format!("{}\n", version))
        .await
        .into_diagnostic()?;

    println!(
        "{}Default toolchain set to version '{}'",
        console::style(console::Emoji("✔ ", "")).green(),
        version
    );

    Ok(())
}

use clap::Parser;
use dialoguer::theme::ColorfulTheme;
use miette::IntoDiagnostic;

use crate::constant;

/// Set the default toolchain
#[derive(Parser, Debug)]
pub struct Args {
    /// Toolchain name, can be 'latest' or a specific version number
    toolchain: Option<String>,
}

pub async fn execute(args: Args) -> miette::Result<()> {
    let version = args.toolchain.unwrap_or_else(|| {
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

                return selections[selection].to_string();
            }
        }

        eprintln!("Please provide a toolchain version to set as default");
        std::process::exit(1);
    });

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

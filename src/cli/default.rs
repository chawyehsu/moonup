use clap::Parser;
use dialoguer::theme::ColorfulTheme;
use miette::IntoDiagnostic;

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
                .map(|t| {
                    if t.latest {
                        "latest".to_string()
                    } else {
                        t.version.clone()
                    }
                })
                .rev()
                .collect::<Vec<_>>();

            if !selections.is_empty() {
                let selection = dialoguer::Select::with_theme(&ColorfulTheme::default())
                    .with_prompt("Pick a installed version")
                    .items(&selections)
                    .default(0)
                    .interact()
                    .into_diagnostic()
                    .expect("can't select a toolchain version");

                return selections[selection].clone();
            }
        }

        eprintln!("Please provide a toolchain version to set as default");
        std::process::exit(1);
    });

    let default_file = crate::moonup_home().join("default");
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

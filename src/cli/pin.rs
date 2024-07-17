use std::env;

use clap::Parser;
use dialoguer::theme::ColorfulTheme;
use miette::IntoDiagnostic;

use crate::utils::detect_toolchain_file;

/// Pin the MoonBit toolchain to a specific version
#[derive(Parser, Debug)]
// #[clap(arg_required_else_help = true)]
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

        eprintln!("Please provide a toolchain version to pin");
        std::process::exit(1);
    });

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

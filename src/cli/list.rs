use clap::Parser;

use crate::toolchain::{installed_toolchains, resolve, InstalledToolchain, ToolchainSpec};

/// List installed and active toolchains
#[derive(Parser, Debug)]
pub struct Args {}

pub async fn execute(_: Args) -> miette::Result<()> {
    let installs = installed_toolchains()?;

    println!("Moonup home: {}\n", crate::moonup_home().display());
    if installs.is_empty() {
        println!("No toolchains installed");
    } else {
        let default = resolve::detect_default_version();

        println!("Installed toolchains:");
        for i in installs {
            let is_default = default
                .as_deref()
                .map(|d| i.name == ToolchainSpec::from(d))
                .unwrap_or(false);

            let tags = match (is_default, i.tag.as_deref()) {
                (true, Some(tag)) => {
                    Some(format!(" ({}, {})", console::style("default").cyan(), tag))
                }
                (true, None) => Some(format!(" ({})", console::style("default").cyan())),
                (false, Some(tag)) => Some(format!(" ({})", tag)),
                (false, None) => None,
            };

            println!("  {}{}", i.name, tags.unwrap_or_default());
        }

        let active = resolve::detect_active_toolchain();
        let t = InstalledToolchain::from_path(&active)?;
        println!("\nActive toolchain: {}", console::style(t.name).green());
    }

    Ok(())
}

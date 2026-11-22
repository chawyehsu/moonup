use clap::Parser;

use crate::toolchain::{ToolchainSpec, installed_toolchains, resolve};

/// List installed and active toolchains
#[derive(Parser, Debug)]
pub struct Args {}

pub async fn execute(_: Args) -> miette::Result<()> {
    let installs = installed_toolchains()?;

    println!("Moonup home: {}\n", crate::moonup_home().display());
    if installs.is_empty() {
        println!("No toolchains installed");
    } else {
        let default = resolve::detect_default_toolchain()
            .map(|value| ToolchainSpec::try_from(value.as_str()))
            .transpose()
            .map_err(|error| miette::miette!(error))?;

        println!("Installed toolchains:");
        for i in installs {
            let is_default = default.as_ref().is_some_and(|d| &i.spec == d);

            let tags = match (is_default, i.release_key.as_deref()) {
                (true, Some(tag)) => {
                    Some(format!(" ({}, {})", console::style("default").cyan(), tag))
                }
                (true, None) => Some(format!(" ({})", console::style("default").cyan())),
                (false, Some(tag)) => Some(format!(" ({})", tag)),
                (false, None) => None,
            };

            println!("  {}{}", i.spec, tags.unwrap_or_default());
        }

        let active = resolve::detect_active_toolchain().map_err(|error| miette::miette!(error))?;
        println!("\nActive toolchain: {}", console::style(active).green());
    }

    Ok(())
}

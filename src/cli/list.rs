use clap::Parser;

use crate::session::Session;
use crate::toolchain::{ToolchainSpec, installed_toolchains, resolve};

/// List installed and active toolchains
#[derive(Parser, Debug)]
pub struct Args {}

pub async fn execute(_: Args, session: &Session) -> miette::Result<()> {
    let installs = installed_toolchains(session).await?;

    println!(
        "Moonup home: {}\n",
        session.layout().moonup_home().display()
    );
    if installs.is_empty() {
        println!("No toolchains installed");
    } else {
        let default = resolve::detect_default_toolchain(session.layout())
            .map(|value| ToolchainSpec::try_from(value.as_str()))
            .transpose()
            .map_err(|error| miette::miette!(error))?;

        println!("Installed toolchains:");
        for i in installs {
            let is_default = default.as_ref().is_some_and(|d| &i.spec == d);

            let tags = match (is_default, i.is_floating()) {
                (true, true) => Some(format!(
                    " ({}, {})",
                    console::style("default").cyan(),
                    i.key()
                )),
                (true, false) => Some(format!(" ({})", console::style("default").cyan())),
                (false, true) => Some(format!(" ({})", i.key())),
                (false, false) => None,
            };

            println!("  {}{}", i.spec, tags.unwrap_or_default());
        }

        let active = resolve::detect_active_toolchain(session.layout())
            .map_err(|error| miette::miette!(error))?;
        println!("\nActive toolchain: {}", console::style(active).green());
    }

    Ok(())
}

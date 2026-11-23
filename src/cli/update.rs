use clap::Parser;

use crate::toolchain::{ToolchainSpec, install::sync_toolchain};

/// Update MoonBit toolchains
#[derive(Parser, Debug)]
pub struct Args {}

pub async fn execute(_: Args) -> miette::Result<()> {
    let specs = installed_channels()?;

    if specs.is_empty() {
        println!("No channel toolchains installed");
        println!(
            "Run 'moonup install <toolchain>' to install one, \
             or check 'moonup install --list-available' for available channels"
        );
        return Ok(());
    }

    let mut failed = false;
    for spec in &specs {
        // A failed channel must not abort the remaining updates; the exit
        // code reports the overall outcome.
        if let Err(e) = sync_toolchain(spec, false).await {
            eprintln!(
                "{} Failed to update the {} toolchain: {}",
                console::style(console::Emoji("✖ ", "")).red(),
                spec,
                e
            );
            failed = true;
        }
    }

    if failed {
        return Err(crate::status::exit_with(1));
    }

    Ok(())
}

/// The floating-channel toolchains currently installed
fn installed_channels() -> miette::Result<Vec<ToolchainSpec>> {
    let installed = crate::toolchain::installed_toolchains()?
        .into_iter()
        .filter(|toolchain| matches!(&toolchain.spec, ToolchainSpec::Channel(_)))
        .map(|toolchain| toolchain.spec)
        .collect();
    Ok(installed)
}

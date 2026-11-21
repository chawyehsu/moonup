use crate::toolchain::{ToolchainSpec, atomic, install::sync_toolchain};
use clap::Parser;

/// Update MoonBit toolchains
#[derive(Parser, Debug)]
pub struct Args {}

pub async fn execute(_: Args) -> miette::Result<()> {
    let mut specs = installed_channels()?;
    for spec in incomplete_channels() {
        if !specs.contains(&spec) {
            specs.push(spec);
        }
    }

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
        .filter(|t| matches!(t.name, ToolchainSpec::Channel(_)))
        .map(|t| t.name)
        .collect();
    Ok(installed)
}

/// The floating-channel toolchains in incomplete staging state, which
/// the next update should recover.
fn incomplete_channels() -> Vec<ToolchainSpec> {
    let staging_dir = atomic::staging_dir();
    let Ok(entries) = std::fs::read_dir(staging_dir).inspect_err(|e| {
        tracing::warn!("Failed to read toolchain staging directory: {}", e);
    }) else {
        return vec![];
    };

    entries
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_str()?.to_owned();
            name.strip_suffix(atomic::STAGING_SUFFIX).map(str::to_owned)
        })
        // Pinned-version staging leftovers are not updated by `moonup update`
        .filter(|name| matches!(ToolchainSpec::from(name.clone()), ToolchainSpec::Channel(_)))
        .map(ToolchainSpec::Channel)
        .collect()
}

use crate::toolchain::{ToolchainSpec, install::sync_toolchain};
use clap::Parser;

/// Update MoonBit toolchains
#[derive(Parser, Debug)]
pub struct Args {}

pub async fn execute(_: Args) -> miette::Result<()> {
    let mut specs = installed_channel_specs();
    for spec in crash_state_channel_specs() {
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

    let mut path = crate::moonup_home();
    path.push("toolchains");

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

/// The floating-channel toolchains currently installed (they carry a version
/// stub; pinned installs do not).
fn installed_channel_specs() -> Vec<ToolchainSpec> {
    crate::toolchain::installed_toolchains()
        .unwrap_or_default()
        .into_iter()
        .filter(|t| matches!(t.name, ToolchainSpec::Channel(_)))
        .map(|t| t.name)
        .collect()
}

/// The floating channels with a staging directory left behind by an
/// interrupted swap (`<name>.new`), which the next update should recover.
fn crash_state_channel_specs() -> Vec<ToolchainSpec> {
    let staging_root = crate::moonup_home().join("toolchains").join(".staging");
    let Ok(entries) = std::fs::read_dir(staging_root) else {
        return vec![];
    };

    entries
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_str()?.to_owned();
            name.strip_suffix(".new").map(str::to_owned)
        })
        // Pinned-version staging leftovers are not updated by `moonup update`
        .filter(|name| matches!(ToolchainSpec::from(name.clone()), ToolchainSpec::Channel(_)))
        .map(ToolchainSpec::Channel)
        .collect()
}

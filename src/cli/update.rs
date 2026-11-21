use clap::Parser;
use std::path::PathBuf;

use crate::cli::install::post_install;
use crate::toolchain::ToolchainSpec;
use crate::toolchain::atomic;
use crate::toolchain::index::build_installrecipe;
use crate::cli::install::{heal_recovered, install_recipe};

/// Update MoonBit toolchains
#[derive(Parser, Debug)]
pub struct Args {}

pub async fn execute(_: Args) -> miette::Result<()> {
    // Update every installed floating-channel toolchain. Channels are data:
    // whatever channel installs exist is what gets updated. A channel left in
    // a crash state (its live directory moved into a complete staging) is
    // discovered from the staging leftovers so the next update heals it.
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
        if let Err(e) = update_toolchain(&mut path, spec).await {
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

async fn update_toolchain(path: &mut PathBuf, spec: &ToolchainSpec) -> miette::Result<()> {
    // Heal a crash state before its metadata is read, so a half-swapped
    // toolchain is not reported as "not installed" and skipped. A promoted
    // toolchain is finalized immediately, before any fallible network
    // resolution, so an interrupted update never leaves it without its
    // post-install steps; the pending marker is acknowledged once done.
    heal_recovered(spec, false)?;

    let name = spec.install_name();
    path.push(&name);
    path.push(crate::toolchain::METADATA_FILENAME);

    let result = match tokio::fs::read_to_string(&path).await {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            println!("{name} toolchain is not installed, run 'moonup install {name}' to install");
            Ok(())
        }
        Err(e) => Err(miette::miette!(e).wrap_err("failed to read metadata file")),
        Ok(content) => {
            let recipe = match build_installrecipe(spec).await? {
                Some(recipe) => recipe,
                None => {
                    println!("No release available for the {} toolchain", spec);
                    return Ok(());
                }
            };

            // The metadata stub records the installed release key (the build
            // date when present, otherwise the version). Legacy stubs hold
            // the key as plain text.
            let local_key = crate::toolchain::install_meta_from_content(&content, spec)
                .map(|meta| meta.key)
                .unwrap_or_else(|| content.trim().to_owned());
            let should_update = local_key != recipe.release.key();

            if should_update {
                println!("Updating the {} toolchain", spec);
                install_recipe(&recipe, false).await?;
            } else {
                println!("The {} toolchain is up to date", spec);
            }
            Ok(())
        }
    };

    path.pop();
    path.pop();
    result
}

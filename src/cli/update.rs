use clap::Parser;

use crate::cli::install::{heal_recovered, install_recipe};
use crate::toolchain::ToolchainSpec;
use crate::toolchain::index::build_installrecipe;

/// Update MoonBit toolchains
#[derive(Parser, Debug)]
pub struct Args {}

pub async fn execute(_: Args) -> miette::Result<()> {
    update_toolchain(&ToolchainSpec::Channel(
        crate::constant::DEFAULT_CHANNEL.to_owned(),
    ))
    .await?;
    update_toolchain(&ToolchainSpec::Channel("nightly".to_owned())).await?;

    Ok(())
}

async fn update_toolchain(spec: &ToolchainSpec) -> miette::Result<()> {
    // Heal a crash state before its metadata is read, so a half-swapped
    // toolchain is not reported as "not installed" and skipped. A promoted
    // toolchain is finalized immediately, before any fallible network
    // resolution, so an interrupted update never leaves it without its
    // post-install steps; the pending marker is acknowledged once done.
    heal_recovered(spec, false)?;

    let name = spec.to_string();

    match spec.install_meta() {
        None => {
            println!("{name} toolchain is not installed, run 'moonup install {name}' to install");
            Ok(())
        }
        Some(meta) => {
            let recipe = build_installrecipe(spec)
                .await?
                .expect("should have recipe");

            // toolchain is considered upgradable if the local release key
            // differs from the channel's current release key
            if meta.key != recipe.release.key() {
                println!("Updating the {} toolchain", name);
                install_recipe(&recipe, false).await?;
            } else {
                println!("The {} toolchain is up to date", name);
            }
            Ok(())
        }
    }
}

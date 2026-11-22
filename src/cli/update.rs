use clap::Parser;

use crate::cli::install::{heal_recovered, install_recipe};
use crate::constant::DEFAULT_CHANNEL;
use crate::toolchain::index::build_installrecipe;
use crate::toolchain::{InstalledToolchain, ToolchainSpec};

/// Update MoonBit toolchains
#[derive(Parser, Debug)]
pub struct Args {}

pub async fn execute(_: Args) -> miette::Result<()> {
    for channel in [DEFAULT_CHANNEL, "nightly"] {
        let spec = ToolchainSpec::Channel(channel.to_owned());
        // Heal before update
        heal_recovered(&spec, false)?;

        let path = spec.install_path();
        if !path.exists() {
            println!("{spec} toolchain is not installed, run 'moonup install {spec}' to install");
            continue;
        }

        update_toolchain(InstalledToolchain::from_path(&path)?).await?;
    }
    Ok(())
}

async fn update_toolchain(t: InstalledToolchain) -> miette::Result<()> {
    let spec = &t.spec;
    let recipe = build_installrecipe(spec)
        .await?
        .expect("should have recipe");

    // toolchain is considered upgradable if the local release key
    // differs from the channel's current release key
    if t.key() != recipe.release.key() {
        println!("Updating the {} toolchain", spec);
        install_recipe(&recipe, false).await?;
    } else {
        println!("The {} toolchain is up to date", spec);
    }
    Ok(())
}

use clap::Parser;
use std::path::PathBuf;

use crate::cli::install::post_install;
use crate::toolchain::index::build_installrecipe;
use crate::toolchain::package::populate_install;
use crate::toolchain::ToolchainSpec;

/// Update MoonBit toolchains
#[derive(Parser, Debug)]
pub struct Args {}

pub async fn execute(_: Args) -> miette::Result<()> {
    let mut path = crate::moonup_home();
    path.push("toolchains");

    update_toolchain(&mut path, &ToolchainSpec::Latest).await?;
    update_toolchain(&mut path, &ToolchainSpec::Nightly).await?;

    Ok(())
}

async fn update_toolchain(path: &mut PathBuf, spec: &ToolchainSpec) -> miette::Result<()> {
    let name = spec.as_str();
    path.push(name);
    path.push("version");

    let result = match tokio::fs::read_to_string(&path).await {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            println!("{name} toolchain is not installed, run 'moonup install {name}' to install");
            Ok(())
        }
        Err(e) => Err(miette::miette!(e).wrap_err("failed to read version file")),
        Ok(local_ver) => {
            let recipe = build_installrecipe(spec)
                .await?
                .expect("should have recipe");

            let should_update = match (spec, local_ver.trim()) {
                (ToolchainSpec::Bleeding, _) => true,
                (ToolchainSpec::Latest, local) => local != recipe.release.version.as_str(),
                (ToolchainSpec::Nightly, local) => {
                    local != recipe.release.date.as_deref().expect("should have date")
                }
                _ => false,
            };

            if should_update {
                println!("Updating the {} toolchain", name);
                populate_install(&recipe).await?;
                post_install(&recipe)?;
            } else {
                println!("The {} toolchain is up to date", name);
            }
            Ok(())
        }
    };

    path.pop();
    path.pop();
    result
}

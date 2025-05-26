use clap::Parser;

use crate::cli::install::post_install;
use crate::toolchain::index::build_installrecipe;
use crate::toolchain::package::populate_install;
use crate::toolchain::ToolchainSpec;

/// Update MoonBit toolchains
#[derive(Parser, Debug)]
pub struct Args {}

pub async fn execute(_: Args) -> miette::Result<()> {
    let mut version_file_path = crate::moonup_home();
    version_file_path.push("toolchains");

    // Update the `latest` toolchain if installed
    {
        version_file_path.push("latest");
        version_file_path.push("version");

        match tokio::fs::read_to_string(&version_file_path).await {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                println!(
                    "latest toolchain is not installed, run 'moonup install latest' to install"
                )
            }
            Err(e) => return Err(miette::miette!(e).wrap_err("failed to read version file")),
            Ok(version_local) => {
                let recipe = build_installrecipe(&ToolchainSpec::Latest)
                    .await?
                    .expect("should have recipe");
                let version_remote = recipe.release.version.as_str();

                if version_local.trim() == version_remote {
                    println!("The latest toolchain is up to date");
                } else {
                    println!("Updating the latest toolchain");
                    populate_install(&recipe).await?;
                    post_install(&recipe)?;
                }
            }
        }

        version_file_path.pop();
        version_file_path.pop();
    }

    // Update the `nightly` toolchain if installed
    {
        version_file_path.push("nightly");
        version_file_path.push("version");

        match tokio::fs::read_to_string(&version_file_path).await {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                println!(
                    "nightly toolchain is not installed, run 'moonup install nightly' to install"
                )
            }
            Err(e) => return Err(miette::miette!(e).wrap_err("failed to read version file")),
            Ok(date_local) => {
                let recipe = build_installrecipe(&ToolchainSpec::Nightly)
                    .await?
                    .expect("should have recipe");
                let date_remote = recipe.release.date.as_deref().expect("should have date");

                if date_local.trim() == date_remote {
                    println!("The nightly toolchain is up to date");
                } else {
                    println!("Updating the nightly toolchain");
                    populate_install(&recipe).await?;
                    post_install(&recipe)?;
                }
            }
        }
    }

    // Update the `bleeding` toolchain if installed
    {
        version_file_path.push("bleeding");
        version_file_path.push("version");

        match tokio::fs::read_to_string(&version_file_path).await {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                println!(
                    "bleeding toolchain is not installed, run 'moonup install bleeding' to install"
                )
            }
            Err(e) => return Err(miette::miette!(e).wrap_err("failed to read version file")),
            Ok(_) => {
                // Always update the bleeding toolchain despite the version
                let recipe = build_installrecipe(&ToolchainSpec::Bleeding)
                    .await?
                    .expect("should have recipe");
                println!("Updating the bleeding toolchain");
                populate_install(&recipe).await?;
                post_install(&recipe)?;
            }
        }
    }

    Ok(())
}

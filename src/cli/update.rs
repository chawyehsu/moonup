use clap::Parser;
use miette::IntoDiagnostic;
use std::env;
use std::path::PathBuf;
use tokio::io::AsyncReadExt;

use crate::archive;
use crate::cli::install::post_install;
use crate::toolchain::index::build_installrecipe;
use crate::toolchain::package::populate_install;
use crate::toolchain::ToolchainSpec;
use crate::utils::{self, build_http_client};

/// Update MoonBit latest toolchain and moonup
#[derive(Parser, Debug)]
pub struct Args {
    /// Don't perform self update when running `moonup update`
    #[clap(long)]
    no_self_update: bool,
}

pub async fn execute(args: Args) -> miette::Result<()> {
    // Checking and updating moonup first so the latest `moonup-shim`
    // can be used later
    if !args.no_self_update {
        self_update().await?;
    }

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
                let recipe = build_installrecipe(&ToolchainSpec::Latest).await?;
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
                let recipe = build_installrecipe(&ToolchainSpec::Nightly).await?;
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

    // TODO(chawyehsu): Update the `bleeding` toolchain if installed
    { /* */ }

    Ok(())
}

async fn self_update() -> miette::Result<()> {
    let current_version = env!("CARGO_PKG_VERSION");

    let updater = self_update::backends::github::Update::configure()
        .repo_owner("chawyehsu")
        .repo_name("moonup")
        .bin_name(env!("CARGO_PKG_NAME"))
        .current_version(current_version)
        .build()
        .into_diagnostic()?;

    let latest_release = updater.get_latest_release().into_diagnostic()?;
    let is_greater =
        self_update::version::bump_is_greater(current_version, &latest_release.version)
            .ok()
            .unwrap_or(false);

    if !is_greater {
        println!("moonup is up to date");
        return Ok(());
    }

    println!(
        "Updating moonup: {} -> {}",
        current_version, latest_release.version
    );

    let assets = latest_release
        .assets
        .iter()
        .filter(|a| a.name.contains(self_update::get_target()))
        .collect::<Vec<_>>();

    tracing::trace!("moonup assets: {:?}", assets);
    assert_eq!(assets.len(), 2, "expected two assets");

    let client = build_http_client();
    let temp_dir = self_update::TempDir::with_prefix("moonup").into_diagnostic()?;

    let mut sha256_actual = String::new();
    let mut sha256_expected = String::new();

    let extract_to = temp_dir.path();

    for asset in assets {
        let url = format!(
            "https://github.com/chawyehsu/moonup/releases/download/v{}/{}",
            latest_release.version, asset.name
        );
        let url = url::Url::parse(url.as_str()).into_diagnostic()?;
        tracing::debug!("downloading {} from {}", asset.name, url);

        let mut reader = utils::url_to_reader(url, client.clone(), None).await?;

        if asset.name.ends_with(".sha256") {
            let mut content = String::new();
            reader
                .read_to_string(&mut content)
                .await
                .into_diagnostic()?;
            sha256_expected = content.trim().to_ascii_lowercase();
        } else {
            tracing::debug!("extracting to {}", extract_to.display());

            let sha256 = match asset.name.ends_with(".zip") {
                false => archive::extract_tar_gz(reader, extract_to).await?,
                true => archive::extract_zip(reader, extract_to).await?,
            };

            sha256_actual = format!("{:x}", sha256);
        }
    }

    assert_eq!(sha256_actual, sha256_expected, "SHA256 checksum mismatch");

    let args = env::args_os().collect::<Vec<_>>();
    for bin in ["moonup", "moonup-shim"] {
        let ext = if cfg!(windows) { ".exe" } else { "" };
        let name = format!("{}{}", bin, ext);

        let src = extract_to.join(&name);
        let dst = env::current_exe()
            .unwrap_or_else(|_| PathBuf::from(&args[0]))
            .with_file_name(name);

        utils::replace_exe(&src, &dst)?;
    }
    Ok(())
}

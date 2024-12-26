use clap::Parser;
use miette::IntoDiagnostic;
use std::env;
use std::path::PathBuf;
use tokio::io::AsyncReadExt;

use crate::archive;
use crate::cli::install::post_install;
use crate::fs::save_file;
use crate::toolchain::index::retrieve_release;
use crate::toolchain::package::populate_package;
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

    let release = retrieve_release("latest").await?;
    let mut version_file_path = crate::moonup_home();
    version_file_path.push("toolchains");
    version_file_path.push("latest");
    version_file_path.push("version");

    match tokio::fs::read_to_string(version_file_path).await {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            println!("latest toolchain is not installed, run 'moonup install latest' to install")
        }
        Err(e) => return Err(miette::miette!(e).wrap_err("Failed to read version file")),
        Ok(local_latest_version) => {
            if let Some(r) = &release.toolchain {
                if local_latest_version.trim() == r.version {
                    println!("The latest toolchain is up to date");
                } else {
                    println!("Updating the latest toolchain");
                    populate_package(&release).await?;
                    post_install(&release)?;
                }
            }
        }
    }

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
    let mut archive_file = PathBuf::new();

    for asset in assets {
        let url = format!(
            "https://github.com/chawyehsu/moonup/releases/download/v{}/{}",
            latest_release.version, asset.name
        );
        let url = url::Url::parse(url.as_str()).into_diagnostic()?;
        tracing::debug!("downloading {} from {}", asset.name, url);
        let file = temp_dir.path().join(&asset.name);
        tracing::debug!("saving to {}", file.display());

        let mut reader = utils::url_to_reader(url, client.clone(), None).await?;
        if !asset.name.ends_with(".sha256") {
            sha256_actual = format!("{:x}", save_file(reader, &file).await?);
            archive_file = file;
        } else {
            let mut content = String::new();
            reader
                .read_to_string(&mut content)
                .await
                .into_diagnostic()?;
            sha256_expected = content.trim().to_ascii_lowercase();
        }
    }

    assert_eq!(sha256_actual, sha256_expected, "SHA256 checksum mismatch");

    let reader = utils::path_to_reader(&archive_file).await?;
    let extract_to = temp_dir.path();
    let extension = archive_file.extension().unwrap().to_str().unwrap();
    match extension {
        "gz" => archive::extract_tar_gz(reader, extract_to).await?,
        "zip" => archive::extract_zip(reader, extract_to).await?,
        _ => unreachable!("unsupported extension"),
    }

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

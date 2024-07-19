use clap::builder::NonEmptyStringValueParser;
use clap::Parser;
use miette::IntoDiagnostic;
#[cfg(not(target_os = "windows"))]
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::{env, process::Command};

use crate::toolchain::index::retrieve_releases;
use crate::toolchain::{
    index::{retrieve_release, ReleaseCombined},
    package::populate_package,
};

/// Install or update a MoonBit toolchain
#[derive(Parser, Debug)]
#[clap(arg_required_else_help = true)]
pub struct Args {
    /// Toolchain name, can be 'latest' or a specific version number
    #[clap(value_parser = NonEmptyStringValueParser::new())]
    toolchain: Option<String>,

    /// List available toolchains
    #[clap(long)]
    list_available: bool,
}

pub async fn execute(args: Args) -> miette::Result<()> {
    if args.list_available {
        let releases = retrieve_releases().await?;
        if releases.is_empty() {
            println!("No available toolchains found");
        } else {
            println!("Available toolchains:");
            for release in releases.iter().rev() {
                println!("{}", release.version);
            }
        }
        return Ok(());
    }

    let version = args.toolchain.unwrap_or_default();

    println!("Installing toolchain '{}'", version);

    let release = retrieve_release(&version).await?;

    if release.core.is_none() && release.toolchain.is_none() {
        return Err(miette::miette!(
            "No toolchain found for version '{}'",
            version
        ));
    }

    populate_package(&release).await?;
    post_install(&release)?;

    println!(
        "{}Installed toolchain version '{}'",
        console::style(console::Emoji("✔ ", "")).green(),
        version
    );
    println!(
        "Make sure '{}' is added to your PATH",
        crate::moon_home().join("bin").display()
    );

    Ok(())
}

// Post installation: pour shims and build the core library
fn post_install(release: &ReleaseCombined) -> miette::Result<()> {
    let args = env::args_os().collect::<Vec<_>>();
    let mut moonup_shim_exe = env::current_exe().unwrap_or_else(|_| PathBuf::from(&args[0]));
    moonup_shim_exe.set_file_name({
        #[cfg(target_os = "windows")]
        {
            "moonup-shim.exe"
        }
        #[cfg(not(target_os = "windows"))]
        {
            "moonup-shim"
        }
    });

    let version = match release.latest {
        true => "latest".to_string(),
        false => release
            .toolchain
            .as_ref()
            .map(|r| r.version.to_string())
            .unwrap_or(
                release
                    .core
                    .as_ref()
                    .map(|r| r.version.to_string())
                    .expect("no version found"),
            ),
    };

    let toolchain_dir = crate::moonup_home().join("toolchains").join(version);
    let bin_dir = toolchain_dir.join("bin");
    let bins = bin_dir
        .read_dir()
        .into_diagnostic()?
        .filter_map(std::io::Result::ok)
        .filter_map(|e| {
            let is_file = e
                .file_type()
                .into_diagnostic()
                .map(|t| t.is_file())
                .unwrap_or(false);
            if is_file {
                #[cfg(not(target_os = "windows"))]
                {
                    std::fs::set_permissions(e.path(), std::fs::Permissions::from_mode(0o755))
                        .unwrap();
                }

                Some(e.file_name())
            } else {
                None
            }
        })
        .collect::<Vec<_>>();

    let moon_home_bin = crate::moon_home().join("bin");

    std::fs::create_dir_all(&moon_home_bin).into_diagnostic()?;

    for bin in bins {
        tracing::debug!("Pouring shim for '{}'", bin.to_string_lossy());
        let dest = moon_home_bin.join(&bin);
        crate::utils::pour_shim(&moonup_shim_exe, &dest)?;
    }

    // Build core library
    let corelib_dir = toolchain_dir.join("lib").join("core");
    let actual_moon_exe = bin_dir.join({
        #[cfg(target_os = "windows")]
        {
            "moon.exe"
        }
        #[cfg(not(target_os = "windows"))]
        {
            "moon"
        }
    });

    let mut cmd = Command::new(actual_moon_exe);
    cmd.arg("bundle");
    cmd.arg("--all");
    cmd.arg("--source-dir");
    cmd.arg(&corelib_dir);
    cmd.env("PATH", bin_dir.display().to_string());
    tracing::debug!("Running command: {:?}", cmd);
    cmd.status().into_diagnostic()?;

    Ok(())
}

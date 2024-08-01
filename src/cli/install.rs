use clap::builder::NonEmptyStringValueParser;
use clap::{CommandFactory, Parser};
use miette::IntoDiagnostic;
#[cfg(not(target_os = "windows"))]
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::{env, process::Command};

use crate::toolchain::index::retrieve_releases;
use crate::toolchain::resolve::detect_pinned_version;
use crate::toolchain::{
    index::{retrieve_release, ReleaseCombined},
    package::populate_package,
};

/// Install or update a MoonBit toolchain
#[derive(Parser, Debug)]
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

    let version = match args.toolchain.or(detect_pinned_version()) {
        Some(v) => v,
        None => {
            let mut cmd = Args::command();
            cmd.print_help().into_diagnostic()?;
            std::process::exit(2);
        }
    };

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
    link_lib(&release)?;

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
pub(super) fn post_install(release: &ReleaseCombined) -> miette::Result<()> {
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
        crate::utils::replace_exe(&moonup_shim_exe, &dest)?;
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

// Link the library directory to `MOON_HOME`/lib
// The latest toolchain's core library will be linked if available,
// otherwise the installed toolchain's core library will be linked
//
// This is a workaround for the issue of MoonBit's VSCode extension
// reporting errors when the core library is not found, as the extension
// always looks for the core library in `MOON_HOME`/lib/core.
fn link_lib(release: &ReleaseCombined) -> miette::Result<()> {
    let lnk = crate::moon_home().join("lib");
    let src = {
        let latest_toolchain_lib_core = crate::moonup_home()
            .join("toolchains")
            .join("latest")
            .join("lib");

        if latest_toolchain_lib_core.exists() {
            latest_toolchain_lib_core
        } else {
            let version = release
                .toolchain
                .as_ref()
                .map(|r| r.version.to_string())
                .expect("should have a toolchain version");
            crate::moonup_home()
                .join("toolchains")
                .join(version)
                .join("lib")
        }
    };

    let _ = std::fs::remove_dir_all(&lnk);
    tracing::debug!(
        "Linking lib directory: {} -> {}",
        lnk.display(),
        src.display()
    );

    #[cfg(target_os = "windows")]
    {
        junction::create(src, lnk)
            .map_err(|e| miette::miette!("Failed to create junction: {}", e))?;
    }

    #[cfg(not(target_os = "windows"))]
    {
        std::os::unix::fs::symlink(src, lnk)
            .map_err(|e| miette::miette!("Failed to create symlink: {}", e))?;
    }

    Ok(())
}

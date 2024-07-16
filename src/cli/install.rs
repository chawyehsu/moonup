use clap::Parser;
use miette::IntoDiagnostic;
use std::path::PathBuf;
use std::{env, process::Command};

use crate::toolchain::{
    index::{retrieve_release, ReleaseCombined},
    package::populate_package,
};

/// Install or update a MoonBit toolchain
#[derive(Parser, Debug)]
#[clap(arg_required_else_help = true)]
pub struct Args {
    /// Toolchain name, can be 'latest' or a specific version number
    toolchain: String,
}

pub async fn execute(args: Args) -> miette::Result<()> {
    let version = args.toolchain.as_str();
    let release = retrieve_release(version).await?;

    populate_package(&release).await?;
    post_install(&release)?;

    println!("Installed toolchain version '{}'", version);

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
        std::fs::copy(&moonup_shim_exe, &dest).into_diagnostic()?;
    }

    // Build core library
    let corelib_dir = toolchain_dir.join("lib/core");
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
    cmd.status().into_diagnostic()?;

    Ok(())
}

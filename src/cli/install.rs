use clap::{CommandFactory, Parser};
use miette::{Context, IntoDiagnostic};
use std::ffi::OsString;
use std::ops::Deref;
#[cfg(not(target_os = "windows"))]
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::{env, process::Command};

use crate::toolchain::index::InstallRecipe;
use crate::toolchain::resolve::detect_pinned_toolchain;
use crate::toolchain::{index, ToolchainSpec};
use crate::toolchain::{index::build_installrecipe, package::populate_install};

use super::ToolchainSpecValueParser;

/// Install or update a MoonBit toolchain
#[derive(Parser, Debug)]
pub struct Args {
    /// Toolchain version tag or channel name [latest, nightly, bleeding]
    #[clap(value_parser = ToolchainSpecValueParser::new())]
    toolchain: Option<ToolchainSpec>,

    /// List available toolchains
    #[clap(long)]
    list_available: bool,
}

pub async fn execute(args: Args) -> miette::Result<()> {
    if args.list_available {
        let index = index::read_index().await?;
        let channels = index.channels.deref();
        if channels.is_empty() {
            println!("No available toolchains found");
        } else {
            println!("Available toolchains:");
            for channel in channels {
                println!("  {}", channel);
            }
        }

        return Ok(());
    }

    let spec = match args
        .toolchain
        .or(detect_pinned_toolchain().map(ToolchainSpec::from))
    {
        Some(v) => v,
        None => {
            let mut cmd = Args::command();
            cmd.print_help().into_diagnostic()?;
            std::process::exit(2);
        }
    };

    let recipe = build_installrecipe(&spec).await?.unwrap_or_else(|| {
        eprintln!("No toolchain available for requested spec '{}'", spec);
        std::process::exit(1);
    });

    println!("Installing toolchain '{}'", spec);
    populate_install(&recipe).await?;
    post_install(&recipe)?;
    link_dirs(&recipe)?;

    println!(
        "{}Installed toolchain version '{}'",
        console::style(console::Emoji("✔ ", "")).green(),
        spec
    );
    println!(
        "Make sure '{}' is added to your PATH",
        crate::moon_home().join("bin").display()
    );

    Ok(())
}

fn toolchain_install_dirname(recipe: &InstallRecipe) -> String {
    if let Some(date) = recipe.release.date.as_ref() {
        if recipe.spec.is_nightly() {
            "nightly".to_owned()
        } else {
            format!("nightly-{}", date)
        }
    } else if recipe.spec.is_latest() {
        "latest".to_owned()
    } else if recipe.spec.is_bleeding() {
        "bleeding".to_owned()
    } else {
        recipe.release.version.clone()
    }
}

// Post installation: pour shims and build the core library
pub(super) fn post_install(recipe: &InstallRecipe) -> miette::Result<()> {
    let args = env::args_os().collect::<Vec<_>>();
    let mut moonup_shim_exe = env::current_exe().unwrap_or_else(|_| PathBuf::from(&args[0]));
    let moonup_shim_name = {
        let ext = if cfg!(windows) { ".exe" } else { "" };
        format!("moonup-shim{}", ext)
    };
    moonup_shim_exe.set_file_name(moonup_shim_name);

    let mut toolchain_dir = crate::moonup_home();
    toolchain_dir.push("toolchains");
    toolchain_dir.push(toolchain_install_dirname(recipe));

    let moon_home_bin = crate::moon_home().join("bin");

    std::fs::create_dir_all(&moon_home_bin).into_diagnostic()?;
    let _ = crate::fs::empty_dir(&moon_home_bin);

    // bins
    let bin_dir = toolchain_dir.join("bin");

    let bins = find_bins(&bin_dir).wrap_err("failed to find bins")?;
    for bin in bins {
        tracing::debug!("pouring shim for '{}'", bin.to_string_lossy());
        let dest = moon_home_bin.join(&bin);
        crate::utils::replace_exe(&moonup_shim_exe, &dest)?;
    }

    // internl bins
    let internal_bin_dir = bin_dir.join("internal");
    if internal_bin_dir.exists() {
        let moon_home_bin_internal = moon_home_bin.join("internal");
        std::fs::create_dir_all(&moon_home_bin_internal).into_diagnostic()?;
        let _ = crate::fs::empty_dir(&moon_home_bin_internal);

        let internal_bins = find_bins(&internal_bin_dir)?;
        for bin in internal_bins {
            tracing::debug!("pouring internal shim for '{}'", bin.to_string_lossy());
            let dest = moon_home_bin_internal.join(&bin);
            crate::utils::replace_exe(&moonup_shim_exe, &dest)?;
        }
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
    tracing::debug!("running command: {:?}", cmd);
    cmd.status().into_diagnostic()?;

    Ok(())
}

fn find_bins(dir: &PathBuf) -> miette::Result<Vec<OsString>> {
    let bins = dir
        .read_dir()
        .into_diagnostic()
        .wrap_err(format!("cannot read dir: {}", dir.display()))?
        .filter_map(std::io::Result::ok)
        .filter_map(|e| {
            let path = e.path();
            let name = e.file_name();
            let ext = path.extension();

            let is_file = e
                .file_type()
                .into_diagnostic()
                .map(|t| t.is_file())
                .unwrap_or(false);

            if is_file {
                #[cfg(target_os = "windows")]
                {
                    ext.and_then(|ext| if ext == "exe" { Some(name) } else { None })
                }

                #[cfg(not(target_os = "windows"))]
                {
                    // Skip if the file has an extension (.h, .a. .o, etc.)
                    if ext.is_some() {
                        return None;
                    }

                    std::fs::set_permissions(e.path(), std::fs::Permissions::from_mode(0o755))
                        .unwrap();
                    Some(name)
                }
            } else {
                None
            }
        })
        .collect::<Vec<_>>();

    Ok(bins)
}

// Link the library directory to `MOON_HOME`/lib
// The latest toolchain's core library will be linked if available,
// otherwise the installed toolchain's core library will be linked
//
// This is a workaround for the issue of MoonBit's VSCode extension
// reporting errors when the core library is not found, as the extension
// always looks for the core library in `MOON_HOME`/lib/core.
//
// Discussion: https://github.com/chawyehsu/moonup/issues/7
fn link_dirs(recipe: &InstallRecipe) -> miette::Result<()> {
    let dirs = ["lib", "include"];
    let toolchains_dir = crate::moonup_home().join("toolchains");

    for dir in dirs {
        let src = {
            let from_latest = toolchains_dir.join("latest").join(dir);

            if from_latest.exists() {
                from_latest
            } else {
                let from_recipe = toolchains_dir
                    .join(toolchain_install_dirname(recipe))
                    .join(dir);

                // Older toolchains may not have the `include` directory
                if !from_recipe.exists() {
                    tracing::debug!("dir '{}' not found in toolchain {}", dir, recipe.spec);
                    continue;
                }
                from_recipe
            }
        };

        let lnk = crate::moon_home().join(dir);
        let _ = std::fs::remove_dir_all(&lnk).inspect_err(|e| {
            tracing::debug!("failed to remove link {} (err: {})", lnk.display(), e);
        });
        tracing::debug!("linking directory: {} -> {}", lnk.display(), src.display());

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
    }

    Ok(())
}

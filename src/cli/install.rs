use clap::builder::TypedValueParser;
use clap::{CommandFactory, Parser};
use miette::IntoDiagnostic;
use std::ops::Deref;
#[cfg(not(target_os = "windows"))]
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::{env, process::Command};

use crate::toolchain::index::InstallRecipe;
use crate::toolchain::resolve::detect_pinned_version;
use crate::toolchain::{index, ToolchainSpec};
use crate::toolchain::{index::build_installrecipe, package::populate_install};

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

#[derive(Copy, Clone, Debug)]
struct ToolchainSpecValueParser;

impl ToolchainSpecValueParser {
    /// Parse non-empty ToolchainSpec value
    pub fn new() -> Self {
        Self {}
    }
}

impl TypedValueParser for ToolchainSpecValueParser {
    type Value = ToolchainSpec;

    fn parse_ref(
        &self,
        cmd: &clap::Command,
        _arg: Option<&clap::Arg>,
        value: &std::ffi::OsStr,
    ) -> Result<Self::Value, clap::Error> {
        if value.is_empty() {
            return Err(clap::Error::new(clap::error::ErrorKind::InvalidValue).with_cmd(cmd));
        }

        let value = value
            .to_str()
            .ok_or_else(|| clap::Error::new(clap::error::ErrorKind::InvalidUtf8).with_cmd(cmd))?;

        Ok(ToolchainSpec::from(value))
    }
}

impl Default for ToolchainSpecValueParser {
    fn default() -> Self {
        Self::new()
    }
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
        .or(detect_pinned_version().map(ToolchainSpec::from))
    {
        Some(v) => v,
        None => {
            let mut cmd = Args::command();
            cmd.print_help().into_diagnostic()?;
            std::process::exit(2);
        }
    };

    if spec.is_bleeding() {
        eprintln!("'bleeding' channel installation is not implemented");
        std::process::exit(1);
    }

    let recipe = build_installrecipe(&spec).await?.unwrap_or_else(|| {
        eprintln!("No toolchain available for requested spec '{}'", spec);
        std::process::exit(1);
    });

    println!("Installing toolchain '{}'", spec);
    populate_install(&recipe).await?;
    post_install(&recipe)?;
    link_lib(&recipe)?;

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
    } else {
        recipe.release.version.clone()
    }
}

// Post installation: pour shims and build the core library
pub(super) fn post_install(recipe: &InstallRecipe) -> miette::Result<()> {
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

    let mut toolchain_dir = crate::moonup_home();
    toolchain_dir.push("toolchains");
    toolchain_dir.push(toolchain_install_dirname(recipe));

    let bin_dir = toolchain_dir.join("bin");
    let bins = bin_dir
        .read_dir()
        .into_diagnostic()?
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

    let moon_home_bin = crate::moon_home().join("bin");

    std::fs::create_dir_all(&moon_home_bin).into_diagnostic()?;
    let _ = crate::fs::empty_dir(&moon_home_bin);

    for bin in bins {
        tracing::debug!("pouring shim for '{}'", bin.to_string_lossy());
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
    tracing::debug!("running command: {:?}", cmd);
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
//
// Discussion: https://github.com/chawyehsu/moonup/issues/7
fn link_lib(recipe: &InstallRecipe) -> miette::Result<()> {
    let lnk = crate::moon_home().join("lib");
    let src = {
        let latest_toolchain_lib_core = crate::moonup_home()
            .join("toolchains")
            .join("latest")
            .join("lib");

        if latest_toolchain_lib_core.exists() {
            latest_toolchain_lib_core
        } else {
            crate::moonup_home()
                .join("toolchains")
                .join(toolchain_install_dirname(recipe))
                .join("lib")
        }
    };

    let _ = std::fs::remove_dir_all(&lnk);
    tracing::debug!(
        "linking lib directory: {} -> {}",
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

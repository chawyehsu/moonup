use clap::{CommandFactory, Parser};
use dialoguer::theme::ColorfulTheme;
use miette::{Context, IntoDiagnostic};
use std::ffi::OsString;
#[cfg(not(target_os = "windows"))]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::{env, process::Command};

#[cfg(target_os = "windows")]
use crate::constant::ALLOWED_EXTENSIONS;
use crate::toolchain::index::{InstallRecipe, build_installrecipe};
use crate::toolchain::package::populate_install;
use crate::toolchain::resolve::detect_pinned_toolchain;
use crate::toolchain::{ToolchainSpec, atomic, index};
use crate::utils::exe_name;

use super::ToolchainSpecValueParser;

/// Install or update a MoonBit toolchain
#[derive(Parser, Debug)]
pub struct Args {
    /// Toolchain version or channel name
    ///
    /// Channels are data: whatever the dist server advertises is installable.
    /// See `moonup install --list-available` for the available channels.
    #[clap(value_parser = ToolchainSpecValueParser::new())]
    toolchain: Option<ToolchainSpec>,

    /// List available channels or versions
    ///
    /// If `toolchain` is not specified, this will list all available channels.
    /// If `toolchain` is specified, this will list all available versions for
    /// that toolchain's channel.
    #[clap(long, short = 'l')]
    list_available: bool,
}

pub async fn execute(args: Args) -> miette::Result<()> {
    if args.list_available && args.toolchain.is_none() {
        let index = index::read_index().await?;
        let channels = index.channels();
        if channels.is_empty() {
            println!("No available channel found");
        } else {
            println!("Available channel(s):");
            for channel in channels {
                println!("  {}", channel);
            }
        }

        return Ok(());
    }

    let spec = match args.toolchain {
        Some(s) => {
            if args.list_available {
                match list_channel_versions(&s).await? {
                    Some(picked) => picked,
                    // A floating-only channel: the listing is the whole answer
                    None => return Ok(()),
                }
            } else {
                s
            }
        }
        None => match detect_pinned_toolchain()
            .map(ToolchainSpec::try_from)
            .transpose()
            .map_err(|error| miette::miette!(error))?
        {
            Some(v) => v,
            None => {
                let mut cmd = Args::command();
                cmd.print_help().into_diagnostic()?;
                return Err(crate::status::exit_with(2));
            }
        },
    };

    if heal_recovered(&spec, true)? {
        return Ok(());
    }

    let recipe = match build_installrecipe(&spec).await? {
        Some(recipe) => recipe,
        None => {
            eprintln!("No toolchain available for requested spec '{}'", spec);
            return Err(crate::status::exit_with(1));
        }
    };

    if recipe.requested_spec != recipe.spec {
        tracing::info!(
            requested = %recipe.requested_spec,
            resolved = %recipe.spec,
            "resolved stable version selector"
        );
    }

    println!("Installing toolchain '{}'", recipe.spec);
    install_recipe(&recipe, true).await?;

    println!(
        "{}Installed toolchain version '{}'",
        console::style(console::Emoji("✔ ", "")).green(),
        recipe.spec
    );
    println!(
        "Make sure '{}' is added to your PATH",
        crate::moon_home().join("bin").display()
    );

    Ok(())
}

/// List the releases of the channel the given spec resolves within, and
/// interactively pick one to install.
///
/// Picking produces a channel-qualified pinned spec
async fn list_channel_versions(spec: &ToolchainSpec) -> miette::Result<Option<ToolchainSpec>> {
    let channel = spec.channel();

    // Validate against the main index so an unknown channel errors with the
    // available ones instead of a bare download failure.
    if let Some(channels) = index::available_channels().await
        && !channels.iter().any(|c| c == channel)
    {
        eprintln!("No toolchain available for requested spec '{}'", spec);
        return Err(crate::status::exit_with(1));
    }

    let channel_index = index::read_channel_index(channel).await?;
    let releases = channel_index
        .releases()
        .iter()
        .filter(|r| r.is_host_supported()) // only show releases that support the current host
        .rev()
        .collect::<Vec<_>>();

    if releases.is_empty() {
        eprintln!("No available toolchain versions in the {channel} channel");
        return Err(crate::status::exit_with(1));
    }

    let selections = releases
        .iter()
        .map(|r| release_label(r))
        .collect::<Vec<_>>();

    let selection = dialoguer::Select::with_theme(&ColorfulTheme::default())
        .with_prompt(format!("Pick a version from {channel} channel"))
        .items(&selections)
        .max_length(crate::constant::MAX_SELECT_ITEMS)
        .default(0)
        .interact()
        .into_diagnostic()?;

    let picked_release = releases[selection];
    let picked = ToolchainSpec::Version {
        channel: channel.to_owned(),
        key: picked_release.key().to_owned(),
    };

    Ok(Some(picked))
}

fn release_label(release: &crate::dist_server::schema::Release) -> String {
    match release.date.as_deref() {
        Some(date) => format!("{} ({})", release.version, date),
        None => release.version.clone(),
    }
}

/// Heal a toolchain left in a half-swapped state by an interrupted operation:
/// promote a complete staging directory, run the post-install steps, and
/// acknowledge the promotion.
///
/// Returns whether a crash state was healed. The caller decides what to do
/// next: `install` treats the toolchain as installed and stops, while `update`
/// continues to its up-to-date check.
///
/// `link` controls whether the `MOON_HOME` directory links are (re)created
/// alongside the post-install steps: `install` does, because it may have just
/// created a new pinned install directory; `update` does not, because it only
/// swaps content in place under an unchanged directory name, leaving the
/// existing links valid.
pub(crate) fn heal_recovered(spec: &ToolchainSpec, link: bool) -> miette::Result<bool> {
    let Some(staged) = atomic::recover(spec)? else {
        return Ok(false);
    };

    let recipe = staged.into_recipe(spec);
    post_install(&recipe)?;
    if link {
        link_dirs(&recipe)?;
    }
    atomic::acknowledge(&recipe.spec);
    Ok(true)
}

/// Perform the install pipeline for a fully resolved recipe: download and
/// extract the components, swap the staging directory into the live location,
/// run the post-install steps, and acknowledge the promotion.
///
/// `link` follows the same rule as [`heal_recovered`].
pub(crate) async fn install_recipe(recipe: &InstallRecipe, link: bool) -> miette::Result<()> {
    populate_install(recipe).await?;
    post_install(recipe)?;
    if link {
        link_dirs(recipe)?;
    }
    atomic::acknowledge(&recipe.spec);
    Ok(())
}

// Post installation: pour shims and build the core library
pub(super) fn post_install(recipe: &InstallRecipe) -> miette::Result<()> {
    let args = env::args_os().collect::<Vec<_>>();
    let mut moonup_shim_exe = env::current_exe().unwrap_or_else(|_| PathBuf::from(&args[0]));
    let moonup_shim_name = exe_name("moonup-shim");
    moonup_shim_exe.set_file_name(moonup_shim_name);

    let mut toolchain_dir = crate::moonup_home();
    toolchain_dir.push("toolchains");
    toolchain_dir.push(recipe.spec.install_name());

    let moon_home_bin = crate::moon_home().join("bin");

    std::fs::create_dir_all(&moon_home_bin).into_diagnostic()?;

    // bins
    let bin_dir = toolchain_dir.join("bin");
    let moon_exe = bin_dir.join(exe_name("moon"));

    // moonx
    if recipe
        .release
        .moonx
        .as_ref()
        .is_none_or(|s| s != "unavailable")
    {
        let moonx_exe = bin_dir.join(exe_name("moonx"));
        if !moonx_exe.exists() {
            #[cfg(target_os = "windows")]
            std::fs::copy(&moon_exe, &moonx_exe).into_diagnostic()?;
            #[cfg(not(target_os = "windows"))]
            std::os::unix::fs::symlink(&moon_exe, &moonx_exe).into_diagnostic()?;
        }
    }

    let bins = find_bins(bin_dir.as_path()).wrap_err("failed to find bins")?;
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

        let internal_bins = find_bins(internal_bin_dir.as_path())?;
        for bin in internal_bins {
            tracing::debug!("pouring internal shim for '{}'", bin.to_string_lossy());
            let dest = moon_home_bin_internal.join(&bin);
            crate::utils::replace_exe(&moonup_shim_exe, &dest)?;
        }
    }

    // Build core library
    let corelib_dir = toolchain_dir.join("lib").join("core");
    let mut cmd = Command::new(moon_exe);

    let bundle_dir_arg = if recipe.release.bundle_source_dir.unwrap_or(false) {
        "--source-dir"
    } else {
        "-C"
    };
    cmd.arg(bundle_dir_arg);
    cmd.arg(&corelib_dir);

    cmd.arg("bundle");
    cmd.arg("--all");
    cmd.env("PATH", bin_dir.display().to_string());
    tracing::debug!("running command: {:?}", cmd);
    cmd.status().into_diagnostic()?;

    Ok(())
}

fn find_bins(dir: &Path) -> miette::Result<Vec<OsString>> {
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
                .map(|t| {
                    #[cfg(target_os = "windows")]
                    {
                        t.is_file()
                    }
                    #[cfg(not(target_os = "windows"))]
                    {
                        t.is_file() || t.is_symlink()
                    }
                })
                .unwrap_or(false);

            if is_file {
                #[cfg(target_os = "windows")]
                {
                    ext.and_then(|ext| {
                        let ext = ext.to_str().unwrap_or_default();
                        ALLOWED_EXTENSIONS.contains(&ext).then_some(name)
                    })
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
            let from_latest = toolchains_dir
                .join(crate::constant::DEFAULT_CHANNEL)
                .join(dir);

            if from_latest.exists() {
                from_latest
            } else {
                let from_recipe = toolchains_dir.join(recipe.spec.install_name()).join(dir);

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

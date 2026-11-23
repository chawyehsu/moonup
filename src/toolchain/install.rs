#[cfg(target_os = "windows")]
use crate::constant::ALLOWED_EXTENSIONS;
use crate::dist_server::schema::{Component, Release};
use crate::toolchain::index::{available_channels, read_channel_index, read_component_index};
use crate::toolchain::package::populate_install;
use crate::toolchain::{InstalledToolchain, ToolchainSpec, atomic, version};
use crate::utils::exe_name;
use miette::{Context, IntoDiagnostic};
use std::env;
use std::ffi::OsString;
#[cfg(not(target_os = "windows"))]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The install recipe for performing a toolchain installation
#[derive(Debug)]
pub struct InstallRecipe {
    /// The effective toolchain spec used for installation and filesystem identity.
    ///
    /// This may be a floating channel spec (`latest`, `nightly`, or any other
    /// index-advertised channel name) or a pinned version.
    pub spec: ToolchainSpec,
    /// The original user-requested spec, retained for diagnostics.
    pub requested_spec: ToolchainSpec,
    /// The release information associated with this install recipe.
    pub release: Release,
    /// The components to install
    pub components: Vec<Component>,
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
fn heal_recovered(spec: &ToolchainSpec, link: bool) -> miette::Result<bool> {
    let Some(staged) = atomic::recover(spec)? else {
        return Ok(false);
    };

    let recipe = staged.into_recipe(spec);
    post_install(&recipe, link)?;
    atomic::acknowledge(&recipe.spec);
    Ok(true)
}

/// Resolve a toolchain spec against its channel index into an install recipe.
///
/// Returns `None` when the channel is not advertised by the main index, or no
/// host-supported release matches the spec.
async fn build_recipe(spec: &ToolchainSpec) -> miette::Result<Option<InstallRecipe>> {
    let channel = spec.channel().clone();
    let channels = available_channels().await?;

    if channels.is_empty() || !channels.iter().any(|candidate| candidate == &channel) {
        tracing::info!("channel '{}' is unavailable", channel);
        return Ok(None);
    }

    let index = read_channel_index(channel.as_str()).await?;
    let mut releases = index.releases().iter().filter(|r| r.is_host_supported());

    let release = match spec {
        ToolchainSpec::Channel(_) => releases.next_back().cloned(),
        ToolchainSpec::Version { key, .. } if version::is_stable_selector(key) => {
            version::resolve_stable_selector(key, releases).cloned()
        }
        ToolchainSpec::Version { key, .. } => releases.find(|&r| r.key() == key.as_str()).cloned(),
    };

    let Some(release) = release else {
        tracing::debug!("no release available for requested spec: {}", spec);
        return Ok(None);
    };

    let components = read_component_index(channel.as_str(), &release)
        .await?
        .components()
        .to_vec();
    let effective_spec = match spec {
        ToolchainSpec::Version { channel, key } if version::is_stable_selector(key) => {
            ToolchainSpec::Version {
                channel: channel.clone(),
                key: release.key().to_owned(),
            }
        }
        _ => spec.clone(),
    };
    let recipe = InstallRecipe {
        spec: effective_spec,
        requested_spec: spec.clone(),
        release,
        components,
    };

    if recipe.requested_spec != recipe.spec {
        tracing::info!(
            requested = %recipe.requested_spec,
            resolved = %recipe.spec,
            "resolved stable version selector"
        );
    }

    Ok(Some(recipe))
}

/// Synchronize the specified toolchain: heal any crash state, install
/// or update the toolchain as needed
///
/// `first_time` indicates whether this is the first time the toolchain
/// is being installed.
pub async fn sync_toolchain(spec: &ToolchainSpec, first_time: bool) -> miette::Result<()> {
    // Attempt to recover previous incomplete installation if possible
    if heal_recovered(spec, first_time)? && first_time {
        return Ok(());
    }

    let Some(recipe) = build_recipe(spec).await? else {
        if first_time {
            eprintln!("No toolchain available for requested spec '{}'", spec);
            return Err(crate::status::exit_with(1));
        }

        return Err(miette::miette!(
            "The '{}' toolchain is unavailable to update",
            spec
        ));
    };

    if first_time {
        println!("Installing toolchain '{}'", recipe.spec);
    } else {
        let install_path = spec.install_path();
        let Some(local) = InstalledToolchain::from_path(&install_path).ok() else {
            println!("The '{}' toolchain is not installed", spec);
            return Ok(());
        };

        if local.key() == recipe.release.key() {
            println!("The {} toolchain is up to date", spec);
            return Ok(());
        }

        println!("Updating the {} toolchain", spec);
    }

    populate_install(&recipe).await?;
    post_install(&recipe, first_time)?;
    atomic::acknowledge(&recipe.spec);

    if first_time {
        println!(
            "{}Installed toolchain version '{}'",
            console::style(console::Emoji("✔ ", "")).green(),
            recipe.spec
        );
        println!(
            "Make sure '{}' is added to your PATH",
            crate::moon_home().join("bin").display()
        );
    }

    Ok(())
}

/// Post installation: pour shims, build the core library, and link
/// directories if needed
fn post_install(recipe: &InstallRecipe, link: bool) -> miette::Result<()> {
    pour_shims(recipe)?;
    build_core_library(recipe)?;

    if link {
        link_dirs(recipe)?;
    }

    Ok(())
}

/// Ensure the toolchain executables and their shims are available in the
/// current `MOON_HOME`.
fn pour_shims(recipe: &InstallRecipe) -> miette::Result<()> {
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

    Ok(())
}

/// Build the core library in the installed toolchain directory.
fn build_core_library(recipe: &InstallRecipe) -> miette::Result<()> {
    let toolchain_dir = crate::moonup_home()
        .join("toolchains")
        .join(recipe.spec.install_name());
    let bin_dir = toolchain_dir.join("bin");
    let moon_exe = bin_dir.join(exe_name("moon"));

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

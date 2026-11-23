use clap::{CommandFactory, Parser};
use dialoguer::theme::ColorfulTheme;

use crate::toolchain::install::sync_toolchain;
use crate::toolchain::resolve::detect_pinned_toolchain;
use crate::toolchain::{ToolchainSpec, index};

use super::ToolchainSpecValueParser;
use miette::IntoDiagnostic;

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

    sync_toolchain(&spec, true).await?;
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

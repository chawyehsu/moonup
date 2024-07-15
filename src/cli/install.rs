use clap::Parser;

use crate::toolchain::{index::retrieve_release, package::populate_package};

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

    populate_package(release).await?;

    Ok(())
}

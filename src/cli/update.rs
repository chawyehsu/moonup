use clap::Parser;

use crate::toolchain::{ToolchainSpec, install::sync_toolchain, installed_toolchains};

/// Update MoonBit toolchains
#[derive(Parser, Debug)]
pub struct Args {}

pub async fn execute(_: Args) -> miette::Result<()> {
    let specs = installed_toolchains()?
        .into_iter()
        .filter(|toolchain| matches!(&toolchain.spec, ToolchainSpec::Channel(_)))
        .map(|toolchain| toolchain.spec)
        .collect::<Vec<_>>();

    if specs.is_empty() {
        println!("No channel toolchains installed");
        println!("Run 'moonup install <channel>' to install one");
        return Ok(());
    }

    let mut failed = false;
    for spec in &specs {
        let _ = sync_toolchain(spec, false).await.inspect_err(|e| {
            eprintln!("{e}");
            failed = true;
        });
    }

    if failed {
        return Err(crate::status::exit_with(1));
    }

    Ok(())
}

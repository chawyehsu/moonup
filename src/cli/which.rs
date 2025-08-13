use clap::Parser;

use crate::toolchain::resolve::{detect_active_toolchain, resolve_exe};

/// Show the actual binary that will be run for a given command
#[derive(Parser, Debug)]
#[clap(arg_required_else_help = true)]
pub struct Args {
    /// The command to inspect
    command: String,
}

pub async fn execute(args: Args) -> miette::Result<()> {
    let mut active_toolchain = detect_active_toolchain();
    active_toolchain.push("bin");

    let exe_resolved = resolve_exe(args.command.as_str(), &active_toolchain);
    match exe_resolved {
        None => eprintln!("No command found for '{}'", args.command),
        Some(exe) => println!("{}", exe.display()),
    }

    Ok(())
}

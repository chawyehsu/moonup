use clap::Parser;

use crate::{
    runner,
    toolchain::{ToolchainSpec, resolve::detect_active_toolchainspec},
};

/// Show the actual binary that will be run for a given command
#[derive(Parser, Debug)]
#[clap(arg_required_else_help = true)]
pub struct Args {
    /// The command to inspect
    command: String,
}

fn format_command(cmd: &std::process::Command) -> String {
    let mut parts = vec![cmd.get_program().to_string_lossy().into_owned()];
    parts.extend(cmd.get_args().map(|arg| arg.to_string_lossy().into_owned()));
    parts.join(" ")
}

pub async fn execute(args: Args) -> miette::Result<()> {
    let active_toolchain = ToolchainSpec::from(detect_active_toolchainspec());

    match runner::build_command(active_toolchain, vec![args.command.as_str()]) {
        Ok(cmd) => println!("{}", format_command(&cmd)),
        Err(err) if err.to_string().starts_with("Command '") => {
            eprintln!("No command found for '{}'", args.command)
        }
        Err(err) => return Err(miette::miette!(err.to_string())),
    }

    Ok(())
}

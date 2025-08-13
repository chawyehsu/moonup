use clap::{Parser, ValueHint};

use crate::{runner, toolchain::ToolchainSpec};

/// Run a command with a specific toolchain
#[derive(Parser, Debug)]
#[clap(arg_required_else_help = true, trailing_var_arg = true)]
pub struct Args {
    /// The toolchain to use for running the command
    toolchain: ToolchainSpec,

    /// The command to run, with arguments if any
    #[clap(required = true, num_args = 1.., value_hint = ValueHint::CommandWithArguments)]
    command: Vec<String>,
}

pub async fn execute(args: Args) -> miette::Result<()> {
    let mut cmd = runner::build_command(args.toolchain, args.command)
        .map_err(|e| miette::miette!("Failed to build command: {}", e))?;

    match cmd.status() {
        Err(e) => Err(miette::miette!("Failed to run command: {}", e)),
        Ok(status) if status.success() => Ok(()),
        Ok(status) => std::process::exit(status.code().unwrap_or(1)),
    }
}

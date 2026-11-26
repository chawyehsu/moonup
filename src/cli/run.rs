use clap::{Parser, ValueHint};

use crate::{runner, session::Session, status, toolchain::ToolchainSpec};

use super::ToolchainSpecValueParser;

/// Run a command with a specific toolchain
#[derive(Parser, Debug)]
#[clap(arg_required_else_help = true, trailing_var_arg = true)]
pub struct Args {
    /// The toolchain to use for running the command
    #[clap(value_parser = ToolchainSpecValueParser::new())]
    toolchain: ToolchainSpec,

    /// The command to run, with arguments if any
    #[clap(required = true, num_args = 1.., value_hint = ValueHint::CommandWithArguments)]
    command: Vec<String>,
}

pub async fn execute(args: Args, session: &Session) -> miette::Result<()> {
    let mut cmd = runner::build_command(session.layout(), args.toolchain, args.command)
        .map_err(|e| miette::miette!("Failed to build command: {}", e))?;

    match cmd.status() {
        Err(e) => Err(miette::miette!("Failed to run command: {}", e)),
        Ok(exit_status) if exit_status.success() => Ok(()),
        Ok(exit_status) => Err(status::exit_with(exit_status.code().unwrap_or(1))),
    }
}

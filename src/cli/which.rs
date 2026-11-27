use clap::Parser;

use crate::{runner, session::Session, toolchain::resolve::detect_active_toolchain};

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

pub async fn execute(args: Args, session: &Session) -> miette::Result<()> {
    let spec = detect_active_toolchain(session.layout()).map_err(|error| miette::miette!(error))?;

    match runner::build_command(session.layout(), spec, vec![args.command.as_str()]) {
        Ok(cmd) => println!("{}", format_command(&cmd)),
        Err(err) if err.to_string().starts_with("Command '") => {
            eprintln!("No command found for '{}'", args.command)
        }
        Err(err) => return Err(miette::miette!(err.to_string())),
    }

    Ok(())
}

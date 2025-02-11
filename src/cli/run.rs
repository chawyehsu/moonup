use clap::{Parser, ValueHint};
use miette::IntoDiagnostic;
use std::{env, process::Command};

use crate::toolchain::ToolchainSpec;

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
    let name = args.command[0].as_str();
    let is_exe_moon = name == "moon";

    let command_name = {
        #[cfg(target_os = "windows")]
        {
            if !name.ends_with(".exe") {
                format!("{}.exe", name)
            } else {
                name.to_string()
            }
        }

        #[cfg(not(target_os = "windows"))]
        name.to_string()
    };

    let mut exe = args.toolchain.install_path();

    if !exe.exists() {
        return Err(miette::miette!(
            "Toolchain '{}' is not installed",
            args.toolchain
        ));
    }

    exe.push("bin");
    exe.push(command_name);

    let mut cmd = Command::new(&exe);
    cmd.args(&args.command[1..]);

    if is_exe_moon {
        let mut libcore = exe.clone();
        libcore.pop();
        libcore.pop();
        libcore.push("lib");
        libcore.push("core");

        // Override the core standard library path to point to the one in
        // the active toolchain.
        // NOTE(chawyehsu): The `MOON_CORE_OVERRIDE` env is undocumented on
        // MoonBit's official documentation. I reverse-engineered this from
        // the `moon` executable. This is a hacky way to make things work
        // and may not work as MoonBit evolves.
        cmd.env(
            "MOON_CORE_OVERRIDE",
            env::var_os("MOON_CORE_OVERRIDE").unwrap_or(libcore.into_os_string()),
        );
    }

    tracing::debug!("running command: {:?}", cmd);
    cmd.status().into_diagnostic()?;

    Ok(())
}

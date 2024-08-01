use clap::Parser;

use crate::toolchain::resolve::detect_active_toolchain;

/// Show the actual binary that will be run for a given command
#[derive(Parser, Debug)]
#[clap(arg_required_else_help = true)]
pub struct Args {
    /// The command to inspect
    command: String,
}

pub async fn execute(args: Args) -> miette::Result<()> {
    let command_name = {
        let name = args.command.as_str();

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
    let mut active_toolchain = detect_active_toolchain();

    active_toolchain.push("bin");
    active_toolchain.push(command_name);

    if active_toolchain.exists() {
        println!("{}", active_toolchain.display());
    } else {
        eprintln!("No command for '{}'", active_toolchain.display());
    }

    Ok(())
}

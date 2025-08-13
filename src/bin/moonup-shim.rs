use anyhow::Result;
use std::env;
use std::ffi::OsString;
use std::path::PathBuf;
use std::process::{Command, ExitStatus};

use moonup::constant::RECURSION_LIMIT;
use moonup::toolchain::resolve::detect_active_toolchainspec;
use moonup::toolchain::ToolchainSpec;
use moonup::{moonup_home, runner};

pub fn main() {
    match run() {
        Err(err) => {
            eprintln!("Error: {err}");
            std::process::exit(1);
        }
        Ok(status) => {
            std::process::exit(status.code().unwrap_or(1));
        }
    }
}

fn run() -> Result<ExitStatus> {
    // Ensure recursion guard is at the top of the function
    let recursion_count = recursion_guard()?;

    let args = env::args_os().collect::<Vec<_>>();
    let current_exe = env::current_exe()
        .unwrap_or_else(|_| PathBuf::from(&args[0]))
        .with_extension(""); // ensure `.exe` is removed
    let current_exe_name = current_exe
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();

    if current_exe_name.is_empty() {
        return Err(anyhow::anyhow!("Unexpected bad shim name"));
    }

    if current_exe_name == "moonup-shim" {
        return Err(anyhow::anyhow!("Cannot run moonup-shim directly"));
    }

    let args_1 = args.get(1).and_then(|arg| arg.to_str());
    let args_1_is_toolchain = args_1.map_or(false, |arg| arg.starts_with('+'));

    // Find the active toolchain:
    // - If the first argument is a toolchain spec, use it.
    // - If the `MOONUP_TOOLCHAIN_SPEC` environment variable is set, use it.
    // - Otherwise, detect the active toolchain.
    let active_toolchain = if args_1_is_toolchain {
        let version = args_1.expect("has arg version").strip_prefix('+').unwrap();
        version.to_string()
    } else if let Some(toolchain_spec) = env::var_os("MOONUP_TOOLCHAIN_SPEC") {
        toolchain_spec
            .to_str()
            .expect("MOONUP_TOOLCHAIN_SPEC should be valid UTF-8")
            .to_string()
    } else {
        detect_active_toolchainspec()
    };

    let active_toolchain_root = moonup_home().join("toolchains").join(&active_toolchain);

    // If the active toolchain is not installed, call `moonup install`
    // to install it.
    if !active_toolchain_root.exists() {
        let version = active_toolchain_root
            .file_name()
            .and_then(|v| v.to_str())
            .expect("should get active toolchain version");

        println!("toolchain version '{version}' not installed");

        let mut cmd = Command::new("moonup");
        cmd.args(["install", version]);

        match cmd.status() {
            Err(e) => return Err(anyhow::anyhow!("Failed to run moonup install: {}", e)),
            Ok(status) if !status.success() => {
                return Err(anyhow::anyhow!("Failed to install active toolchain"));
            }
            Ok(_) => {}
        }
    }

    if current_exe_name == "moon" {
        // intercept `moon upgrade` and proxy it to `moonup upgrade`
        if args.len() > 1 && args[1] == "upgrade" {
            let mut cmd = Command::new("moonup");
            cmd.args(["update"]);

            return cmd
                .status()
                .map_err(|e| anyhow::anyhow!("Failed to run moonup upgrade: {}", e));
        }
    }

    let spec = ToolchainSpec::from(active_toolchain.as_str());
    let mut run_args = vec![OsString::from(current_exe_name)];

    let idx = if args_1_is_toolchain { 2 } else { 1 };
    run_args.extend(args[idx..].iter().cloned());

    let mut cmd = runner::build_command(spec, run_args)?;
    cmd.env("MOONUP_RECURSION_COUNT", (recursion_count + 1).to_string());

    cmd.status().map_err(anyhow::Error::from)
}

fn recursion_guard() -> Result<u8> {
    let recursion_count = env::var("MOONUP_RECURSION_COUNT")
        .map(|var| var.parse::<u8>().unwrap_or(0u8))
        .unwrap_or(0u8);

    if recursion_count > RECURSION_LIMIT {
        return Err(anyhow::anyhow!("Infinite recursion detected"));
    }

    Ok(recursion_count)
}

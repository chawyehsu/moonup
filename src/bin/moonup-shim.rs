use anyhow::Result;
use std::env;
use std::path::PathBuf;
use std::process::{Command, ExitStatus};

use moonup::constant::RECURSION_LIMIT;
use moonup::toolchain::resolve::detect_active_toolchain;
use moonup::{moon_home, moonup_home};

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
    let active_toolchain_root = if args_1_is_toolchain {
        let version = args_1.expect("has arg version").strip_prefix('+').unwrap();
        moonup_home().join("toolchains").join(version)
    } else if let Some(toolchain_spec) = env::var_os("MOONUP_TOOLCHAIN_SPEC") {
        moonup_home().join("toolchains").join(toolchain_spec)
    } else {
        detect_active_toolchain()
    };

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

    let exe_relative_path = current_exe
        .strip_prefix(moon_home())
        .expect("should extract exe path");

    let actual_exe = active_toolchain_root.join(exe_relative_path);
    // println!("Running '{}'", actual_exe.display());
    let actual_libcore = active_toolchain_root.join("lib").join("core");

    let mut cmd = Command::new(actual_exe);
    let idx = if args_1_is_toolchain { 2 } else { 1 };
    cmd.args(args[idx..].iter().cloned());
    cmd.env("MOONUP_RECURSION_COUNT", (recursion_count + 1).to_string());

    // NOTE(chawyehsu): It is not ideal and hacky to store the toolchain spec
    // in an extra environment variable, but it is the only way to spread the
    // toolchain spec to the child shim processes without requiring upstream
    // changes in MoonBit build system... All of these are because of and for
    // the weak isolation of the MoonBit toolchain...
    cmd.env(
        "MOONUP_TOOLCHAIN_SPEC",
        env::var_os("MOONUP_TOOLCHAIN_SPEC").unwrap_or(
            active_toolchain_root
                .file_name()
                .expect("should have toolchain version")
                .to_os_string(),
        ),
    );

    if current_exe_name == "moon" {
        // intercept `moon upgrade` and proxy it to `moonup upgrade`
        if args.len() > 1 && args[1] == "upgrade" {
            let mut cmd = Command::new("moonup");
            cmd.args(["update"]);

            return cmd
                .status()
                .map_err(|e| anyhow::anyhow!("Failed to run moonup upgrade: {}", e));
        }

        // Override the core standard library path to point to the one in
        // the active toolchain.
        // NOTE(chawyehsu): The `MOON_CORE_OVERRIDE` env is undocumented on
        // MoonBit's official documentation. I reverse-engineered this from
        // the `moon` executable. This is a hacky way to make things work
        // and may not work as MoonBit evolves.
        cmd.env(
            "MOON_CORE_OVERRIDE",
            env::var_os("MOON_CORE_OVERRIDE").unwrap_or(actual_libcore.into_os_string()),
        );
    }

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

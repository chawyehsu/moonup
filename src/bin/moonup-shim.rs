use anyhow::Result;
use std::env;
use std::path::PathBuf;
use std::process::Command;

use moonup::constant::RECURSION_LIMIT;
use moonup::toolchain::resolve::detect_active_toolchain;

pub fn main() {
    if let Err(err) = run() {
        eprintln!("{err:?}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    // Ensure recursion guard is at the top of the function
    let recursion_count = recursion_guard()?;

    let args = env::args_os().collect::<Vec<_>>();
    let current_exe = env::current_exe()
        .unwrap_or_else(|_| PathBuf::from(&args[0]))
        .with_extension("");
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

    let active_toolchain_root = detect_active_toolchain();

    // If the active toolchain is not installed, call `moonup install`
    // to install it.
    if !active_toolchain_root.exists() {
        let version = active_toolchain_root
            .file_name()
            .and_then(|v| v.to_str())
            .expect("Cannot get active toolchain version");

        println!("Active toolchain version '{}' not installed", version);

        let mut cmd = Command::new("moonup")
            .args(["install", version])
            .spawn()
            .map_err(|e| anyhow::anyhow!("Failed to spawn moonup install: {}", e))?;

        let code = cmd.wait()?;
        if !code.success() {
            return Err(anyhow::anyhow!("Failed to install active toolchain"));
        }
    }

    let actual_exe = active_toolchain_root.join("bin").join(current_exe_name);
    let actual_libcore = active_toolchain_root.join("lib").join("core");

    let mut cmd = Command::new(actual_exe);
    cmd.args(args[1..].iter().cloned());
    cmd.env("MOONUP_RECURSION_COUNT", (recursion_count + 1).to_string());

    if current_exe_name == "moon" {
        // intercept `moon upgrade` and proxy it to `moonup upgrade`
        if args.len() > 1 && args[1] == "upgrade" {
            let mut cmd = Command::new("moonup")
                .args(["update", "--no-self-update"])
                .spawn()
                .map_err(|e| anyhow::anyhow!("Failed to spawn moonup upgrade: {}", e))?;
            let code = cmd.wait()?;
            return code
                .success()
                .then(|| Ok(()))
                .unwrap_or_else(|| Err(anyhow::anyhow!("Failed to upgrade toolchains: {}", code)));
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

    Ok(cmd.status().map(|_| ())?)
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

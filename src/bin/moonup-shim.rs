use anyhow::Result;
use std::env;
use std::path::PathBuf;
use std::process::Command;

use moonup::constant::RECURSION_LIMIT;
use moonup::utils::detect_active_toolchain;

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
    let current_exe = env::current_exe().unwrap_or_else(|_| PathBuf::from(&args[0]));
    let current_exe_name = current_exe
        .file_name()
        .map(|name| name.to_string_lossy().to_ascii_lowercase());

    let shim_name = current_exe_name.unwrap_or_default();
    let is_shim_moon = shim_name == "moon";

    if shim_name.is_empty() {
        return Err(anyhow::anyhow!("bad shim name"));
    }

    let shim_itself = {
        #[cfg(target_os = "windows")]
        {
            shim_name == "moonup-shim.exe"
        }
        #[cfg(not(target_os = "windows"))]
        {
            shim_name == "moonup-shim"
        }
    };

    if shim_itself {
        return Err(anyhow::anyhow!("cannot run moonup-shim directly"));
    }

    let active_toolchain_root = detect_active_toolchain();

    // If the active toolchain is not installed, call `moonup install`
    // to install it.
    if !active_toolchain_root.exists() {
        let version = active_toolchain_root
            .file_name()
            .map(|v| v.to_string_lossy())
            .expect("Cannot get active toolchain version");

        println!("Active toolchain version '{}' not installed", version);

        let mut cmd = Command::new("moonup")
            .args(["install", version.as_ref()])
            .spawn()
            .map_err(|e| anyhow::anyhow!("Failed to spawn moonup install: {}", e))?;

        let code = cmd.wait()?;
        if !code.success() {
            return Err(anyhow::anyhow!("Failed to install active toolchain"));
        }
    }

    let actual_exe = active_toolchain_root.join("bin").join(&shim_name);
    let actual_libcore = active_toolchain_root.join("lib").join("core");

    let mut cmd = Command::new(actual_exe);
    cmd.args(args[1..].iter().cloned());
    cmd.env("MOONUP_RECURSION_COUNT", (recursion_count + 1).to_string());

    if is_shim_moon {
        // Override the core standard library path to point to the one in
        // the active toolchain.
        // NOTE(chawyehsu): The `MOON_CORE_OVERRIDE` env is undocumented on
        // MoonBit's official documentation. I reverse-engineered this from
        // the `moon` executable. This is a hacky way to make things work
        // and may not work as MoonBit evolves.
        let env_moon_core_override = env::var("MOON_CORE_OVERRIDE")
            .ok()
            .unwrap_or(actual_libcore.to_string_lossy().to_string());
        cmd.env("MOON_CORE_OVERRIDE", env_moon_core_override);
    }

    Ok(cmd.status().map(|_| ())?)
}

fn recursion_guard() -> Result<u8> {
    let recursion_count = env::var("MOONUP_RECURSION_COUNT")
        .map(|var| var.parse::<u8>().unwrap_or(0u8))
        .unwrap_or(0u8);

    if recursion_count > RECURSION_LIMIT {
        return Err(anyhow::anyhow!("recursion limit reached"));
    }

    Ok(recursion_count)
}

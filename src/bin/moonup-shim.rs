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
    let mut recursion_count = recursion_guard()?;
    recursion_count += 1;

    let args = env::args_os().collect::<Vec<_>>();
    let current_exe = env::current_exe().unwrap_or_else(|_| PathBuf::from(&args[0]));
    let current_exe_name = current_exe
        .file_name()
        .map(|name| name.to_string_lossy().to_ascii_lowercase());

    let shim_name = current_exe_name.unwrap_or_default();

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

    let toolchain_root = detect_active_toolchain();
    let actual_exe = toolchain_root.join("bin").join(&shim_name);

    let mut cmd = Command::new(actual_exe);
    cmd.args(args[1..].iter().cloned());
    cmd.env("MOONUP_RECURSION_COUNT", recursion_count.to_string());

    Ok(cmd.status().map(|_| ())?)
}

fn recursion_guard() -> Result<u8> {
    let recursion_count = env::var("MOONUP_RECURSION_COUNT")
        .map(|var| var.parse::<u8>().unwrap_or(1u8))
        .unwrap_or(1u8);

    if recursion_count > RECURSION_LIMIT {
        return Err(anyhow::anyhow!("recursion limit reached"));
    }

    Ok(recursion_count)
}

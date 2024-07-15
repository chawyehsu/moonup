use anyhow::Result;
use std::env;
use std::path::PathBuf;
use std::process::Command;

use moonup::constant::RECURSION_LIMIT;
use moonup::utils::detect_toolchain_version;

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

    if let None = current_exe_name {
        eprintln!("can't get shim executable name");
        std::process::exit(1);
    }

    let shim_name = current_exe_name.unwrap();
    let toolchain_root = detect_toolchain_version();
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

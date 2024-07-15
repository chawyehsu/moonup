use anyhow::Result;
use std::env;
use std::path::PathBuf;
use std::process::Command;

use moonup::constant::{RECURSION_LIMIT, TOOLCHAIN_FILE};
use moonup::moonup_home;

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

/// Iterates over the current directory and all its parent directories
/// to find if there is a `MOON_TOOLCHAIN_SPECIFIER` and detect the
/// toolchain version.
///
/// # Returns
///
/// The path to actual versioned toolchain
fn detect_toolchain_version() -> PathBuf {
    let current_dir = env::current_dir().expect("can't access current directory");

    let version = std::iter::successors(Some(current_dir.as_path()), |prev| prev.parent())
        .find_map(|dir| {
            let path = dir.join(TOOLCHAIN_FILE);
            if path.is_file() {
                let version = std::fs::read_to_string(&path)
                    .expect(&format!("can't read {}", TOOLCHAIN_FILE));

                Some(version.trim().to_string())
            } else {
                Some("latest".to_string())
            }
        });

    moonup_home()
        .join("toolchains")
        .join(version.unwrap_or_else(|| "latest".to_string()))
}

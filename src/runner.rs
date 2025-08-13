use std::env;
use std::ffi::OsStr;
use std::process::Command;

use crate::toolchain::{resolve, ToolchainSpec};

pub fn build_command<S: AsRef<OsStr>>(
    toolchain: ToolchainSpec,
    command: Vec<S>,
) -> anyhow::Result<Command> {
    let exe_name = command[0].as_ref();
    let is_exe_moon = exe_name == "moon";

    let mut bin_dir = toolchain.install_path();
    bin_dir.push("bin");

    if !bin_dir.exists() {
        return Err(anyhow::anyhow!("Toolchain '{toolchain}' is not installed"));
    }

    let err_msg = anyhow::anyhow!(
        "Command '{}' not found in toolchain '{toolchain}'",
        exe_name.to_string_lossy()
    );

    let exe_resolved = resolve::resolve_exe(exe_name, &bin_dir).ok_or(err_msg)?;

    let mut cmd = Command::new(&exe_resolved);
    cmd.args(&command[1..]);

    // NOTE(chawyehsu): It is not ideal and hacky to store the toolchain spec
    // in an extra environment variable, but it is the only way to spread the
    // toolchain spec to the child shim processes without requiring upstream
    // changes in MoonBit build system... All of these are because of and for
    // the weak isolation of the MoonBit toolchain...
    let spec = std::ffi::OsStr::new(toolchain.as_str());
    cmd.env(
        "MOONUP_TOOLCHAIN_SPEC",
        env::var_os("MOONUP_TOOLCHAIN_SPEC").unwrap_or(spec.into()),
    );

    if is_exe_moon {
        let mut libcore = bin_dir.clone();
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

    tracing::debug!("build command: {:?}", cmd);
    Ok(cmd)
}

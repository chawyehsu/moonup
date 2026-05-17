use std::env;
use std::ffi::OsStr;
use std::process::Command;

use crate::toolchain::{ToolchainSpec, resolve};

pub fn build_command<S: AsRef<OsStr>>(
    toolchain: ToolchainSpec,
    command: Vec<S>,
) -> anyhow::Result<Command> {
    let exe_name = command[0].as_ref();

    let mut bin_dir = toolchain.install_path();
    bin_dir.push("bin");

    if !bin_dir.exists() {
        return Err(anyhow::anyhow!("Toolchain '{toolchain}' is not installed"));
    }

    let err_msg = anyhow::anyhow!(
        "Command '{}' not found in toolchain '{toolchain}'",
        exe_name.to_string_lossy()
    );

    let internal_bin_dir = bin_dir.join("internal");

    let paths = env::join_paths([&bin_dir, &internal_bin_dir])
        .map_err(|e| anyhow::anyhow!("Failed to build PATH environment variable: {}", e))?;

    let mut cmd = if let Some(exe_resolved) = resolve::resolve_exe(exe_name, &paths) {
        tracing::debug!(
            "Resolved executable for '{}': {}",
            exe_name.to_string_lossy(),
            exe_resolved.display()
        );
        Command::new(exe_resolved)
    } else if exe_name == "moon-lsp" {
        // LSP delegation special case
        let host_paths = std::env::var_os("PATH")
            .ok_or(anyhow::anyhow!("Failed to get PATH environment variable"))?;

        // `moonbit-lsp` and `lsp-server.js` are JS executables, so invoke
        // them with a JS runtime.
        // NOTE(chawyehsu): availability listed below,
        // - `moon-lsp`, version 0.9.2+bbe2b338f onwards
        // - `moonbit-lsp`, 0.6.23+906028000 ~ 0.9.1+cd5b07232
        // - `lsp-server.js`, version 0.6.22 and earlier
        let lsp_exe = resolve::resolve_exe("moonbit-lsp", &paths)
            .or_else(|| resolve::resolve_file("moonbit-lsp", &paths))
            .or_else(|| resolve::resolve_file("lsp-server.js", &paths))
            .ok_or(err_msg)?;
        tracing::debug!("Resolved LSP server executable: {}", lsp_exe.display());
        let runtime = resolve::resolve_exe("bun", &host_paths)
            .or_else(|| resolve::resolve_exe("node", &host_paths))
            .ok_or(anyhow::anyhow!(
                "No JavaScript runtime ('bun' or 'node') found for running LSP server"
            ))?;
        tracing::debug!(
            "Resolved JavaScript runtime for LSP server: {}",
            runtime.display()
        );

        let mut cmd = Command::new(runtime);
        cmd.arg(lsp_exe);

        // Set `MOON_HOME` to the root of the active toolchain for `moonbit-lsp`
        // NOTE(chawyehsu): see
        //  https://github.com/chawyehsu/moonup/issues/7#issuecomment-3571190037
        // Hope `moonbit-lsp` doesn't call other bins, otherwise it'll cause chaos
        // because of this ...
        let mut toolchain_root = bin_dir.clone();
        toolchain_root.pop();
        cmd.env("MOON_HOME", toolchain_root.into_os_string());

        cmd
    } else {
        return Err(err_msg);
    };

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

    if exe_name == "moon" {
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

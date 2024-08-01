pub mod index;
pub mod package;
pub mod resolve;

pub struct InstalledToolchain {
    /// Whether the installed toolchain is marked as default
    pub default: bool,

    /// Whether this toolchain is installed as 'latest'
    pub latest: bool,

    /// The actual version of the installed toolchain
    pub version: String,
}

pub fn installed_toolchains() -> miette::Result<Vec<InstalledToolchain>> {
    let toolchains_dir = crate::moonup_home().join("toolchains");
    let default_version = resolve::detect_default_version();

    let toolchains = match toolchains_dir.read_dir() {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => vec![],
        Err(e) => return Err(miette::miette!(e).wrap_err("Failed to read toolchains directory")),
        Ok(read_dir) => read_dir
            .filter_map(std::io::Result::ok)
            .filter_map(|e| {
                let path = e.path();
                let version = path.file_name().map(|n| {
                    let n = n.to_ascii_lowercase();
                    let latest = n == "latest";
                    let default = match default_version.as_deref() {
                        Some(v) => v == n,
                        None => false,
                    };
                    let version = match latest {
                        false => n.to_string_lossy().to_string(),
                        true => std::fs::read_to_string(path.join("version"))
                            .ok()
                            .map(|s| s.trim().to_string())
                            .unwrap_or_else(|| "latest".to_string()),
                    };

                    InstalledToolchain {
                        default,
                        latest,
                        version,
                    }
                });

                version
            })
            .collect::<Vec<_>>(),
    };

    Ok(toolchains)
}

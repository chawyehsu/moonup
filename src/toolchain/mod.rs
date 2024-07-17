use miette::IntoDiagnostic;

pub mod index;
pub mod package;

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
    let default_version = default_toolchain();

    let toolchains = toolchains_dir
        .read_dir()
        .into_diagnostic()?
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
        .collect::<Vec<_>>();

    Ok(toolchains)
}

fn default_toolchain() -> Option<String> {
    let default_file = crate::moonup_home().join("default");

    std::fs::read_to_string(default_file).ok().and_then(|s| {
        let v = s.trim().to_string();
        if v.is_empty() {
            None
        } else {
            Some(v)
        }
    })
}

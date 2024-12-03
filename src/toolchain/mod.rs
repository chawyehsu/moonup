use std::path::Path;

pub mod index;
pub mod package;
pub mod resolve;

pub struct InstalledToolchain {
    /// Whether this toolchain is installed as 'latest'
    pub latest: bool,

    /// The actual version of the installed toolchain
    pub version: String,
}

impl InstalledToolchain {
    pub fn from_path(path: &Path) -> miette::Result<Self> {
        let n = path
            .file_name()
            .map(|n| n.to_ascii_lowercase().to_string_lossy().to_string())
            .ok_or_else(|| miette::miette!("Failed to get toolchain version"))?;
        let latest = n == "latest";
        let version = match latest {
            false => n,
            true => std::fs::read_to_string(path.join("version"))
                .ok()
                .map(|s| s.trim().to_string())
                .unwrap_or_else(|| "latest".to_string()),
        };

        Ok(Self { latest, version })
    }
}

pub fn installed_toolchains() -> miette::Result<Vec<InstalledToolchain>> {
    let toolchains_dir = crate::moonup_home().join("toolchains");

    let toolchains = match toolchains_dir.read_dir() {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => vec![],
        Err(e) => return Err(miette::miette!(e).wrap_err("Failed to read toolchains directory")),
        Ok(read_dir) => {
            let mut t = read_dir
                .filter_map(std::io::Result::ok)
                .filter_map(|e| InstalledToolchain::from_path(&e.path()).ok())
                .collect::<Vec<_>>();
            t.sort_by_key(|t| t.version.clone());
            t
        }
    };

    Ok(toolchains)
}

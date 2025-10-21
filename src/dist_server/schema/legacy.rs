use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Index {
    pub core: Properties,
    pub darwin_arm64: Properties,
    pub darwin_x64: Properties,
    pub linux_x64: Properties,
    pub win_x64: Properties,
}

#[derive(Debug, Deserialize)]
pub struct Properties {
    pub last_modified: String,
    pub releases: Vec<Release>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Release {
    pub version: String,
    pub name: String,
    pub sha256: String,
}

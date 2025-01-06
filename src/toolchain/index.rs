use std::path::{Path, PathBuf};

use chrono::{DateTime, Duration, Local};
use miette::{Context, IntoDiagnostic};
use serde::Deserialize;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use url::Url;

use crate::{
    constant,
    utils::{build_http_client, url_to_reader},
};

use super::ToolchainSpec;

/// The main index
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Index {
    pub version: u8,
    pub last_modified: String,
    pub channels: Vec<Channel>,
    pub targets: Vec<Target>,
}

#[derive(Debug, Deserialize)]
pub struct Channel {
    pub name: ChannelName,
    pub version: String,
    pub date: Option<String>,
}

impl std::fmt::Display for Channel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.version, self.name)
    }
}

#[derive(Debug, Deserialize, Clone, Copy, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum ChannelName {
    Latest,
    Nightly,
}

impl std::fmt::Display for ChannelName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ChannelName::Latest => write!(f, "latest"),
            ChannelName::Nightly => write!(f, "nightly"),
        }
    }
}

#[derive(Debug, Deserialize, Clone, Copy, PartialEq)]
pub enum Target {
    #[serde(rename = "aarch64-apple-darwin")]
    Aarch64MacOS,
    #[serde(rename = "x86_64-apple-darwin")]
    Amd64MacOS,
    #[serde(rename = "x86_64-unknown-linux")]
    Amd64Linux,
    #[serde(rename = "x86_64-pc-windows")]
    Amd64Windows,
}

impl Target {
    pub fn from_host() -> miette::Result<Self> {
        match std::env::consts::OS {
            "macos" => match std::env::consts::ARCH {
                "aarch64" => Ok(Target::Aarch64MacOS),
                "x86_64" => Ok(Target::Amd64MacOS),
                _ => Err(miette::miette!("unsupported architecture")),
            },
            "linux" => Ok(Target::Amd64Linux),
            "windows" => Ok(Target::Amd64Windows),
            _ => Err(miette::miette!("unsupported platform")),
        }
    }
}

impl std::fmt::Display for Target {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Target::Aarch64MacOS => write!(f, "aarch64-apple-darwin"),
            Target::Amd64MacOS => write!(f, "x86_64-apple-darwin"),
            Target::Amd64Linux => write!(f, "x86_64-unknown-linux"),
            Target::Amd64Windows => write!(f, "x86_64-pc-windows"),
        }
    }
}

/// The channel index
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChannelIndex {
    pub version: u8,
    pub last_modified: String,
    pub releases: Vec<Release>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Release {
    /// The (compiler) version number of the release
    pub version: String,
    /// Flag to indicate if the distribution layout of the release is version 1
    pub layout_version1: Option<bool>,
    /// The (nightly build) date of the release
    pub date: Option<String>,
}

/// The component index
#[derive(Debug, Deserialize)]
pub struct ComponentIndex {
    pub version: u8,
    pub components: Vec<Component>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Component {
    pub name: String,
    pub file: String,
    pub sha256: String,
}

/// The install recipe for performing a toolchain installation
#[derive(Debug)]
pub struct InstallRecipe {
    /// The requested toolchain spec
    pub spec: ToolchainSpec,
    /// The release information
    pub release: Release,
    /// The components to install
    pub components: Vec<Component>,
}

/// Read the main index
pub async fn read_index() -> miette::Result<Index> {
    let index_filename = "index.json";
    let mut index_file = crate::moonup_home();
    index_file.push("downloads");
    index_file.push(index_filename);

    let (cache_valid, mut content) = match read_json_with_lock(&index_file).await {
        Ok((cache_valid, content)) => (cache_valid, content),
        Err(e) => {
            tracing::debug!("failed to read index json: {}", e);
            (false, "".to_string())
        }
    };

    if cache_valid {
        return serde_json::from_str(&content)
            .into_diagnostic()
            .inspect_err(|e| {
                tracing::info!("malformed index json: {}", e);
                let _ = std::fs::remove_file(&index_file);
            });
    }

    let main_index_url =
        Url::parse(format!("{}/{}", constant::MOONUP_DIST_SERVER, index_filename).as_str())
            .into_diagnostic()?;

    content.clear();

    let mut reader = url_to_reader(main_index_url, build_http_client(), None).await?;
    reader
        .read_to_string(&mut content)
        .await
        .into_diagnostic()?;

    let index = serde_json::from_str(&content)
        .into_diagnostic()
        .wrap_err("malformed index json")?;

    write_json_with_lock(&index_file, content.as_bytes()).await?;

    Ok(index)
}

/// Read the channel index
pub async fn read_channel_index(channel: ChannelName) -> miette::Result<ChannelIndex> {
    let channel_index_filename = format!("channel-{}.json", channel);

    let mut channel_index_file = crate::moonup_home();
    channel_index_file.push("downloads");
    channel_index_file.push(&channel_index_filename);

    let (cache_valid, mut content) = match read_json_with_lock(&channel_index_file).await {
        Ok((cache_valid, content)) => (cache_valid, content),
        Err(e) => {
            tracing::debug!("failed to read channel index json: {}", e);
            (false, "".to_string())
        }
    };

    if cache_valid {
        return serde_json::from_str(&content)
            .into_diagnostic()
            .inspect_err(|e| {
                tracing::info!("malformed channel index json: {}", e);
                let _ = std::fs::remove_file(&channel_index_file);
            });
    }

    let channel_index_url = Url::parse(
        format!(
            "{}/{}",
            constant::MOONUP_DIST_SERVER,
            channel_index_filename
        )
        .as_str(),
    )
    .into_diagnostic()?;

    content.clear();

    let mut reader = url_to_reader(channel_index_url, build_http_client(), None).await?;
    reader
        .read_to_string(&mut content)
        .await
        .into_diagnostic()?;

    let index = serde_json::from_str(&content)
        .into_diagnostic()
        .wrap_err("malformed channel index json")?;

    write_json_with_lock(&channel_index_file, content.as_bytes()).await?;

    Ok(index)
}

/// Read the component index
pub async fn read_component_index(release: &Release) -> miette::Result<ComponentIndex> {
    let host_target = Target::from_host()?;
    let component_index_filename = format!("{}.json", host_target);

    let mut component_index_file = crate::moonup_home();
    component_index_file.push("downloads");

    match &release.date {
        Some(date) => {
            component_index_file.push("nightly");
            component_index_file.push(date);
        }
        None => component_index_file.push(&release.version),
    }

    component_index_file.push(&component_index_filename);

    let (cache_valid, mut content) = match read_json_with_lock(&component_index_file).await {
        Ok((cache_valid, content)) => (cache_valid, content),
        Err(e) => {
            tracing::debug!("failed to read component index json: {}", e);
            (false, "".to_string())
        }
    };

    if cache_valid {
        return serde_json::from_str(&content)
            .into_diagnostic()
            .inspect_err(|e| {
                tracing::info!("malformed component index json: {}", e);
                let _ = std::fs::remove_file(&component_index_file);
            });
    }

    let component_index_url = Url::parse(
        {
            match &release.date {
                Some(date) => format!(
                    "{}/nightly/{}/{}",
                    constant::MOONUP_DIST_SERVER,
                    date,
                    component_index_filename
                ),
                None => format!(
                    "{}/latest/{}/{}",
                    constant::MOONUP_DIST_SERVER,
                    release.version,
                    component_index_filename
                ),
            }
        }
        .as_str(),
    )
    .into_diagnostic()?;

    content.clear();

    let mut reader = url_to_reader(component_index_url, build_http_client(), None).await?;
    reader
        .read_to_string(&mut content)
        .await
        .into_diagnostic()?;

    let index = serde_json::from_str(&content)
        .into_diagnostic()
        .wrap_err("malformed component index json")?;

    write_json_with_lock(&component_index_file, content.as_bytes()).await?;

    Ok(index)
}

async fn write_json_with_lock(path: &Path, content: &[u8]) -> miette::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| miette::miette!("failed to get parent directory"))?;
    tokio::fs::create_dir_all(parent).await.into_diagnostic()?;

    let mut file = tokio::fs::File::create(path)
        .await
        .into_diagnostic()
        .wrap_err("failed to create index file")?;

    file.write_all(content).await.into_diagnostic()?;
    file.flush().await.into_diagnostic()?;

    // Write the timestamp to the lockfile
    let lockfile_path = PathBuf::from(format!("{}.lock", path.display()));
    let mut file = tokio::fs::File::create(&lockfile_path)
        .await
        .into_diagnostic()
        .wrap_err("failed to create lockfile")?;
    let timestamp = chrono::Local::now().timestamp_micros();
    file.write_all(timestamp.to_string().as_bytes())
        .await
        .into_diagnostic()?;
    file.flush().await.into_diagnostic()?;

    Ok(())
}

async fn read_json_with_lock(path: &Path) -> miette::Result<(bool, String)> {
    let lockfile_path = PathBuf::from(format!("{}.lock", path.display()));
    let mut lockfile = tokio::fs::File::open(&lockfile_path)
        .await
        .into_diagnostic()
        .wrap_err("failed to open lockfile")?;

    let mut content = String::new();

    lockfile
        .read_to_string(&mut content)
        .await
        .into_diagnostic()
        .wrap_err("failed to read lockfile")?;

    let lastupdated = DateTime::<Local>::from(
        DateTime::from_timestamp_micros(content.parse::<i64>().expect("valid timestamp"))
            .expect("valid timestamp"),
    );
    let now = Local::now();
    let duration = Duration::hours(constant::INDEX_EXPIRATION);
    let cache_valid = now < lastupdated + duration;

    tracing::debug!("index {} cache valid '{}'", path.display(), cache_valid,);
    if !cache_valid {
        tracing::debug!("index cache last updated {}, now {}", lastupdated, now);
    }

    match cache_valid {
        false => Ok((false, "".to_string())),
        true => {
            let mut file = tokio::fs::File::open(&path)
                .await
                .into_diagnostic()
                .wrap_err("failed to open index file")?;

            content = String::new();
            file.read_to_string(&mut content)
                .await
                .into_diagnostic()
                .wrap_err("failed to read index")?;
            Ok((true, content))
        }
    }
}

pub async fn build_installrecipe(spec: &ToolchainSpec) -> miette::Result<Option<InstallRecipe>> {
    let release = match spec {
        ToolchainSpec::Bleeding => unimplemented!(),
        ToolchainSpec::Latest => {
            let index = read_channel_index(ChannelName::Latest).await?;
            index.releases.last().cloned().or(None)
        }
        ToolchainSpec::Nightly => {
            let index = read_channel_index(ChannelName::Nightly).await?;
            index.releases.last().cloned().or(None)
        }
        ToolchainSpec::Version(s) => {
            let is_nightly = s.starts_with("nightly");

            let req_version = if is_nightly {
                s.trim_start_matches("nightly-")
            } else {
                s
            };

            let index = if is_nightly {
                read_channel_index(ChannelName::Nightly).await?
            } else {
                read_channel_index(ChannelName::Latest).await?
            };

            index
                .releases
                .iter()
                .find(|&release| {
                    if is_nightly {
                        release
                            .date
                            .as_deref()
                            .map(|d| d == req_version)
                            .unwrap_or(false)
                    } else {
                        release.version.as_str() == req_version
                    }
                })
                .cloned()
                .or(None)
        }
    };

    let release = match release {
        Some(r) => r,
        None => {
            tracing::debug!("no release available for requested spec: {}", spec);
            return Ok(None);
        }
    };

    let components = read_component_index(&release).await?.components;
    let recipe = InstallRecipe {
        spec: spec.clone(),
        release,
        components,
    };

    Ok(Some(recipe))
}

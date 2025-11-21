use std::path::{Path, PathBuf};

use chrono::{DateTime, Duration, Local};
use miette::{Context, IntoDiagnostic};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::constant;
use crate::dist_server::schema::{
    ChannelIndex, ChannelName, Component, ComponentIndex, Index, Release, Target,
};
use crate::utils::{build_dist_server_api, build_http_client_with_retry, url_to_reader};

use super::ToolchainSpec;

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

    let main_index_url = build_dist_server_api(index_filename)?;

    content.clear();

    let mut reader = url_to_reader(main_index_url, &build_http_client_with_retry(), None).await?;
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
pub async fn read_channel_index(channel: &ChannelName) -> miette::Result<ChannelIndex> {
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

    let channel_index_url = build_dist_server_api(&channel_index_filename)?;

    content.clear();

    let mut reader =
        url_to_reader(channel_index_url, &build_http_client_with_retry(), None).await?;
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
pub async fn read_component_index(
    channel: &ChannelName,
    release: &Release,
) -> miette::Result<ComponentIndex> {
    let host_target = Target::from_host()?;
    let filename = format!("{}.json", host_target);

    let mut component_index_local = crate::moonup_home();
    component_index_local.push("downloads");

    match channel {
        ChannelName::Bleeding => component_index_local.push("bleeding"),
        ChannelName::Latest => {
            component_index_local.push("latest");
            component_index_local.push(&release.version);
        }
        ChannelName::Nightly => {
            component_index_local.push("nightly");
            component_index_local.push(
                &release
                    .date
                    .as_deref()
                    .expect("nightly release should have build date"),
            );
        }
        _ => return Err(miette::miette!("unsupported channel: {}", channel)),
    }

    component_index_local.push(&filename);

    let (cache_valid, mut content) = match read_json_with_lock(&component_index_local).await {
        Ok((cache_valid, content)) => (cache_valid, content),
        Err(e) => {
            tracing::debug!("failed to read component index json: {}", e);
            (false, "".to_string())
        }
    };

    // For bleeding channel, the component index is always fetched from remote
    if cache_valid && channel != &ChannelName::Bleeding {
        return serde_json::from_str(&content)
            .into_diagnostic()
            .inspect_err(|e| {
                tracing::info!("malformed component index json: {}", e);
                let _ = std::fs::remove_file(&component_index_local);
            });
    }

    let component_index_urlpath = match channel {
        ChannelName::Bleeding => format!("/bleeding/{}", filename),
        ChannelName::Latest => format!("/latest/{}/{}", release.version, filename),
        ChannelName::Nightly => format!(
            "/nightly/{}/{}",
            (release.date.as_deref()).expect("nightly release should have build date"),
            filename
        ),
        _ => return Err(miette::miette!("unsupported channel: {}", channel)),
    };

    let component_index_url = build_dist_server_api(&component_index_urlpath)?;

    content.clear();

    let mut reader =
        url_to_reader(component_index_url, &build_http_client_with_retry(), None).await?;
    reader
        .read_to_string(&mut content)
        .await
        .into_diagnostic()?;

    let index = serde_json::from_str(&content)
        .into_diagnostic()
        .wrap_err("malformed component index json")?;

    write_json_with_lock(&component_index_local, content.as_bytes()).await?;

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
    let channel = ChannelName::from(spec);
    let index = read_channel_index(&channel).await?;
    let mut releases = index.releases().iter().filter(|r| r.is_host_supported());

    let release = match spec {
        ToolchainSpec::Bleeding | ToolchainSpec::Latest | ToolchainSpec::Nightly => {
            releases.last().cloned().or(None)
        }
        ToolchainSpec::Version(s) => {
            let is_nightly = s.starts_with("nightly");

            let req_version = if is_nightly {
                s.trim_start_matches("nightly-")
            } else {
                s
            };

            releases
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

    let components = read_component_index(&channel, &release)
        .await?
        .components()
        .to_vec();
    let recipe = InstallRecipe {
        spec: spec.clone(),
        release,
        components,
    };

    Ok(Some(recipe))
}

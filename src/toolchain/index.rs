use std::path::{Path, PathBuf};

use chrono::{DateTime, Duration, Local};
use miette::{Context, IntoDiagnostic};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::constant;
use crate::dist_server::schema::{ChannelIndex, ComponentIndex, Index, Release, Target};
use crate::utils::{build_dist_server_api, build_http_client_with_retry, url_to_reader};

use super::{ToolchainSpec, install::InstallRecipe, version};

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
pub async fn read_channel_index(channel: &str) -> miette::Result<ChannelIndex> {
    let channel_index_filename = format!("channel-{channel}.json");

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
    channel: &str,
    release: &Release,
) -> miette::Result<ComponentIndex> {
    let host_target = Target::from_host()?;
    let filename = format!("{}.json", host_target);

    let mut component_index_local = crate::moonup_home();
    component_index_local.push("downloads");
    component_index_local.push(channel);
    component_index_local.push(release.key());
    component_index_local.push(&filename);

    let (cache_valid, mut content) = match read_json_with_lock(&component_index_local).await {
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
                let _ = std::fs::remove_file(&component_index_local);
            });
    }

    let component_index_urlpath = format!("/{}/{}/{}", channel, release.key(), filename);

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

/// Interpret a user-supplied toolchain string into a [`ToolchainSpec`].
///
/// Interpretation is index-driven at resolve time: a string exactly matching a
/// valid channel name advertised by the (cached) main index is a floating
/// channel spec, except that the reserved legacy nightly-date spelling is
/// interpreted as a pin first. Other strings use the syntactic spec parser.
pub async fn interpret_spec(raw: &str) -> miette::Result<ToolchainSpec> {
    let parsed = ToolchainSpec::try_from(raw).map_err(|error| miette::miette!(error))?;

    // The compatibility spelling is reserved even if a server advertises a
    // channel with the same literal name.
    if super::legacy_nightly_key(raw).is_some() {
        return Ok(parsed);
    }

    match read_index().await {
        Ok(index) => {
            let is_channel = index.channels().iter().any(|c| c.name == raw);
            Ok(if is_channel {
                ToolchainSpec::Channel(raw.to_owned())
            } else {
                parsed
            })
        }
        Err(e) => {
            tracing::debug!(
                "failed to read main index for spec interpretation: {}; falling back to syntactic interpretation",
                e
            );
            Ok(parsed)
        }
    }
}

/// The valid channels advertised by a readable main-index catalog.
///
/// `None` means no usable channel catalog was available. `Some(vec![])` means
/// a catalog was read but contained no valid channel names.
pub async fn available_channels() -> Option<Vec<String>> {
    match read_index().await {
        Ok(index) if index.has_channel_catalog() => Some(
            index
                .channels()
                .iter()
                .map(|channel| channel.name.clone())
                .collect(),
        ),
        Ok(_) => None,
        Err(e) => {
            tracing::debug!("failed to read main index for available channels: {}", e);
            None
        }
    }
}

/// Resolve a toolchain spec against its channel index into an install recipe.
///
/// Returns `None` when the channel is not advertised by the main index, or no
/// host-supported release matches the spec.
pub(crate) async fn build_installrecipe(
    spec: &ToolchainSpec,
) -> miette::Result<Option<InstallRecipe>> {
    let channel = spec.channel().to_owned();

    // Validate the channel against the main index so an unknown channel
    // resolves to no recipe (the caller reports it with the available
    // channels) instead of a bare download failure. Best-effort offline: if
    // the main index cannot be read, let the channel index fetch produce
    // its own error.
    if let Some(channels) = available_channels().await
        && !channels.contains(&channel)
    {
        return Ok(None);
    }

    let index = read_channel_index(&channel).await?;
    let mut releases = index.releases().iter().filter(|r| r.is_host_supported());

    let release = match spec {
        ToolchainSpec::Channel(_) => releases.next_back().cloned(),
        ToolchainSpec::Version { key, .. } if version::is_stable_selector(key) => {
            version::resolve_stable_selector(key, releases).cloned()
        }
        ToolchainSpec::Version { key, .. } => releases.find(|&r| r.key() == key).cloned(),
    };

    let Some(release) = release else {
        tracing::debug!("no release available for requested spec: {}", spec);
        return Ok(None);
    };

    let components = read_component_index(&channel, &release)
        .await?
        .components()
        .to_vec();
    let effective_spec = match spec {
        ToolchainSpec::Version { channel, key } if version::is_stable_selector(key) => {
            ToolchainSpec::Version {
                channel: channel.clone(),
                key: release.version.clone(),
            }
        }
        _ => spec.clone(),
    };
    let recipe = InstallRecipe {
        spec: effective_spec,
        requested_spec: spec.clone(),
        release,
        components,
    };

    Ok(Some(recipe))
}

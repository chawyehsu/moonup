use std::path::{Path, PathBuf};

use jiff::{SignedDuration, Timestamp};
use miette::{Context, IntoDiagnostic};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::constant;
use crate::dist_server::schema::{ChannelIndex, ComponentIndex, Index, Release, Target};
use crate::layout::Layout;
use crate::session::Session;
use crate::utils::{build_dist_server_api, url_to_reader};

/// Load the main index from the on-disk cache (when fresh) or the
/// distribution server.
///
/// Returns the on-disk lock timestamp the content was validated against (or
/// the one just written), so the session can bind its cache entry to the
/// file's state. Cache population is the session's business: this function
/// always consults disk/server, and the caller stores the parsed result.
pub(crate) async fn load_index(session: &Session) -> miette::Result<(Index, Timestamp)> {
    let index_file = session.layout().index_path();

    let mut content = match read_json_with_lock(&index_file).await {
        Ok(Some((timestamp, content))) => {
            let index = parse_index(&content).inspect_err(|e| {
                tracing::info!("malformed index json: {}", e);
                let _ = std::fs::remove_file(&index_file);
            })?;
            return Ok((index, timestamp));
        }
        Ok(None) => String::new(),
        Err(e) => {
            tracing::debug!("failed to read index json: {}", e);
            String::new()
        }
    };

    let main_index_url = build_dist_server_api("index.json")?;

    let mut reader = url_to_reader(main_index_url, &session.http_client(), None).await?;
    content.clear();
    reader
        .read_to_string(&mut content)
        .await
        .into_diagnostic()?;

    let index = parse_index(&content)?;

    let timestamp = write_json_with_lock(&index_file, content.as_bytes()).await?;

    Ok((index, timestamp))
}

/// Load a channel index from the on-disk cache (when fresh) or the
/// distribution server.
pub(crate) async fn load_channel_index(
    session: &Session,
    channel: &str,
) -> miette::Result<(ChannelIndex, Timestamp)> {
    let channel_index_file = session.layout().channel_index_path(channel);

    let mut content = match read_json_with_lock(&channel_index_file).await {
        Ok(Some((timestamp, content))) => {
            let index = serde_json::from_str(&content)
                .into_diagnostic()
                .inspect_err(|e| {
                    tracing::info!("malformed channel index json: {}", e);
                    let _ = std::fs::remove_file(&channel_index_file);
                })?;
            return Ok((index, timestamp));
        }
        Ok(None) => String::new(),
        Err(e) => {
            tracing::debug!("failed to read channel index json: {}", e);
            String::new()
        }
    };

    let channel_index_filename = format!("channel-{channel}.json");
    let channel_index_url = build_dist_server_api(&channel_index_filename)?;

    let mut reader = url_to_reader(channel_index_url, &session.http_client(), None).await?;
    content.clear();
    reader
        .read_to_string(&mut content)
        .await
        .into_diagnostic()?;

    let index = serde_json::from_str(&content)
        .into_diagnostic()
        .wrap_err("malformed channel index json")?;

    let timestamp = write_json_with_lock(&channel_index_file, content.as_bytes()).await?;

    Ok((index, timestamp))
}

/// Load a component index from the on-disk cache (when fresh) or the
/// distribution server.
pub(crate) async fn load_component_index(
    session: &Session,
    channel: &str,
    release: &Release,
) -> miette::Result<(ComponentIndex, Timestamp)> {
    let host_target = Target::from_host()?;
    let filename = format!("{}.json", host_target);

    let component_index_local = session
        .layout()
        .downloads_dir()
        .join(channel)
        .join(release.key())
        .join(&filename);

    let mut content = match read_json_with_lock(&component_index_local).await {
        Ok(Some((timestamp, content))) => {
            let index = serde_json::from_str(&content)
                .into_diagnostic()
                .inspect_err(|e| {
                    tracing::info!("malformed component index json: {}", e);
                    let _ = std::fs::remove_file(&component_index_local);
                })?;
            return Ok((index, timestamp));
        }
        Ok(None) => String::new(),
        Err(e) => {
            tracing::debug!("failed to read component index json: {}", e);
            String::new()
        }
    };

    let component_index_urlpath = format!("/{}/{}/{}", channel, release.key(), filename);

    let component_index_url = build_dist_server_api(&component_index_urlpath)?;

    let mut reader = url_to_reader(component_index_url, &session.http_client(), None).await?;
    content.clear();
    reader
        .read_to_string(&mut content)
        .await
        .into_diagnostic()?;

    let index = serde_json::from_str(&content)
        .into_diagnostic()
        .wrap_err("malformed component index json")?;

    let timestamp = write_json_with_lock(&component_index_local, content.as_bytes()).await?;

    Ok((index, timestamp))
}

/// Load the main index from disk alone, ignoring freshness. Best-effort:
/// `None` on any read or parse error. Returns the on-disk lock timestamp
/// when one is present, so the session can later validate its cache entry
/// against the file's state.
pub(crate) fn load_index_offline(layout: &Layout) -> Option<(Index, Option<Timestamp>)> {
    let index_file = layout.index_path();
    let content = std::fs::read_to_string(&index_file).ok()?;
    let index = parse_index(&content).ok()?;
    Some((index, read_lock_timestamp(&index_file)))
}

/// Load a channel index from disk alone, ignoring freshness. Best-effort:
/// `None` on any read or parse error.
pub(crate) fn load_channel_index_offline(
    layout: &Layout,
    channel: &str,
) -> Option<(ChannelIndex, Option<Timestamp>)> {
    let channel_index_file = layout.channel_index_path(channel);
    let content = std::fs::read_to_string(&channel_index_file).ok()?;
    let index = serde_json::from_str(&content).ok()?;
    Some((index, read_lock_timestamp(&channel_index_file)))
}

fn parse_index(content: &str) -> miette::Result<Index> {
    serde_json::from_str(content)
        .into_diagnostic()
        .wrap_err("malformed index json")
}

/// The on-disk lock timestamp for an index file, `None` when the lockfile is
/// missing or unreadable.
pub(crate) fn read_lock_timestamp(index_file: &Path) -> Option<Timestamp> {
    let content = std::fs::read_to_string(lockfile_path(index_file)).ok()?;
    Timestamp::from_microsecond(content.trim().parse::<i64>().ok()?).ok()
}

/// Whether the given lock timestamp is inside the freshness window.
pub(crate) fn lock_is_fresh(timestamp: &Timestamp) -> bool {
    Timestamp::now() < *timestamp + SignedDuration::from_hours(constant::INDEX_EXPIRATION)
}

fn lockfile_path(index_file: &Path) -> PathBuf {
    PathBuf::from(format!("{}.lock", index_file.display()))
}

async fn write_json_with_lock(path: &Path, content: &[u8]) -> miette::Result<Timestamp> {
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
    let timestamp = Timestamp::now();
    let mut file = tokio::fs::File::create(lockfile_path(path))
        .await
        .into_diagnostic()
        .wrap_err("failed to create lockfile")?;
    file.write_all(timestamp.as_microsecond().to_string().as_bytes())
        .await
        .into_diagnostic()?;
    file.flush().await.into_diagnostic()?;

    Ok(timestamp)
}

/// Read the index file when its lock timestamp is inside the freshness
/// window. Returns `Ok(None)` when the lockfile is missing or the timestamp
/// is stale (the caller should fetch), and an error only when the lockfile
/// cannot be read.
async fn read_json_with_lock(path: &Path) -> miette::Result<Option<(Timestamp, String)>> {
    let Some(timestamp) = read_lock_timestamp(path) else {
        return Err(miette::miette!("failed to read lockfile"));
    };

    let cache_valid = lock_is_fresh(&timestamp);
    tracing::debug!("index {} cache valid '{}'", path.display(), cache_valid,);
    if !cache_valid {
        tracing::debug!(
            "index cache last updated {}, now {}",
            timestamp,
            Timestamp::now()
        );
        return Ok(None);
    }

    let mut file = tokio::fs::File::open(path)
        .await
        .into_diagnostic()
        .wrap_err("failed to open index file")?;

    let mut content = String::new();
    file.read_to_string(&mut content)
        .await
        .into_diagnostic()
        .wrap_err("failed to read index")?;
    Ok(Some((timestamp, content)))
}

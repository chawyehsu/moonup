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
    let layout = session.layout();
    let index_file = layout.index_path();

    if let Ok((index, Some(timestamp))) = load_index_offline(layout).await {
        if lock_is_fresh(&timestamp) {
            tracing::debug!("index {} cache valid", index_file.display());
            return Ok((index, timestamp));
        }
        tracing::debug!("index cache outdated, last updated {}", timestamp);
    }

    let main_index_url = build_dist_server_api("index.json")?;
    let mut reader = url_to_reader(main_index_url, &session.http_client(), None).await?;
    let mut content = String::new();
    reader
        .read_to_string(&mut content)
        .await
        .into_diagnostic()?;

    let index = serde_json::from_str(&content)
        .into_diagnostic()
        .wrap_err("malformed index json")?;
    let timestamp = write_json_with_lock(&index_file, content.as_bytes()).await?;
    tracing::debug!("index cache updated at {}", timestamp);
    Ok((index, timestamp))
}

/// Load a channel index from the on-disk cache (when fresh) or the
/// distribution server.
pub(crate) async fn load_channel_index(
    session: &Session,
    channel: &str,
) -> miette::Result<(ChannelIndex, Timestamp)> {
    let layout = session.layout();
    let channel_index_file = layout.channel_index_path(channel);

    if let Ok((index, Some(timestamp))) = load_channel_index_offline(layout, channel).await {
        if lock_is_fresh(&timestamp) {
            tracing::debug!("channel index {} cache valid", channel_index_file.display());
            return Ok((index, timestamp));
        }
        tracing::debug!("channel index cache outdated, last updated {}", timestamp);
    }

    let channel_index_filename = format!("channel-{channel}.json");
    let channel_index_url = build_dist_server_api(&channel_index_filename)?;
    let mut reader = url_to_reader(channel_index_url, &session.http_client(), None).await?;
    let mut content = String::new();
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
    let layout = session.layout();
    let target = Target::from_host()?.to_string();

    if let Ok((index, Some(timestamp))) =
        load_component_index_offline(layout, channel, release, &target).await
    {
        if lock_is_fresh(&timestamp) {
            tracing::debug!("component index cache valid");
            return Ok((index, timestamp));
        }
        tracing::debug!("component index cache outdated, last updated {}", timestamp);
    }

    let component_index_urlpath = format!("/{}/{}/{}.json", channel, release.key(), target);
    let component_index_url = build_dist_server_api(&component_index_urlpath)?;
    let mut reader = url_to_reader(component_index_url, &session.http_client(), None).await?;
    let mut content = String::new();
    reader
        .read_to_string(&mut content)
        .await
        .into_diagnostic()?;

    let index = serde_json::from_str(&content)
        .into_diagnostic()
        .wrap_err("malformed component index json")?;
    let component_index_file = layout.component_index_path(channel, &release.key(), &target);
    let timestamp = write_json_with_lock(&component_index_file, content.as_bytes()).await?;
    Ok((index, timestamp))
}

/// Load the main index from disk alone
pub(crate) async fn load_index_offline(
    layout: &Layout,
) -> miette::Result<(Index, Option<Timestamp>)> {
    let (timestamp, content) = read_json_with_lock(&layout.index_path()).await?;
    let index = serde_json::from_str(&content)
        .into_diagnostic()
        .wrap_err("malformed index json")?;
    Ok((index, timestamp))
}

/// Load a channel index from disk alone
pub(crate) async fn load_channel_index_offline(
    layout: &Layout,
    channel: &str,
) -> miette::Result<(ChannelIndex, Option<Timestamp>)> {
    let channel_index_file = layout.channel_index_path(channel);

    let (timestamp, content) = read_json_with_lock(&channel_index_file).await?;
    let index = serde_json::from_str(&content)
        .into_diagnostic()
        .wrap_err("malformed channel index json")?;
    Ok((index, timestamp))
}

/// Load a component index from disk alone
pub(crate) async fn load_component_index_offline(
    layout: &Layout,
    channel: &str,
    release: &Release,
    target: &str,
) -> miette::Result<(ComponentIndex, Option<Timestamp>)> {
    let component_index_file = layout.component_index_path(channel, &release.key(), target);

    let (timestamp, content) = read_json_with_lock(&component_index_file).await?;
    let index = serde_json::from_str(&content)
        .into_diagnostic()
        .wrap_err("malformed component index json")?;
    Ok((index, timestamp))
}

/// The on-disk lock timestamp for an index file, `None` when the lockfile is
/// missing or unreadable.
pub(crate) fn read_lock_timestamp(index_file: &Path) -> Option<Timestamp> {
    let content = std::fs::read_to_string(lockfile_path(index_file)).ok()?;
    Timestamp::from_microsecond(content.trim().parse::<i64>().ok()?).ok()
}

/// Write the on-disk lock timestamp for an index file.
async fn write_lock_timestamp(index_file: &Path) -> miette::Result<Timestamp> {
    let timestamp = Timestamp::now().as_microsecond();
    let mut file = tokio::fs::File::create(lockfile_path(index_file))
        .await
        .into_diagnostic()
        .wrap_err("failed to create lockfile")?;
    file.write_all(timestamp.to_string().as_bytes())
        .await
        .into_diagnostic()?;
    file.flush().await.into_diagnostic()?;

    Ok(Timestamp::from_microsecond(timestamp).unwrap())
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
    let timestamp = write_lock_timestamp(path).await?;
    Ok(timestamp)
}

/// Read the index file along with its lock timestamp.
///
/// Returns an error if the index file cannot be read.
async fn read_json_with_lock(path: &Path) -> miette::Result<(Option<Timestamp>, String)> {
    let timestamp = read_lock_timestamp(path);

    let mut file = tokio::fs::File::open(path)
        .await
        .into_diagnostic()
        .wrap_err("failed to open index file")?;

    let mut content = String::new();
    file.read_to_string(&mut content)
        .await
        .into_diagnostic()
        .inspect_err(|e| {
            tracing::debug!("failed to read index json: {}", e);
        })
        .wrap_err("failed to read index file")?;
    Ok((timestamp, content))
}

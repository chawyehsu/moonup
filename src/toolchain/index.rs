use chrono::{DateTime, Duration, Local};
use miette::{Context, IntoDiagnostic};
use serde::Deserialize;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use url::Url;

use crate::{
    constant,
    utils::{build_http_client, url_to_reader},
};

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

#[derive(Debug)]
pub struct ReleaseCombined {
    /// Whether the requested version is "latest"
    pub latest: bool,
    pub toolchain: Option<Release>,
    pub core: Option<Release>,
}

async fn retrieve_index() -> miette::Result<Index> {
    let parent = crate::moonup_home().join("downloads");
    let lockfile_path = parent.join("index.json.lock");
    let indexfile_path = parent.join("index.json");

    if let Ok(mut lockfile) = tokio::fs::File::open(&lockfile_path)
        .await
        .into_diagnostic()
    {
        let mut content = String::new();
        lockfile
            .read_to_string(&mut content)
            .await
            .into_diagnostic()?;

        let lastupdated = DateTime::<Local>::from(
            DateTime::from_timestamp_micros(content.parse::<i64>().expect("invalid timestamp"))
                .expect("invalid timestamp"),
        );
        let now = Local::now();
        let duration = Duration::hours(constant::INDEX_EXPIRATION);
        let index_cache_valid = now < lastupdated + duration;

        tracing::debug!(
            "index cache: last updated {} (offset: {}), now: {}, valid: {}",
            lastupdated,
            lastupdated + duration,
            now,
            index_cache_valid
        );

        if index_cache_valid {
            if let Ok(mut indexfile) = tokio::fs::File::open(&indexfile_path)
                .await
                .into_diagnostic()
            {
                let mut content = String::new();
                indexfile
                    .read_to_string(&mut content)
                    .await
                    .into_diagnostic()?;
                let index = serde_json::from_str(&content).expect("invalid index json");

                return Ok(index);
            }
        }
    }

    let client = build_http_client();
    let url = Url::parse(
        std::env::var("MOONUP_TOOLCHAIN_INDEX")
            .as_deref()
            .unwrap_or(crate::constant::TOOLCHAIN_INDEX),
    )
    .into_diagnostic()
    .wrap_err("Invalid MOONUP_TOOLCHAIN_INDEX string, should be a valid URL")?;

    let mut reader = url_to_reader(url, client, None).await?;
    let mut content = String::new();

    reader
        .read_to_string(&mut content)
        .await
        .into_diagnostic()?;

    let index = serde_json::from_str(&content).expect("invalid index json");

    std::fs::create_dir_all(&parent).into_diagnostic()?;

    let mut file = tokio::fs::File::create(&indexfile_path)
        .await
        .into_diagnostic()?;

    file.write_all(content.as_bytes()).await.into_diagnostic()?;
    file.flush().await.into_diagnostic()?;

    // Write the timestamp to the lockfile
    let mut file = tokio::fs::File::create(&lockfile_path)
        .await
        .into_diagnostic()?;
    let timestamp = chrono::Local::now().timestamp_micros();
    file.write_all(timestamp.to_string().as_bytes())
        .await
        .into_diagnostic()?;
    file.flush().await.into_diagnostic()?;

    Ok(index)
}

pub async fn retrieve_releases() -> miette::Result<Vec<Release>> {
    let index = retrieve_index().await?;
    let platform = {
        #[cfg(target_os = "macos")]
        {
            if cfg!(target_arch = "aarch64") {
                &index.darwin_arm64
            } else {
                &index.darwin_x64
            }
        }

        #[cfg(target_os = "linux")]
        {
            &index.linux_x64
        }

        #[cfg(target_os = "windows")]
        {
            &index.win_x64
        }
    };
    Ok(platform.releases.clone())
}

pub async fn retrieve_release(version: &str) -> miette::Result<ReleaseCombined> {
    let index = retrieve_index().await?;
    let platform = {
        #[cfg(target_os = "macos")]
        {
            if cfg!(target_arch = "aarch64") {
                &index.darwin_arm64
            } else {
                &index.darwin_x64
            }
        }

        #[cfg(target_os = "linux")]
        {
            &index.linux_x64
        }

        #[cfg(target_os = "windows")]
        {
            &index.win_x64
        }
    };

    let latest = version == "latest";
    let (core, toolchain) = match version {
        "latest" => (
            index.core.releases.first().cloned().or(None),
            platform.releases.first().cloned().or(None),
        ),
        _ => {
            let c = index
                .core
                .releases
                .iter()
                .find(|&release| release.version.as_str() == version)
                .cloned()
                .or(None);
            let t = platform
                .releases
                .iter()
                .find(|&release| release.version.as_str() == version)
                .cloned()
                .or(None);
            (c, t)
        }
    };

    Ok(ReleaseCombined {
        latest,
        toolchain,
        core,
    })
}

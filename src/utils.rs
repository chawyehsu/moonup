use futures_util::TryStreamExt;
use miette::IntoDiagnostic;
use reqwest::Client;
use std::{
    env,
    path::{Path, PathBuf},
    time::Duration,
};
use tokio::io::{AsyncRead, BufReader};
use tokio_util::io::StreamReader;
use url::Url;

use crate::{constant::TOOLCHAIN_FILE, moonup_home};

pub(crate) fn build_http_client() -> Client {
    static APP_USER_AGENT: &str = concat!(
        env!("CARGO_PKG_NAME"),
        "/",
        env!("CARGO_PKG_VERSION"),
        " (",
        env!("CARGO_PKG_HOMEPAGE"),
        ")"
    );

    Client::builder()
        .user_agent(APP_USER_AGENT)
        .read_timeout(Duration::from_secs(crate::constant::HTTP_READ_TIMEOUT))
        .build()
        .expect("failed to build HTTP client")
}

pub async fn url_to_reader(url: Url, client: Client) -> miette::Result<impl AsyncRead> {
    tracing::debug!("Streaming: {}", url);
    let request = client.get(url);
    let response = request.send().await.into_diagnostic()?;

    let byte_stream = response
        .bytes_stream()
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::Other, err));

    Ok(StreamReader::new(byte_stream))
}

pub async fn path_to_reader(path: &Path) -> miette::Result<impl AsyncRead> {
    let file = tokio::fs::File::open(path).await.into_diagnostic()?;
    Ok(BufReader::new(file))
}

pub fn detect_toolchain_file() -> Option<PathBuf> {
    let current_dir = env::current_dir().expect("can't access current directory");

    std::iter::successors(Some(current_dir.as_path()), |prev| prev.parent()).find_map(|dir| {
        let path = dir.join(TOOLCHAIN_FILE);
        if path.is_file() {
            Some(path)
        } else {
            None
        }
    })
}

/// Iterates over the current directory and all its parent directories
/// to find if there is a `TOOLCHAIN_FILE` and detect the toolchain version.
///
/// # Returns
///
/// The path to actual versioned toolchain
pub fn detect_active_toolchain() -> PathBuf {
    detect_toolchain_file().map_or_else(
        || moonup_home().join("toolchains").join("latest"),
        |path| {
            let version = std::fs::read_to_string(path)
                .unwrap_or_else(|_| panic!("can't read {}", TOOLCHAIN_FILE));

            moonup_home().join("toolchains").join(version.trim())
        },
    )
}

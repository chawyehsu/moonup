use futures_util::TryStreamExt;
use miette::IntoDiagnostic;
use reqwest::Client;
use std::{path::Path, time::Duration};
use tokio::io::{AsyncRead, BufReader};
use tokio_util::io::StreamReader;
use url::Url;

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

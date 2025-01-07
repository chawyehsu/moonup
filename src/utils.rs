use futures_util::TryStreamExt;
#[cfg(target_os = "windows")]
use miette::Context;
use miette::IntoDiagnostic;
use reqwest::Client;
use std::env;
#[cfg(not(target_os = "windows"))]
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncRead, BufReader};
use tokio_util::io::StreamReader;
use url::Url;

use crate::reporter::Reporter;

pub(crate) fn build_http_client() -> Client {
    static APP_USER_AGENT: &str = concat!(
        env!("CARGO_PKG_NAME"),
        "/",
        env!("CARGO_PKG_VERSION"),
        " (+",
        env!("CARGO_PKG_HOMEPAGE"),
        ")"
    );

    Client::builder()
        .user_agent(APP_USER_AGENT)
        .read_timeout(Duration::from_secs(crate::constant::HTTP_READ_TIMEOUT))
        .build()
        .expect("failed to build HTTP client")
}

pub(crate) fn build_dist_server_api(path: &str) -> miette::Result<Url> {
    let path = path.trim_start_matches('/');

    let baseurl = env::var(crate::constant::ENVNAME_MOONUP_DIST_SERVER)
        .unwrap_or_else(|_| crate::constant::MOONUP_DIST_SERVER.to_string());
    Url::parse(&format!("{}/{}", baseurl, path))
        .into_diagnostic()
        .inspect(|u| {
            tracing::trace!("constructed dist server API: {}", u);
        })
}

pub async fn url_to_reader(
    url: Url,
    client: Client,
    reporter: Option<Arc<dyn Reporter>>,
) -> miette::Result<impl AsyncRead> {
    tracing::debug!("streaming: {}", url);
    let request = client.get(url);
    let response = request.send().await.into_diagnostic()?;

    if !response.status().is_success() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!(
                "failed to download {} (code: {})",
                response.url(),
                response.status()
            ),
        ))
        .into_diagnostic();
    }

    if let Some(reporter) = &reporter {
        reporter.on_start(
            response
                .content_length()
                .map(|len| len as usize)
                .unwrap_or(0),
        );
    }

    let mut current = 0;

    let byte_stream = response
        .bytes_stream()
        .inspect_ok(move |chunk| {
            current += chunk.len();
            if let Some(reporter) = &reporter {
                reporter.on_progress(current);
            }
        })
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::Other, err));

    Ok(StreamReader::new(byte_stream))
}

pub async fn path_to_reader(path: &Path) -> miette::Result<impl AsyncRead> {
    let file = tokio::fs::File::open(path).await.into_diagnostic()?;
    Ok(BufReader::new(file))
}

#[inline]
pub(crate) fn trimmed_or_none(s: &str) -> Option<String> {
    Some(s.trim().to_string()).filter(|v| !v.is_empty())
}

/// Pour the new executable to the destination path.
///
/// On Windows, this function will try to rename and remove the old exe
/// before copying the new one. On other platforms, it will just remove
/// the old exe and copy the new one.
pub fn replace_exe(new: &Path, old: &Path) -> miette::Result<()> {
    #[cfg(target_os = "windows")]
    {
        let mut older = old.to_path_buf();
        older.set_extension("exe.old");

        std::fs::remove_file(&older).or_else(|err| match err.kind() {
            std::io::ErrorKind::NotFound => Ok(()),
            std::io::ErrorKind::PermissionDenied => Err(err)
                .into_diagnostic()
                .wrap_err(format!("failed to remove old exe: {}", older.display()))
                .wrap_err(
                    "files may be in use, please close applications using moonbit and try again",
                ),
            _ => Err(err).into_diagnostic(),
        })?;

        match std::fs::rename(old, &older) {
            Ok(_) => tracing::debug!("renamed old exe: {}", &older.display()),
            Err(err) => match err.kind() {
                std::io::ErrorKind::NotFound => { // ignore
                }
                _ => {
                    return Err(err).into_diagnostic().wrap_err(format!(
                        "failed to rename {} to {}",
                        old.display(),
                        &older.display()
                    ))
                }
            },
        }

        std::fs::copy(new, old).into_diagnostic().wrap_err(format!(
            "failed to copy {} to {}",
            new.display(),
            old.display()
        ))?;

        tracing::debug!("replaced new exe: {}", old.display());
        let _ = std::fs::remove_file(&older);
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = std::fs::remove_file(old);
        std::fs::copy(new, old).into_diagnostic()?;
        std::fs::set_permissions(old, std::fs::Permissions::from_mode(0o755)).into_diagnostic()?;
        tracing::debug!("replaced new exe: {}", old.display());
    }

    Ok(())
}

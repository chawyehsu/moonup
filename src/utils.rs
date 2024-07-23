use futures_util::TryStreamExt;
use miette::IntoDiagnostic;
use reqwest::Client;
use std::env;
#[cfg(not(target_os = "windows"))]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncRead, BufReader};
use tokio_util::io::StreamReader;
use url::Url;

use crate::{constant::TOOLCHAIN_FILE, moonup_home, reporter::Reporter};

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

pub async fn url_to_reader(
    url: Url,
    client: Client,
    reporter: Option<Arc<dyn Reporter>>,
) -> miette::Result<impl AsyncRead> {
    tracing::debug!("Streaming: {}", url);
    let request = client.get(url);
    let response = request.send().await.into_diagnostic()?;
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
/// to find if there is a [`TOOLCHAIN_FILE`] and detect the toolchain version.
///
/// # Returns
///
/// The path to actual versioned toolchain
pub fn detect_active_toolchain() -> PathBuf {
    detect_toolchain_file().map_or_else(
        || {
            let default_file = moonup_home().join("default");
            let version =
                std::fs::read_to_string(default_file).unwrap_or_else(|_| "latest".to_string());

            moonup_home().join("toolchains").join(version.trim())
        },
        |path| {
            let version = std::fs::read_to_string(path)
                .unwrap_or_else(|_| panic!("can't read {}", TOOLCHAIN_FILE));

            moonup_home().join("toolchains").join(version.trim())
        },
    )
}

/// Pour the new executable to the destination path.
///
/// On Windows, this function will try to rename and remove the old exe
/// before copying the new one. On other platforms, it will just remove
/// the old exe and copy the new one.
pub fn replace_exe(new: &Path, old: &Path) -> miette::Result<()> {
    #[cfg(target_os = "windows")]
    {
        let mut dest_old = old.to_path_buf();
        dest_old.set_extension("exe.old");
        let _ = std::fs::remove_file(&dest_old);

        tracing::debug!("Renaming current exe: {}", old.display());
        if old.exists() {
            std::fs::rename(old, &dest_old).into_diagnostic()?;
        }
        std::fs::copy(new, old).into_diagnostic()?;

        tracing::debug!("Removing old exe: {}", &dest_old.display());
        let _ = std::fs::remove_file(&dest_old);
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = std::fs::remove_file(old);
        std::fs::copy(new, old).into_diagnostic()?;
        std::fs::set_permissions(old, std::fs::Permissions::from_mode(0o755)).into_diagnostic()?;
        tracing::debug!("Replaced new exe: {}", old.display());
    }

    Ok(())
}

use futures_util::TryStreamExt;
#[cfg(target_os = "windows")]
use miette::Context;
use miette::IntoDiagnostic;
use reqwest::Client;
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use reqwest_retry::{RetryTransientMiddleware, policies::ExponentialBackoff};
use std::env;
#[cfg(not(target_os = "windows"))]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncRead, BufReader};
use tokio_util::io::StreamReader;
use url::Url;

use crate::reporter::Reporter;

/// Build a basic HTTP client
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

/// Build an HTTP client with exponential backoff retry policy
pub(crate) fn build_http_client_with_retry() -> ClientWithMiddleware {
    let retry_policy = ExponentialBackoff::builder().build_with_max_retries(3);
    let retry_middleware = RetryTransientMiddleware::new_with_policy(retry_policy);

    ClientBuilder::new(build_http_client())
        .with(retry_middleware)
        .build()
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
    client: &ClientWithMiddleware,
    reporter: Option<Arc<dyn Reporter>>,
) -> miette::Result<impl AsyncRead + use<>> {
    tracing::debug!("streaming: {}", url);
    let request = client.get(url);
    let response = request.send().await.into_diagnostic()?;

    if !response.status().is_success() {
        return Err(std::io::Error::other(format!(
            "failed to download {} (code: {})",
            response.url(),
            response.status()
        )))
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
        .map_err(std::io::Error::other);

    Ok(StreamReader::new(byte_stream))
}

pub async fn path_to_reader(path: &Path) -> miette::Result<impl AsyncRead + use<>> {
    let file = tokio::fs::File::open(path).await.into_diagnostic()?;
    Ok(BufReader::new(file))
}

/// Trim the given string and return `None` if the string is empty.
#[inline]
pub(crate) fn trimmed_or_none(s: &str) -> Option<&str> {
    let trimmed = s.trim();
    (!trimmed.is_empty()).then_some(trimmed)
}

/// The hidden directory under `MOON_HOME/bin/` where retired executables are
/// swept from best-effort once their processes exit.
#[cfg(target_os = "windows")]
const TRASH_DIR: &str = ".trash";

/// Return `true` if the files at `new` and `old` are byte-identical.
///
/// A missing or unreadable destination counts as *different* so a
/// replacement is never skipped on weak evidence, while an unreadable source
/// propagates as a real error.
pub(crate) fn files_identical(new: &Path, old: &Path) -> std::io::Result<bool> {
    const CHUNK_SIZE: usize = 64 * 1024;

    let new_len = std::fs::metadata(new)?.len();
    let old_len = match std::fs::metadata(old) {
        Ok(meta) => meta.len(),
        Err(_) => return Ok(false),
    };
    if new_len != old_len {
        return Ok(false);
    }

    let mut new_file = std::fs::File::open(new)?;
    let mut old_file = match std::fs::File::open(old) {
        Ok(file) => file,
        Err(_) => return Ok(false),
    };

    let mut new_buf = vec![0u8; CHUNK_SIZE];
    let mut old_buf = vec![0u8; CHUNK_SIZE];

    loop {
        let n = read_up_to(&mut new_file, &mut new_buf)?;
        if n == 0 {
            // the source is exhausted; the destination must be too
            return Ok(read_up_to(&mut old_file, &mut old_buf)? == 0);
        }

        let m = read_up_to(&mut old_file, &mut old_buf[..n])?;
        if m != n || new_buf[..n] != old_buf[..n] {
            return Ok(false);
        }
    }
}

fn read_up_to(file: &mut std::fs::File, buf: &mut [u8]) -> std::io::Result<usize> {
    use std::io::Read;

    let mut filled = 0;
    while filled < buf.len() {
        match file.read(&mut buf[filled..]) {
            Ok(0) => break,
            Ok(read) => filled += read,
            Err(err) if err.kind() == std::io::ErrorKind::Interrupted => {}
            Err(err) => return Err(err),
        }
    }
    Ok(filled)
}

/// Move `old` to a unique name inside `bin/.trash/` so the live name is freed
/// immediately, even while the file is still mapped by a running process
/// (Windows permits renaming but not deleting a running image). Falls back to
/// a unique sibling name next to `old` if the trash directory cannot be
/// created. Returns the retired path.
#[cfg(target_os = "windows")]
fn retire_exe(old: &Path) -> miette::Result<PathBuf> {
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    let bin_dir = old
        .parent()
        .ok_or_else(|| miette::miette!("retired exe has no parent directory"))?;
    let file_name = old
        .file_name()
        .ok_or_else(|| miette::miette!("retired exe has no file name"))?;

    let pid = std::process::id();
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let mut unique_name = file_name.to_os_string();
    unique_name.push(format!(".old.{pid}.{n}"));

    let trash_dir = bin_dir.join(TRASH_DIR);
    let retired = match std::fs::create_dir_all(&trash_dir) {
        Ok(()) => trash_dir.join(&unique_name),
        Err(_) => bin_dir.join(&unique_name),
    };

    match std::fs::rename(old, &retired) {
        Ok(()) => {
            tracing::debug!("retired exe to: {}", retired.display());
            Ok(retired)
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(retired),
        Err(err) => Err(err).into_diagnostic().wrap_err(format!(
            "failed to rename {} to {}",
            old.display(),
            retired.display()
        )),
    }
}

/// Pour the new executable to the destination path.
///
/// On Windows, the destination is retired into the hidden `bin/.trash/`
/// directory under a unique name before the new executable is copied in, so
/// a shim held open by a running command can still be replaced. On other
/// platforms, the old executable is simply removed before the copy.
pub fn replace_exe(new: &Path, old: &Path) -> miette::Result<()> {
    #[cfg(target_os = "windows")]
    // On Windows, ensure shims are created with the `.exe` extension
    let dest = old.with_extension("exe");
    #[cfg(not(target_os = "windows"))]
    let dest = old.to_path_buf();

    // Skip the replacement when the destination already matches the source,
    // so repeated installs stop churning untouched shims.
    if files_identical(new, &dest).into_diagnostic()? {
        tracing::debug!("skip replace: {} is already up to date", dest.display());
        #[cfg(not(target_os = "windows"))]
        let _ = std::fs::set_permissions(&dest, std::fs::Permissions::from_mode(0o755));
        return Ok(());
    }

    #[cfg(target_os = "windows")]
    {
        let retired = retire_exe(&dest)?;

        // migrate the legacy `moon.exe.old` convention
        let legacy = dest.with_extension("exe.old");
        let _ = std::fs::remove_file(&legacy);

        std::fs::copy(new, &dest)
            .into_diagnostic()
            .wrap_err(format!(
                "failed to copy {} to {}",
                new.display(),
                dest.display()
            ))?;

        tracing::debug!("replaced new exe: {}", dest.display());

        // sweep best-effort now the new exe is in place; a retired file still
        // mapped by a running process simply waits for the next replacement
        if let Some(bin_dir) = dest.parent() {
            let _ = crate::fs::remove_dir_all(bin_dir.join(TRASH_DIR));
        }
        let _ = std::fs::remove_file(&retired);
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = std::fs::remove_file(&dest);
        std::fs::copy(new, &dest).into_diagnostic()?;
        std::fs::set_permissions(&dest, std::fs::Permissions::from_mode(0o755))
            .into_diagnostic()?;
        tracing::debug!("replaced new exe: {}", dest.display());
    }

    Ok(())
}

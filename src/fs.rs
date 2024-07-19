use miette::IntoDiagnostic;
use rattler_digest::{HashingReader, Sha256, Sha256Hash};
use std::fs::File;
use std::io::{copy, BufReader, Read};
use std::path::Path;
use tokio::io::AsyncRead;
use tokio_util::io::SyncIoBridge;

fn save_file_sync(stream: impl Read, destination: &Path) -> miette::Result<Sha256Hash> {
    std::fs::create_dir_all(destination.parent().expect("invalid destination"))
        .into_diagnostic()?;

    let mut file = File::create(destination).into_diagnostic()?;
    let mut sha256_reader = HashingReader::<_, Sha256>::new(BufReader::new(stream));

    copy(&mut sha256_reader, &mut file).into_diagnostic()?;

    let (_, sha256) = sha256_reader.finalize();

    Ok(sha256)
}

pub async fn save_file(
    reader: impl AsyncRead + Send + 'static,
    destination: &Path,
) -> miette::Result<Sha256Hash> {
    // Create a async -> sync bridge
    let reader = SyncIoBridge::new(Box::pin(reader));

    let destination = destination.to_owned();
    match tokio::task::spawn_blocking(move || save_file_sync(reader, &destination)).await {
        Ok(result) => result,
        Err(err) => Err(err).into_diagnostic(),
    }
}
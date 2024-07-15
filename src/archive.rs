use miette::IntoDiagnostic;
use std::io::Read;
use std::path::Path;
use tokio::io::AsyncRead;
use tokio_util::io::SyncIoBridge;
use zip::read::read_zipfile_from_stream;

pub fn stream_tar_gz(reader: impl Read) -> tar::Archive<impl Read + Sized> {
    tar::Archive::new(flate2::read::GzDecoder::new(reader))
}

fn extract_tar_gz_sync(reader: impl Read, destination: &Path) -> miette::Result<()> {
    std::fs::create_dir_all(destination).into_diagnostic()?;

    let mut reader = std::io::BufReader::new(reader);

    stream_tar_gz(&mut reader)
        .unpack(destination)
        .into_diagnostic()?;

    Ok(())
}

pub async fn extract_tar_gz(
    reader: impl AsyncRead + Send + 'static,
    destination: &Path,
) -> miette::Result<()> {
    let reader = SyncIoBridge::new(Box::pin(reader));

    let destination = destination.to_owned();
    match tokio::task::spawn_blocking(move || extract_tar_gz_sync(reader, &destination)).await {
        Ok(result) => result,
        Err(err) => Err(err).into_diagnostic(),
    }
}

fn extract_zip_sync(reader: impl Read, destination: &Path) -> miette::Result<()> {
    std::fs::create_dir_all(destination).into_diagnostic()?;

    let mut reader = std::io::BufReader::new(reader);

    while let Some(file) = read_zipfile_from_stream(&mut reader).into_diagnostic()? {
        let path = file.mangled_name();
        let path = destination.join(path);

        if file.is_dir() {
            std::fs::create_dir_all(&path).into_diagnostic()?;
        } else {
            std::fs::create_dir_all(path.parent().unwrap()).into_diagnostic()?;
            let mut file = file;
            let mut dest = std::fs::File::create(&path).into_diagnostic()?;
            std::io::copy(&mut file, &mut dest).into_diagnostic()?;
        }
    }

    Ok(())
}

pub async fn extract_zip(
    reader: impl AsyncRead + Send + 'static,
    destination: &Path,
) -> miette::Result<()> {
    let reader = SyncIoBridge::new(Box::pin(reader));

    let destination = destination.to_owned();
    match tokio::task::spawn_blocking(move || extract_zip_sync(reader, &destination)).await {
        Ok(result) => result,
        Err(err) => Err(err).into_diagnostic(),
    }
}

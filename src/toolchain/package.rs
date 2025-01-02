use miette::{Context, IntoDiagnostic};
use std::sync::Arc;
use url::Url;

use crate::{
    archive::{extract_tar_gz, extract_zip},
    fs::save_file,
    reporter::{ProgressReporter, Reporter},
    utils::{build_http_client, path_to_reader, url_to_reader},
};

use super::index::ReleaseCombined;

pub async fn populate_package(release: &ReleaseCombined) -> miette::Result<()> {
    if release.core.is_none() && release.toolchain.is_none() {
        return Ok(());
    }

    let downloads_dir = crate::moonup_home().join("downloads");
    let toolchains_dir = crate::moonup_home().join("toolchains");

    if let Some(toolchain) = release.toolchain.as_ref() {
        let version = toolchain.version.as_str();

        let downloads_version_dir = downloads_dir.join(version);
        let pkg_toolchain = downloads_version_dir.join(&toolchain.name);
        let mut destination = toolchains_dir.join({
            match release.latest {
                true => "latest",
                false => version,
            }
        });

        crate::fs::empty_dir(&destination)
            .into_diagnostic()
            .wrap_err(format!(
                "failed to delete old toolchain: {}",
                destination.display()
            ))
            .wrap_err(
                "files may be in use, please close applications using moonbit and try again",
            )?;

        let reader = if let Ok(reader) = path_to_reader(&pkg_toolchain).await {
            reader
        } else {
            let client = build_http_client();
            let url = format!(
                "https://github.com/chawyehsu/moonbit-binaries/releases/download/v{}/{}",
                version,
                toolchain.name.as_str()
            );

            let progress_reporter = ProgressReporter::new("Downloading toolchain".to_string());
            let reporter = Some(Arc::new(progress_reporter) as Arc<dyn Reporter>);

            let reader = url_to_reader(Url::parse(&url).unwrap(), client, reporter.clone()).await?;
            let sha256 = format!("{:x}", save_file(reader, &pkg_toolchain).await?);

            if let Some(reporter) = &reporter {
                reporter.on_complete();
            }

            if sha256 != toolchain.sha256 {
                let msg = format!(
                    "SHA256 checksum mismatch for file {}\nExpected: {}\n  Actual: {}",
                    toolchain.name, toolchain.sha256, sha256
                );
                let err = std::io::Error::new(std::io::ErrorKind::InvalidData, msg);
                return Err(err).into_diagnostic();
            }

            path_to_reader(&pkg_toolchain).await?
        };

        let extension = pkg_toolchain.extension().unwrap().to_str().unwrap();
        match extension {
            "gz" => extract_tar_gz(reader, &destination).await?,
            "zip" => extract_zip(reader, &destination).await?,
            _ => unreachable!("unsupported extension"),
        }

        let bin_subdir = destination.join("bin");
        if !bin_subdir.is_dir() {
            tracing::debug!("old toolchain archive layout detected (version: {version})");
            // older toolchains (<= v0.1.20241223+62b9a1a85) don't have a `bin` subdirectory,
            // move all files into a `bin` subdirectory
            let files = std::fs::read_dir(&destination)
                .into_diagnostic()
                .wrap_err(format!(
                    "failed to read directory: {}",
                    destination.display()
                ))?;

            std::fs::create_dir(&bin_subdir)
                .into_diagnostic()
                .wrap_err(format!(
                    "failed to create directory: {}",
                    bin_subdir.display()
                ))?;

            for f in files {
                let f = f
                    .into_diagnostic()
                    .wrap_err("failed to read directory entry")?;
                let path = f.path();
                let name = path.file_name().unwrap();
                let new_path = bin_subdir.join(name);
                std::fs::rename(&path, &new_path)
                    .into_diagnostic()
                    .wrap_err(format!(
                        "failed to move file from {} to {}",
                        path.display(),
                        new_path.display()
                    ))?;
            }
        }

        // create a stub to store the actual version of the `latest` toolchain
        if release.latest {
            destination.push("version");
            tokio::fs::write(&destination, format!("{}\n", version))
                .await
                .into_diagnostic()?;
        }
    }

    if let Some(core) = release.core.as_ref() {
        let version = core.version.as_str();

        let downloads_version_dir = downloads_dir.join(version);
        let pkg_core = downloads_version_dir.join(&core.name);
        let destination = toolchains_dir
            .join({
                match release.latest {
                    true => "latest",
                    false => version,
                }
            })
            .join("lib");

        let old_core = destination.join("core");
        crate::fs::remove_dir_all(&old_core)
            .into_diagnostic()
            .wrap_err(format!("failed to delete old core: {}", old_core.display()))
            .wrap_err(
                "files may be in use, please close applications using moonbit and try again",
            )?;

        let reader = if let Ok(reader) = path_to_reader(&pkg_core).await {
            reader
        } else {
            let client = build_http_client();
            let url = format!(
                "https://github.com/chawyehsu/moonbit-binaries/releases/download/v{}/{}",
                version,
                core.name.as_str()
            );

            let progress_reporter = ProgressReporter::new("Downloading core".to_string());
            let reporter = Some(Arc::new(progress_reporter) as Arc<dyn Reporter>);

            let reader = url_to_reader(Url::parse(&url).unwrap(), client, reporter.clone()).await?;
            let sha256 = format!("{:x}", save_file(reader, &pkg_core).await?);

            if let Some(reporter) = &reporter {
                reporter.on_complete();
            }

            if sha256 != core.sha256 {
                let msg = format!(
                    "SHA256 checksum mismatch for file {}\nExpected: {}\n  Actual: {}",
                    core.name, core.sha256, sha256
                );
                let err = std::io::Error::new(std::io::ErrorKind::InvalidData, msg);
                return Err(err).into_diagnostic();
            }

            path_to_reader(&pkg_core).await?
        };

        extract_zip(reader, &destination).await?;
    }

    Ok(())
}

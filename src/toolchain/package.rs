use miette::{Context, IntoDiagnostic};
use std::sync::Arc;

use crate::{
    archive::{extract_tar_gz, extract_zip},
    fs::save_file,
    reporter::{ProgressReporter, Reporter},
    utils::{build_dist_server_api, build_http_client, path_to_reader, url_to_reader},
};

use super::index::InstallRecipe;

pub async fn populate_install(recipe: &InstallRecipe) -> miette::Result<()> {
    let mut download_dir = crate::moonup_home();
    download_dir.push("downloads");

    let mut install_dir_root = crate::moonup_home();
    install_dir_root.push("toolchains");

    // version release tag
    let mut tag = format!("v{}", recipe.release.version.as_str());

    if let Some(date) = recipe.release.date.as_ref() {
        download_dir.push("nightly");
        download_dir.push(date);

        let tag_nightly = format!("nightly-{}", date);
        tag = tag_nightly;

        if recipe.spec.is_nightly() {
            install_dir_root.push("nightly");
        } else {
            install_dir_root.push(&tag);
        }
    } else {
        let version = recipe.release.version.as_str();
        download_dir.push(version);

        if recipe.spec.is_latest() {
            install_dir_root.push("latest");
        } else {
            install_dir_root.push(version);
        }
    }

    crate::fs::empty_dir(&install_dir_root)
        .into_diagnostic()
        .wrap_err(format!("Failed to delete {}", install_dir_root.display()))
        .wrap_err("Unable to clean up existing installation, files may be in use")?;

    // ensure all components are downloaded in the first loop
    for component in recipe.components.iter() {
        let name = component.name.as_str();
        let file = component.file.as_str();
        let sha256_expected = component.sha256.as_str();

        let local_file = download_dir.join(file);

        if let Err(e) = path_to_reader(&local_file).await {
            tracing::trace!("failed to read local file: {}", e);
            tracing::debug!("downloading {} to {}", name, local_file.display());

            let client = build_http_client();

            let pathname = format!("/download/{}/{}", tag, file);
            let url = build_dist_server_api(&pathname)?;

            let progress_reporter = ProgressReporter::new(format!("Downloading {}", name));
            let reporter = Some(Arc::new(progress_reporter) as Arc<dyn Reporter>);

            let reader = url_to_reader(url, &client, reporter.clone()).await?;
            let sha256_actual = format!("{:x}", save_file(reader, &local_file).await?);

            if let Some(reporter) = &reporter {
                reporter.on_complete();
            }

            if sha256_actual != sha256_expected {
                let msg = format!(
                    "Checksum mismatch for file {}\nExpected: {}\n  Actual: {}",
                    file, sha256_expected, sha256_actual
                );

                // remove the downloaded invalid file
                let _ = std::fs::remove_file(&local_file).inspect_err(|e| {
                    tracing::debug!("failed to remove invalid download: {}", e);
                });

                let err = std::io::Error::new(std::io::ErrorKind::InvalidData, msg);
                return Err(err).into_diagnostic();
            }
        }
    }

    // do the actual installation in the second loop
    for component in recipe.components.iter() {
        let mut component_install_dir = install_dir_root.clone();
        let name = component.name.as_str();
        let file = component.file.as_str();
        let sha256_expected = component.sha256.as_str();

        let local_file = download_dir.join(file);
        tracing::debug!("installing {} from {}", name, local_file.display());

        let reader = path_to_reader(&local_file)
            .await
            .wrap_err("failed to read local file")?;

        // older toolchains (<= v0.1.20241223+62b9a1a85) don't have a `bin` subdirectory,
        // install all toolchain files into the `bin` subdirectory
        if name == "toolchain" && recipe.release.layout_version1.unwrap_or(false) {
            let version = recipe.release.version.as_str();
            tracing::debug!("old toolchain archive layout detected (version: {version})");
            component_install_dir.push("bin");
        }

        // the core library distribution does not have a `lib` top-level directory
        if name == "libcore" {
            component_install_dir.push("lib");
        }

        let is_zip = file.ends_with(".zip");
        let sha256 = match is_zip {
            true => extract_zip(reader, &component_install_dir).await?,
            false => extract_tar_gz(reader, &component_install_dir).await?,
        };

        let sha256_actual = format!("{:x}", sha256);

        if sha256_actual != sha256_expected {
            let msg = format!(
                "Checksum mismatch for file {}\nExpected: {}\n  Actual: {}",
                file, sha256_expected, sha256_actual
            );

            // remove the downloaded invalid file
            let _ = std::fs::remove_file(&local_file).inspect_err(|e| {
                tracing::debug!("failed to remove invalid component download: {}", e);
            });
            // clean up the invalid installation
            let _ = crate::fs::empty_dir(&install_dir_root).inspect_err(|e| {
                tracing::debug!("failed to clean up invalid installation: {}", e);
            });

            let err = std::io::Error::new(std::io::ErrorKind::InvalidData, msg);
            return Err(err).into_diagnostic();
        }
    }

    // create a stub to store the actual version when the spec is latest or nightly
    if recipe.spec.is_latest() {
        let actual_version = recipe.release.version.as_str();
        install_dir_root.push("version");
        tokio::fs::write(&install_dir_root, format!("{}\n", actual_version))
            .await
            .into_diagnostic()?;
    } else if recipe.spec.is_nightly() {
        let actual_date = recipe.release.date.as_ref().expect("should have a date");
        install_dir_root.push("version");
        tokio::fs::write(&install_dir_root, format!("{}\n", actual_date))
            .await
            .into_diagnostic()?;
    }

    Ok(())
}

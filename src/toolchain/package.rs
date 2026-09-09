use miette::{Context, IntoDiagnostic};
use std::sync::Arc;

use crate::{
    archive::{extract_tar_gz, extract_zip},
    fs::{compute_file_sha256, save_file},
    reporter::{ProgressReporter, Reporter},
    toolchain::ToolchainSpec,
    utils::{build_dist_server_api, build_http_client_with_retry, path_to_reader, url_to_reader},
};

use super::{atomic, index::InstallRecipe};

pub async fn populate_install(recipe: &InstallRecipe) -> miette::Result<()> {
    let mut download_dir = crate::moonup_home();
    download_dir.push("downloads");

    // GitHub release tag
    let tag = match recipe.spec {
        ToolchainSpec::Bleeding => {
            download_dir.push("bleeding");

            "bleeding".to_string()
        }
        ToolchainSpec::Nightly => {
            let date = recipe.release.date.as_deref().expect("should have date");

            download_dir.push("nightly");
            download_dir.push(date);

            format!("nightly-{date}")
        }
        ToolchainSpec::Latest => {
            let version = recipe.release.version.as_str();

            download_dir.push("latest");
            download_dir.push(version);

            format!("v{}", version)
        }
        ToolchainSpec::Version(ref v) => {
            if v.starts_with("nightly-") {
                let date = recipe.release.date.as_deref().expect("should have date");

                download_dir.push("nightly");
                download_dir.push(date);

                v.to_owned()
            } else {
                download_dir.push("latest");
                download_dir.push(v);

                format!("v{}", v)
            }
        }
    };

    let live_dir = recipe.spec.install_path();
    let staging_dir = atomic::staging_dir_for(&recipe.spec);

    // A previous run may have finished assembling the staging directory but
    // failed while swapping it into place; retry the swap rather than
    // discarding the fully-assembled toolchain, as long as it holds the same
    // release the recipe resolves to.
    if atomic::is_complete(&recipe.spec) && atomic::staged_matches(&recipe.spec, recipe) {
        atomic::swap(&live_dir, &staging_dir)?;
        return Ok(());
    }

    // Assemble the new toolchain in a staging directory and swap it into the
    // live location only once it is fully assembled. A failed run leaves a
    // discarded staging directory, never a damaged live toolchain. A stale
    // completeness marker is cleaned too, so a partial extraction is never
    // misread as complete.
    crate::fs::remove_dir_all(&staging_dir)
        .into_diagnostic()
        .wrap_err(format!(
            "failed to clean the staging directory {}",
            staging_dir.display()
        ))?;
    let _ = std::fs::remove_file(atomic::completeness_marker_for(&recipe.spec));

    let is_bleeding = recipe.spec.is_bleeding();

    // ensure all components are downloaded in the first loop
    for component in recipe.components.iter() {
        let name = component.name.as_str();
        let file = component.file.as_str();
        let sha256_expected = component.sha256.as_str();

        let local_file = download_dir.join(file);

        let mut use_cache = false;
        if !is_bleeding && local_file.exists() {
            match compute_file_sha256(&local_file).await {
                Ok(sha256) => {
                    let sha256_actual = hex::encode(sha256);
                    if sha256_actual == sha256_expected {
                        tracing::debug!("cache hit for {} at {}", name, local_file.display());
                        use_cache = true;
                    } else {
                        tracing::debug!(
                            "cache checksum mismatch for {} at {}, redownloading",
                            name,
                            local_file.display()
                        );
                        let _ = std::fs::remove_file(&local_file).inspect_err(|e| {
                            tracing::debug!("failed to remove invalid cache file: {}", e);
                        });
                    }
                }
                Err(e) => {
                    tracing::debug!(
                        "failed to verify cache for {} at {}: {}, redownloading",
                        name,
                        local_file.display(),
                        e
                    );
                    let _ = std::fs::remove_file(&local_file).inspect_err(|e| {
                        tracing::debug!("failed to remove unreadable cache file: {}", e);
                    });
                }
            }
        }

        if is_bleeding || !use_cache {
            tracing::debug!("downloading {} to {}", name, local_file.display());

            let client = build_http_client_with_retry();

            let pathname = format!("/download/{}/{}", tag, file);
            let url = build_dist_server_api(&pathname)?;

            let progress_reporter = ProgressReporter::new(format!("Downloading {}", name));
            let reporter = Some(Arc::new(progress_reporter) as Arc<dyn Reporter>);

            let reader = url_to_reader(url, &client, reporter.clone()).await?;
            let sha256_actual = hex::encode(save_file(reader, &local_file).await?);

            if let Some(reporter) = &reporter {
                reporter.on_complete();
            }

            if sha256_actual != sha256_expected {
                let msg = format!(
                    "Checksum mismatch for file {}\nExpected: {}\n  Actual: {}\n\nPlease try again.",
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
        let mut component_install_dir = staging_dir.clone();
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

        let sha256_actual = hex::encode(sha256);

        if sha256_actual != sha256_expected {
            let msg = format!(
                "Checksum mismatch for file {}\nExpected: {}\n  Actual: {}\n\nPlease try again.",
                file, sha256_expected, sha256_actual
            );

            // remove the downloaded invalid file
            let _ = std::fs::remove_file(&local_file).inspect_err(|e| {
                tracing::debug!("failed to remove invalid component download: {}", e);
            });
            // discard the invalid staging directory
            let _ = crate::fs::remove_dir_all(&staging_dir).inspect_err(|e| {
                tracing::debug!("failed to clean up invalid staging directory: {}", e);
            });

            let err = std::io::Error::new(std::io::ErrorKind::InvalidData, msg);
            return Err(err).into_diagnostic();
        }
    }

    // create a stub to store the actual version when the spec is latest or nightly
    if recipe.spec.is_latest() || recipe.spec.is_bleeding() {
        let actual_version = recipe.release.version.as_str();
        let version_file = staging_dir.join("version");
        tokio::fs::write(&version_file, format!("{}\n", actual_version))
            .await
            .into_diagnostic()?;
    } else if recipe.spec.is_nightly() {
        let actual_date = recipe.release.date.as_ref().expect("should have a date");
        let version_file = staging_dir.join("version");
        tokio::fs::write(&version_file, format!("{}\n", actual_date))
            .await
            .into_diagnostic()?;
    }

    // mark the staging directory as fully assembled, recording the release
    // identity, so that recovery never promotes a partial extraction and can
    // finalize the promoted toolchain from its own metadata
    let marker = atomic::completeness_marker_for(&recipe.spec);
    let staged = atomic::StagedRelease::from_recipe(recipe);
    let marker_json = serde_json::to_string(&staged)
        .into_diagnostic()
        .wrap_err("failed to serialize staged release metadata")?;
    tokio::fs::write(&marker, marker_json)
        .await
        .into_diagnostic()?;

    atomic::swap(&live_dir, &staging_dir)?;

    Ok(())
}

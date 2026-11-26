use miette::{Context, IntoDiagnostic};
use std::sync::Arc;

use crate::{
    archive::{extract_tar_gz, extract_zip},
    fs::{compute_file_sha256, save_file},
    reporter::{ProgressReporter, Reporter},
    session::Session,
    utils::{build_dist_server_api, path_to_reader, url_to_reader},
};

use super::{atomic, install::InstallRecipe};

pub async fn populate_install(session: &Session, recipe: &InstallRecipe) -> miette::Result<()> {
    atomic::ensure_install_path_available(&recipe.spec)?;

    // A previous run may have finished assembling the staging directory but
    // failed while swapping it into place; retry the swap rather than
    // discarding the fully-assembled toolchain, as long as it holds the same
    // release the recipe resolves to.
    if atomic::staged_matches(&recipe.spec, recipe)? {
        atomic::swap(&recipe.spec)?;
        return Ok(());
    }

    let staging_dir = atomic::staging_dir_for(&recipe.spec);
    // Discard staging leftovers from any previous run — a partial extraction
    // or a staging left by an older moonup, neither of which carries a
    // finalization payload — then assemble the new toolchain in a staging
    // directory and swap it into the live location only once it is fully
    // assembled. A failed run leaves a discarded staging directory, never a
    // damaged live toolchain.
    atomic::sweep_staging(&recipe.spec);
    crate::fs::remove_dir_all(&staging_dir)
        .into_diagnostic()
        .wrap_err(format!(
            "failed to clean the staging directory {}",
            staging_dir.display()
        ))?;

    let mut download_dir = session.downloads_dir();

    let channel = recipe.spec.channel().as_str();
    let release_key = recipe.release.key().to_owned();

    download_dir.push(channel);
    download_dir.push(&release_key);

    // ensure all components are downloaded in the first loop
    for component in recipe.components.iter() {
        let name = component.name.as_str();
        let file = component.file.as_str();
        let sha256_expected = component.sha256.as_str();

        let local_file = download_dir.join(file);

        let mut use_cache = false;
        if local_file.exists() {
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

        if !use_cache {
            tracing::debug!("downloading {} to {}", name, local_file.display());

            // GitHub release tag: dated releases publish as `{channel}-{date}`,
            // versioned releases as `v{version}`.
            let tag = match recipe.release.date.as_ref() {
                Some(date) => format!("{channel}-{date}"),
                None => format!("v{}", recipe.release.version),
            };

            let client = session.http_client();

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

    // Every install records its identity in the install record inside the
    // staging directory. The record carries the finalization inputs so the
    // post-install steps never depend on the network, and the presence of its
    // finalization payload marks the staging as complete and ready to
    // promote; `acknowledge` rewrites it without the payload once the
    // post-install steps succeed.
    super::InstallMeta::from(recipe).save(&staging_dir)?;

    atomic::swap(&recipe.spec)?;

    Ok(())
}

//! A cross-process advisory lock serializing toolchain mutations.
//!
//! moonup operations that change the installed toolchains — `install`,
//! `update`, `uninstall`, and the recovery they run — hold an exclusive
//! advisory lock on `toolchains/.lock` for their duration, so a
//! shim-triggered auto-install waits for a running `update` instead of
//! racing it around the same staging paths. The lock is advisory (see
//! [`std::fs::File::lock`]): it is released automatically when the holding
//! process exits or dies, so a killed moonup never leaves a stuck lock
//! behind.

use std::fs::{File, OpenOptions};

use miette::{Context, IntoDiagnostic};

use crate::session::Session;

/// The held toolchain mutation lock; dropping it releases the lock.
pub(crate) struct ToolchainLock {
    _file: File,
}

impl ToolchainLock {
    /// Acquire the toolchain mutation lock, blocking until any other moonup
    /// process finishes its own.
    pub(crate) fn acquire(session: &Session) -> miette::Result<Self> {
        let path = session.toolchains_lock_file();
        std::fs::create_dir_all(path.parent().expect("lock path should have a parent"))
            .into_diagnostic()
            .wrap_err("failed to create the toolchains directory")?;

        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)
            .into_diagnostic()
            .wrap_err_with(|| format!("failed to open the lock file {}", path.display()))?;

        if file.try_lock().is_err() {
            eprintln!("waiting for another moonup operation to finish...");
            file.lock()
                .into_diagnostic()
                .wrap_err("failed to acquire the toolchain lock")?;
        }

        Ok(Self { _file: file })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lock_is_exclusive_across_threads() {
        let tempdir = assert_fs::TempDir::new().expect("should create tempdir");
        temp_env::with_var(
            crate::constant::ENVNAME_MOONUP_HOME,
            Some(tempdir.path().as_os_str()),
            || {
                let session = crate::session::Session::new();
                let first = ToolchainLock::acquire(&session).expect("should acquire the lock");

                // a second acquisition — from another thread and its own
                // session, which resolves the same home from the same
                // environment — must block until the first holder drops
                // its guard
                let (tx, rx) = std::sync::mpsc::channel();
                let handle = std::thread::spawn(move || {
                    let session = crate::session::Session::new();
                    let _second =
                        ToolchainLock::acquire(&session).expect("should acquire after release");
                    tx.send(()).expect("should signal");
                });

                assert!(
                    rx.recv_timeout(std::time::Duration::from_millis(300))
                        .is_err(),
                    "the second acquisition should block while the lock is held"
                );

                drop(first);
                rx.recv_timeout(std::time::Duration::from_secs(5))
                    .expect("second holder should acquire after release");
                handle.join().expect("second holder thread should finish");
            },
        );
    }
}

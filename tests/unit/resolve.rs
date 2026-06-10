use assert_fs::prelude::*;
use moonup::{constant, toolchain::resolve};
use serial_test::serial;
use std::{env, fs};

use crate::util;

#[test]
#[serial]
fn test_resolve_toolchain() {
    util::apply_common_filters!();

    let temp = assert_fs::TempDir::new().unwrap();
    env::set_current_dir(temp.path()).unwrap();
    assert_eq!(resolve::resolve_toolchain_file(), None);

    // Create a toolchain file in the temp directory
    let toolchain_file = temp.child(constant::TOOLCHAIN_FILE);
    toolchain_file.touch().unwrap();
    toolchain_file.assert("");

    let resolved = resolve::resolve_toolchain_file();
    let expected = toolchain_file.to_path_buf();
    #[cfg(target_os = "macos")]
    let expected = expected.canonicalize().expect("should canonicalize");

    assert_eq!(resolved.as_deref(), Some(expected.as_path()));

    fs::write(expected, "latest\n").expect("should write to file");

    assert_eq!(
        resolve::detect_pinned_toolchain(),
        Some("latest".to_string())
    );
}

#[test]
#[serial]
fn test_resolve_file() {
    util::apply_common_filters!();

    let temp = assert_fs::TempDir::new().unwrap();
    env::set_current_dir(temp.path()).unwrap();

    let file = temp.child("notexecutable");
    file.touch().unwrap();

    let paths = env::join_paths([temp.path()]).unwrap();
    let resolved = resolve::resolve_file("notexecutable", &paths);
    assert_eq!(resolved.as_deref(), Some(file.path()));
}

#[test]
#[cfg(target_os = "windows")]
fn test_resolve_exe_without_pathext() {
    util::apply_common_filters!();

    // Remove PATHEXT to simulate it being unset
    unsafe {
        env::remove_var("PATHEXT");
    }

    // `cmd` should still be resolvable via the DEFAULT_PATHEXT fallback
    let sysroot = env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".to_string());
    let paths = env::join_paths([format!(r"{sysroot}\System32")]).unwrap();
    let resolved = resolve::resolve_exe("cmd", &paths);
    assert!(
        resolved.is_some(),
        "resolve_exe should find cmd.exe even without PATHEXT"
    );
}

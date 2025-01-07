use assert_fs::prelude::*;
use moonup::{constant, toolchain::resolve};
use std::{env, fs};

use crate::util;

#[test]
fn test_resolve_toolchain() {
    util::apply_common_filters!();

    let temp = assert_fs::TempDir::new().unwrap();
    env::set_current_dir(temp.path()).unwrap();
    assert_eq!(resolve::resolve_toolchain_file(), None);

    // Create a toolchain file in the temp directory
    let toolchain_file = temp.child(constant::TOOLCHAIN_FILE);
    toolchain_file.touch().unwrap();
    toolchain_file.assert("");

    assert_eq!(
        resolve::resolve_toolchain_file(),
        Some(toolchain_file.to_path_buf())
    );

    fs::write(toolchain_file.path(), "latest\n").expect("should write to file");

    assert_eq!(resolve::detect_pinned_version(), Some("latest".to_string()));
}

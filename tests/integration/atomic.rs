use moonup::{
    constant,
    toolchain::{ToolchainSpec, atomic, installed_toolchains},
};

#[test]
fn test_recover_promotes_complete_staging() {
    let tempdir = assert_fs::TempDir::new().expect("should create tempdir");
    let moonup_home = tempdir.path().join(".moonup");

    temp_env::with_var(
        constant::ENVNAME_MOONUP_HOME,
        Some(moonup_home.as_os_str()),
        || {
            let spec = ToolchainSpec::Latest;
            let live_dir = spec.install_path();
            let staging_dir = atomic::staging_dir_for(&spec);

            // simulate a crash left a fully assembled staging directory
            std::fs::create_dir_all(staging_dir.join("bin")).expect("should create staging dir");
            std::fs::write(staging_dir.join("bin").join("moon"), b"moon")
                .expect("should write staging content");
            std::fs::write(atomic::completeness_marker_for(&spec), "")
                .expect("should write completeness marker");

            assert!(!live_dir.exists());

            let recovered = atomic::recover(&spec).expect("recovery should not fail");
            assert!(recovered, "complete staging should be promoted");

            assert!(
                live_dir.join("bin").join("moon").exists(),
                "staging should be promoted to the live location"
            );
            assert!(
                !staging_dir.exists(),
                "staging dir should be consumed by the swap"
            );
        },
    );
}

#[test]
fn test_recover_does_not_promote_incomplete_staging() {
    let tempdir = assert_fs::TempDir::new().expect("should create tempdir");
    let moonup_home = tempdir.path().join(".moonup");

    temp_env::with_var(
        constant::ENVNAME_MOONUP_HOME,
        Some(moonup_home.as_os_str()),
        || {
            let spec = ToolchainSpec::Latest;

            // a staging dir without the completeness marker was cut short
            let staging_dir = atomic::staging_dir_for(&spec);
            std::fs::create_dir_all(staging_dir.join("bin")).expect("should create staging dir");

            let recovered = atomic::recover(&spec).expect("recovery should not fail");
            assert!(!recovered, "partial staging should not be promoted");
            assert!(
                !spec.install_path().exists(),
                "no live toolchain should exist"
            );
        },
    );
}

#[test]
fn test_installed_toolchains_excludes_staging() {
    let tempdir = assert_fs::TempDir::new().expect("should create tempdir");
    let moonup_home = tempdir.path().join(".moonup");

    temp_env::with_var(
        constant::ENVNAME_MOONUP_HOME,
        Some(moonup_home.as_os_str()),
        || {
            let toolchains_dir = moonup_home.join("toolchains");
            std::fs::create_dir_all(toolchains_dir.join("latest"))
                .expect("should create toolchain dir");
            std::fs::write(toolchains_dir.join("latest").join("version"), "0.1.0\n")
                .expect("should write version stub");
            std::fs::create_dir_all(atomic::staging_dir_for(&ToolchainSpec::Latest))
                .expect("should create staging dir");

            let installed = installed_toolchains().expect("should list installed toolchains");
            assert_eq!(installed.len(), 1);
            assert_eq!(installed[0].name, ToolchainSpec::Latest);
        },
    );
}

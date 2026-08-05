use moonup::{
    constant,
    dist_server::schema::Release,
    toolchain::{ToolchainSpec, atomic, index::InstallRecipe, installed_toolchains},
};

fn staged_marker(spec: &ToolchainSpec, json: &str) {
    std::fs::write(atomic::completeness_marker_for(spec), json).expect("should write marker");
}

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
            staged_marker(&spec, r#"{"version":"0.1.0"}"#);

            assert!(!live_dir.exists());

            let staged = atomic::recover(&spec).expect("recovery should not fail");
            assert!(staged.is_some(), "complete staging should be promoted");
            assert_eq!(staged.unwrap().version, "0.1.0");

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

            let staged = atomic::recover(&spec).expect("recovery should not fail");
            assert!(staged.is_none(), "partial staging should not be promoted");
            assert!(
                !spec.install_path().exists(),
                "no live toolchain should exist"
            );
        },
    );
}

#[test]
fn test_swap_promotes_and_retires() {
    let tempdir = assert_fs::TempDir::new().expect("should create tempdir");
    let moonup_home = tempdir.path().join(".moonup");

    temp_env::with_var(
        constant::ENVNAME_MOONUP_HOME,
        Some(moonup_home.as_os_str()),
        || {
            let spec = ToolchainSpec::Latest;
            let live_dir = spec.install_path();
            let staging_dir = atomic::staging_dir_for(&spec);
            let retired_dir = moonup_home
                .join("toolchains")
                .join(".staging")
                .join("latest.old");

            std::fs::create_dir_all(live_dir.join("bin")).expect("should create live dir");
            std::fs::write(live_dir.join("bin").join("moon"), b"old")
                .expect("should write live content");
            std::fs::create_dir_all(staging_dir.join("bin")).expect("should create staging dir");
            std::fs::write(staging_dir.join("bin").join("moon"), b"new")
                .expect("should write staging content");

            atomic::swap(&live_dir, &staging_dir).expect("swap should succeed");

            assert_eq!(
                std::fs::read_to_string(live_dir.join("bin").join("moon"))
                    .expect("should read live moon"),
                "new"
            );
            assert!(!staging_dir.exists(), "staging should be consumed");
            assert!(!retired_dir.exists(), "retired dir should be removed");
        },
    );
}

#[test]
fn test_swap_restores_retired_on_promotion_failure() {
    let tempdir = assert_fs::TempDir::new().expect("should create tempdir");
    let moonup_home = tempdir.path().join(".moonup");

    temp_env::with_var(
        constant::ENVNAME_MOONUP_HOME,
        Some(moonup_home.as_os_str()),
        || {
            let spec = ToolchainSpec::Latest;
            let live_dir = spec.install_path();
            let staging_dir = atomic::staging_dir_for(&spec);
            let retired_dir = moonup_home
                .join("toolchains")
                .join(".staging")
                .join("latest.old");

            // crash state: the previous live toolchain was moved to `.old`,
            // the promotion never happened, and the staging is gone
            std::fs::create_dir_all(retired_dir.join("bin")).expect("should create retired dir");
            std::fs::write(retired_dir.join("bin").join("moon"), b"old")
                .expect("should write retired content");

            let result = atomic::swap(&live_dir, &staging_dir);
            assert!(
                result.is_err(),
                "promotion of a missing staging should fail"
            );

            assert!(
                live_dir.join("bin").join("moon").exists(),
                "retired toolchain should be restored to the live name"
            );
            assert_eq!(
                std::fs::read_to_string(live_dir.join("bin").join("moon"))
                    .expect("should read restored moon"),
                "old"
            );
            assert!(
                !retired_dir.exists(),
                "retired dir should be consumed by the restore"
            );
        },
    );
}

#[test]
fn test_staged_matches_requires_same_release() {
    let tempdir = assert_fs::TempDir::new().expect("should create tempdir");
    let moonup_home = tempdir.path().join(".moonup");

    temp_env::with_var(
        constant::ENVNAME_MOONUP_HOME,
        Some(moonup_home.as_os_str()),
        || {
            let spec = ToolchainSpec::Latest;
            std::fs::create_dir_all(atomic::staging_dir_for(&spec))
                .expect("should create staging dir");
            staged_marker(&spec, r#"{"version":"0.1.0"}"#);

            let recipe = |version: &str| InstallRecipe {
                spec: spec.clone(),
                release: Release {
                    version: version.to_string(),
                    layout_version1: None,
                    bundle_source_dir: None,
                    date: None,
                    targets: None,
                },
                components: Vec::new(),
            };

            assert!(
                atomic::staged_matches(&spec, &recipe("0.1.0")),
                "matching release should be promoted"
            );
            assert!(
                !atomic::staged_matches(&spec, &recipe("0.2.0")),
                "a stale staging must not be promoted over a newer release"
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

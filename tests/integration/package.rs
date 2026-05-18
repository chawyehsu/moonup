use moonup::{
    constant,
    dist_server::schema::{Component, Release},
    toolchain::{ToolchainSpec, index::InstallRecipe, package::populate_install},
};

#[test]
fn test_populate_install_redownloads_invalid_cache() {
    let tempdir = assert_fs::TempDir::new().expect("should create tempdir");
    let moonup_home = tempdir.path().join(".moonup");

    let version = "0.1.20241231+ba15a9a4e";
    let archive_file = "test.tar.gz";
    let archive_data = include_bytes!("../fixtures/archive/test.tar.gz");
    let expected_sha256 = "65f77ae8d172385a19157f338ca63f6cdb836e1fce82751c2ea8d7e5c7991823";

    let mut server = mockito::Server::new();
    let download_path = format!("/download/v{version}/{archive_file}");
    let _mock_download = server
        .mock("GET", download_path.as_str())
        .with_body(archive_data.as_ref())
        .expect(2)
        .create();

    let recipe = InstallRecipe {
        spec: ToolchainSpec::Version(version.to_string()),
        release: Release {
            version: version.to_string(),
            layout_version1: None,
            bundle_source_dir: None,
            date: None,
            targets: None,
        },
        components: vec![Component {
            name: "toolchain".to_string(),
            file: archive_file.to_string(),
            sha256: expected_sha256.to_string(),
        }],
    };

    temp_env::with_var(
        constant::ENVNAME_MOONUP_DIST_SERVER,
        Some(server.url()),
        || {
            temp_env::with_var(
                constant::ENVNAME_MOONUP_HOME,
                Some(moonup_home.as_os_str()),
                || {
                    let rt = tokio::runtime::Runtime::new().expect("should create runtime");
                    rt.block_on(async {
                        populate_install(&recipe)
                            .await
                            .expect("first install should succeed");

                        let cache_file = moonup_home
                            .join("downloads")
                            .join("latest")
                            .join(version)
                            .join(archive_file);
                        assert!(cache_file.exists(), "cache file should exist");

                        std::fs::write(&cache_file, b"corrupted")
                            .expect("should overwrite cache with corrupted data");

                        populate_install(&recipe)
                            .await
                            .expect("second install should redownload and succeed");

                        let installed_file = moonup_home
                            .join("toolchains")
                            .join(version)
                            .join("world.txt");
                        assert!(
                            installed_file.exists(),
                            "installation should still succeed after cache recovery"
                        );
                    });
                },
            );
        },
    );
}

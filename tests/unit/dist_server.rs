use moonup::dist_server::schema::{
    Channel, ChannelIndex, ComponentIndex, Index, Release, Target, VersionedChannelIndex,
    VersionedIndex,
};

#[test]
fn test_handle_future_channel() {
    let json = r#"
    {
        "name": "lts",
        "version": "1.0.0"
    }"#;

    let channel = serde_json::from_str::<Channel>(json).expect("should parse channel successfully");

    assert_eq!(channel.name.as_str(), "lts");
}

#[test]
fn test_schema_v2_handle_future_target() {
    let json = r#"
    {
        "version": 2,
        "lastModified": "20250525T1906552765Z",
        "channels": [
            {
                "name": "latest",
                "version": "0.1.20250522+2b70d2531"
            }
        ],
        "targets": [
            "aarch64-apple-darwin",
            "x86_64-apple-darwin",
            "x86_64-unknown-linux",
            "x86_64-pc-windows",
            "riscv64-unknown-linux"
        ]
    }"#;

    let index = serde_json::from_str::<Index>(json).expect("should parse json successfully");

    let inner = match index {
        Index::Versioned(VersionedIndex::V2(i)) => Some(i),
        _ => None,
    };

    let index = inner.expect("should be v2 index");

    assert_eq!(
        index.targets.last(),
        Some(&Target::Unknown("riscv64-unknown-linux".to_string()))
    );
}

#[test]
fn test_schema_v3_index_parse() {
    let json = r#"
    {
        "version": 3,
        "lastModified": "20250525T1906552765Z",
        "channels": [
            {
                "name": "latest",
                "version": "0.1.20250522+2b70d2531"
            }
        ]
    }"#;

    let index = serde_json::from_str::<Index>(json).expect("should parse json successfully");

    let inner = match index {
        Index::Versioned(VersionedIndex::V3(i)) => Some(i),
        _ => None,
    };

    assert!(inner.is_some());
}

#[test]
fn test_channel_name_and_date_validation_across_index_versions() {
    let legacy_properties = serde_json::json!({
        "last_modified": "20251020T1405571173Z",
        "releases": [{
            "version": "0.1.20240704+647946c28",
            "name": "moonbit",
            "sha256": "deadbeef"
        }]
    });
    let legacy = serde_json::json!({
        "core": legacy_properties.clone(),
        "darwin-arm64": legacy_properties.clone(),
        "darwin-x64": legacy_properties.clone(),
        "linux-x64": legacy_properties.clone(),
        "win-x64": legacy_properties
    });
    assert!(matches!(
        serde_json::from_value::<Index>(legacy),
        Ok(Index::Legacy(_))
    ));

    for version in [2, 3] {
        let valid_index = serde_json::json!({
            "version": version,
            "lastModified": "20251020T1405571173Z",
            "channels": [{
                "name": "nightly-preview",
                "version": "0.1.20240704+647946c28",
                "date": "2024-02-29"
            }],
            "targets": []
        });
        let index = serde_json::from_value::<Index>(valid_index)
            .unwrap_or_else(|error| panic!("v{version} index should parse: {error}"));
        assert_eq!(index.channels()[0].date.unwrap().to_string(), "2024-02-29");

        let invalid_channel = serde_json::json!({
            "version": version,
            "lastModified": "20251020T1405571173Z",
            "channels": [{"name": "Nightly", "version": "1.0.0"}],
            "targets": []
        });
        assert!(
            serde_json::from_value::<Index>(invalid_channel).is_err(),
            "v{version} index should reject an invalid channel name"
        );

        let invalid_channel_date = serde_json::json!({
            "version": version,
            "lastModified": "20251020T1405571173Z",
            "channels": [{
                "name": "nightly",
                "version": "1.0.0",
                "date": "2026-02-31"
            }],
            "targets": []
        });
        assert!(
            serde_json::from_value::<Index>(invalid_channel_date).is_err(),
            "v{version} index should reject an invalid channel date"
        );

        let valid_channel_index = serde_json::json!({
            "version": version,
            "lastModified": "20251020T1405571173Z",
            "releases": [{
                "version": "0.1.20240704+647946c28",
                "date": "2024-02-29"
            }]
        });
        let channel_index = serde_json::from_value::<ChannelIndex>(valid_channel_index)
            .unwrap_or_else(|error| panic!("v{version} channel index should parse: {error}"));
        assert_eq!(channel_index.releases()[0].key(), "2024-02-29");

        let invalid_release_date = serde_json::json!({
            "version": version,
            "lastModified": "20251020T1405571173Z",
            "releases": [{
                "version": "0.1.20240704+647946c28",
                "date": "2026-02-31"
            }]
        });
        assert!(
            serde_json::from_value::<ChannelIndex>(invalid_release_date).is_err(),
            "v{version} channel index should reject an invalid release date"
        );
    }
}

#[test]
fn test_schema_unsupported_index_parse() {
    let json = r#"
    {
        "version": 999,
        "channels": []
    }"#;

    let index = serde_json::from_str::<Index>(json).expect("should parse json successfully");
    let inner = match index {
        Index::Versioned(VersionedIndex::Unsupported) => Some(VersionedIndex::Unsupported),
        _ => None,
    };

    assert!(inner.is_some());

    let json = r#"
    {
        "malformedField": []
    }"#;

    let index = serde_json::from_str::<Index>(json).expect("should parse json successfully");
    let inner = match index {
        Index::Unsupported(value) => Some(value),
        _ => None,
    };

    assert!(inner.is_some());
}

#[test]
fn test_schema_v2_channel_index_parse() {
    let json = r#"
    {
        "version": 2,
        "lastModified": "20251020T1405571173Z",
        "releases": [
            {
                "version": "0.1.20240704+647946c28",
                "layoutVersion1": true
            }
        ]
    }"#;

    let index = serde_json::from_str::<ChannelIndex>(json).expect("should parse json successfully");
    assert_eq!(index.releases().len(), 1);
}

#[test]
fn test_schema_v3_channel_index_parse() {
    let json = r#"
    {
        "version": 3,
        "lastModified": "20251020T1405571173Z",
        "releases": [
            {
                "version": "0.1.20240704+647946c28",
                "layoutVersion1": true,
                "targets": [
                    "aarch64-apple-darwin",
                    "x86_64-apple-darwin",
                    "x86_64-unknown-linux",
                    "x86_64-pc-windows"
                ]
            }
        ]
    }"#;

    let index = serde_json::from_str::<ChannelIndex>(json).expect("should parse json successfully");
    assert_eq!(index.releases().len(), 1);
}

#[test]
fn test_schema_bad_channel_index_parse() {
    let json = r#"
    {
        "lastModified": "20251020T1405571173Z",
        "malformedField": []
    }"#;

    let index = serde_json::from_str::<ChannelIndex>(json).expect("should parse json successfully");
    assert_eq!(index.releases().len(), 0);
}

#[test]
fn test_schema_component_index_empty_parse() {
    let json = r#"
    {
        "version": 2,
        "components": []
    }"#;

    let index =
        serde_json::from_str::<ComponentIndex>(json).expect("should parse json successfully");
    assert_eq!(index.components().len(), 0);
}

#[test]
fn test_schema_bad_component_index_parse() {
    let json = r#"
    {
        "malformedField": []
    }"#;

    let index =
        serde_json::from_str::<ComponentIndex>(json).expect("should parse json successfully");
    assert_eq!(index.components().len(), 0);
}

#[test]
fn test_schema_unsupported_channel_index_parse() {
    let json = r#"
    {
        "version": 999,
        "lastModified": "20251020T1405571173Z",
        "malformedField": []
    }"#;

    let index = serde_json::from_str::<ChannelIndex>(json).expect("should parse json successfully");
    let inner = match index {
        ChannelIndex::Versioned(VersionedChannelIndex::Unsupported) => {
            Some(VersionedChannelIndex::Unsupported)
        }
        _ => None,
    };

    assert!(inner.is_some());

    let json = r#"
    {
        "malformedField": []
    }"#;

    let index = serde_json::from_str::<ChannelIndex>(json).expect("should parse json successfully");
    let inner = match index {
        ChannelIndex::Unsupported(value) => Some(value),
        _ => None,
    };

    assert!(inner.is_some());
}

#[test]
fn test_release_target_from_platform() {
    let cases = [
        ["macos", "aarch64", "aarch64-apple-darwin"],
        ["macos", "x86_64", "x86_64-apple-darwin"],
        ["linux", "x86_64", "x86_64-unknown-linux"],
        ["windows", "x86_64", "x86_64-pc-windows"],
        ["linux", "aarch64", "aarch64-unknown-linux"],
    ];

    for case in cases {
        let target = Target::from(case[0], case[1]).expect("should parse target");
        assert_eq!(target.to_string(), case[2]);
    }

    assert_eq!(Target::from("macos", "powerpc64").ok(), None);
    assert_eq!(Target::from("linux", "arm").ok(), None);
    assert_eq!(Target::from("riscv64", "x86_64").ok(), None);
}

#[test]
fn test_release_unsupported_target() {
    let json = r#"
    {
        "version": "0.1.20241231+ba15a9a4e",
        "targets": [
            "aarch64-apple-darwin",
            "x86_64-unknown-linux",
            "x86_64-pc-windows",
            "riscv64-unknown-linux"
        ]
    }"#;
    let release = serde_json::from_str::<Release>(json).expect("should parse json successfully");
    let host = Target::from("linux", "aarch64").expect("should parse target");
    assert!(!release.is_target_supported(&host));

    let json = r#"
    {
        "version": "0.1.20241231+ba15a9a4e"
    }"#;
    let release = serde_json::from_str::<Release>(json).expect("should parse json successfully");
    let host = Target::from("linux", "x86_64").expect("should parse target");
    // NOTE(chaweyhsu): assume supported if no target info provided in the release
    assert!(release.is_target_supported(&host));
}

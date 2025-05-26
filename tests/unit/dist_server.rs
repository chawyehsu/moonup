use moonup::dist_server::schema::{Channel, ChannelName, Index, Target};

#[test]
fn test_handle_future_channel() {
    let json = r#"
    {
        "name": "lts",
        "version": "1.0.0"
    }"#;

    let channel = serde_json::from_str::<Channel>(json).expect("should parse channel successfully");

    assert_eq!(channel.name, ChannelName::Unknown("lts".to_string()));
}

#[test]
fn test_handle_future_target() {
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
            "aarch64-unknown-linux"
        ]
    }"#;

    let index = serde_json::from_str::<Index>(json).expect("should parse json successfully");

    assert_eq!(
        index.targets.last(),
        Some(&Target::Unknown("aarch64-unknown-linux".to_string()))
    );
}

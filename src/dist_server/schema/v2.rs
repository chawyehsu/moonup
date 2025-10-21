use serde::Deserialize;

use crate::dist_server::schema::{Channel, Component, Release, Target};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Index {
    /// The last modified datetime
    pub last_modified: String,

    /// The available channels
    pub channels: Vec<Channel>,

    /// The available targets
    pub targets: Vec<Target>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChannelIndex {
    pub last_modified: String,
    pub releases: Vec<Release>,
}

#[derive(Debug, Deserialize)]
pub struct ComponentIndex {
    /// Available components
    pub components: Vec<Component>,
}

use serde::Deserialize;

use crate::dist_server::schema::{Channel, Release};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Index {
    /// The last modified timestamp
    pub last_modified: String,

    /// The available channels
    pub channels: Vec<Channel>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChannelIndex {
    pub last_modified: String,
    pub releases: Vec<Release>,
}

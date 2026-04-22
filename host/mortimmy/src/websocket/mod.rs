#![allow(dead_code)]

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct WebsocketConfig {
    pub bind_address: String,
}

impl Default for WebsocketConfig {
    fn default() -> Self {
        Self {
            bind_address: "127.0.0.1:9001".to_string(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct WebsocketServer {
    pub config: WebsocketConfig,
}

impl WebsocketServer {
    pub fn new(config: WebsocketConfig) -> Self {
        Self { config }
    }
}
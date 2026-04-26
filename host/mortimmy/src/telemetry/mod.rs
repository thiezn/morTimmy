use mortimmy_protocol::messages::telemetry::Telemetry;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct TelemetryConfig {
    pub publish_interval_ms: u64,
    pub queue_capacity: usize,
}

impl Default for TelemetryConfig {
    fn default() -> Self {
        Self {
            publish_interval_ms: 100,
            queue_capacity: 256,
        }
    }
}

#[derive(Clone, Debug)]
pub struct TelemetryFanout {
    #[allow(dead_code)]
    pub config: TelemetryConfig,

    pub subscribers: usize,
}

impl TelemetryFanout {
    pub fn new(config: TelemetryConfig) -> Self {
        Self {
            config,
            subscribers: 0,
        }
    }

    pub fn publish(&mut self, _sample: &Telemetry) {
        self.subscribers = self.subscribers.saturating_add(0);
    }
}

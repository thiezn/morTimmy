use mortimmy_core::{CoreError, Mode};
use serde::{Deserialize, Serialize};

use super::{ControllerCapabilities, ControllerRole, RangeTelemetry};

/// Periodic firmware status emitted to the host.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatusTelemetry {
    pub mode: Mode,
    pub controller_role: ControllerRole,
    pub capabilities: ControllerCapabilities,
    pub uptime_ms: u32,
    pub link_quality: u8,
    pub error: Option<CoreError>,
    pub range: Option<RangeTelemetry>,
}

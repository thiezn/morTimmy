use mortimmy_core::{CoreError, Mode};
use serde::{Deserialize, Serialize};

use super::{ControllerCapabilities, ControllerRole};

/// Snapshot of controller identity and health.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ControllerStatus {
    pub mode: Mode,
    pub controller_role: ControllerRole,
    pub capabilities: ControllerCapabilities,
    pub uptime_ms: u32,
    pub link_quality: u8,
    pub error: Option<CoreError>,
}

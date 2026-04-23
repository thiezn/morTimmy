use serde::{Deserialize, Serialize};

/// Typed parameter keys supported by the current scaffold.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ParameterKey {
    /// Maximum absolute PWM magnitude accepted by the firmware.
    MaxDrivePwm,
    /// Maximum per-command servo delta accepted by the firmware.
    MaxServoStep,
    /// Host-link timeout in milliseconds before the firmware falls back to defaults.
    LinkTimeoutMs,
    /// Trellis LED brightness level.
    TrellisBrightness,
    /// Trellis scan interval in milliseconds.
    TrellisPollIntervalMs,
    /// Audio chunk size accepted from the host.
    AudioChunkSamples,
}

/// Generic typed parameter update sent from the host to the firmware.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterUpdate {
    pub key: ParameterKey,
    pub value: i32,
}
use serde::{Deserialize, Serialize};

/// Errors that can cross the host/firmware boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CoreError {
    /// Frame checksum validation failed.
    CrcFailure,
    /// Framed transport bytes could not be resynchronized.
    FrameSync,
    /// The host link stopped refreshing health or control state before the timeout expired.
    LinkTimedOut,
    /// A sensor did not respond inside its expected sampling window.
    SensorTimeout,
    /// A syntactically valid message contained unsupported or invalid control data.
    InvalidCommand,
}

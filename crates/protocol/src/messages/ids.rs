use serde::{Deserialize, Serialize};

/// Correlation identifier for request/response traffic.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct RequestId(pub u16);

/// Configurable classes of controller-originated reports.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReportKind {
    /// Applied desired-control reports.
    ControlApplied,
    /// Range sensor reports.
    Range,
    /// Battery monitor reports.
    Battery,
    /// Audio status reports.
    AudioStatus,
}

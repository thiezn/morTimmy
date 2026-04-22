#![no_std]

pub mod errors;
pub mod limits;
pub mod mode;
pub mod units;

pub use errors::CoreError;
pub use limits::{DEFAULT_LIMITS, RobotLimits};
pub use mode::Mode;
pub use units::{Millimeters, Milliseconds, PwmTicks, ServoTicks};

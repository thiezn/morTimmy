//! Host-side robot brain loop and protocol transport integration.

mod autonomy;
pub mod command;
mod robot;
pub mod transport;

pub use self::command::BrainCommand;
#[allow(unused_imports)]
pub use self::robot::{BrainStep, RobotBrain};

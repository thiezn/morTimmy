//! Host-to-firmware protocol command surface.

pub mod audio;
pub mod desired_state;
pub mod drive;
pub mod parameter;
pub mod servo;
pub mod trellis_led;

pub use self::audio::{AUDIO_CHUNK_CAPACITY_SAMPLES, AudioChunkCommand, AudioEncoding};
pub use self::desired_state::DesiredStateCommand;
pub use self::drive::DriveCommand;
pub use self::parameter::{ParameterKey, ParameterUpdate};
pub use self::servo::ServoCommand;
pub use self::trellis_led::TrellisLedCommand;

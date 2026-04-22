#![no_std]

pub mod actuators;
pub mod sensors;
pub mod ui;

pub use actuators::motor::{MotorChannel, MotorDriver};
pub use actuators::servo::{PanTiltAxis, ServoDriver};
pub use sensors::ultrasonic::UltrasonicSensor;
pub use ui::audio::{AudioOutput, AudioSampleFormat, AudioStreamConfig};
pub use ui::trellis::{PadEvent, PadEventKind, PadIndex, TrellisPadMatrix};

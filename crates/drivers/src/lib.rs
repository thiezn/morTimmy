#![no_std]

pub mod actuators;
pub mod sensors;
pub mod ui;

pub use actuators::motor::l298n::{
    L298nBridge, L298nChannelConfig, L298nDriveMotorDriver, L298nError, L298nSideDriver,
    MotorPolarity,
};
pub use actuators::motor::{
    MotorChannel, MotorDirection, MotorDriver, MotorPowerCommand, MotorStopMode,
};
pub use actuators::servo::{PanTiltAxis, ServoDriver};
pub use sensors::ultrasonic::UltrasonicSensor;
pub use sensors::ultrasonic::hc_sr04::{
    HcSr04, HcSr04Config, HcSr04Error, MicrosecondClock,
};
pub use ui::audio::{AudioOutput, AudioSampleFormat, AudioStreamConfig};
pub use ui::audio::pico_audio_pack::{
    AmpEnablePolarity, PicoAudioPack, PicoAudioPackConfig, PicoAudioPackError,
    PicoAudioPackTransport,
};
pub use ui::display::CharacterDisplay;
pub use ui::display::hd44780::{Hd44780Config, Hd44780Error, Hd44780Lcd1602};
pub use ui::trellis::{PadEvent, PadEventKind, PadIndex, TrellisPadMatrix};

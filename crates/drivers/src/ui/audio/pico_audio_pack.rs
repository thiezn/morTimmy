use embedded_hal::digital::OutputPin;

use super::{AudioOutput, AudioStreamConfig};

/// Low-level transport required by the Pico Audio Pack wrapper.
pub trait PicoAudioPackTransport {
    /// Driver-specific transport error.
    type Error;

    /// Start or reconfigure the underlying audio transport.
    fn start_stream(&mut self, config: AudioStreamConfig) -> Result<(), Self::Error>;

    /// Push interleaved PCM samples into the transport.
    fn write_samples(&mut self, samples: &[i16]) -> Result<(), Self::Error>;

    /// Stop the transport and drain any buffered samples.
    fn stop_stream(&mut self) -> Result<(), Self::Error>;
}

/// Amp-enable polarity for the Pico Audio Pack mute control pin.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AmpEnablePolarity {
    #[default]
    EnabledHigh,
    EnabledLow,
}

/// Driver-level Pico Audio Pack configuration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PicoAudioPackConfig {
    /// Amp-enable polarity.
    pub amp_enable_polarity: AmpEnablePolarity,
    /// Maximum channel count accepted by the driver.
    pub max_channels: u8,
}

impl Default for PicoAudioPackConfig {
    fn default() -> Self {
        Self {
            amp_enable_polarity: AmpEnablePolarity::EnabledHigh,
            max_channels: 2,
        }
    }
}

/// Errors surfaced by the Pico Audio Pack wrapper.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PicoAudioPackError<TransportError, PinError> {
    Transport(TransportError),
    AmpEnable(PinError),
    InvalidChannels(u8),
    MisalignedFrameCount { sample_count: usize, channels: u8 },
    StreamNotStarted,
}

/// Concrete audio output for the Pico Audio Pack DAC and amp-enable control.
#[derive(Debug)]
pub struct PicoAudioPack<Transport, AmpEnable> {
    transport: Transport,
    amp_enable: AmpEnable,
    config: PicoAudioPackConfig,
    active_stream: Option<AudioStreamConfig>,
}

impl<Transport, AmpEnable> PicoAudioPack<Transport, AmpEnable> {
    /// Construct the driver with default configuration.
    pub fn new(transport: Transport, amp_enable: AmpEnable) -> Self {
        Self::with_config(transport, amp_enable, PicoAudioPackConfig::default())
    }

    /// Construct the driver with explicit amp-enable configuration.
    pub const fn with_config(
        transport: Transport,
        amp_enable: AmpEnable,
        config: PicoAudioPackConfig,
    ) -> Self {
        Self {
            transport,
            amp_enable,
            config,
            active_stream: None,
        }
    }

    /// Return the active stream configuration, if any.
    pub const fn active_stream(&self) -> Option<AudioStreamConfig> {
        self.active_stream
    }

    fn set_amp_enabled<TransportError, PinError>(
        &mut self,
        enabled: bool,
    ) -> Result<(), PicoAudioPackError<TransportError, PinError>>
    where
        AmpEnable: OutputPin<Error = PinError>,
    {
        let drive_high = match self.config.amp_enable_polarity {
            AmpEnablePolarity::EnabledHigh => enabled,
            AmpEnablePolarity::EnabledLow => !enabled,
        };

        if drive_high {
            self.amp_enable
                .set_high()
                .map_err(PicoAudioPackError::AmpEnable)
        } else {
            self.amp_enable
                .set_low()
                .map_err(PicoAudioPackError::AmpEnable)
        }
    }
}

impl<Transport, AmpEnable, TransportError, PinError> AudioOutput
    for PicoAudioPack<Transport, AmpEnable>
where
    Transport: PicoAudioPackTransport<Error = TransportError>,
    AmpEnable: OutputPin<Error = PinError>,
{
    type Error = PicoAudioPackError<TransportError, PinError>;

    fn start(&mut self, config: AudioStreamConfig) -> Result<(), Self::Error> {
        if config.channels == 0 || config.channels > self.config.max_channels {
            return Err(PicoAudioPackError::InvalidChannels(config.channels));
        }

        self.transport
            .start_stream(config)
            .map_err(PicoAudioPackError::Transport)?;
        self.set_amp_enabled(true)?;
        self.active_stream = Some(config);
        Ok(())
    }

    fn enqueue_samples(&mut self, samples: &[i16], _is_final_chunk: bool) -> Result<(), Self::Error> {
        let Some(stream) = self.active_stream else {
            return Err(PicoAudioPackError::StreamNotStarted);
        };

        if !samples.len().is_multiple_of(usize::from(stream.channels)) {
            return Err(PicoAudioPackError::MisalignedFrameCount {
                sample_count: samples.len(),
                channels: stream.channels,
            });
        }

        if samples.is_empty() {
            return Ok(());
        }

        self.transport
            .write_samples(samples)
            .map_err(PicoAudioPackError::Transport)
    }

    fn stop(&mut self) -> Result<(), Self::Error> {
        self.set_amp_enabled(false)?;
        self.transport
            .stop_stream()
            .map_err(PicoAudioPackError::Transport)?;
        self.active_stream = None;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::{
        cell::RefCell,
        rc::Rc,
        vec::Vec,
    };

    use embedded_hal::digital::{ErrorType, OutputPin};

    use super::{
        AmpEnablePolarity, PicoAudioPack, PicoAudioPackConfig, PicoAudioPackError,
        PicoAudioPackTransport,
    };
    use crate::ui::audio::{AudioOutput, AudioSampleFormat, AudioStreamConfig};

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    struct FakePinError;

    impl embedded_hal::digital::Error for FakePinError {
        fn kind(&self) -> embedded_hal::digital::ErrorKind {
            embedded_hal::digital::ErrorKind::Other
        }
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    struct FakeTransportError;

    #[derive(Clone, Debug, Default)]
    struct RecordingTransport {
        started: Vec<AudioStreamConfig>,
        writes: Vec<Vec<i16>>,
        stop_calls: usize,
    }

    impl PicoAudioPackTransport for RecordingTransport {
        type Error = FakeTransportError;

        fn start_stream(&mut self, config: AudioStreamConfig) -> Result<(), Self::Error> {
            self.started.push(config);
            Ok(())
        }

        fn write_samples(&mut self, samples: &[i16]) -> Result<(), Self::Error> {
            self.writes.push(samples.to_vec());
            Ok(())
        }

        fn stop_stream(&mut self) -> Result<(), Self::Error> {
            self.stop_calls += 1;
            Ok(())
        }
    }

    #[derive(Clone, Debug)]
    struct FakeAmpPin {
        states: Rc<RefCell<Vec<bool>>>,
    }

    impl ErrorType for FakeAmpPin {
        type Error = FakePinError;
    }

    impl OutputPin for FakeAmpPin {
        fn set_low(&mut self) -> Result<(), Self::Error> {
            self.states.borrow_mut().push(false);
            Ok(())
        }

        fn set_high(&mut self) -> Result<(), Self::Error> {
            self.states.borrow_mut().push(true);
            Ok(())
        }
    }

    fn sample_stream() -> AudioStreamConfig {
        AudioStreamConfig {
            sample_rate_hz: 24_000,
            channels: 2,
            format: AudioSampleFormat::SignedPcm16Le,
        }
    }

    #[test]
    fn pico_audio_pack_starts_stream_and_toggles_amp_enable() {
        let amp_states = Rc::new(RefCell::new(Vec::new()));
        let mut output = PicoAudioPack::new(
            RecordingTransport::default(),
            FakeAmpPin {
                states: amp_states.clone(),
            },
        );

        output.start(sample_stream()).unwrap();
        output.enqueue_samples(&[1, 2, 3, 4], false).unwrap();
        output.stop().unwrap();

        assert_eq!(output.active_stream(), None);
        assert_eq!(&*amp_states.borrow(), &[true, false]);
        assert_eq!(output.transport.started, Vec::from([sample_stream()]));
        assert_eq!(output.transport.writes, Vec::from([Vec::from([1, 2, 3, 4]) ]));
        assert_eq!(output.transport.stop_calls, 1);
    }

    #[test]
    fn pico_audio_pack_rejects_invalid_channel_counts() {
        let mut output = PicoAudioPack::new(
            RecordingTransport::default(),
            FakeAmpPin {
                states: Rc::new(RefCell::new(Vec::new())),
            },
        );

        assert_eq!(
            output.start(AudioStreamConfig {
                channels: 3,
                ..sample_stream()
            }),
            Err(PicoAudioPackError::InvalidChannels(3))
        );
    }

    #[test]
    fn pico_audio_pack_honors_enable_polarity() {
        let amp_states = Rc::new(RefCell::new(Vec::new()));
        let mut output = PicoAudioPack::with_config(
            RecordingTransport::default(),
            FakeAmpPin {
                states: amp_states.clone(),
            },
            PicoAudioPackConfig {
                amp_enable_polarity: AmpEnablePolarity::EnabledLow,
                max_channels: 2,
            },
        );

        output.start(sample_stream()).unwrap();
        output.stop().unwrap();

        assert_eq!(&*amp_states.borrow(), &[false, true]);
    }
}

//! Audio output traits for embedded playback devices such as the Pico Audio Pack.

/// Supported sample formats for firmware-side audio output.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AudioSampleFormat {
    /// Signed 16-bit PCM in little-endian sample order.
    SignedPcm16Le,
}

/// Stream configuration for an audio output device.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AudioStreamConfig {
    /// The sample rate in hertz.
    pub sample_rate_hz: u32,
    /// The number of interleaved channels.
    pub channels: u8,
    /// The PCM sample format.
    pub format: AudioSampleFormat,
}

/// Trait implemented by firmware-side audio outputs.
pub trait AudioOutput {
    /// Driver-specific error type.
    type Error;

    /// Start or reconfigure the audio stream.
    fn start(&mut self, config: AudioStreamConfig) -> Result<(), Self::Error>;

    /// Queue PCM samples for playback.
    fn enqueue_samples(&mut self, samples: &[i16], is_final_chunk: bool) -> Result<(), Self::Error>;

    /// Stop playback and drain any queued audio.
    fn stop(&mut self) -> Result<(), Self::Error>;
}
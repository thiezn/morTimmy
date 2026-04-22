#![allow(dead_code)]

use mortimmy_core::CoreError;
use mortimmy_drivers::{AudioSampleFormat, AudioStreamConfig};
use mortimmy_protocol::messages::{AUDIO_CHUNK_CAPACITY_SAMPLES, AudioChunkCommand, AudioEncoding};

/// Default playback configuration for the Pico Audio Pack.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AudioPackConfig {
    /// Sample rate in hertz.
    pub sample_rate_hz: u32,
    /// Number of channels.
    pub channels: u8,
    /// PCM sample format.
    pub format: AudioSampleFormat,
    /// Number of samples buffered per chunk.
    pub chunk_samples: usize,
}

impl Default for AudioPackConfig {
    fn default() -> Self {
        Self {
            sample_rate_hz: 24_000,
            channels: 1,
            format: AudioSampleFormat::SignedPcm16Le,
            chunk_samples: AUDIO_CHUNK_CAPACITY_SAMPLES,
        }
    }
}

/// Source responsible for generating or forwarding audio to the DAC.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum AudioRoute {
    /// Audio is forwarded from the host over the protocol link.
    #[default]
    HostWaveformBridge,
    /// Audio is synthesized locally on the microcontroller.
    LocalSynthesis,
}

/// Firmware-side audio playback task state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AudioOutputTask {
    /// Playback configuration.
    pub config: AudioPackConfig,
    /// Active audio route.
    pub route: AudioRoute,
    /// Number of audio chunks currently buffered.
    pub queued_chunks: u16,
}

impl Default for AudioOutputTask {
    fn default() -> Self {
        Self {
            config: AudioPackConfig::default(),
            route: AudioRoute::HostWaveformBridge,
            queued_chunks: 0,
        }
    }
}

impl AudioOutputTask {
    /// Return the audio stream configuration used by the DAC path.
    pub const fn stream_config(&self) -> AudioStreamConfig {
        AudioStreamConfig {
            sample_rate_hz: self.config.sample_rate_hz,
            channels: self.config.channels,
            format: self.config.format,
        }
    }

    /// Apply a new host-configured chunk size.
    pub fn set_chunk_samples(&mut self, chunk_samples: usize) -> Result<(), CoreError> {
        if chunk_samples == 0 || chunk_samples > AUDIO_CHUNK_CAPACITY_SAMPLES {
            return Err(CoreError::InvalidCommand);
        }

        self.config.chunk_samples = chunk_samples;
        Ok(())
    }

    /// Accept an audio chunk from the host-side protocol bridge.
    pub fn enqueue_chunk(&mut self, command: &AudioChunkCommand) -> Result<(), CoreError> {
        if command.sample_rate_hz != self.config.sample_rate_hz
            || command.channels != self.config.channels
            || command.samples.len() > self.config.chunk_samples
        {
            return Err(CoreError::InvalidCommand);
        }

        match (command.encoding, self.config.format) {
            (AudioEncoding::SignedPcm16Le, AudioSampleFormat::SignedPcm16Le) => {
                self.queued_chunks = self.queued_chunks.saturating_add(1);
                Ok(())
            }
        }
    }
}
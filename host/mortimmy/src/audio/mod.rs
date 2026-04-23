//! Host-side audio planning and routing for speech playback on the Pico Audio Pack.

use clap::ValueEnum;
use heapless::Vec as HeaplessVec;
use mortimmy_protocol::messages::{
    command::Command,
    commands::{AUDIO_CHUNK_CAPACITY_SAMPLES, AudioChunkCommand, AudioEncoding},
};
use serde::{Deserialize, Serialize};

/// Errors returned while translating host audio into protocol commands.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioPlanError {
    /// Audio forwarding is disabled in the current config.
    Disabled,
    /// The selected backend does not produce firmware audio commands.
    UnsupportedBackend,
    /// A chunk exceeded the protocol's fixed-size sample capacity.
    ChunkTooLarge,
    /// The waveform required more chunks than fit in the protocol chunk index field.
    TooManyChunks,
}

/// Audio routing backend used by the host daemon.
#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AudioBackendKind {
    /// Disable all host-side audio forwarding.
    #[default]
    Disabled,
    /// Forward PCM chunks to the firmware for playback on the Pico Audio Pack.
    FirmwareBridge,
}

/// Configuration for host-side speech playback routing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioConfig {
    /// Whether audio forwarding is enabled.
    pub enabled: bool,
    /// Audio backend used for playback routing.
    pub backend: AudioBackendKind,
    /// PCM sample rate in hertz.
    pub sample_rate_hz: u32,
    /// Number of interleaved channels.
    pub channels: u8,
    /// Number of samples per forwarded chunk.
    pub chunk_samples: usize,
    /// Output gain applied by the playback path.
    pub volume_percent: u8,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            backend: AudioBackendKind::Disabled,
            sample_rate_hz: 24_000,
            channels: 1,
            chunk_samples: AUDIO_CHUNK_CAPACITY_SAMPLES,
            volume_percent: 100,
        }
    }
}

/// Planned chunking information for a waveform.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WaveformPlan {
    /// Total number of PCM samples to deliver.
    pub total_samples: usize,
    /// Samples per chunk based on the current config.
    pub chunk_samples: usize,
    /// Number of chunks required to deliver the waveform.
    pub chunk_count: usize,
}

/// Host-side audio subsystem scaffold.
#[derive(Debug, Clone)]
pub struct AudioSubsystem {
    config: AudioConfig,
    #[cfg_attr(not(test), allow(dead_code))]
    route_name: &'static str,
}

impl AudioSubsystem {
    /// Create an audio subsystem from config.
    pub fn from_config(config: AudioConfig) -> Self {
        let route_name = match (config.enabled, config.backend) {
            (false, _) | (_, AudioBackendKind::Disabled) => "disabled",
            (true, AudioBackendKind::FirmwareBridge) => "firmware-bridge",
        };

        Self { config, route_name }
    }

    /// Return the effective audio configuration.
    pub fn config(&self) -> &AudioConfig {
        &self.config
    }

    /// Return the currently selected route name.
    #[cfg_attr(not(test), allow(dead_code))]
    pub const fn route_name(&self) -> &'static str {
        self.route_name
    }

    /// Compute how a waveform should be chunked for forwarding.
    pub fn plan_waveform(&self, total_samples: usize) -> WaveformPlan {
        let chunk_samples = self.config.chunk_samples.max(1);
        let chunk_count = total_samples.div_ceil(chunk_samples);

        WaveformPlan {
            total_samples,
            chunk_samples,
            chunk_count,
        }
    }

    /// Split a waveform into protocol `PlayAudio` commands for the firmware bridge.
    #[allow(dead_code)]
    pub fn build_audio_commands(
        &self,
        utterance_id: u32,
        waveform: &[i16],
    ) -> Result<Vec<Command>, AudioPlanError> {
        if !self.config.enabled {
            return Err(AudioPlanError::Disabled);
        }
        if self.config.backend != AudioBackendKind::FirmwareBridge {
            return Err(AudioPlanError::UnsupportedBackend);
        }

        let plan = self.plan_waveform(waveform.len());
        if plan.chunk_count > usize::from(u16::MAX) + 1 {
            return Err(AudioPlanError::TooManyChunks);
        }

        let mut commands = Vec::with_capacity(plan.chunk_count);
        for (chunk_index, chunk) in waveform.chunks(plan.chunk_samples).enumerate() {
            let mut samples = HeaplessVec::<i16, AUDIO_CHUNK_CAPACITY_SAMPLES>::new();
            samples
                .extend_from_slice(chunk)
                .map_err(|_| AudioPlanError::ChunkTooLarge)?;

            commands.push(Command::PlayAudio(AudioChunkCommand {
                utterance_id,
                chunk_index: u16::try_from(chunk_index)
                    .map_err(|_| AudioPlanError::TooManyChunks)?,
                sample_rate_hz: self.config.sample_rate_hz,
                channels: self.config.channels,
                encoding: AudioEncoding::SignedPcm16Le,
                is_final_chunk: chunk_index + 1 == plan.chunk_count,
                samples,
            }));
        }

        Ok(commands)
    }
}

#[cfg(test)]
mod tests {
    use mortimmy_protocol::messages::{command::Command, commands::AUDIO_CHUNK_CAPACITY_SAMPLES};

    use super::{AudioBackendKind, AudioConfig, AudioPlanError, AudioSubsystem};

    #[test]
    fn disabled_audio_routes_to_disabled() {
        let subsystem = AudioSubsystem::from_config(AudioConfig::default());
        assert_eq!(subsystem.route_name(), "disabled");
    }

    #[test]
    fn enabled_audio_uses_firmware_bridge() {
        let subsystem = AudioSubsystem::from_config(AudioConfig {
            enabled: true,
            backend: AudioBackendKind::FirmwareBridge,
            ..AudioConfig::default()
        });

        assert_eq!(subsystem.route_name(), "firmware-bridge");
        assert_eq!(subsystem.plan_waveform(1_000).chunk_count, 5);
    }

    #[test]
    fn default_chunk_size_matches_protocol_capacity() {
        assert_eq!(
            AudioConfig::default().chunk_samples,
            AUDIO_CHUNK_CAPACITY_SAMPLES
        );
    }

    #[test]
    fn chunks_waveform_into_protocol_audio_commands() {
        let subsystem = AudioSubsystem::from_config(AudioConfig {
            enabled: true,
            backend: AudioBackendKind::FirmwareBridge,
            ..AudioConfig::default()
        });
        let waveform = vec![7i16; AUDIO_CHUNK_CAPACITY_SAMPLES * 2 + 12];

        let commands = subsystem.build_audio_commands(11, &waveform).unwrap();

        assert_eq!(commands.len(), 3);
        assert!(matches!(commands[0], Command::PlayAudio(_)));
        match &commands[2] {
            Command::PlayAudio(command) => {
                assert_eq!(command.chunk_index, 2);
                assert!(command.is_final_chunk);
                assert_eq!(command.samples.len(), 12);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn disabled_audio_rejects_protocol_chunking() {
        let subsystem = AudioSubsystem::from_config(AudioConfig::default());

        assert_eq!(
            subsystem.build_audio_commands(1, &[1, 2, 3]),
            Err(AudioPlanError::Disabled)
        );
    }
}

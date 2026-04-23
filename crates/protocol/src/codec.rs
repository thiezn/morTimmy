//! Postcard-based serialization helpers for wire messages.

use crate::messages::WireMessage;

/// Maximum encoded message payload length used by the framing layer.
///
/// The current ceiling is sized to hold a full-size default audio chunk plus
/// its control metadata without requiring an additional fragmentation layer.
pub const MAX_PAYLOAD_LEN: usize = 640;

/// Error returned when message serialization or deserialization fails.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CodecError {
    Deserialize,
    Serialize,
}

/// Encode a wire message into the provided scratch buffer.
pub fn encode_message<'a>(
    message: &WireMessage,
    buffer: &'a mut [u8],
) -> Result<&'a [u8], CodecError> {
    postcard::to_slice(message, buffer)
        .map(|encoded| &encoded[..])
        .map_err(|_| CodecError::Serialize)
}

/// Decode a wire message from postcard bytes.
pub fn decode_message(bytes: &[u8]) -> Result<WireMessage, CodecError> {
    postcard::from_bytes(bytes).map_err(|_| CodecError::Deserialize)
}

#[cfg(test)]
mod tests {
    use heapless::Vec;
    use mortimmy_core::{Mode, PwmTicks, ServoTicks};

    use super::{MAX_PAYLOAD_LEN, decode_message, encode_message};
    use crate::messages::{
        WireMessage,
        command::Command,
        commands::{
            AUDIO_CHUNK_CAPACITY_SAMPLES, AudioChunkCommand, AudioEncoding,
            DesiredStateCommand, DriveCommand, ParameterKey, ParameterUpdate, ServoCommand,
            TrellisLedCommand,
        },
        telemetry::{
            AudioStatusTelemetry, DesiredStateTelemetry, MotorStateTelemetry, PadEventKind,
            ServoStateTelemetry, Telemetry, TrellisPadTelemetry,
        },
    };

    #[test]
    fn roundtrips_audio_chunk_command() {
        let mut samples = Vec::<i16, AUDIO_CHUNK_CAPACITY_SAMPLES>::new();
        samples.extend_from_slice(&[1, 2, 3, 4, 5, 6]).unwrap();

        let message = WireMessage::Command(Command::PlayAudio(AudioChunkCommand {
            utterance_id: 7,
            chunk_index: 2,
            sample_rate_hz: 24_000,
            channels: 1,
            encoding: AudioEncoding::SignedPcm16Le,
            is_final_chunk: false,
            samples,
        }));

        let mut buffer = [0u8; 256];
        let encoded = encode_message(&message, &mut buffer).unwrap();
        let decoded = decode_message(encoded).unwrap();

        assert_eq!(decoded, message);
    }

    #[test]
    fn roundtrips_trellis_messages() {
        let command = WireMessage::Command(Command::SetTrellisLeds(TrellisLedCommand {
            led_mask: 0x00ff,
        }));
        let telemetry = WireMessage::Telemetry(Telemetry::TrellisPad(TrellisPadTelemetry {
            pad_index: 3,
            event: PadEventKind::Pressed,
        }));
        let audio_status = WireMessage::Telemetry(Telemetry::AudioStatus(AudioStatusTelemetry {
            queued_chunks: 4,
            speaking: true,
            underrun_count: 0,
        }));

        let mut buffer = [0u8; 256];
        let encoded_command = encode_message(&command, &mut buffer).unwrap();
        assert_eq!(decode_message(encoded_command).unwrap(), command);

        let encoded_telemetry = encode_message(&telemetry, &mut buffer).unwrap();
        assert_eq!(decode_message(encoded_telemetry).unwrap(), telemetry);

        let encoded_audio_status = encode_message(&audio_status, &mut buffer).unwrap();
        assert_eq!(decode_message(encoded_audio_status).unwrap(), audio_status);
    }

    #[test]
    fn roundtrips_parameter_command() {
        let parameter_update = WireMessage::Command(Command::SetParam(ParameterUpdate {
            key: ParameterKey::LinkTimeoutMs,
            value: 500,
        }));

        let mut buffer = [0u8; 256];
        let encoded_parameter = encode_message(&parameter_update, &mut buffer).unwrap();
        assert_eq!(decode_message(encoded_parameter).unwrap(), parameter_update);
    }

    #[test]
    fn roundtrips_desired_state_messages() {
        let command = WireMessage::Command(Command::SetDesiredState(DesiredStateCommand::new(
            Mode::Teleop,
            DriveCommand {
                left: PwmTicks(320),
                right: PwmTicks(-125),
            },
            ServoCommand {
                pan: ServoTicks(1_540),
                tilt: ServoTicks(1_220),
            },
        )));
        let telemetry =
            WireMessage::Telemetry(Telemetry::DesiredState(DesiredStateTelemetry::new(
                Mode::Teleop,
                MotorStateTelemetry {
                    left_pwm: PwmTicks(320),
                    right_pwm: PwmTicks(-125),
                    current_limit_hit: false,
                },
                ServoStateTelemetry {
                    pan: ServoTicks(1_540),
                    tilt: ServoTicks(1_220),
                },
                None,
            )));

        let mut buffer = [0u8; 256];
        let encoded_command = encode_message(&command, &mut buffer).unwrap();
        assert_eq!(decode_message(encoded_command).unwrap(), command);

        let encoded_telemetry = encode_message(&telemetry, &mut buffer).unwrap();
        assert_eq!(decode_message(encoded_telemetry).unwrap(), telemetry);
    }

    #[test]
    fn encodes_full_size_default_audio_chunk() {
        let mut samples = Vec::<i16, AUDIO_CHUNK_CAPACITY_SAMPLES>::new();
        for sample in 0..AUDIO_CHUNK_CAPACITY_SAMPLES {
            samples.push(sample as i16).unwrap();
        }

        let message = WireMessage::Command(Command::PlayAudio(AudioChunkCommand {
            utterance_id: 17,
            chunk_index: 0,
            sample_rate_hz: 24_000,
            channels: 1,
            encoding: AudioEncoding::SignedPcm16Le,
            is_final_chunk: true,
            samples,
        }));

        let mut buffer = [0u8; MAX_PAYLOAD_LEN];
        let encoded = encode_message(&message, &mut buffer).unwrap();

        assert!(encoded.len() <= MAX_PAYLOAD_LEN);
        assert_eq!(decode_message(encoded).unwrap(), message);
    }
}

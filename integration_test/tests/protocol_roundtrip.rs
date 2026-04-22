use heapless::Vec;
use mortimmy_core::{Mode, PwmTicks, ServoTicks};
use mortimmy_protocol::{
    decode_message, encode_message, wrap_payload, FrameDecoder,
    messages::{
        AUDIO_CHUNK_CAPACITY_SAMPLES, AudioChunkCommand, AudioEncoding, Command,
        DesiredStateCommand, DriveCommand, ServoCommand, WireMessage,
    },
};
use mortimmy_rp2350::FirmwareScaffold;

#[test]
fn capture_roundtrip_matches_protocol_contract() {
    let mut samples = Vec::new();
    for sample in 0..AUDIO_CHUNK_CAPACITY_SAMPLES {
        samples.push((sample as i16) * 2).unwrap();
    }

    let message = WireMessage::Command(Command::PlayAudio(AudioChunkCommand {
        utterance_id: 11,
        chunk_index: 1,
        sample_rate_hz: 24_000,
        channels: 1,
        encoding: AudioEncoding::SignedPcm16Le,
        is_final_chunk: false,
        samples,
    }));

    let mut payload_buffer = [0u8; mortimmy_protocol::MAX_PAYLOAD_LEN];
    let payload = encode_message(&message, &mut payload_buffer).unwrap();

    let mut frame_buffer = [0u8; mortimmy_protocol::MAX_FRAME_BODY_LEN + 1];
    let frame = wrap_payload(payload, 99, &mut frame_buffer).unwrap();

    let mut decoder = FrameDecoder::default();
    let mut decoded = None;
    for byte in frame {
        if let Some(frame) = decoder.push(*byte).unwrap() {
            decoded = Some(frame);
        }
    }

    let decoded = decoded.expect("expected a frame");
    let decoded_message = decode_message(decoded.payload.as_slice()).unwrap();
    assert_eq!(decoded_message, message);
}

#[test]
fn desired_state_roundtrip_matches_protocol_contract() {
    let message = WireMessage::Command(Command::SetDesiredState(DesiredStateCommand::new(
        Mode::Teleop,
        DriveCommand {
            left: PwmTicks(300),
            right: PwmTicks(-180),
        },
        ServoCommand {
            pan: ServoTicks(24),
            tilt: ServoTicks(36),
        },
    )));

    let mut payload_buffer = [0u8; mortimmy_protocol::MAX_PAYLOAD_LEN];
    let payload = encode_message(&message, &mut payload_buffer).unwrap();

    let mut frame_buffer = [0u8; mortimmy_protocol::MAX_FRAME_BODY_LEN + 1];
    let frame = wrap_payload(payload, 100, &mut frame_buffer).unwrap();

    let mut decoder = FrameDecoder::default();
    let mut decoded = None;
    for byte in frame {
        if let Some(frame) = decoder.push(*byte).unwrap() {
            decoded = Some(frame);
        }
    }

    let decoded = decoded.expect("expected a frame");
    let decoded_message = decode_message(decoded.payload.as_slice()).unwrap();
    assert_eq!(decoded_message, message);
}

#[test]
fn firmware_scaffold_uses_latest_desired_state() {
    let mut scaffold = FirmwareScaffold::default();

    let first = DesiredStateCommand::new(
        Mode::Teleop,
        DriveCommand {
            left: PwmTicks(220),
            right: PwmTicks(220),
        },
        ServoCommand {
            pan: ServoTicks(24),
            tilt: ServoTicks(12),
        },
    );
    let second = DesiredStateCommand::new(
        Mode::Teleop,
        DriveCommand {
            left: PwmTicks(-180),
            right: PwmTicks(300),
        },
        ServoCommand {
            pan: ServoTicks(48),
            tilt: ServoTicks(36),
        },
    );

    scaffold.handle_command(Command::SetDesiredState(first));
    let response = scaffold.handle_command(Command::SetDesiredState(second));

    assert_eq!(scaffold.control.mode, Mode::Teleop);
    assert_eq!(scaffold.control.drive.left_pwm, PwmTicks(-180));
    assert_eq!(scaffold.control.drive.right_pwm, PwmTicks(300));
    assert_eq!(scaffold.control.servo.pan, ServoTicks(48));
    assert_eq!(scaffold.control.servo.tilt, ServoTicks(36));
    assert_eq!(
        response,
        Some(mortimmy_protocol::messages::Telemetry::DesiredState(
            scaffold.desired_state_telemetry(),
        ))
    );
}

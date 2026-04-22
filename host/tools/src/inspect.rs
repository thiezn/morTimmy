//! Capture inspection helpers backed by the shared protocol crate.

use std::path::Path;

use anyhow::{Context, Result, anyhow};
use mortimmy_protocol::{decode_message, FrameDecoder};

use crate::cli::InspectCommand;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaptureSummary {
    /// Number of transport frames seen in the capture.
    pub frame_count: usize,
    /// Number of successfully decoded wire messages in the capture.
    pub message_count: usize,
}

/// Inspect a capture file and print a compact summary.
pub fn run(command: InspectCommand) -> Result<()> {
    let summary = inspect_capture(&command.input)?;
    tracing::info!(frames = summary.frame_count, messages = summary.message_count, "capture inspected");
    println!("frames={} messages={}", summary.frame_count, summary.message_count);
    Ok(())
}

/// Decode a capture file using the shared framing and postcard layers.
pub fn inspect_capture(path: &Path) -> Result<CaptureSummary> {
    let bytes = std::fs::read(path).with_context(|| format!("failed to read capture {}", path.display()))?;
    let mut decoder = FrameDecoder::default();
    let mut frame_count = 0;
    let mut message_count = 0;

    for byte in bytes {
        if let Some(frame) = decoder
            .push(byte)
            .map_err(|error| anyhow!("failed to decode frame from capture: {error:?}"))?
        {
            frame_count += 1;
            let _message = decode_message(frame.payload.as_slice())
                .map_err(|error| anyhow!("failed to decode protocol message from capture: {error:?}"))?;
            message_count += 1;
        }
    }

    Ok(CaptureSummary {
        frame_count,
        message_count,
    })
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use heapless::Vec;
    use mortimmy_protocol::{
        encode_message,
        messages::{AudioChunkCommand, AudioEncoding, Command, WireMessage},
        wrap_payload,
    };

    use super::inspect_capture;

    fn unique_temp_path(prefix: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        std::env::temp_dir().join(format!("{prefix}_{}_{}.bin", std::process::id(), nanos))
    }

    #[test]
    fn counts_frames_in_capture() {
        let path = unique_temp_path("mortimmy_tools_inspect");
        let mut payload_buffer = [0u8; 256];
        let mut frame_buffer = [0u8; 256];
        let mut samples = Vec::new();
        samples.extend_from_slice(&[1, 2, 3, 4]).unwrap();

        let message = WireMessage::Command(Command::PlayAudio(AudioChunkCommand {
            utterance_id: 1,
            chunk_index: 0,
            sample_rate_hz: 24_000,
            channels: 1,
            encoding: AudioEncoding::SignedPcm16Le,
            is_final_chunk: true,
            samples,
        }));

        let encoded_message = encode_message(&message, &mut payload_buffer).unwrap();
        let frame = wrap_payload(encoded_message, 1, &mut frame_buffer).unwrap();
        std::fs::write(&path, frame).unwrap();

        let summary = inspect_capture(&path).unwrap();
        assert_eq!(summary.frame_count, 1);
        assert_eq!(summary.message_count, 1);

        let _ = std::fs::remove_file(path);
    }
}

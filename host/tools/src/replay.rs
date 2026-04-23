//! Replay helpers for exercising captures and future hardware bridges.

use std::path::Path;

use anyhow::Result;

use crate::{cli::ReplayCommand, inspect::inspect_capture};

/// Replay a capture file or validate it in dry-run mode.
pub fn run(command: ReplayCommand) -> Result<()> {
    let frame_count = replay_capture(&command.input, command.dry_run)?;
    tracing::info!(frames = frame_count, dry_run = command.dry_run, "capture replay completed");
    println!("replayed_frames={frame_count}");
    Ok(())
}

/// Replay a capture file against the current transport implementation.
pub fn replay_capture(path: &Path, _dry_run: bool) -> Result<usize> {
    let summary = inspect_capture(path)?;
    Ok(summary.frame_count)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use heapless::Vec;
    use mortimmy_protocol::{
        encode_message,
        messages::{
            WireMessage,
            command::Command,
            commands::{AudioChunkCommand, AudioEncoding},
        },
        wrap_payload,
    };

    use super::replay_capture;

    fn unique_temp_path(prefix: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        std::env::temp_dir().join(format!("{prefix}_{}_{}.bin", std::process::id(), nanos))
    }

    #[test]
    fn dry_run_replay_counts_frames() {
        let path = unique_temp_path("mortimmy_tools_replay");
        let mut payload_buffer = [0u8; 256];
        let mut frame_buffer = [0u8; 256];
        let mut samples = Vec::new();
        samples.extend_from_slice(&[9, 8, 7, 6]).unwrap();

        let message = WireMessage::Command(Command::PlayAudio(AudioChunkCommand {
            utterance_id: 2,
            chunk_index: 1,
            sample_rate_hz: 24_000,
            channels: 1,
            encoding: AudioEncoding::SignedPcm16Le,
            is_final_chunk: false,
            samples,
        }));

        let encoded_message = encode_message(&message, &mut payload_buffer).unwrap();
        let frame = wrap_payload(encoded_message, 2, &mut frame_buffer).unwrap();
        std::fs::write(&path, frame).unwrap();

        assert_eq!(replay_capture(&path, true).unwrap(), 1);

        let _ = std::fs::remove_file(path);
    }
}

use heapless::Vec;
use serde::{Deserialize, Serialize};

/// Maximum number of PCM samples carried in a single audio chunk command.
///
/// This matches the current host and firmware default chunk size so a default
/// audio plan can cross the wire without further fragmentation.
pub const AUDIO_CHUNK_CAPACITY_SAMPLES: usize = 240;

/// Encoding used by forwarded audio chunks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AudioEncoding {
    SignedPcm16Le,
}

/// PCM audio chunk forwarded from the host to the firmware.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudioChunkCommand {
    pub utterance_id: u32,
    pub chunk_index: u16,
    pub sample_rate_hz: u32,
    pub channels: u8,
    pub encoding: AudioEncoding,
    pub is_final_chunk: bool,
    pub samples: Vec<i16, AUDIO_CHUNK_CAPACITY_SAMPLES>,
}

//! Framing helpers for byte-oriented transports such as USB CDC and UART.

use heapless::Vec;

use crate::checksum::crc16;
use crate::codec::MAX_PAYLOAD_LEN;

pub const FRAME_DELIMITER: u8 = 0;
pub const PROTOCOL_VERSION: u8 = 1;

const HEADER_LEN: usize = 5;
const CRC_LEN: usize = 2;
const DELIMITER_LEN: usize = 1;
const MAX_RAW_FRAME_LEN: usize = HEADER_LEN + MAX_PAYLOAD_LEN + CRC_LEN;

pub const MAX_FRAME_BODY_LEN: usize = cobs_max_encoded_len(MAX_RAW_FRAME_LEN);

const fn cobs_max_encoded_len(raw_len: usize) -> usize {
    if raw_len == 0 {
        1
    } else {
        raw_len + 1 + ((raw_len - 1) / 254)
    }
}

/// Errors returned while encoding or decoding framed protocol packets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameError {
    BufferTooSmall,
    PayloadTooLarge,
    FrameTooLarge,
    InvalidVersion(u8),
    TruncatedFrame,
    LengthMismatch,
    CobsDecode,
    CrcMismatch,
}

/// A successfully decoded transport frame.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecodedFrame {
    pub version: u8,
    pub sequence: u16,
    pub payload: Vec<u8, MAX_PAYLOAD_LEN>,
}

/// Incremental byte-stream decoder for framed protocol packets.
#[derive(Clone, Debug, Default)]
pub struct FrameDecoder {
    buffer: Vec<u8, MAX_FRAME_BODY_LEN>,
}

pub const fn encoded_frame_len(payload_len: usize) -> usize {
    cobs_max_encoded_len(HEADER_LEN + payload_len + CRC_LEN) + DELIMITER_LEN
}

/// Decode a single COBS-framed packet body without the trailing delimiter.
pub fn decode_frame(frame: &[u8]) -> Result<DecodedFrame, FrameError> {
    let mut raw_frame = [0u8; MAX_RAW_FRAME_LEN];
    let raw_len = cobs::decode(frame, &mut raw_frame).map_err(|_| FrameError::CobsDecode)?;
    let frame = &raw_frame[..raw_len];

    if frame.len() < HEADER_LEN + CRC_LEN {
        return Err(FrameError::TruncatedFrame);
    }

    let version = frame[0];
    if version != PROTOCOL_VERSION {
        return Err(FrameError::InvalidVersion(version));
    }

    let sequence = u16::from_le_bytes([frame[1], frame[2]]);
    let payload_len = u16::from_le_bytes([frame[3], frame[4]]) as usize;
    if payload_len > MAX_PAYLOAD_LEN {
        return Err(FrameError::PayloadTooLarge);
    }

    let expected_len = HEADER_LEN + payload_len + CRC_LEN;
    if frame.len() < expected_len {
        return Err(FrameError::TruncatedFrame);
    }
    if frame.len() != expected_len {
        return Err(FrameError::LengthMismatch);
    }

    let payload_end = HEADER_LEN + payload_len;
    let expected_crc = crc16(&frame[..payload_end]);
    let actual_crc = u16::from_le_bytes([frame[payload_end], frame[payload_end + 1]]);
    if expected_crc != actual_crc {
        return Err(FrameError::CrcMismatch);
    }

    let payload = Vec::from_slice(&frame[HEADER_LEN..payload_end]).map_err(|_| FrameError::PayloadTooLarge)?;

    Ok(DecodedFrame {
        version,
        sequence,
        payload,
    })
}

impl FrameDecoder {
    /// Push one byte from a transport stream and emit a frame once a delimiter is seen.
    pub fn push(&mut self, byte: u8) -> Result<Option<DecodedFrame>, FrameError> {
        if byte == FRAME_DELIMITER {
            if self.buffer.is_empty() {
                return Ok(None);
            }

            let decoded = decode_frame(self.buffer.as_slice());
            self.buffer.clear();
            return decoded.map(Some);
        }

        self.buffer.push(byte).map_err(|_| {
            self.buffer.clear();
            FrameError::FrameTooLarge
        })?;

        Ok(None)
    }
}

/// Wrap a postcard payload into a CRC-protected COBS frame terminated by `FRAME_DELIMITER`.
pub fn wrap_payload<'a>(payload: &[u8], sequence: u16, buffer: &'a mut [u8]) -> Result<&'a [u8], FrameError> {
    if payload.len() > MAX_PAYLOAD_LEN {
        return Err(FrameError::PayloadTooLarge);
    }

    let payload_len = u16::try_from(payload.len()).map_err(|_| FrameError::PayloadTooLarge)?;
    let frame_len = encoded_frame_len(payload.len());
    if buffer.len() < frame_len {
        return Err(FrameError::BufferTooSmall);
    }

    let mut raw_frame = [0u8; MAX_RAW_FRAME_LEN];
    raw_frame[0] = PROTOCOL_VERSION;
    raw_frame[1..3].copy_from_slice(&sequence.to_le_bytes());
    raw_frame[3..5].copy_from_slice(&payload_len.to_le_bytes());
    raw_frame[HEADER_LEN..HEADER_LEN + payload.len()].copy_from_slice(payload);

    let crc_offset = HEADER_LEN + payload.len();
    let checksum = crc16(&raw_frame[..crc_offset]);
    raw_frame[crc_offset..crc_offset + CRC_LEN].copy_from_slice(&checksum.to_le_bytes());

    let encoded_len = cobs::encode(&raw_frame[..crc_offset + CRC_LEN], &mut buffer[..frame_len - DELIMITER_LEN]);
    buffer[encoded_len] = FRAME_DELIMITER;

    Ok(&buffer[..encoded_len + DELIMITER_LEN])
}

#[cfg(test)]
mod tests {
    use super::{
        decode_frame, wrap_payload, FrameDecoder, FrameError, FRAME_DELIMITER, HEADER_LEN, MAX_RAW_FRAME_LEN,
        MAX_FRAME_BODY_LEN, PROTOCOL_VERSION, encoded_frame_len,
    };
    use crate::codec::MAX_PAYLOAD_LEN;

    #[test]
    fn decodes_wrapped_frame() {
        let payload = [1u8, 2, 3, 4];
        let mut frame = [0u8; 32];
        let encoded = wrap_payload(&payload, 42, &mut frame).unwrap();
        let decoded = decode_frame(&encoded[..encoded.len() - 1]).unwrap();

        assert_eq!(decoded.version, PROTOCOL_VERSION);
        assert_eq!(decoded.sequence, 42);
        assert_eq!(decoded.payload.as_slice(), &payload);
    }

    #[test]
    fn detects_crc_mismatch() {
        let payload = [9u8, 8, 7];
        let mut frame = [0u8; 32];
        let encoded = wrap_payload(&payload, 7, &mut frame).unwrap();
        let mut raw_frame = [0u8; MAX_RAW_FRAME_LEN];
        let decoded_len = cobs::decode(&encoded[..encoded.len() - 1], &mut raw_frame).unwrap();
        raw_frame[HEADER_LEN] ^= 0xff;

        let mut corrupted = [0u8; 32];
        let corrupted_len = cobs::encode(&raw_frame[..decoded_len], &mut corrupted);

        assert_eq!(decode_frame(&corrupted[..corrupted_len]), Err(FrameError::CrcMismatch));
    }

    #[test]
    fn frame_decoder_resynchronizes_on_delimiters() {
        let payload = [0x11u8, 0x22, 0x33];
        let mut frame = [0u8; 32];
        let encoded = wrap_payload(&payload, 5, &mut frame).unwrap();
        let mut decoder = FrameDecoder::default();
        let mut last = None;

        for byte in [FRAME_DELIMITER]
            .into_iter()
            .chain(encoded.iter().copied())
            .chain([FRAME_DELIMITER])
        {
            if let Some(decoded) = decoder.push(byte).unwrap() {
                last = Some(decoded);
            }
        }

        let decoded = last.expect("expected decoded frame");
        assert_eq!(decoded.sequence, 5);
        assert_eq!(decoded.payload.as_slice(), &payload);
    }

    #[test]
    fn roundtrips_payloads_with_zero_bytes() {
        let payload = [0u8, 1, 0, 2, 0, 3];
        let mut frame = [0u8; 32];
        let encoded = wrap_payload(&payload, 9, &mut frame).unwrap();
        let decoded = decode_frame(&encoded[..encoded.len() - 1]).unwrap();

        assert_eq!(decoded.sequence, 9);
        assert_eq!(decoded.payload.as_slice(), &payload);
    }

    #[test]
    fn sizes_buffer_for_large_payloads() {
        let payload = [0x55u8; MAX_PAYLOAD_LEN];
        let mut frame = [0u8; MAX_FRAME_BODY_LEN + 1];
        let encoded = wrap_payload(&payload, 123, &mut frame).unwrap();

        assert_eq!(encoded.len(), encoded_frame_len(MAX_PAYLOAD_LEN));
        assert_eq!(decode_frame(&encoded[..encoded.len() - 1]).unwrap().payload.as_slice(), &payload);
    }
}

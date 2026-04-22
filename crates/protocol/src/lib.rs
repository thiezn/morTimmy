#![no_std]

pub mod checksum;
pub mod codec;
pub mod framing;
pub mod messages;

pub use checksum::crc16;
pub use codec::{CodecError, MAX_PAYLOAD_LEN, decode_message, encode_message};
pub use framing::{
    DecodedFrame, FRAME_DELIMITER, FrameDecoder, FrameError, MAX_FRAME_BODY_LEN, PROTOCOL_VERSION,
    decode_frame, encoded_frame_len, wrap_payload,
};

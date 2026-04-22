//! CRC helpers shared by the host and embedded protocol stack.

use crc::{CRC_16_IBM_SDLC, Crc};

const CRC16: Crc<u16> = Crc::<u16>::new(&CRC_16_IBM_SDLC);

/// Compute the configured CRC16 checksum for a byte slice.
pub fn crc16(bytes: &[u8]) -> u16 {
    CRC16.checksum(bytes)
}

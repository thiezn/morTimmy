//! UF2 post-processing for RP2350 deployment.

use std::path::Path;

use anyhow::{Result, bail};

const UF2_BLOCK_SIZE: usize = 512;
const UF2_MAGIC_START0: u32 = 0x0A32_4655;
const UF2_MAGIC_START1: u32 = 0x9E5D_5157;
const UF2_MAGIC_END: u32 = 0x0AB1_6F30;
const UF2_FLAG_FAMILY_ID: u32 = 0x0000_2000;
const UF2_FLAG_EXTENSION_TAGS: u32 = 0x0000_8000;
const UF2_PAYLOAD_SIZE: usize = 256;
const ABSOLUTE_FAMILY_ID: u32 = 0xE48B_FF57;
const UF2_EXTENSION_RP2_IGNORE_BLOCK: u32 = 0x9957_E304;

/// Patch a UF2 to the requested family and optionally prepend the RP2350 ignore block workaround.
pub fn patch_uf2(path: &Path, family_id: u32, absolute_block_location: Option<u32>) -> Result<Uf2PatchSummary> {
    let patched_blocks = patch_family(path, family_id)?;
    let absolute_block_added = ensure_absolute_ignore_block(path, family_id, absolute_block_location)?;

    Ok(Uf2PatchSummary {
        patched_blocks,
        absolute_block_added,
    })
}

/// Summary of UF2 mutations performed in place.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Uf2PatchSummary {
    pub patched_blocks: usize,
    pub absolute_block_added: bool,
}

fn patch_family(path: &Path, family_id: u32) -> Result<usize> {
    let mut data = std::fs::read(path)?;
    if data.len() % UF2_BLOCK_SIZE != 0 {
        bail!("{} is not a whole-number UF2 block stream", path.display());
    }

    let mut patched_blocks = 0;
    for offset in (0..data.len()).step_by(UF2_BLOCK_SIZE) {
        let block = &data[offset..offset + UF2_BLOCK_SIZE];
        validate_block(block, offset)?;
        if is_absolute_ignore_block(block) {
            continue;
        }
        let flags = read_u32(&data, offset + 8) | UF2_FLAG_FAMILY_ID;
        write_u32(&mut data, offset + 8, flags);
        write_u32(&mut data, offset + 28, family_id);
        patched_blocks += 1;
    }

    std::fs::write(path, data)?;
    Ok(patched_blocks)
}

fn ensure_absolute_ignore_block(
    path: &Path,
    family_id: u32,
    absolute_block_location: Option<u32>,
) -> Result<bool> {
    let Some(absolute_block_location) = absolute_block_location else {
        return Ok(false);
    };

    if family_id == ABSOLUTE_FAMILY_ID {
        return Ok(false);
    }

    let data = std::fs::read(path)?;
    if data.len() >= UF2_BLOCK_SIZE && is_absolute_ignore_block(&data[..UF2_BLOCK_SIZE]) {
        return Ok(false);
    }

    let mut patched = Vec::with_capacity(data.len() + UF2_BLOCK_SIZE);
    patched.extend_from_slice(&make_absolute_ignore_block(absolute_block_location));
    patched.extend_from_slice(&data);
    std::fs::write(path, patched)?;
    Ok(true)
}

fn validate_block(block: &[u8], offset: usize) -> Result<()> {
    if read_u32(block, 0) != UF2_MAGIC_START0
        || read_u32(block, 4) != UF2_MAGIC_START1
        || read_u32(block, UF2_BLOCK_SIZE - 4) != UF2_MAGIC_END
    {
        bail!("invalid UF2 block at offset {offset}");
    }

    Ok(())
}

fn make_absolute_ignore_block(absolute_block_location: u32) -> [u8; UF2_BLOCK_SIZE] {
    let mut block = [0u8; UF2_BLOCK_SIZE];
    write_u32(&mut block, 0, UF2_MAGIC_START0);
    write_u32(&mut block, 4, UF2_MAGIC_START1);
    write_u32(&mut block, 8, UF2_FLAG_FAMILY_ID | UF2_FLAG_EXTENSION_TAGS);
    write_u32(&mut block, 12, absolute_block_location);
    write_u32(&mut block, 16, UF2_PAYLOAD_SIZE as u32);
    write_u32(&mut block, 20, 0);
    write_u32(&mut block, 24, 2);
    write_u32(&mut block, 28, ABSOLUTE_FAMILY_ID);
    block[32..32 + UF2_PAYLOAD_SIZE].fill(0xef);
    write_u32(&mut block, 32 + UF2_PAYLOAD_SIZE, UF2_EXTENSION_RP2_IGNORE_BLOCK);
    write_u32(&mut block, UF2_BLOCK_SIZE - 4, UF2_MAGIC_END);
    block
}

fn is_absolute_ignore_block(block: &[u8]) -> bool {
    block.len() == UF2_BLOCK_SIZE
        && read_u32(block, 0) == UF2_MAGIC_START0
        && read_u32(block, 4) == UF2_MAGIC_START1
        && read_u32(block, 8) == (UF2_FLAG_FAMILY_ID | UF2_FLAG_EXTENSION_TAGS)
        && read_u32(block, 16) == UF2_PAYLOAD_SIZE as u32
        && read_u32(block, 20) == 0
        && read_u32(block, 24) == 2
        && read_u32(block, 28) == ABSOLUTE_FAMILY_ID
        && block[32..32 + UF2_PAYLOAD_SIZE].iter().all(|byte| *byte == 0xef)
        && read_u32(block, 32 + UF2_PAYLOAD_SIZE) == UF2_EXTENSION_RP2_IGNORE_BLOCK
        && read_u32(block, UF2_BLOCK_SIZE - 4) == UF2_MAGIC_END
}

fn read_u32(buffer: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(buffer[offset..offset + 4].try_into().expect("u32 slice"))
}

fn write_u32(buffer: &mut [u8], offset: usize, value: u32) {
    buffer[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

#[cfg(test)]
mod tests {
    use super::{UF2_BLOCK_SIZE, UF2_MAGIC_END, UF2_MAGIC_START0, UF2_MAGIC_START1, patch_uf2, read_u32, write_u32};

    fn unique_temp_path(prefix: &str) -> std::path::PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        std::env::temp_dir().join(format!("{prefix}_{}_{}.uf2", std::process::id(), nanos))
    }

    fn sample_block() -> [u8; UF2_BLOCK_SIZE] {
        let mut block = [0u8; UF2_BLOCK_SIZE];
        write_u32(&mut block, 0, UF2_MAGIC_START0);
        write_u32(&mut block, 4, UF2_MAGIC_START1);
        write_u32(&mut block, 8, 0);
        write_u32(&mut block, 12, 0x1000_0000);
        write_u32(&mut block, 16, 256);
        write_u32(&mut block, 20, 0);
        write_u32(&mut block, 24, 1);
        write_u32(&mut block, 28, 0);
        write_u32(&mut block, UF2_BLOCK_SIZE - 4, UF2_MAGIC_END);
        block
    }

    #[test]
    fn patches_family_and_adds_absolute_ignore_block() {
        let path = unique_temp_path("mortimmy_uf2_patch");
        std::fs::write(&path, sample_block()).unwrap();

        let summary = patch_uf2(&path, 0xE48B_FF59, Some(0x10FF_FF00)).unwrap();
        let data = std::fs::read(&path).unwrap();

        assert_eq!(summary.patched_blocks, 1);
        assert!(summary.absolute_block_added);
        assert_eq!(data.len(), UF2_BLOCK_SIZE * 2);
        assert_eq!(read_u32(&data, UF2_BLOCK_SIZE + 28), 0xE48B_FF59);

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn does_not_duplicate_absolute_ignore_block() {
        let path = unique_temp_path("mortimmy_uf2_patch_existing");
        let mut data = Vec::new();
        data.extend_from_slice(&super::make_absolute_ignore_block(0x10FF_FF00));
        data.extend_from_slice(&sample_block());
        std::fs::write(&path, data).unwrap();

        let summary = patch_uf2(&path, 0xE48B_FF59, Some(0x10FF_FF00)).unwrap();
        let patched = std::fs::read(&path).unwrap();

        assert_eq!(summary.patched_blocks, 1);
        assert!(!summary.absolute_block_added);
        assert_eq!(patched.len(), UF2_BLOCK_SIZE * 2);

        let _ = std::fs::remove_file(path);
    }
}

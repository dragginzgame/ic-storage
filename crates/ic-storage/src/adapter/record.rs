//! Bounded, integrity-checked records for the owned B-tree.
use super::{
    Memory,
    error::{CollectionError as Error, Result, ensure},
};
use crate::{WASM_PAGE_SIZE_BYTES as PAGE, hash};

pub const HEADER_BYTES: u64 = 4096;
pub const MAX_RECORD: usize = 16 * 1024 * 1024;
const METADATA_BYTES: usize = 12; // u32 length followed by u64 checksum
pub(super) const PREFIX: usize = 8 + METADATA_BYTES;
pub fn encode_payload<const N: usize>(magic: [u8; N], payload: &[u8]) -> Result<Vec<u8>> {
    encode_payload_with(magic, payload.len(), |bytes| {
        bytes.extend_from_slice(payload);
        Ok(())
    })
}

// Append directly into the final frame, then seal its admitted payload. The
// caller supplies the exact size so node serialization needs only one buffer.
pub fn encode_payload_with<const N: usize>(
    magic: [u8; N],
    length: usize,
    write: impl FnOnce(&mut Vec<u8>) -> Result<()>,
) -> Result<Vec<u8>> {
    ensure(length <= MAX_RECORD, Error::Budget)?;
    let header = N + METADATA_BYTES;
    let mut bytes = Vec::with_capacity(header + length);
    bytes.resize(header, 0);
    write(&mut bytes)?;
    ensure(
        bytes.len() == header + length,
        Error::Corrupt("record payload length"),
    )?;
    let digest = hash::fnv64(hash::FNV_OFFSET, &bytes[header..]);
    bytes[..N].copy_from_slice(&magic);
    bytes[N..N + 4].copy_from_slice(
        &u32::try_from(length)
            .map_err(|_| Error::Budget)?
            .to_le_bytes(),
    );
    bytes[N + 4..header].copy_from_slice(&digest.to_le_bytes());
    Ok(bytes)
}

pub fn decode_payload(magic: [u8; 8], bytes: &[u8]) -> Result<&[u8]> {
    ensure(bytes.len() >= PREFIX && bytes[..8] == magic, Error::Format)?;
    let length = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
    ensure(
        length <= MAX_RECORD && bytes.len() == PREFIX + length,
        Error::Budget,
    )?;
    let payload = &bytes[PREFIX..];
    check_digest(&bytes[12..PREFIX], payload)?;
    Ok(payload)
}

fn check_digest(digest: &[u8], payload: &[u8]) -> Result<()> {
    let digest = u64::from_le_bytes(digest.try_into().unwrap());
    ensure(
        hash::fnv64(hash::FNV_OFFSET, payload) == digest,
        Error::Checksum,
    )
}

pub fn read_bytes(
    memory: &impl Memory,
    offset: u64,
    capacity: u64,
    magic: [u8; 8],
) -> Result<Vec<u8>> {
    let (prefix, length) = read_prefix::<PREFIX>(memory, offset, capacity, &magic)?;
    let mut bytes = vec![0; PREFIX + length];
    bytes[..PREFIX].copy_from_slice(&prefix);
    memory.read(offset + PREFIX as u64, &mut bytes[PREFIX..]);
    Ok(bytes)
}

// External values need an owned payload, not an owned frame followed by a
// second allocation/copy. Keep admission and checksum policy shared with frames.
pub fn read_payload(
    memory: &impl Memory,
    offset: u64,
    capacity: u64,
    magic: [u8; 8],
) -> Result<Vec<u8>> {
    read_framed_payload::<PREFIX>(memory, offset, capacity, &magic)
}

pub(super) fn read_map_header(memory: &impl Memory, magic: &[u8; 9]) -> Result<Vec<u8>> {
    read_framed_payload::<{ 9 + METADATA_BYTES }>(memory, 0, HEADER_BYTES, magic)
}

fn read_framed_payload<const H: usize>(
    memory: &impl Memory,
    offset: u64,
    capacity: u64,
    magic: &[u8],
) -> Result<Vec<u8>> {
    let (prefix, length) = read_prefix::<H>(memory, offset, capacity, magic)?;
    let mut payload = vec![0; length];
    memory.read(offset + H as u64, &mut payload);
    check_digest(&prefix[H - 8..], &payload)?;
    Ok(payload)
}

fn read_prefix<const H: usize>(
    memory: &impl Memory,
    offset: u64,
    capacity: u64,
    magic: &[u8],
) -> Result<([u8; H], usize)> {
    ensure(capacity >= H as u64, Error::Budget)?;
    let end = offset.checked_add(capacity).ok_or(Error::Budget)?;
    ensure(
        end <= memory.size().checked_mul(PAGE).ok_or(Error::Budget)?,
        Error::Corrupt("record outside memory"),
    )?;
    let mut prefix = [0; H];
    memory.read(offset, &mut prefix);
    ensure(&prefix[..H - METADATA_BYTES] == magic, Error::Format)?;
    let length = u32::from_le_bytes(prefix[H - METADATA_BYTES..H - 8].try_into().unwrap()) as usize;
    ensure(
        length <= MAX_RECORD && (H + length) as u64 <= capacity,
        Error::Budget,
    )?;
    Ok((prefix, length))
}

pub fn reserve(memory: &impl Memory, bytes: u64) -> Result<()> {
    let pages = bytes.div_ceil(PAGE);
    let current = memory.size();
    if pages > current {
        ensure(memory.grow(pages - current) != -1, Error::Growth)?;
    }
    Ok(())
}

pub fn bound<T: super::Storable>() -> Option<(u32, bool)> {
    match T::BOUND {
        super::Bound::Unbounded => None,
        super::Bound::Bounded {
            max_size,
            is_fixed_size,
        } => Some((max_size, is_fixed_size)),
    }
}

//! Bounded two-root envelope around the product map's portable codec.
use alloc::vec::Vec;
use ic_storage_native::map::{Error, Map};
use sha2::{Digest, Sha256};

/// Maximum records in each independently identified map.
pub const LIMIT: usize = 64;
/// Complete envelope, two maximum product checkpoints, and outer digest.
pub const MAX_BYTES: usize = 96 + 2 * (56 + LIMIT * 16);
const MAGIC: &[u8; 8] = b"NKPAIR01";
const SEAL: &[u8; 8] = b"SEALED01";

/// Bind both logical maps and their order to one epoch and complete publication.
pub fn encode(maps: &[Map; 2], epoch: u64) -> Result<Vec<u8>, Error> {
    if maps
        .iter()
        .any(|map| map.len().is_ok_and(|count| count > LIMIT))
    {
        return Err(Error::Limit);
    }
    let parts = [maps[0].checkpoint(epoch)?, maps[1].checkpoint(epoch)?];
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(96 + parts[0].len() + parts[1].len())
        .map_err(|_| Error::Allocation)?;
    bytes.extend_from_slice(MAGIC);
    for word in [1, epoch, 1, 2, parts[0].len() as u64, parts[1].len() as u64] {
        bytes.extend_from_slice(&word.to_le_bytes());
    }
    bytes.extend_from_slice(SEAL);
    bytes.extend_from_slice(&parts[0]);
    bytes.extend_from_slice(&parts[1]);
    bytes.extend_from_slice(&Sha256::digest(&bytes));
    Ok(bytes)
}

/// Validate complete framing, root identities, epoch and both product payloads.
/// Both returned maps are fresh transient candidates; failure publishes neither.
pub fn decode(bytes: &[u8], epoch: u64) -> Result<[Map; 2], Error> {
    if !(96..=MAX_BYTES).contains(&bytes.len()) {
        return Err(Error::Checkpoint);
    }
    // Every requested word is inside the already admitted 64-byte header.
    let word = |offset| u64::from_le_bytes(core::array::from_fn(|index| bytes[offset + index]));
    if &bytes[..8] != MAGIC
        || word(8) != 1
        || word(24) != 1
        || word(32) != 2
        || &bytes[56..64] != SEAL
    {
        return Err(Error::Checkpoint);
    }
    if word(16) != epoch {
        return Err(Error::Epoch);
    }
    let a = usize::try_from(word(40)).map_err(|_| Error::Checkpoint)?;
    let b = usize::try_from(word(48)).map_err(|_| Error::Checkpoint)?;
    if !(56..=56 + LIMIT * 16).contains(&a)
        || !(56..=56 + LIMIT * 16).contains(&b)
        || 96 + a + b != bytes.len()
    {
        return Err(Error::Checkpoint);
    }
    let end = bytes.len() - 32;
    if Sha256::digest(&bytes[..end]).as_slice() != &bytes[end..] {
        return Err(Error::Integrity);
    }
    Ok([
        Map::from_checkpoint(&bytes[64..64 + a], epoch, LIMIT)?,
        Map::from_checkpoint(&bytes[64 + a..end], epoch, LIMIT)?,
    ])
}

#[cfg(test)]
mod tests;

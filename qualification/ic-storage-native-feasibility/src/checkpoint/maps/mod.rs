//! Bounded two-root envelope around the product map's portable codec.
use alloc::vec::Vec;
use ic_storage_native::map::{Error, Map};
use sha2::{Digest, Sha256};

pub mod paged;

/// Maximum records in each independently identified map.
pub const LIMIT: usize = 64;
/// Complete envelope, two maximum product checkpoints, and outer digest.
pub const MAX_BYTES: usize = 96 + 2 * (56 + LIMIT * 16);
const MAGIC: &[u8; 8] = b"NKPAIR01";
const SEAL: &[u8; 8] = b"SEALED01";

/// Transient logical records: root 1 then root 2, ascending u64 key per root.
/// These values carry no native layout, allocation or retained ownership.
pub type Records = [Vec<(u64, u64)>; 2];

/// Compare one bounded native root with admitted, key-sorted checkpoint records.
/// The scalar reader supplies each occupied native slot once in any order after
/// native custody admission. No native map or record buffer is reconstructed.
/// Repeated records, wrong values and missing/unexpected keys refuse. This is a
/// logical comparison, not raw-pointer, allocator or transaction admission.
pub fn verify_records(
    expected: &[(u64, u64)],
    count: usize,
    mut read: impl FnMut(usize) -> (u64, u64),
) -> Result<(), Error> {
    if expected.len() > LIMIT || count > LIMIT {
        return Err(Error::Limit);
    }
    if count != expected.len() || expected.windows(2).any(|pair| pair[0].0 >= pair[1].0) {
        return Err(Error::Checkpoint);
    }
    // One bit per possible checkpoint record; independent of native bucket
    // order. Counting matching values alone would admit a duplicated record
    // substituted for a missing record with the same total native count.
    let mut seen = [0u64; LIMIT.div_ceil(64)];
    for index in 0..count {
        let (key, value) = read(index);
        let position = expected
            .binary_search_by_key(&key, |record| record.0)
            .map_err(|_| Error::Checkpoint)?;
        let mask = 1u64 << (position % 64);
        let word = &mut seen[position / 64];
        if expected[position].1 != value || *word & mask != 0 {
            return Err(Error::Checkpoint);
        }
        *word |= mask;
    }
    // Equal counts and unique matching indices prove that every expected record
    // was visited, including a complete 64-record root without a shift by 64.
    Ok(())
}

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
/// Returns sorted logical records without rebuilding native hash tables.
/// Failure publishes neither root; native construction belongs to the adapter.
pub fn decode(bytes: &[u8], epoch: u64) -> Result<Records, Error> {
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
        Map::records_from_checkpoint(&bytes[64..64 + a], epoch, LIMIT)?,
        Map::records_from_checkpoint(&bytes[64 + a..end], epoch, LIMIT)?,
    ])
}

#[cfg(test)]
mod prefix_tests;
#[cfg(test)]
mod tests;

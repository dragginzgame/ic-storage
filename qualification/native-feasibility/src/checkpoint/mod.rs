//! Bounded logical scalar/vector checkpoint. See ../../checkpoint-format.md.
//! This fixture codec does not admit native layouts or own durable publication.

use sha2::{Digest, Sha256};

// Wire limit: independent of the ownership model's operational capacity.
const MAX_VALUES: usize = 16;
const HEADER_BYTES: usize = 80;
const SEAL: &[u8; 8] = b"SEALED01";
const MIN_BYTES: usize = HEADER_BYTES + SEAL.len() + 32;
const MAX_BYTES: usize = MIN_BYTES + MAX_VALUES * 8;

/// A fully decoded logical value; owns only fresh transient allocations.
#[derive(Debug, PartialEq, Eq)]
pub struct Checkpoint {
    /// Caller-bound source epoch; not a native allocation generation.
    pub source_epoch: u64,
    /// Record's integer field.
    pub counter: u64,
    /// Ordered initialized elements of the unique vector.
    pub values: Vec<u64>,
}

/// Typed refusal, without partial import or implicit initialization.
#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    /// Input size or collection count exceeds the declared limits.
    Limit,
    /// Incomplete, noncanonical or inconsistent byte framing.
    Framing,
    /// Unknown envelope identity or version.
    Format,
    /// Unknown logical schema identity or version.
    Schema,
    /// Unsupported root/ownership identities.
    Ownership,
    /// The bytes fail their SHA-256 integrity check.
    Integrity,
    /// The checkpoint does not belong to the expected source epoch.
    Epoch,
    /// A bounded transient allocation failed.
    Allocation,
}

/// Encodes one logical record; the fixed format never includes spare capacity.
pub fn encode(counter: u64, values: &[u64], source_epoch: u64) -> Result<Vec<u8>, Error> {
    if values.len() > MAX_VALUES {
        return Err(Error::Limit);
    }
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(MIN_BYTES + values.len() * 8)
        .map_err(|_| Error::Allocation)?;
    bytes.extend_from_slice(b"ICNCP001");
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&0u16.to_le_bytes());
    bytes.extend_from_slice(&(HEADER_BYTES as u32).to_le_bytes());
    bytes.extend_from_slice(b"RECU64V1");
    bytes.extend_from_slice(&1u32.to_le_bytes());
    bytes.extend_from_slice(&(values.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&source_epoch.to_le_bytes());
    bytes.extend_from_slice(&1u64.to_le_bytes());
    bytes.extend_from_slice(&2u64.to_le_bytes());
    bytes.extend_from_slice(&counter.to_le_bytes());
    bytes.extend_from_slice(&(values.len() as u64 * 8).to_le_bytes());
    bytes.extend_from_slice(&0u64.to_le_bytes());
    for value in values {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes.extend_from_slice(SEAL);
    let digest = Sha256::digest(&bytes);
    bytes.extend_from_slice(&digest);
    Ok(bytes)
}

/// Validates the whole envelope before allocating a bounded logical result.
/// A digest verifies integrity, not the identity or authority of its author.
pub fn decode(bytes: &[u8], expected_epoch: u64) -> Result<Checkpoint, Error> {
    if bytes.len() > MAX_BYTES {
        return Err(Error::Limit);
    }
    if bytes.len() < MIN_BYTES {
        return Err(Error::Framing);
    }
    if &bytes[..8] != b"ICNCP001" || bytes[8..10] != 1u16.to_le_bytes() {
        return Err(Error::Format);
    }
    if bytes[10..12] != [0; 2]
        || bytes[12..16] != (HEADER_BYTES as u32).to_le_bytes()
        || bytes[72..80] != [0; 8]
    {
        return Err(Error::Framing);
    }
    if &bytes[16..24] != b"RECU64V1" || bytes[24..28] != 1u32.to_le_bytes() {
        return Err(Error::Schema);
    }
    let count = u32::from_le_bytes(bytes[28..32].try_into().map_err(|_| Error::Framing)?);
    if count > MAX_VALUES as u32 {
        return Err(Error::Limit);
    }
    let count = count as usize;
    if word(bytes, 64)? != count as u64 * 8 || bytes.len() != MIN_BYTES + count * 8 {
        return Err(Error::Framing);
    }
    if word(bytes, 40)? != 1 || word(bytes, 48)? != 2 {
        return Err(Error::Ownership);
    }
    let seal_offset = HEADER_BYTES + count * 8;
    let digest_offset = seal_offset + SEAL.len();
    if &bytes[seal_offset..digest_offset] != SEAL {
        return Err(Error::Framing);
    }
    let digest = Sha256::digest(&bytes[..digest_offset]);
    if digest.as_slice() != &bytes[digest_offset..] {
        return Err(Error::Integrity);
    }
    let source_epoch = word(bytes, 32)?;
    if source_epoch != expected_epoch {
        return Err(Error::Epoch);
    }
    let counter = word(bytes, 56)?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| Error::Allocation)?;
    for index in 0..count {
        values.push(word(bytes, HEADER_BYTES + index * 8)?);
    }
    Ok(Checkpoint {
        source_epoch,
        counter,
        values,
    })
}

fn word(bytes: &[u8], offset: usize) -> Result<u64, Error> {
    let array = bytes.get(offset..offset + 8).ok_or(Error::Framing)?;
    Ok(u64::from_le_bytes(
        array.try_into().map_err(|_| Error::Framing)?,
    ))
}

#[cfg(test)]
mod tests;

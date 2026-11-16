//! Bounded logical scalar/vector checkpoint. See ../../checkpoint-format.md.
//! This fixture codec does not admit native layouts or own durable publication.

use crate::record::{self, Record};
use sha2::{Digest, Sha256};

// Wire limit: independent of the ownership model's operational capacity.
const MAX_VALUES: usize = 16;
/// Version-2 wire limit, covering the retained-vector fixture's complete extent.
pub const V2_MAX_VALUES: usize = 512;
const HEADER_BYTES: usize = 80;
const SEAL: &[u8; 8] = b"SEALED01";
const MIN_BYTES: usize = HEADER_BYTES + SEAL.len() + 32;
const MAX_BYTES: usize = MIN_BYTES + MAX_VALUES * 8;
/// Largest supported encoded checkpoint, including header, seal and digest.
pub const MAX_CHECKPOINT_BYTES: usize = MIN_BYTES + V2_MAX_VALUES * 8;

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

/// Logical record and vector decoded with explicit application schema selection.
#[derive(Debug, PartialEq, Eq)]
pub struct RecordCheckpoint {
    /// Caller-bound checkpoint epoch.
    pub source_epoch: u64,
    /// Validated scalar record; this enum is transient, not a stored Rust layout.
    pub record: Record,
    /// Fresh transient vector containing only initialized logical elements.
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
    /// Scalar fields violate the selected application schema's invariant.
    Record(record::Error),
}

/// Encodes the original version-1 record, with at most 16 elements.
/// The fixed format never includes spare capacity.
pub fn encode(counter: u64, values: &[u64], source_epoch: u64) -> Result<Vec<u8>, Error> {
    encode_version(
        Record::V1(record::V1 { counter }),
        values,
        source_epoch,
        1,
        MAX_VALUES,
    )
}

/// Encodes a version-2 record, with at most 512 initialized elements.
/// This explicit version change leaves the original writer's bytes unchanged.
pub fn encode_v2(counter: u64, values: &[u64], source_epoch: u64) -> Result<Vec<u8>, Error> {
    encode_version(
        Record::V1(record::V1 { counter }),
        values,
        source_epoch,
        2,
        V2_MAX_VALUES,
    )
}

/// Encode a concrete record: V1 keeps envelope v2; V2 uses envelope v3/schema 2.
/// The new ceiling occupies the old reserved word; earlier readers reject v3.
pub fn encode_record(record: Record, values: &[u64], source_epoch: u64) -> Result<Vec<u8>, Error> {
    let version = match record {
        Record::V1(_) => 2,
        Record::V2(_) => 3,
    };
    encode_version(record, values, source_epoch, version, V2_MAX_VALUES)
}

fn encode_version(
    record: Record,
    values: &[u64],
    source_epoch: u64,
    version: u16,
    limit: usize,
) -> Result<Vec<u8>, Error> {
    if values.len() > limit {
        return Err(Error::Limit);
    }
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(MIN_BYTES + values.len() * 8)
        .map_err(|_| Error::Allocation)?;
    bytes.extend_from_slice(b"ICNCP001");
    bytes.extend_from_slice(&version.to_le_bytes());
    bytes.extend_from_slice(&0u16.to_le_bytes());
    bytes.extend_from_slice(&(HEADER_BYTES as u32).to_le_bytes());
    bytes.extend_from_slice(record::SCHEMA_ID);
    let (schema, ceiling) = match record {
        Record::V1(_) => (1u32, 0),
        Record::V2(value) => (2u32, value.ceiling()),
    };
    bytes.extend_from_slice(&schema.to_le_bytes());
    bytes.extend_from_slice(&(values.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&source_epoch.to_le_bytes());
    bytes.extend_from_slice(&1u64.to_le_bytes());
    bytes.extend_from_slice(&2u64.to_le_bytes());
    bytes.extend_from_slice(&record.counter().to_le_bytes());
    bytes.extend_from_slice(&(values.len() as u64 * 8).to_le_bytes());
    bytes.extend_from_slice(&ceiling.to_le_bytes());
    for value in values {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes.extend_from_slice(SEAL);
    let digest = Sha256::digest(&bytes);
    bytes.extend_from_slice(&digest);
    Ok(bytes)
}

/// Validates a version-1 or version-2 envelope before allocating a bounded result.
/// A digest verifies integrity, not the identity or authority of its author.
pub fn decode(bytes: &[u8], expected_epoch: u64) -> Result<Checkpoint, Error> {
    let decoded = decode_inner(bytes, expected_epoch, false)?;
    Ok(Checkpoint {
        source_epoch: decoded.source_epoch,
        counter: decoded.record.counter(),
        values: decoded.values,
    })
}

/// Decode old or new record schemas without silently discarding added fields.
pub fn decode_record(bytes: &[u8], expected_epoch: u64) -> Result<RecordCheckpoint, Error> {
    decode_inner(bytes, expected_epoch, true)
}

fn decode_inner(
    bytes: &[u8],
    expected_epoch: u64,
    allow_v3: bool,
) -> Result<RecordCheckpoint, Error> {
    if bytes.len() > MAX_CHECKPOINT_BYTES {
        return Err(Error::Limit);
    }
    if bytes.len() < MIN_BYTES {
        return Err(Error::Framing);
    }
    if &bytes[..8] != b"ICNCP001" {
        return Err(Error::Format);
    }
    let version = u16::from_le_bytes(bytes[8..10].try_into().map_err(|_| Error::Framing)?);
    let limit = match version {
        1 if bytes.len() <= MAX_BYTES => MAX_VALUES,
        1 => return Err(Error::Limit),
        2 => V2_MAX_VALUES,
        3 if allow_v3 => V2_MAX_VALUES,
        _ => return Err(Error::Format),
    };
    if bytes[10..12] != [0; 2]
        || bytes[12..16] != (HEADER_BYTES as u32).to_le_bytes()
        || (version != 3 && bytes[72..80] != [0; 8])
    {
        return Err(Error::Framing);
    }
    let schema: u32 = if version == 3 { 2 } else { 1 };
    if &bytes[16..24] != record::SCHEMA_ID || bytes[24..28] != schema.to_le_bytes() {
        return Err(Error::Schema);
    }
    let count = u32::from_le_bytes(bytes[28..32].try_into().map_err(|_| Error::Framing)?);
    if count > limit as u32 {
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
    let record = if version == 3 {
        Record::V2(record::V2::new(counter, word(bytes, 72)?).map_err(Error::Record)?)
    } else {
        Record::V1(record::V1 { counter })
    };
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| Error::Allocation)?;
    for index in 0..count {
        values.push(word(bytes, HEADER_BYTES + index * 8)?);
    }
    Ok(RecordCheckpoint {
        source_epoch,
        record,
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

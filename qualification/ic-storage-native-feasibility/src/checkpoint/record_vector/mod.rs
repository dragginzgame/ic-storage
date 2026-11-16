//! Bounded V2/V3 record-vector checkpoints and explicit scalar-width conversion.
//! The wire payload uses generated field codecs, never native padding or pointers.

use crate::record::{
    V2,
    narrow::{self, V3},
};
use alloc::vec::Vec;
use ic_storage_native::record::Error as CodecError;
use sha2::{Digest, Sha256};

/// Maximum initialized records in this closed qualification profile.
pub const LIMIT: usize = 16;
const HEADER: usize = 32;
const DIGEST: usize = 32;
const MAGIC: &[u8; 8] = b"RECVEC01";
/// Maximum complete envelope size accepted by the governed provider.
pub const MAX_BYTES: usize = HEADER + LIMIT * V2::STORE_BYTES + DIGEST;

/// Refusal before destination allocation or publication.
#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    /// Envelope or count exceeds the declared bound.
    Limit,
    /// Incomplete or inconsistent framing.
    Framing,
    /// Unknown logical schema.
    Schema,
    /// Caller-selected source epoch differs.
    Epoch,
    /// Digest mismatch.
    Integrity,
    /// Generated field codec rejected an element.
    Record(CodecError),
    /// Transient allocation failed.
    Allocation,
    /// Explicit V2-to-V3 conversion failed at this ordered element.
    Conversion {
        /// Zero-based element in logical collection order.
        index: usize,
        /// Checked application conversion failure.
        cause: narrow::Error,
    },
}

/// Encode initialized fields only, with schema, count, epoch and integrity digest.
pub fn encode(values: &[V3], epoch: u64) -> Result<Vec<u8>, Error> {
    encode_records(values, epoch, V3::STORE_VERSION, V3::store_encode)
}

/// Encode the explicit V2 source schema; never cast its native representation.
pub fn encode_v2(values: &[V2], epoch: u64) -> Result<Vec<u8>, Error> {
    encode_records(values, epoch, V2::STORE_VERSION, V2::store_encode)
}

fn encode_records<T, const WIDTH: usize>(
    values: &[T],
    epoch: u64,
    schema: u32,
    payload: impl Fn(&T) -> Result<[u8; WIDTH], CodecError>,
) -> Result<Vec<u8>, Error> {
    if values.len() > LIMIT {
        return Err(Error::Limit);
    }
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(HEADER + values.len() * WIDTH + DIGEST)
        .map_err(|_| Error::Allocation)?;
    bytes.extend_from_slice(MAGIC);
    bytes.extend_from_slice(V3::STORE_ID);
    bytes.extend_from_slice(&schema.to_le_bytes());
    bytes.extend_from_slice(&(values.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&epoch.to_le_bytes());
    for value in values {
        bytes.extend_from_slice(&payload(value).map_err(Error::Record)?);
    }
    let digest = Sha256::digest(&bytes);
    bytes.extend_from_slice(&digest);
    Ok(bytes)
}

/// Validate all bytes before returning fresh scalar records in a bounded array.
/// No destination allocation or retained owner is created by this decoder.
pub fn decode(bytes: &[u8], epoch: u64) -> Result<([Option<V3>; LIMIT], usize), Error> {
    decode_records::<V3, { V3::STORE_BYTES }>(bytes, epoch, V3::STORE_VERSION, V3::store_decode)
}

/// Decode only the V2 source schema, preserving every u64 value and ceiling.
pub fn decode_v2(bytes: &[u8], epoch: u64) -> Result<([Option<V2>; LIMIT], usize), Error> {
    decode_records::<V2, { V2::STORE_BYTES }>(bytes, epoch, V2::STORE_VERSION, V2::store_decode)
}

/// Explicit whole-collection narrowing. Validate the entire V2 envelope first;
/// convert in order into fresh transient scalars before publishing any target.
pub fn decode_narrow_v2(bytes: &[u8], epoch: u64) -> Result<([Option<V3>; LIMIT], usize), Error> {
    let (source, count) = decode_v2(bytes, epoch)?;
    let mut target = [None; LIMIT];
    for (index, record) in source.into_iter().take(count).enumerate() {
        target[index] = Some(
            V3::try_from(record.ok_or(Error::Framing)?)
                .map_err(|cause| Error::Conversion { index, cause })?,
        );
    }
    Ok((target, count))
}

/// Explicit exact widening. Validate the complete V3 envelope before converting
/// its ordered fields to transient V2 records; no retained owner is constructed.
pub fn decode_widen_v3(bytes: &[u8], epoch: u64) -> Result<([Option<V2>; LIMIT], usize), Error> {
    let (source, count) = decode(bytes, epoch)?;
    Ok((source.map(|record| record.map(V2::from)), count))
}

fn decode_records<T: Copy, const WIDTH: usize>(
    bytes: &[u8],
    epoch: u64,
    schema: u32,
    payload: impl Fn(&[u8]) -> Result<T, CodecError>,
) -> Result<([Option<T>; LIMIT], usize), Error> {
    if bytes.len() > MAX_BYTES {
        return Err(Error::Limit);
    }
    if bytes.len() < HEADER + DIGEST || &bytes[..8] != MAGIC {
        return Err(Error::Framing);
    }
    if &bytes[8..16] != V3::STORE_ID || bytes[16..20] != schema.to_le_bytes() {
        return Err(Error::Schema);
    }
    let count = u32::from_le_bytes(bytes[20..24].try_into().map_err(|_| Error::Framing)?) as usize;
    if count > LIMIT {
        return Err(Error::Limit);
    }
    let payload_end = HEADER + count * WIDTH;
    if bytes.len() != payload_end + DIGEST {
        return Err(Error::Framing);
    }
    if bytes[24..32] != epoch.to_le_bytes() {
        return Err(Error::Epoch);
    }
    if bytes[payload_end..] != Sha256::digest(&bytes[..payload_end])[..] {
        return Err(Error::Integrity);
    }
    let mut records = [None; LIMIT];
    // Exact framing above establishes a whole number of fixed-size payloads.
    let (payloads, _) = bytes[HEADER..payload_end].as_chunks::<WIDTH>();
    for (slot, bytes) in records.iter_mut().zip(payloads) {
        *slot = Some(payload(bytes).map_err(Error::Record)?);
    }
    Ok((records, count))
}

#[cfg(test)]
mod tests;

//! Record-vector allocation, same-live-domain custody and logical reconstruction.
//! This private test codec is independent of the installed scalar/u64 checkpoint.

use super::Arena;
use ic_storage_native::record::Error as CodecError;
use ic_storage_native_feasibility::record::{
    V2,
    narrow::{self, V3},
};
use sha2::{Digest, Sha256};

const LIMIT: usize = 16;
const HEADER: usize = 32;
const DIGEST: usize = 32;
const MAGIC: &[u8; 8] = b"RECVEC01";
const MAX_BYTES: usize = HEADER + LIMIT * V3::STORE_BYTES + DIGEST;

#[derive(Debug, PartialEq, Eq)]
enum Error {
    Limit,
    Framing,
    Schema,
    Epoch,
    Integrity,
    Record(CodecError),
    Conversion(narrow::Error),
    Allocation,
    Interrupted,
}

fn encode(values: &[V3], epoch: u64) -> Result<Vec<u8>, Error> {
    if values.len() > LIMIT {
        return Err(Error::Limit);
    }
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(HEADER + values.len() * V3::STORE_BYTES + DIGEST)
        .map_err(|_| Error::Allocation)?;
    bytes.extend_from_slice(MAGIC);
    bytes.extend_from_slice(V3::STORE_ID);
    bytes.extend_from_slice(&V3::STORE_VERSION.to_le_bytes());
    bytes.extend_from_slice(&(values.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&epoch.to_le_bytes());
    for value in values {
        bytes.extend_from_slice(&value.store_encode().map_err(Error::Record)?);
    }
    let digest = Sha256::digest(&bytes);
    bytes.extend_from_slice(&digest);
    Ok(bytes)
}

// Validate the complete bounded envelope and every element before allocating in
// the destination arena. This transient array contains scalars, never old owners.
fn decode(bytes: &[u8], epoch: u64) -> Result<([Option<V3>; LIMIT], usize), Error> {
    if bytes.len() > MAX_BYTES {
        return Err(Error::Limit);
    }
    if bytes.len() < HEADER + DIGEST || &bytes[..8] != MAGIC {
        return Err(Error::Framing);
    }
    if &bytes[8..16] != V3::STORE_ID || bytes[16..20] != V3::STORE_VERSION.to_le_bytes() {
        return Err(Error::Schema);
    }
    let count = u32::from_le_bytes(bytes[20..24].try_into().map_err(|_| Error::Framing)?) as usize;
    if count > LIMIT {
        return Err(Error::Limit);
    }
    let payload_end = HEADER + count * V3::STORE_BYTES;
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
    let (payloads, _) = bytes[HEADER..payload_end].as_chunks::<{ V3::STORE_BYTES }>();
    for (slot, payload) in records.iter_mut().zip(payloads) {
        *slot = Some(V3::store_decode(payload).map_err(Error::Record)?);
    }
    Ok((records, count))
}

fn import<'a>(
    bytes: &[u8],
    epoch: u64,
    arena: &'a Arena,
    fail_at: Option<usize>,
) -> Result<Vec<V3, &'a Arena>, Error> {
    let (records, count) = decode(bytes, epoch)?;
    let mut candidate = Vec::new_in(arena);
    candidate
        .try_reserve_exact(count)
        .map_err(|_| Error::Allocation)?;
    for (index, record) in records.into_iter().take(count).enumerate() {
        if fail_at == Some(index) {
            return Err(Error::Interrupted);
        }
        candidate.push(record.ok_or(Error::Framing)?);
    }
    Ok(candidate)
}

fn convert<'a>(
    source: &[V2],
    arena: &'a Arena,
    fail_at: Option<usize>,
) -> Result<Vec<V3, &'a Arena>, Error> {
    if source.len() > LIMIT {
        return Err(Error::Limit);
    }
    let mut candidate = Vec::new_in(arena);
    candidate
        .try_reserve_exact(source.len())
        .map_err(|_| Error::Allocation)?;
    for (index, source) in source.iter().enumerate() {
        if fail_at == Some(index) {
            return Err(Error::Interrupted);
        }
        // Equal native sizes do not authorize a cast or reuse as another type.
        candidate.push(V3::try_from(*source).map_err(Error::Conversion)?);
    }
    Ok(candidate)
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod custody;

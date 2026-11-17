//! Record-vector allocation, same-live-domain custody and logical reconstruction.
//! The shared record-vector codec remains distinct from the scalar/u64 checkpoint.

use super::Arena;
use ic_storage::record::Error as CodecError;
use ic_storage_native_feasibility::record::{
    V2,
    narrow::{self, V3},
};
use sha2::{Digest, Sha256};

use ic_storage_native_feasibility::checkpoint::record_vector::{
    Error as CheckpointError, LIMIT, MAX_BYTES, decode, encode,
};

const HEADER: usize = 32;
const DIGEST: usize = 32;

#[derive(Debug, PartialEq, Eq)]
enum Error {
    Checkpoint(CheckpointError),
    Limit,
    Conversion(narrow::Error),
    Allocation,
    Interrupted,
}

fn import<'a>(
    bytes: &[u8],
    epoch: u64,
    arena: &'a Arena,
    fail_at: Option<usize>,
) -> Result<Vec<V3, &'a Arena>, Error> {
    let (records, count) = decode(bytes, epoch).map_err(Error::Checkpoint)?;
    let mut candidate = Vec::new_in(arena);
    candidate
        .try_reserve_exact(count)
        .map_err(|_| Error::Allocation)?;
    for (index, record) in records.into_iter().take(count).enumerate() {
        if fail_at == Some(index) {
            return Err(Error::Interrupted);
        }
        candidate.push(record.ok_or(Error::Checkpoint(CheckpointError::Framing))?);
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

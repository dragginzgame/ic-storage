//! IC Wire owns bounded serialization; Metrics owns numeric restoration.

use ic_wire::{BytesRef, native};
use serde::{Deserialize, Serialize};

use crate::{MAX_UNIT_BYTES, MetricIdentity, StoreError, metric::StoredMetric};

const MAGIC: &[u8; 4] = b"ICMW";
pub const MAX_RECORD_BYTES: usize = 6144;
// Conservative traversal allowances, not measured heap sizes. Borrowed envelope
// fields and the finite histogram list bound allocations before restoration.
const LIMITS: ic_wire::Limits = ic_wire::Limits {
    bytes: MAX_RECORD_BYTES - MAGIC.len(),
    allocation: 1024 * 1024,
    nodes: 16 * 1024,
    depth: 16,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum MetricKind {
    Aggregate = 1,
    Summary = 2,
    Histogram = 3,
    Counter = 4,
}

#[derive(Deserialize, Serialize)]
struct Record<'a> {
    kind: u8,
    unit: &'a str,
    epoch: u64,
    #[serde(borrow)]
    state: BytesRef<'a, MAX_RECORD_BYTES>,
}

const fn error(error: ic_wire::Error) -> StoreError {
    match error {
        ic_wire::Error::Malformed | ic_wire::Error::Trailing => StoreError::MalformedRecord,
        _ => StoreError::Codec,
    }
}

pub fn envelope<'a>(
    bytes: &'a [u8],
    identity: &MetricIdentity,
) -> Result<(MetricKind, &'a [u8]), StoreError> {
    if bytes.len() > MAX_RECORD_BYTES {
        return Err(StoreError::RecordTooLarge);
    }
    if bytes.len() < MAGIC.len() {
        return Err(StoreError::MalformedRecord);
    }
    let payload = bytes
        .strip_prefix(MAGIC)
        .ok_or(StoreError::UnsupportedFormat)?;
    let record: Record<'_> = native::decode(payload, LIMITS).map_err(error)?;
    let kind = match record.kind {
        1 => MetricKind::Aggregate,
        2 => MetricKind::Summary,
        3 => MetricKind::Histogram,
        4 => MetricKind::Counter,
        _ => return Err(StoreError::KindMismatch),
    };
    crate::identifier(record.unit.as_bytes(), MAX_UNIT_BYTES)
        .map_err(|_| StoreError::MalformedRecord)?;
    if record.unit != &*identity.unit || record.epoch != identity.epoch {
        return Err(StoreError::IdentityMismatch);
    }
    Ok((kind, record.state.as_slice()))
}

pub fn encode<T: StoredMetric>(
    identity: &MetricIdentity,
    value: &T,
) -> Result<Vec<u8>, StoreError> {
    T::ADMISSION?;
    let state = native::encode(&value.state(), LIMITS).map_err(error)?;
    let record = Record {
        kind: T::KIND as u8,
        unit: &identity.unit,
        epoch: identity.epoch,
        state: BytesRef::new(&state).map_err(|_| StoreError::Codec)?,
    };
    let payload = native::encode(&record, LIMITS).map_err(|err| match err {
        ic_wire::Error::Limit => StoreError::RecordTooLarge,
        other => error(other),
    })?;
    let mut bytes = Vec::with_capacity(MAGIC.len() + payload.len());
    bytes.extend_from_slice(MAGIC);
    bytes.extend_from_slice(&payload);
    Ok(bytes)
}

pub fn decode<T: StoredMetric>(
    bytes: &[u8],
    identity: &MetricIdentity,
    configuration: &T::Configuration,
) -> Result<T, StoreError> {
    T::ADMISSION?;
    let (kind, state) = envelope(bytes, identity)?;
    if kind != T::KIND {
        return Err(StoreError::KindMismatch);
    }
    let state = native::decode(state, LIMITS).map_err(error)?;
    T::restore(configuration, state)
}

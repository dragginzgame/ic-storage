//! Optional durable snapshots in one host-owned stable B-tree map.
//!
//! The host bootstraps `ic-storage` and supplies one governed IC Storage map,
//! selecting fresh creation or retained reopening. This crate owns no global registry, lifecycle hook,
//! platform read or endpoint. Keys are namespaced, not authentication boundaries.
//! Stable writes must run in update execution; queries cannot make them durable.
//!
//! Stored values have an explicit IC Wire encoding, independent of Rust layout. The
//! adapter rejects incompatible identities/configuration and malformed values.
//! Underlying collection geometry, allocation failures and IC execution traps
//! retain IC Storage's collection contract, not recoverable codec errors.

#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

use ic_metrics::persistence::{MeasurementStateError, validate_histogram_bounds};
use ic_metrics::{
    MeasurementAggregate, MeasurementCounter, MeasurementHistogram, MeasurementSummary,
};
use ic_storage::Memory;

use ic_storage::adapter::StableBTreeMap;
use metric::StoredMetric;

mod codec;
mod metric;

#[cfg(test)]
mod tests;

/// Maximum finite bounds per stored histogram, bounding codec work and records.
pub const MAX_HISTOGRAM_BOUNDS: usize = 256;

///
/// `StoreError`
///
/// Typed failures at key, identity, stored-value and snapshot admission boundaries.
///

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum StoreError {
    /// The governed collection rejected encoding, integrity or capacity admission.
    #[error("storage: {0}")]
    Storage(#[from] ic_storage::adapter::CollectionError),
    /// Namespace, name or unit is empty, too long or contains unsupported bytes.
    #[error("invalid bounded metric identifier")]
    InvalidIdentifier,
    /// The stored record exceeds the admitted byte budget.
    #[error("metric record exceeds byte budget")]
    RecordTooLarge,
    /// Missing bytes, invalid flags or trailing data in a Wire record.
    #[error("malformed metric record")]
    MalformedRecord,
    /// The record has an unknown format discriminator; it is never reinterpreted.
    #[error("unsupported metric record format")]
    UnsupportedFormat,
    /// IC Wire refused serialization or resource traversal.
    #[error("metric codec: {0}")]
    Codec(#[from] ic_wire::Error),
    /// The stored metric uses a different arithmetic kind.
    #[error("metric kind differs")]
    KindMismatch,
    /// Unit or reset epoch differs from the caller's expected identity.
    #[error("metric unit or reset epoch differs")]
    IdentityMismatch,
    /// Histogram finite bounds differ from the caller's expected configuration.
    #[error("histogram bounds differ")]
    BoundsMismatch,
    /// The histogram exceeds the adapter's bound-count limit.
    #[error("histogram has too many finite bounds")]
    TooManyBounds,
    /// Expected bounds or restored fields violate arithmetic invariants.
    #[error("invalid metric state: {0}")]
    InvalidState(#[from] MeasurementStateError),
    /// A checkpoint decreases counts/totals/maxima, clears sticky overflow, or its
    /// exact-count changes cannot be explained by appended observations.
    #[error("metric checkpoint regresses")]
    StateRegression,
}

const MAX_UNIT_BYTES: usize = 64;

fn identifier(value: &[u8], limit: usize) -> Result<(), StoreError> {
    if value.is_empty()
        || value.len() > limit
        || !value
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._/-:".contains(byte))
    {
        return Err(StoreError::InvalidIdentifier);
    }
    Ok(())
}

/// Permanent bounded namespace/name pair; neither component can contain NUL.
///
/// Encoded as `namespace`, NUL, `name`. No hashes, implicit consumer IDs or
/// process-local registration order participate in durable identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MetricKey(Vec<u8>);

impl MetricKey {
    /// Admit an ASCII namespace (up to 64 bytes) and name (up to 128 bytes).
    ///
    /// # Errors
    /// Rejects empty/oversized identifiers or bytes other than letters, digits,
    /// dot, underscore, slash, hyphen and colon.
    pub fn new(namespace: &str, name: &str) -> Result<Self, StoreError> {
        identifier(namespace.as_bytes(), 64)?;
        identifier(name.as_bytes(), 128)?;
        let mut bytes = Vec::with_capacity(namespace.len() + 1 + name.len());
        bytes.extend_from_slice(namespace.as_bytes());
        bytes.push(0);
        bytes.extend_from_slice(name.as_bytes());
        Ok(Self(bytes))
    }
}

/// Caller-owned measurement unit and reset epoch, stored in every record.
///
/// Upgrade alone does not advance the epoch of a deliberately retained lifetime
/// metric. A reset does; counter/call-context continuity is a separate contract.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MetricIdentity {
    unit: Box<str>,
    epoch: u64,
}

impl MetricIdentity {
    /// Admit a unit (same ASCII alphabet as keys, up to 64 bytes) and reset epoch.
    ///
    /// # Errors
    /// Returns `StoreError::InvalidIdentifier` for an invalid unit.
    pub fn new(unit: &str, epoch: u64) -> Result<Self, StoreError> {
        identifier(unit.as_bytes(), MAX_UNIT_BYTES)?;
        Ok(Self {
            unit: unit.into(),
            epoch,
        })
    }
}

/// Exact IC Storage collection used for Metrics snapshot rows.
///
/// This alias fixes the byte key/value shape without wrapping the collection or
/// changing its allocation, initialization or retained-format contract. Hosts may
/// use the equivalent `ic_storage::adapter::StableBTreeMap` type directly.
pub type MetricsMap<M> = StableBTreeMap<Vec<u8>, Vec<u8>, M>;

/// Shared durable metric snapshots in one application memory region.
///
/// Hosts supply IC Storage's public governed map and own its allocation, bootstrap
/// and fresh-versus-retained decision. Opening that map requires an admitted
/// `ic-storage` handle; a raw backing cannot bypass allocation admission:
///
/// ```compile_fail
/// use ic_storage::VectorMemory;
/// use ic_metrics_storage::MetricsStore;
/// use ic_storage::adapter::StableBTreeMap;
/// let _ = MetricsStore::new(StableBTreeMap::<Vec<u8>, Vec<u8>, _>::create(VectorMemory::default()).unwrap());
/// ```
///
/// ```compile_fail
/// use ic_storage::{Memory, RuntimeMemory};
/// use ic_metrics_storage::MetricsStore;
/// fn store<M: Memory>(memory: RuntimeMemory<M>) -> MetricsStore<M> {
///     MetricsStore::new(memory)
/// }
/// ```
pub struct MetricsStore<M: Memory> {
    map: MetricsMap<M>,
}

impl<M: Memory> MetricsStore<M> {
    /// Wrap a host-opened IC Storage map without initializing or reopening it.
    ///
    /// The map retains its allocation lease. Hosts choose fresh creation or
    /// retained reopening through IC Storage before transferring it here.
    #[must_use]
    pub const fn new(map: MetricsMap<M>) -> Self {
        Self { map }
    }

    /// Return the public IC Storage map for host-owned collection administration.
    ///
    /// Consuming the store transfers its allocation lease with the map; the
    /// allocation remains open until the returned map is dropped.
    #[must_use]
    pub fn into_map(self) -> MetricsMap<M> {
        self.map
    }

    /// Load a counter, checking its stored kind, unit and reset epoch.
    ///
    /// Absence is `None`, distinct from a stored empty or zero value.
    ///
    /// # Errors
    /// Rejects malformed records, incompatible identities/kinds and invalid state.
    pub fn load_counter(
        &self,
        key: &MetricKey,
        identity: &MetricIdentity,
    ) -> Result<Option<MeasurementCounter>, StoreError> {
        self.load::<MeasurementCounter>(key, identity, &())
    }

    /// Checkpoint a counter, preserving numeric continuity and sticky overflow.
    ///
    /// Identical retained values succeed without encoding or stable writes.
    ///
    /// # Errors
    /// Rejects retained-record errors or state regression before mutating the map.
    pub fn save_counter(
        &mut self,
        key: &MetricKey,
        identity: &MetricIdentity,
        value: &MeasurementCounter,
    ) -> Result<(), StoreError> {
        self.save(key, identity, value)
    }

    /// Load a aggregate, checking its stored kind, unit and reset epoch.
    ///
    /// Absence is `None`, distinct from a stored empty or zero value.
    ///
    /// # Errors
    /// Rejects malformed records, incompatible identities/kinds and invalid state.
    pub fn load_aggregate(
        &self,
        key: &MetricKey,
        identity: &MetricIdentity,
    ) -> Result<Option<MeasurementAggregate>, StoreError> {
        self.load::<MeasurementAggregate>(key, identity, &())
    }

    /// Checkpoint a aggregate, preserving numeric continuity and sticky overflow.
    ///
    /// Identical retained values succeed without encoding or stable writes.
    ///
    /// # Errors
    /// Rejects retained-record errors or state regression before mutating the map.
    pub fn save_aggregate(
        &mut self,
        key: &MetricKey,
        identity: &MetricIdentity,
        value: &MeasurementAggregate,
    ) -> Result<(), StoreError> {
        self.save(key, identity, value)
    }

    /// Load a summary, checking its stored kind, unit and reset epoch.
    ///
    /// Absence is `None`, distinct from a stored empty or zero value.
    ///
    /// # Errors
    /// Rejects malformed records, incompatible identities/kinds and invalid state.
    pub fn load_summary(
        &self,
        key: &MetricKey,
        identity: &MetricIdentity,
    ) -> Result<Option<MeasurementSummary>, StoreError> {
        self.load::<MeasurementSummary>(key, identity, &())
    }

    /// Checkpoint a summary, preserving numeric continuity and sticky overflow.
    ///
    /// Identical retained values succeed without encoding or stable writes.
    ///
    /// # Errors
    /// Rejects retained-record errors or state regression before mutating the map.
    pub fn save_summary(
        &mut self,
        key: &MetricKey,
        identity: &MetricIdentity,
        value: &MeasurementSummary,
    ) -> Result<(), StoreError> {
        self.save(key, identity, value)
    }

    /// Load a histogram with exactly the expected strictly increasing finite bounds.
    ///
    /// Absence is `None`. Bounds are configuration only; no empty histogram is needed.
    ///
    /// # Errors
    /// Rejects more than 256 bounds or invalid expected bounds before inspecting a row,
    /// then malformed/incompatible records or invalid stored arithmetic state.
    pub fn load_histogram<const N: usize>(
        &self,
        key: &MetricKey,
        identity: &MetricIdentity,
        bounds: &[u64; N],
    ) -> Result<Option<MeasurementHistogram<N>>, StoreError> {
        MeasurementHistogram::<N>::ADMISSION?;
        if validate_histogram_bounds(bounds).is_err() {
            return Err(StoreError::InvalidState(
                MeasurementStateError::InvalidBounds,
            ));
        }
        self.load_admitted::<MeasurementHistogram<N>>(key, identity, bounds)
    }

    /// Checkpoint a histogram with its immutable bounds and complete distribution.
    ///
    /// Identical retained values succeed without encoding or stable writes.
    ///
    /// # Errors
    /// Rejects more than 256 bounds, retained-record errors or any state regression
    /// before mutating the map, including decreased buckets or cleared overflow flags.
    pub fn save_histogram<const N: usize>(
        &mut self,
        key: &MetricKey,
        identity: &MetricIdentity,
        value: &MeasurementHistogram<N>,
    ) -> Result<(), StoreError> {
        self.save(key, identity, value)
    }

    fn load<T: StoredMetric>(
        &self,
        key: &MetricKey,
        identity: &MetricIdentity,
        configuration: &T::Configuration,
    ) -> Result<Option<T>, StoreError> {
        T::ADMISSION?;
        self.load_admitted::<T>(key, identity, configuration)
    }

    // Callers admit the static shape before inspecting stored bytes.
    fn load_admitted<T: StoredMetric>(
        &self,
        key: &MetricKey,
        identity: &MetricIdentity,
        configuration: &T::Configuration,
    ) -> Result<Option<T>, StoreError> {
        self.map
            .get(&key.0)
            .map(|bytes| codec::decode::<T>(&bytes, identity, configuration))
            .transpose()
    }

    // One checkpoint flow for all concrete adapters: validate the retained row,
    // skip identical state, reject regression, then encode and write.
    fn save<T: StoredMetric>(
        &mut self,
        key: &MetricKey,
        identity: &MetricIdentity,
        value: &T,
    ) -> Result<(), StoreError> {
        T::ADMISSION?;
        if let Some(previous) = self.load_admitted::<T>(key, identity, value.configuration())? {
            if value.same_state(&previous) {
                return Ok(());
            }
            if !value.continues(&previous) {
                return Err(StoreError::StateRegression);
            }
        }
        let bytes = codec::encode(identity, value)?;
        self.map.insert_discard(key.0.clone(), bytes);
        Ok(())
    }

    /// Explicitly discard a row after checking its stored envelope and identity.
    ///
    /// This deliberate reset/retirement does not free the map's memory ID. The
    /// payload may be corrupt: removal is an explicit disposition, never a load
    /// fallback. Save a replacement with a new epoch in the same update message,
    /// without an intervening await. The host owns that atomic reset boundary.
    ///
    /// # Errors
    /// Rejects invalid/unknown envelopes or differing unit/reset epoch.
    pub fn remove(
        &mut self,
        key: &MetricKey,
        identity: &MetricIdentity,
    ) -> Result<bool, StoreError> {
        let Some(bytes) = self.map.get(&key.0) else {
            return Ok(false);
        };
        codec::envelope(&bytes, identity)?;
        self.map.remove_discard(&key.0);
        Ok(true)
    }
}

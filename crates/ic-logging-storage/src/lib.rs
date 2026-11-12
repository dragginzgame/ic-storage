//! Explicit bounded logging checkpoints over a host-owned governed Storage map.
//!
//! Logging owns admission, retention and paging. Hosts own allocation, identity,
//! clocks, lifecycle, authorization and checkpoint cadence. This adapter owns no
//! global recorder, append loop, timer or endpoint. Stable writes require update
//! execution; collection/backing traps retain IC Storage's contract.

#![doc = include_str!("../README.md")]

use ic_logging::{Limits, Log, LogCheckpoint, RestoreError};
use ic_storage::Memory;
use ic_storage::adapter::StableBTreeMap;

mod codec;
#[cfg(test)]
mod tests;

/// Maximum encoded checkpoint bytes, including its envelope and all retained rows.
pub const MAX_RECORD_BYTES: usize = 65_536;
/// Maximum configured row count accepted by this adapter.
pub const MAX_ENTRIES: usize = 1_024;
/// Maximum configured message-byte limit accepted by this adapter.
pub const MAX_MESSAGE_BYTES: usize = 4_096;
/// Maximum configured source/topic-byte limit accepted by this adapter.
pub const MAX_LABEL_BYTES: usize = 128;
/// Maximum configured combined retained source/topic/message UTF-8 bytes.
pub const MAX_TEXT_BYTES: usize = MAX_RECORD_BYTES;

///
/// `StoreError`
///
/// Typed refusal before stable mutation; backing traps retain host semantics.
///

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum StoreError {
    /// The governed collection rejected encoding, integrity or capacity admission.
    #[error("storage: {0}")]
    Storage(#[from] ic_storage::adapter::CollectionError),
    /// Namespace or name violates its bounded ASCII contract.
    #[error("invalid log identifier")]
    InvalidIdentifier,
    /// Configured engine limits exceed this adapter's supported bounds.
    #[error("unsupported log limits")]
    UnsupportedLimits,
    /// Encoded checkpoint exceeds the fixed byte ceiling.
    #[error("log checkpoint exceeds byte budget")]
    RecordTooLarge,
    /// A stored field, length, flag, UTF-8 string or trailing byte is invalid.
    #[error("malformed log checkpoint")]
    MalformedRecord,
    /// Shared codec allocation or resource admission failed.
    #[error("log checkpoint codec failed: {0}")]
    Codec(#[from] ic_wire::Error),
    /// Unknown durable format discriminator.
    #[error("unsupported log checkpoint format")]
    UnsupportedFormat,
    /// Stored epoch differs from the host's expected identity.
    #[error("log checkpoint identity differs")]
    IdentityMismatch,
    /// Fixed stored limits differ from the host's expected limits.
    #[error("log checkpoint limits differ")]
    LimitsMismatch,
    /// Core Logging refuses the decoded state.
    #[error("invalid log state: {0}")]
    InvalidState(#[from] RestoreError),
    /// A checkpoint rewinds watermarks, rewrites issued rows or resurrects history.
    #[error("log checkpoint regresses")]
    StateRegression,
}

///
/// `LogKey`
///
/// Permanent namespace/name pair; neither a credential nor an allocation declaration.
///

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LogKey(Vec<u8>);

impl LogKey {
    /// Admit namespace/name lengths of 1..=64 / 1..=128 bytes.
    /// Letters, digits, dot, underscore, slash, hyphen and colon are permitted.
    ///
    /// # Errors
    /// Returns [`StoreError::InvalidIdentifier`] for empty, oversized or
    /// unsupported namespace/name bytes.
    pub fn new(namespace: &str, name: &str) -> Result<Self, StoreError> {
        for (value, maximum) in [(namespace, 64), (name, 128)] {
            if value.is_empty()
                || value.len() > maximum
                || !value
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"._/-:".contains(&byte))
            {
                return Err(StoreError::InvalidIdentifier);
            }
        }
        let mut bytes = Vec::with_capacity(namespace.len() + name.len() + 1);
        bytes.extend_from_slice(namespace.as_bytes());
        bytes.push(0);
        bytes.extend_from_slice(name.as_bytes());
        Ok(Self(bytes))
    }
}

///
/// `LogIdentity`
///
/// Host-selected durable generation; upgrades alone do not reset a retained log.
///

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LogIdentity {
    /// Explicit host epoch, carried unchanged across ordinary reopen operations.
    pub epoch: u64,
}

/// Exact governed Storage map used for namespaced checkpoint rows.
pub type LoggingMap<M> = StableBTreeMap<Vec<u8>, Vec<u8>, M>;

///
/// `LoggingStore`
///
/// Explicit checkpoints; the host chooses fresh creation versus retained loading
/// of the supplied map. No constructor initializes, repairs or resets storage.
///
/// ```compile_fail
/// use ic_storage::VectorMemory;
/// use ic_storage::adapter::StableBTreeMap;
/// use ic_logging_storage::LoggingStore;
/// let _ = LoggingStore::new(StableBTreeMap::<Vec<u8>, Vec<u8>, _>::create(VectorMemory::default()).unwrap());
/// ```
pub struct LoggingStore<M: Memory> {
    map: LoggingMap<M>,
}

impl<M: Memory> LoggingStore<M> {
    /// Transfer a host-opened governed map and its exclusive allocation lease.
    #[must_use]
    pub const fn new(map: LoggingMap<M>) -> Self {
        Self { map }
    }

    /// Transfer the same map/lease back to its host for explicit administration.
    #[must_use]
    pub fn into_map(self) -> LoggingMap<M> {
        self.map
    }

    /// Load and canonically restore one log under its exact identity and limits.
    /// Absence is `None`, distinct from a stored empty or exhausted log.
    ///
    /// # Errors
    /// Rejects unsupported limits, malformed/unknown records, identity or limits
    /// mismatch and state refused by canonical Logging restoration.
    ///
    /// # Panics
    /// Backing IO failures retain the host's panic/rollback behavior.
    pub fn load(
        &self,
        key: &LogKey,
        identity: LogIdentity,
        limits: Limits,
    ) -> Result<Option<Log>, StoreError> {
        validate_limits(limits)?;
        self.map
            .get(&key.0)
            .map(|bytes| codec::decode(&bytes, identity, limits))
            .transpose()
    }

    /// Checkpoint an admitted log, rejecting regression before any map write.
    /// Identical snapshots validate retained bytes and perform no stable write.
    /// Borrowed checkpoint encoding and comparison scale with retained history, not page size.
    ///
    /// # Errors
    /// Rejects unsupported limits, retained-record admission failures, regressing
    /// checkpoints and records exceeding the byte ceiling before map mutation.
    ///
    /// # Panics
    /// Backing IO, allocation and collection failures retain Storage's behavior.
    pub fn save(
        &mut self,
        key: &LogKey,
        identity: LogIdentity,
        log: &Log,
    ) -> Result<(), StoreError> {
        validate_limits(log.limits())?;
        let current = log.checkpoint();
        if let Some(bytes) = self.map.get(&key.0) {
            let previous = codec::decode(&bytes, identity, current.limits())?;
            let previous = previous.checkpoint();
            if previous.next_sequence() == current.next_sequence()
                && previous.last_created_at_secs() == current.last_created_at_secs()
                && previous.entries().eq(current.entries())
            {
                return Ok(());
            }
            validate_checkpoint(previous, current)?;
        }
        let bytes = codec::encode(identity, current)?;
        self.map.insert_discard(key.0.clone(), bytes);
        Ok(())
    }

    /// Explicitly remove a valid checkpoint after admitting its old identity/limits.
    /// For deliberate reset, save with a new host epoch in the same update without
    /// an await. Corrupt rows require host-owned map administration, never fallback.
    ///
    /// # Errors
    /// Rejects the same limits, identity and retained-record failures as
    /// [`Self::load`] before removal.
    ///
    /// # Panics
    /// Backing IO failures retain the host's panic/rollback behavior.
    pub fn remove(
        &mut self,
        key: &LogKey,
        identity: LogIdentity,
        limits: Limits,
    ) -> Result<bool, StoreError> {
        if self.load(key, identity, limits)?.is_none() {
            return Ok(false);
        }
        self.map.remove_discard(&key.0);
        Ok(true)
    }
}

const fn validate_limits(limits: Limits) -> Result<(), StoreError> {
    if limits.max_entries.get() > MAX_ENTRIES
        || limits.max_message_bytes > MAX_MESSAGE_BYTES
        || limits.max_label_bytes > MAX_LABEL_BYTES
        || limits.max_text_bytes > MAX_TEXT_BYTES
    {
        return Err(StoreError::UnsupportedLimits);
    }
    Ok(())
}

fn validate_checkpoint(
    previous: LogCheckpoint<'_>,
    current: LogCheckpoint<'_>,
) -> Result<(), StoreError> {
    let advanced = match (previous.next_sequence(), current.next_sequence()) {
        (Some(old), Some(new)) if new >= old => new != old,
        (Some(_), None) => true,
        (None, None) => false,
        _ => return Err(StoreError::StateRegression),
    };
    match (
        previous.last_created_at_secs(),
        current.last_created_at_secs(),
    ) {
        (Some(old), Some(new)) if new >= old && (advanced || new == old) => {}
        (None, _) => {}
        _ => return Err(StoreError::StateRegression),
    }
    let mut issued = current.entries().take_while(|entry| {
        previous
            .next_sequence()
            .is_none_or(|next| entry.sequence < next)
    });
    if let Some(first) = issued.next() {
        let start = previous
            .entries()
            .next()
            .and_then(|oldest| first.sequence.checked_sub(oldest.sequence))
            .and_then(|index| usize::try_from(index).ok())
            .ok_or(StoreError::StateRegression)?;
        let mut retained = previous.entries().skip(start);
        if std::iter::once(first)
            .chain(issued)
            .any(|entry| retained.next() != Some(entry))
        {
            return Err(StoreError::StateRegression);
        }
    }
    Ok(())
}

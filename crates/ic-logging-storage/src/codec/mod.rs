//! IC Wire owns bounded serialization; Logging owns restored-state admission.

use std::num::NonZeroUsize;

use ic_logging::{Entry, Level, Limits, Log, LogCheckpoint, LogSnapshot};
use ic_wire::{List, native};
use serde::{Deserialize, Serialize, Serializer};

use crate::{LogIdentity, MAX_ENTRIES, MAX_RECORD_BYTES, StoreError};

const MAGIC: &[u8] = b"ICL\x03";
// These are conservative Serde traversal allowances, not measured heap sizes.
// Static row bounds and borrowed text keep decoding bounded before restoration.
const CODEC_LIMITS: ic_wire::Limits = ic_wire::Limits {
    bytes: MAX_RECORD_BYTES - MAGIC.len(),
    allocation: 4 * 1024 * 1024,
    nodes: 64 + MAX_ENTRIES * 64,
    depth: 16,
};

#[derive(Deserialize, Serialize)]
struct Checkpoint<E> {
    epoch: u64,
    #[serde(with = "StoredLimits")]
    limits: Limits,
    next_sequence: Option<u64>,
    last_created_at_secs: Option<u64>,
    entries: E,
}

#[derive(Deserialize, Serialize)]
struct StoredEntry<'a> {
    sequence: u64,
    created_at_secs: u64,
    #[serde(with = "StoredLevel")]
    level: Level,
    truncated: bool,
    source: &'a str,
    #[serde(borrow)]
    topic: Option<&'a str>,
    message: &'a str,
}

// Remote derives describe Storage's schema without adding serialization to
// the dependency-free Logging engine or duplicating its semantic admission.
#[derive(Deserialize, Serialize)]
#[serde(remote = "Limits")]
#[expect(
    clippy::struct_field_names,
    reason = "Serde remote derives must match the canonical Logging limit field names"
)]
struct StoredLimits {
    max_entries: NonZeroUsize,
    max_message_bytes: usize,
    max_label_bytes: usize,
    max_text_bytes: usize,
}

#[derive(Deserialize, Serialize)]
#[serde(remote = "Level")]
enum StoredLevel {
    Debug,
    Info,
    Ok,
    Warn,
    Error,
}

// Iterate borrowed entries without collecting another row buffer. Decoding
// still uses the bounded List before any owned Logging restoration.
struct StoredEntries<'a>(LogCheckpoint<'a>);

fn serialize_entries<'a, S: Serializer>(
    entries: impl IntoIterator<Item = &'a Entry>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    serializer.collect_seq(entries.into_iter().map(|entry| StoredEntry {
        sequence: entry.sequence,
        created_at_secs: entry.created_at_secs,
        level: entry.level,
        truncated: entry.truncated,
        source: &entry.source,
        topic: entry.topic.as_deref(),
        message: &entry.message,
    }))
}

impl Serialize for StoredEntries<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serialize_entries(self.0.entries(), serializer)
    }
}

pub fn encode(identity: LogIdentity, state: LogCheckpoint<'_>) -> Result<Vec<u8>, StoreError> {
    if state.entries().len() > MAX_ENTRIES {
        return Err(StoreError::Codec);
    }
    encode_checkpoint(&Checkpoint {
        epoch: identity.epoch,
        limits: state.limits(),
        next_sequence: state.next_sequence(),
        last_created_at_secs: state.last_created_at_secs(),
        entries: StoredEntries(state),
    })
}

fn encode_checkpoint(checkpoint: &Checkpoint<impl Serialize>) -> Result<Vec<u8>, StoreError> {
    let payload = native::encode(checkpoint, CODEC_LIMITS).map_err(|error| match error {
        ic_wire::Error::Limit => StoreError::RecordTooLarge,
        _ => StoreError::Codec,
    })?;
    let mut bytes = Vec::with_capacity(MAGIC.len() + payload.len());
    bytes.extend_from_slice(MAGIC);
    bytes.extend_from_slice(&payload);
    Ok(bytes)
}

#[cfg(test)]
mod tests;
#[cfg(test)]
pub use tests::encode_snapshot;

pub fn decode(bytes: &[u8], identity: LogIdentity, limits: Limits) -> Result<Log, StoreError> {
    if bytes.len() > MAX_RECORD_BYTES {
        return Err(StoreError::RecordTooLarge);
    }
    let payload = bytes
        .strip_prefix(MAGIC)
        .ok_or(StoreError::UnsupportedFormat)?;
    let checkpoint: Checkpoint<List<StoredEntry<'_>, MAX_ENTRIES>> =
        native::decode(payload, CODEC_LIMITS).map_err(|error| match error {
            ic_wire::Error::Malformed | ic_wire::Error::Trailing => StoreError::MalformedRecord,
            _ => StoreError::Codec,
        })?;
    if checkpoint.epoch != identity.epoch {
        return Err(StoreError::IdentityMismatch);
    }
    if checkpoint.limits != limits {
        return Err(StoreError::LimitsMismatch);
    }
    if checkpoint.entries.as_slice().len() > limits.max_entries.get() {
        return Err(StoreError::InvalidState(
            ic_logging::RestoreError::TooManyEntries,
        ));
    }
    // Text borrows admitted input bytes. Apply the caller's field limits before
    // constructing owned Logging entries; canonical restoration follows once.
    let mut text_bytes = Some(0_usize);
    for entry in checkpoint.entries.as_slice() {
        if entry.source.len() > limits.max_label_bytes
            || entry
                .topic
                .is_some_and(|topic| topic.len() > limits.max_label_bytes)
            || entry.message.len() > limits.max_message_bytes
        {
            return Err(StoreError::MalformedRecord);
        }
        text_bytes = text_bytes.and_then(|total| {
            total.checked_add(
                entry.source.len() + entry.topic.map_or(0, str::len) + entry.message.len(),
            )
        });
    }
    if text_bytes.is_none_or(|bytes| bytes > limits.max_text_bytes) {
        return Err(StoreError::InvalidState(
            ic_logging::RestoreError::TextBudgetExceeded,
        ));
    }
    let entries = checkpoint
        .entries
        .into_vec()
        .into_iter()
        .map(|entry| Entry {
            sequence: entry.sequence,
            created_at_secs: entry.created_at_secs,
            level: entry.level,
            truncated: entry.truncated,
            source: entry.source.to_owned(),
            topic: entry.topic.map(str::to_owned),
            message: entry.message.to_owned(),
        })
        .collect();
    Log::restore(LogSnapshot {
        limits,
        entries,
        next_sequence: checkpoint.next_sequence,
        last_created_at_secs: checkpoint.last_created_at_secs,
    })
    .map_err(Into::into)
}

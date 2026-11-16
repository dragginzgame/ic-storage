//! Bounded durable migration admission for the two native allocation domains.
//! Integrity is not caller authentication; the installed adapter owns authority.

use crate::checkpoint;
use alloc::{vec, vec::Vec};
use core::marker::PhantomData;
use sha2::{Digest, Sha256};

mod payload;
pub use payload::{MapPair, Payload, RecordVector, ScalarVector};
/// Scalar counter/u64-vector job under the original envelope.
pub type Job = MigrationJob<ScalarVector>;
/// V3 record-vector job under its distinct envelope.
pub type RecordJob = MigrationJob<RecordVector>;
/// Two-map job under its own envelope and allocation-domain contract.
pub type MapJob = MigrationJob<MapPair>;

mod staged;
pub use staged::Batch;

const HEADER: usize = 64;
const OVERHEAD: usize = HEADER + 8 + 32;
/// Fixed metadata size when the unchanged nested checkpoint is stored separately.
pub const METADATA_BYTES: usize = OVERHEAD;
/// Maximum governed cell payload, including the nested version-2 checkpoint.
pub const MAX_BYTES: usize = OVERHEAD + ScalarVector::MAX_BYTES;
/// Maximum record job, including sixteen V3 records.
pub const RECORD_MAX_BYTES: usize = OVERHEAD + RecordVector::MAX_BYTES;
/// Maximum two-map job, including both bounded logical checkpoints.
pub const MAP_MAX_BYTES: usize = OVERHEAD + MapPair::MAX_BYTES;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Ready = 0,
    Sealed = 1,
    Consumed = 2,
    Cancelled = 3,
    Importing = 4,
}

/// Validated job; fields cannot be changed without the transition checks.
#[derive(Debug, PartialEq, Eq)]
pub struct MigrationJob<P: Payload> {
    payload: PhantomData<P>,
    phase: Phase,
    epoch: u64,
    source: u64,
    target: u64,
    checkpoint: Vec<u8>,
    progress: Option<staged::Progress>,
}

/// Refusal before publication. Installed callers trap to roll back native writes.
#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    /// Unknown or inconsistent envelope.
    Framing,
    /// Digest mismatch.
    Integrity,
    /// Unsupported or mismatched source/destination allocation domain.
    Domain,
    /// Stale, skipped or exhausted epoch.
    Epoch,
    /// Invalid phase or changed retry payload.
    State,
    /// Nested logical checkpoint refusal.
    Checkpoint(checkpoint::Error),
    /// V3 record checkpoint refusal.
    RecordCheckpoint(checkpoint::record_vector::Error),
    /// Two-map checkpoint refusal.
    MapCheckpoint(ic_storage_native::map::Error),
    /// Bounded allocation failed.
    Allocation,
}

fn domain<P: Payload>(value: u64) -> Result<(), Error> {
    if P::DOMAINS.contains(&value) {
        Ok(())
    } else {
        Err(Error::Domain)
    }
}

impl<P: Payload> MigrationJob<P> {
    /// Fresh governed locator only; never used as fallback after decode failure.
    pub fn initial(source: u64) -> Result<Self, Error> {
        domain::<P>(source)?;
        Ok(Self {
            payload: PhantomData,
            phase: Phase::Ready,
            epoch: 0,
            source,
            target: source,
            checkpoint: vec![],
            progress: None,
        })
    }

    // One transition owner for both concrete payload encoders. Encode only after
    // phase/domain/epoch admission, preserving the original refusal ordering.
    fn export_payload(
        &self,
        source: u64,
        target: u64,
        epoch: u64,
        encode: impl FnOnce() -> Result<Vec<u8>, Error>,
    ) -> Result<Self, Error> {
        if self.phase == Phase::Importing {
            return Err(Error::State);
        }
        domain::<P>(source)?;
        domain::<P>(target)?;
        if source == target {
            return Err(Error::Domain);
        }
        let owner = if self.phase == Phase::Consumed {
            self.target
        } else {
            self.source
        };
        if owner != source {
            return Err(Error::Domain);
        }
        if self.phase == Phase::Sealed {
            if epoch != self.epoch {
                return Err(Error::Epoch);
            }
            if target != self.target {
                return Err(Error::Domain);
            }
        } else if self.epoch.checked_add(1) != Some(epoch) {
            return Err(Error::Epoch);
        }
        let checkpoint = encode()?;
        if self.phase == Phase::Sealed && checkpoint != self.checkpoint {
            return Err(Error::State);
        }
        Ok(Self {
            payload: PhantomData,
            phase: Phase::Sealed,
            epoch,
            source,
            target,
            checkpoint,
            progress: None,
        })
    }

    /// Admit a sealed source for frozen resumption without changing the job.
    /// The caller must separately compare these logical values with its retained
    /// frozen source; matching job metadata alone does not establish continuity.
    pub fn frozen(&self, source: u64, target: u64, epoch: u64) -> Result<P::Decoded, Error> {
        if self.phase != Phase::Sealed {
            return Err(Error::State);
        }
        if self.target != target || self.source != source {
            return Err(Error::Domain);
        }
        if self.epoch != epoch {
            return Err(Error::Epoch);
        }
        P::decode(&self.checkpoint, epoch)
    }

    /// Admit and consume exactly one sealed job in transient state. The adapter
    /// publishes this record atomically with successful native reconstruction.
    pub fn consume(&mut self, target: u64, source: u64, epoch: u64) -> Result<P::Decoded, Error> {
        let decoded = self.frozen(source, target, epoch)?;
        self.phase = Phase::Consumed;
        Ok(decoded)
    }

    /// Cancel only on the original source, retaining the retired epoch and
    /// checkpoint. Returns true for a new cancellation, false for an identical
    /// retry. The adapter must atomically publish this and restore native custody.
    pub fn cancel(&mut self, source: u64, target: u64, epoch: u64) -> Result<bool, Error> {
        if !matches!(self.phase, Phase::Sealed | Phase::Cancelled) {
            return Err(Error::State);
        }
        if self.source != source || self.target != target {
            return Err(Error::Domain);
        }
        if self.epoch != epoch {
            return Err(Error::Epoch);
        }
        let changed = self.phase == Phase::Sealed;
        self.phase = Phase::Cancelled;
        Ok(changed)
    }

    /// Byte-defined job envelope; preserves the checkpoint after consumption.
    pub fn encode(&self) -> Result<Vec<u8>, Error> {
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(OVERHEAD + self.checkpoint.len())
            .map_err(|_| Error::Allocation)?;
        bytes.extend_from_slice(P::MAGIC);
        bytes.extend_from_slice(&if self.progress.is_some() { 2u16 } else { 1 }.to_le_bytes());
        bytes.extend_from_slice(&(self.phase as u16).to_le_bytes());
        bytes.extend_from_slice(&(HEADER as u32).to_le_bytes());
        for word in [self.epoch, self.source, self.target] {
            bytes.extend_from_slice(&word.to_le_bytes());
        }
        bytes.extend_from_slice(&(self.checkpoint.len() as u32).to_le_bytes());
        if let Some(progress) = self.progress {
            for word in [progress.next, progress.start, progress.limit] {
                bytes.extend_from_slice(&word.to_le_bytes());
            }
            bytes.extend_from_slice(&[0; 8]);
        } else {
            bytes.extend_from_slice(&[0; 20]);
        }
        bytes.extend_from_slice(&self.checkpoint);
        bytes.extend_from_slice(b"JOBDONE1");
        let digest = Sha256::digest(&bytes);
        bytes.extend_from_slice(&digest);
        Ok(bytes)
    }

    /// Separate the existing envelope into fixed metadata and checkpoint bytes.
    /// Metadata contains the header followed by the completeness marker/digest;
    /// its digest still covers the complete canonical envelope, including payload.
    /// Both parts must be published atomically by the storage adapter.
    pub fn encode_split(&self) -> Result<([u8; METADATA_BYTES], &[u8]), Error> {
        let bytes = self.encode()?;
        let mut metadata = [0; METADATA_BYTES];
        metadata[..HEADER].copy_from_slice(&bytes[..HEADER]);
        metadata[HEADER..].copy_from_slice(&bytes[bytes.len() - (OVERHEAD - HEADER)..]);
        Ok((metadata, &self.checkpoint))
    }

    /// Reassemble and validate separately stored metadata and checkpoint bytes.
    /// This is the same envelope admission as `decode`, including cross-part
    /// integrity and nested validation. It still reads the complete checkpoint.
    pub fn decode_split(metadata: &[u8], checkpoint: &[u8]) -> Result<Self, Error> {
        if metadata.len() != METADATA_BYTES || checkpoint.len() > P::MAX_BYTES {
            return Err(Error::Framing);
        }
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(OVERHEAD + checkpoint.len())
            .map_err(|_| Error::Allocation)?;
        bytes.extend_from_slice(&metadata[..HEADER]);
        bytes.extend_from_slice(checkpoint);
        bytes.extend_from_slice(&metadata[HEADER..]);
        Self::decode(&bytes)
    }

    /// Check framing, domain, phase and nested integrity before returning state.
    pub fn decode(bytes: &[u8]) -> Result<Self, Error> {
        if !(OVERHEAD..=OVERHEAD + P::MAX_BYTES).contains(&bytes.len()) {
            return Err(Error::Framing);
        }
        let word = |offset| {
            bytes[offset..offset + 8]
                .try_into()
                .map(u64::from_le_bytes)
                .map_err(|_| Error::Framing)
        };
        let len =
            u32::from_le_bytes(bytes[40..44].try_into().map_err(|_| Error::Framing)?) as usize;
        if &bytes[..8] != P::MAGIC
            || !matches!(bytes[8..10], [1, 0] | [2, 0])
            || bytes[12..16] != (HEADER as u32).to_le_bytes()
            || bytes[56..64] != [0; 8]
            || len != bytes.len() - OVERHEAD
            || &bytes[bytes.len() - 40..bytes.len() - 32] != b"JOBDONE1"
        {
            return Err(Error::Framing);
        }
        if Sha256::digest(&bytes[..bytes.len() - 32])[..] != bytes[bytes.len() - 32..] {
            return Err(Error::Integrity);
        }
        let phase = match u16::from_le_bytes(bytes[10..12].try_into().map_err(|_| Error::Framing)?)
        {
            0 => Phase::Ready,
            1 => Phase::Sealed,
            2 => Phase::Consumed,
            3 => Phase::Cancelled,
            4 => Phase::Importing,
            _ => return Err(Error::State),
        };
        let (epoch, source, target) = (word(16)?, word(24)?, word(32)?);
        domain::<P>(source)?;
        domain::<P>(target)?;
        let payload = &bytes[HEADER..HEADER + len];
        let count = if phase == Phase::Ready {
            if epoch != 0 || source != target || !payload.is_empty() {
                return Err(Error::State);
            }
            0
        } else {
            if epoch == 0 || source == target {
                return Err(Error::State);
            }
            P::len(&P::decode(payload, epoch)?) as u32
        };
        let progress = staged::Progress::decode(bytes, phase, count)?;
        let mut checkpoint = Vec::new();
        checkpoint
            .try_reserve_exact(len)
            .map_err(|_| Error::Allocation)?;
        checkpoint.extend_from_slice(payload);
        Ok(Self {
            payload: PhantomData,
            phase,
            epoch,
            source,
            target,
            checkpoint,
            progress,
        })
    }
}

impl Job {
    /// Seal scalar state at the next epoch, or retry the identical frozen export.
    /// The adapter must freeze the native source and authorize its caller first.
    pub fn export(
        &self,
        source: u64,
        target: u64,
        epoch: u64,
        counter: u64,
        values: &[u64],
    ) -> Result<Self, Error> {
        self.export_payload(source, target, epoch, || {
            checkpoint::encode_v2(counter, values, epoch).map_err(Error::Checkpoint)
        })
    }
}

impl RecordJob {
    /// Seal the complete V3 collection at the next epoch, or retry identically.
    /// The adapter must freeze the native source and authorize its caller first.
    pub fn export(
        &self,
        source: u64,
        target: u64,
        epoch: u64,
        values: &[crate::record::narrow::V3],
    ) -> Result<Self, Error> {
        self.export_payload(source, target, epoch, || {
            checkpoint::record_vector::encode(values, epoch).map_err(Error::RecordCheckpoint)
        })
    }
}

impl MapJob {
    /// Seal both logical maps at the next epoch, or retry the identical export.
    /// The adapter must freeze both roots and authorize its caller first.
    pub fn export(
        &self,
        source: u64,
        target: u64,
        epoch: u64,
        maps: &[ic_storage_native::map::Map; 2],
    ) -> Result<Self, Error> {
        self.export_payload(source, target, epoch, || {
            checkpoint::maps::encode(maps, epoch).map_err(Error::MapCheckpoint)
        })
    }
}

#[cfg(test)]
mod map_tests;
#[cfg(test)]
mod record_tests;
#[cfg(test)]
mod tests;

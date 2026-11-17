//! Bounded durable migration admission for the two native allocation domains.
//! Integrity is not caller authentication; the installed adapter owns authority.

use crate::checkpoint;
use alloc::{vec, vec::Vec};
use core::marker::PhantomData;
use sha2::{Digest, Sha256};

mod payload;
pub use payload::{MapPair, OrderedRoot, PagedMapPair, Payload, RecordVector, ScalarVector};
/// Scalar counter/u64-vector job under the original envelope.
pub type Job = MigrationJob<ScalarVector>;
/// V3 record-vector job under its distinct envelope.
pub type RecordJob = MigrationJob<RecordVector>;
/// Two-map job under its own envelope and allocation-domain contract.
pub type MapJob = MigrationJob<MapPair>;
/// Two paged map roots; progress counts pages, not records. Governed host and
/// installed native adapters qualify storage transactions and custody separately.
pub type PagedMapJob = MigrationJob<PagedMapPair>;
/// Ordered row/index root; progress counts bounded variable-length pages.
pub type OrderedJob = MigrationJob<OrderedRoot>;

mod ordered;
pub mod ordered_family;
/// Four independently declared ordered roots, with page-counted progress.
pub type OrderedFamilyJob = MigrationJob<ordered_family::Family>;

mod staged;
pub use staged::Batch;
pub mod paged_continuation;

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
    MapCheckpoint(ic_storage::map::Error),
    /// Ordered root manifest, page or continuation refusal.
    OrderedCheckpoint(ic_storage::ordered::Error),
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

// Hash the canonical ordering without joining separately stored parts. The
// digest's scratch space is independent of checkpoint size.
fn envelope_digest(header: &[u8], checkpoint: &[u8], seal: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(header);
    digest.update(checkpoint);
    digest.update(seal);
    digest.finalize().into()
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

    // One transition owner for the concrete payload encoders. Encode only after
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
    /// A paged payload returns only a manifest: the adapter must also read and
    /// verify its pages before comparing records with the frozen source.
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
    /// For paged maps, verifying all referenced pages is also the adapter's duty.
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
        let (metadata, checkpoint) = self.encode_split()?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(OVERHEAD + checkpoint.len())
            .map_err(|_| Error::Allocation)?;
        bytes.extend_from_slice(&metadata[..HEADER]);
        bytes.extend_from_slice(checkpoint);
        bytes.extend_from_slice(&metadata[HEADER..]);
        Ok(bytes)
    }

    /// Separate the existing envelope into fixed metadata and checkpoint bytes.
    /// Metadata contains the header followed by the completeness marker/digest;
    /// its digest still covers the complete canonical envelope, including payload.
    /// Both parts must be published atomically by the storage adapter.
    /// Encoding uses fixed scratch space and borrows the unchanged checkpoint.
    pub fn encode_split(&self) -> Result<([u8; METADATA_BYTES], &[u8]), Error> {
        let length = u32::try_from(self.checkpoint.len()).map_err(|_| Error::Framing)?;
        let mut metadata = [0; METADATA_BYTES];
        metadata[..8].copy_from_slice(P::MAGIC);
        metadata[8..10]
            .copy_from_slice(&if self.progress.is_some() { 2u16 } else { 1 }.to_le_bytes());
        metadata[10..12].copy_from_slice(&(self.phase as u16).to_le_bytes());
        metadata[12..16].copy_from_slice(&(HEADER as u32).to_le_bytes());
        for (index, word) in [self.epoch, self.source, self.target]
            .into_iter()
            .enumerate()
        {
            let offset = 16 + index * 8;
            metadata[offset..offset + 8].copy_from_slice(&word.to_le_bytes());
        }
        metadata[40..44].copy_from_slice(&length.to_le_bytes());
        if let Some(progress) = self.progress {
            for (index, word) in [progress.next, progress.start, progress.limit]
                .into_iter()
                .enumerate()
            {
                let offset = 44 + index * 4;
                metadata[offset..offset + 4].copy_from_slice(&word.to_le_bytes());
            }
        }
        metadata[HEADER..HEADER + 8].copy_from_slice(b"JOBDONE1");
        let digest = envelope_digest(
            &metadata[..HEADER],
            &self.checkpoint,
            &metadata[HEADER..HEADER + 8],
        );
        metadata[HEADER + 8..].copy_from_slice(&digest);
        Ok((metadata, &self.checkpoint))
    }

    /// Validate separately stored metadata and checkpoint bytes without joining
    /// them into a temporary envelope allocation.
    /// This is the same envelope admission as `decode`, including cross-part
    /// integrity and nested validation. It reads the complete stored payload
    /// and allocates its owned copy and decoded values. A paged payload contains
    /// only a manifest; admitting it does not validate the referenced page data.
    pub fn decode_split(metadata: &[u8], checkpoint: &[u8]) -> Result<Self, Error> {
        Self::decode_split_parts(metadata, checkpoint, false).map(|(job, _)| job)
    }

    fn decode_split_parts(
        metadata: &[u8],
        checkpoint: &[u8],
        retain_decoded: bool,
    ) -> Result<(Self, Option<P::Decoded>), Error> {
        if metadata.len() != METADATA_BYTES {
            return Err(Error::Framing);
        }
        Self::decode_parts(
            &metadata[..HEADER],
            checkpoint,
            &metadata[HEADER..],
            retain_decoded,
        )
    }

    /// Check framing, domain, phase and nested integrity before returning state.
    pub fn decode(bytes: &[u8]) -> Result<Self, Error> {
        if bytes.len() < OVERHEAD {
            return Err(Error::Framing);
        }
        let end = bytes.len() - (OVERHEAD - HEADER);
        Self::decode_parts(&bytes[..HEADER], &bytes[HEADER..end], &bytes[end..], false)
            .map(|(job, _)| job)
    }

    // One admission path for contiguous and split storage. Slice lengths are
    // checked before any field access, and integrity precedes semantic decoding.
    fn decode_parts(
        header: &[u8],
        payload: &[u8],
        trailer: &[u8],
        retain_decoded: bool,
    ) -> Result<(Self, Option<P::Decoded>), Error> {
        if header.len() != HEADER
            || trailer.len() != OVERHEAD - HEADER
            || payload.len() > P::MAX_BYTES
        {
            return Err(Error::Framing);
        }
        let word = |offset| {
            header[offset..offset + 8]
                .try_into()
                .map(u64::from_le_bytes)
                .map_err(|_| Error::Framing)
        };
        let len =
            u32::from_le_bytes(header[40..44].try_into().map_err(|_| Error::Framing)?) as usize;
        if &header[..8] != P::MAGIC
            || !matches!(header[8..10], [1, 0] | [2, 0])
            || header[12..16] != (HEADER as u32).to_le_bytes()
            || header[56..64] != [0; 8]
            || len != payload.len()
            || &trailer[..8] != b"JOBDONE1"
        {
            return Err(Error::Framing);
        }
        if envelope_digest(header, payload, &trailer[..8])[..] != trailer[8..] {
            return Err(Error::Integrity);
        }
        let phase = match u16::from_le_bytes(header[10..12].try_into().map_err(|_| Error::Framing)?)
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
        let decoded = if phase == Phase::Ready {
            if epoch != 0 || source != target || !payload.is_empty() {
                return Err(Error::State);
            }
            None
        } else {
            if epoch == 0 || source == target {
                return Err(Error::State);
            }
            Some(P::decode(payload, epoch)?)
        };
        let count =
            u32::try_from(decoded.as_ref().map_or(0, P::len)).map_err(|_| Error::Framing)?;
        let progress = staged::Progress::decode(header, phase, count)?;
        // Keep candidates only for the combined load-and-batch operation. Plain
        // reads still drop them before copying the job's owned checkpoint.
        let decoded = if retain_decoded {
            decoded
        } else {
            drop(decoded);
            None
        };
        let mut checkpoint = Vec::new();
        checkpoint
            .try_reserve_exact(len)
            .map_err(|_| Error::Allocation)?;
        checkpoint.extend_from_slice(payload);
        Ok((
            Self {
                payload: PhantomData,
                phase,
                epoch,
                source,
                target,
                checkpoint,
                progress,
            },
            decoded,
        ))
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
        maps: &[ic_storage::map::Map; 2],
    ) -> Result<Self, Error> {
        self.export_payload(source, target, epoch, || {
            checkpoint::maps::encode(maps, epoch).map_err(Error::MapCheckpoint)
        })
    }
}

impl PagedMapJob {
    /// Bind an already stored complete pair to the next job epoch.
    /// The adapter must authorize and freeze the source, write both page chains
    /// before sealing, and publish all parts atomically. This validates manifest
    /// bytes, not page contents or native custody. Import adapters must verify
    /// pages before committing progress and keep both target roots private until
    /// completion. The shared job does not supply those storage transactions.
    pub fn seal(
        &self,
        source: u64,
        target: u64,
        epoch: u64,
        manifest: &checkpoint::maps::paged::Manifest,
    ) -> Result<Self, Error> {
        self.export_payload(source, target, epoch, || {
            let bytes = manifest.encode();
            PagedMapPair::decode(&bytes, epoch)?;
            let mut payload = Vec::new();
            payload
                .try_reserve_exact(bytes.len())
                .map_err(|_| Error::Allocation)?;
            payload.extend_from_slice(&bytes);
            Ok(payload)
        })
    }
}

#[cfg(test)]
mod map_tests;
#[cfg(test)]
mod paged_tests;
#[cfg(test)]
mod record_tests;
#[cfg(test)]
mod tests;

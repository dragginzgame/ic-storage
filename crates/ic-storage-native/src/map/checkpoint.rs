//! Portable logical format: magic, epoch, count, key/value pairs, SHA-256.
//! Native bucket order may change; no slot tags, hashes, pointers or capacities
//! are imported. The caller owns authenticated checkpoint custody and freshness.

use alloc::vec::Vec;
use core::{alloc::Allocator, convert::Infallible};
use sha2::{Digest, Sha256};

use super::{Error, Map};

const MAGIC: &[u8; 8] = b"NKEYMAP1";
const HEADER: usize = 24;
const DIGEST: usize = 32;

/// Export failure; a failed sink may contain an incomplete, unusable checkpoint.
#[derive(Debug, PartialEq, Eq)]
pub enum ExportError<E> {
    /// Native owner, framing or allocation failed before writing any bytes.
    Map(Error),
    /// The caller's sink rejected a chunk; the map remains unchanged.
    Sink(E),
}

/// Import failure; no candidate is published and its allocation is reclaimed.
#[derive(Debug, PartialEq, Eq)]
pub enum ImportError<E> {
    /// Envelope, entries, integrity or allocation failed validation.
    Map(Error),
    /// The source could not fill a requested buffer completely.
    Source(E),
}

impl Map {
    /// Validate a complete portable checkpoint and return logical records in
    /// ascending key order, without reconstructing native hash-table storage.
    ///
    /// Framing, epoch, count and integrity are checked before allocating records.
    /// Duplicate keys refuse even with a valid digest. The output is transient
    /// and mutable; callers own staging, custody and atomic publication. This
    /// still reads the entire checkpoint and allocates one pair per record.
    pub fn records_from_checkpoint(
        bytes: &[u8],
        epoch: u64,
        limit: usize,
    ) -> Result<Vec<(u64, u64)>, Error> {
        let payload = checked_payload(bytes, epoch, limit)?;
        let mut records = Vec::new();
        records
            .try_reserve_exact(payload.len() / 16)
            .map_err(|_| Error::Allocation)?;
        for record in payload.as_chunks::<16>().0 {
            records.push((word(&record[..8]), word(&record[8..])));
        }
        records.sort_unstable_by_key(|record| record.0);
        if records.windows(2).any(|pair| pair[0].0 == pair[1].0) {
            return Err(Error::Checkpoint);
        }
        Ok(records)
    }

    /// Rebuild fresh native storage from a bounded, validated logical checkpoint.
    pub fn from_checkpoint(bytes: &[u8], epoch: u64, limit: usize) -> Result<Self, Error> {
        Self::from_checkpoint_in(bytes, epoch, limit, alloc::alloc::Global)
    }

    /// Stream a checkpoint into fresh host storage; see [`Self::from_reader_in`].
    pub fn from_reader<E>(
        length: usize,
        epoch: u64,
        limit: usize,
        read_exact: impl FnMut(&mut [u8]) -> Result<(), E>,
    ) -> Result<Self, ImportError<E>> {
        Self::from_reader_in(length, epoch, limit, alloc::alloc::Global, read_exact)
    }
}

impl<A: Allocator + Clone> Map<A> {
    /// Stream a portable logical checkpoint using constant encoder scratch space.
    ///
    /// The sink receives chunks of at most 32 bytes. It must buffer/coalesce I/O
    /// and publish only after this call succeeds. This neither snapshots stable
    /// memory nor authenticates its custody. Unwinding in the read-only sink
    /// cannot partially mutate the map. SHA-256 detects accidental corruption;
    /// the trusted caller selects the expected epoch on recovery.
    pub fn export<E>(
        &self,
        epoch: u64,
        mut sink: impl FnMut(&[u8]) -> Result<(), E>,
    ) -> Result<(), ExportError<E>> {
        self.healthy().map_err(ExportError::Map)?;
        let mut digest = Sha256::new();
        let mut write = |bytes: &[u8]| {
            digest.update(bytes);
            sink(bytes).map_err(ExportError::Sink)
        };
        write(MAGIC)?;
        write(&epoch.to_le_bytes())?;
        write(&(self.len as u64).to_le_bytes())?;
        for slot in &self.slots {
            if slot.occupied == 1 {
                let mut record = [0; 16];
                record[..8].copy_from_slice(&slot.key.to_le_bytes());
                record[8..].copy_from_slice(&slot.value.to_le_bytes());
                write(&record)?;
            }
        }
        sink(&digest.finalize()).map_err(ExportError::Sink)
    }

    /// Allocate a complete checkpoint in transient host memory.
    /// Use [`Self::export`] when a buffered external sink is preferable.
    pub fn checkpoint(&self, epoch: u64) -> Result<Vec<u8>, Error> {
        self.healthy()?;
        let length = encoded_length(self.len)?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(length)
            .map_err(|_| Error::Allocation)?;
        let result = self.export(epoch, |part| {
            bytes.extend_from_slice(part);
            Ok::<_, Infallible>(())
        });
        match result {
            Ok(()) => Ok(bytes),
            Err(ExportError::Map(error)) => Err(error),
            Err(ExportError::Sink(never)) => match never {},
        }
    }

    /// Restore under an explicit allocator after checking the entire envelope.
    /// Duplicate keys refuse, even when the attacker recomputes the digest.
    /// Native hash placement is rebuilt, never imported from checkpoint bytes.
    pub fn from_checkpoint_in(
        bytes: &[u8],
        epoch: u64,
        limit: usize,
        allocator: A,
    ) -> Result<Self, Error> {
        let payload = checked_payload(bytes, epoch, limit)?;
        let mut candidate = Self::new_in(limit, allocator);
        candidate.reserve(payload.len() / 16)?;
        insert_records(&mut candidate, payload)?;
        Ok(candidate)
    }

    /// Recover using a 4 KiB record buffer plus fixed header and digest state.
    ///
    /// `length` is the exact checkpoint object's length. The callback must fill
    /// each supplied buffer with the next consecutive bytes or return an error;
    /// requests never exceed 4096 bytes. The caller owns object framing, trusted
    /// epoch selection and source custody. Trailing bytes inside the declared
    /// length refuse; bytes outside that object are not read.
    ///
    /// Header/length/limit checks precede allocation. Unlike the slice importer,
    /// this single-pass path builds a private candidate before verifying its final
    /// digest. No candidate escapes until all entries and integrity pass. Errors
    /// or an unwinding source drop the candidate; an existing map is untouched.
    /// The allocator must support coexistence if the caller keeps an old map.
    /// This is synchronous streaming, not resumable cross-message recovery.
    pub fn from_reader_in<E>(
        length: usize,
        epoch: u64,
        limit: usize,
        allocator: A,
        mut read_exact: impl FnMut(&mut [u8]) -> Result<(), E>,
    ) -> Result<Self, ImportError<E>> {
        if length < HEADER + DIGEST {
            return Err(ImportError::Map(Error::Checkpoint));
        }
        let mut header = [0; HEADER];
        read_exact(&mut header).map_err(ImportError::Source)?;
        let count = header_count(&header, length, epoch, limit).map_err(ImportError::Map)?;
        let mut digest = Sha256::new();
        digest.update(header);
        let mut candidate = Self::new_in(limit, allocator);
        candidate.reserve(count).map_err(ImportError::Map)?;
        let mut buffer = [0; 4096];
        let mut remaining = count;
        while remaining != 0 {
            let records = remaining.min(buffer.len() / 16);
            let chunk = &mut buffer[..records * 16];
            read_exact(chunk).map_err(ImportError::Source)?;
            digest.update(&*chunk);
            insert_records(&mut candidate, chunk).map_err(ImportError::Map)?;
            remaining -= records;
        }
        let mut expected = [0; DIGEST];
        read_exact(&mut expected).map_err(ImportError::Source)?;
        if digest.finalize().as_slice() != expected {
            return Err(ImportError::Map(Error::Integrity));
        }
        Ok(candidate)
    }

    /// Replace even a poisoned owner only after constructing a complete candidate.
    /// Invalid input or failed allocation leaves the old state and poison intact.
    /// The allocator must permit the candidate to coexist with the old backing.
    pub fn recover(&mut self, bytes: &[u8], epoch: u64) -> Result<(), Error> {
        let candidate =
            Self::from_checkpoint_in(bytes, epoch, self.limit, self.slots.allocator().clone())?;
        *self = candidate;
        Ok(())
    }
}

// Both slice consumers use the same admission before choosing logical records
// or native reconstruction. Streaming keeps its distinct private-candidate path.
fn checked_payload(bytes: &[u8], epoch: u64, limit: usize) -> Result<&[u8], Error> {
    if bytes.len() < HEADER + DIGEST {
        return Err(Error::Checkpoint);
    }
    header_count(&bytes[..HEADER], bytes.len(), epoch, limit)?;
    let end = bytes.len() - DIGEST;
    if Sha256::digest(&bytes[..end]).as_slice() != &bytes[end..] {
        return Err(Error::Integrity);
    }
    Ok(&bytes[HEADER..end])
}

fn header_count(header: &[u8], length: usize, epoch: u64, limit: usize) -> Result<usize, Error> {
    if &header[..8] != MAGIC {
        return Err(Error::Checkpoint);
    }
    if word(&header[8..16]) != epoch {
        return Err(Error::Epoch);
    }
    let count = usize::try_from(word(&header[16..24])).map_err(|_| Error::Limit)?;
    if count > limit {
        return Err(Error::Limit);
    }
    if encoded_length(count)? != length {
        return Err(Error::Checkpoint);
    }
    Ok(count)
}

fn insert_records<A: Allocator + Clone>(map: &mut Map<A>, bytes: &[u8]) -> Result<(), Error> {
    map.with(|mut view| {
        for record in bytes.as_chunks::<16>().0 {
            if view
                .insert(word(&record[..8]), word(&record[8..]))?
                .is_some()
            {
                return Err(Error::Checkpoint);
            }
        }
        Ok(())
    })?
}

fn encoded_length(count: usize) -> Result<usize, Error> {
    count
        .checked_mul(16)
        .and_then(|bytes| bytes.checked_add(HEADER + DIGEST))
        .ok_or(Error::Limit)
}

fn word(bytes: &[u8]) -> u64 {
    let mut word = [0; 8];
    word.copy_from_slice(bytes);
    u64::from_le_bytes(word)
}

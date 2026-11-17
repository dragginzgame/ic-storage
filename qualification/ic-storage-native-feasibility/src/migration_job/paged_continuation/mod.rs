//! Governed continuation bound to job metadata and observed native prefixes.
//! Checksums detect corruption, not replacement by the trusted storage authority.
use super::{Error, PagedMapJob, Phase};
use core::convert::Infallible;
use ic_storage_native::map::{ExportError, export_records};
use ic_storage_native::map::{
    Map,
    paged::{CONTINUATION_BYTES, PAGE_RECORDS, Reader},
};
use sha2::{Digest, Sha256};

/// Header, exact job metadata, two reader cursors, native fingerprints and seal.
pub const BYTES: usize = 512;
const CURSORS: usize = 120;
const PREFIXES: usize = CURSORS + 2 * CONTINUATION_BYTES;
const SEAL: usize = PREFIXES + 64;

/// Transient observation of an admitted native root, never a retained Rust value.
/// Fingerprints include native bucket iteration order; they do not prove custody.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativePrefix {
    count: usize,
    fingerprint: [u8; 32],
}

impl NativePrefix {
    fn capture(
        count: usize,
        export: impl FnOnce(&mut Sha256) -> Result<(), ExportError<Infallible>>,
    ) -> Result<Self, Error> {
        let mut digest = Sha256::new();
        digest.update(b"NKPNAT01");
        export(&mut digest).map_err(|error| match error {
            ExportError::Map(error) => Error::MapCheckpoint(error),
            ExportError::Sink(never) => match never {},
        })?;
        Ok(Self {
            count,
            fingerprint: digest.finalize().into(),
        })
    }

    /// Observe owned host maps without changing their bucket order.
    pub fn from_maps(maps: &[Map; 2]) -> Result<[Self; 2], Error> {
        let observe = |map: &Map| {
            Self::capture(map.len().map_err(Error::MapCheckpoint)?, |digest| {
                map.export(0, |bytes| {
                    digest.update(bytes);
                    Ok(())
                })
            })
        };
        Ok([observe(&maps[0])?, observe(&maps[1])?])
    }

    /// Observe an admitted scalar scan without reconstructing a Rust map.
    /// The caller must prove native ownership, unique keys and an immutable scan
    /// in native bucket order. Wrong counts refuse; this is not native admission
    /// or authentication of a requester-supplied record list.
    pub fn from_records(
        count: usize,
        records: impl IntoIterator<Item = (u64, u64)>,
    ) -> Result<Self, Error> {
        Self::capture(count, |digest| {
            export_records(count, 0, records, |bytes| {
                digest.update(bytes);
                Ok(())
            })
        })
    }

    /// Number of records observed, independent of allocated capacity.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.count
    }

    /// Whether the admitted native root had no records.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// Exact NKPNAT01 fingerprint stored by the continuation codec.
    #[must_use]
    pub const fn fingerprint(&self) -> [u8; 32] {
        self.fingerprint
    }
}

fn readers(bytes: &[u8], job: &PagedMapJob) -> Result<([Reader; 2], [usize; 2]), Error> {
    if !matches!(job.phase, Phase::Importing | Phase::Consumed) {
        return Err(Error::State);
    }
    let progress = job.progress.ok_or(Error::State)?;
    let manifest = <super::PagedMapPair as super::Payload>::decode(&job.checkpoint, job.epoch)?;
    let mut remaining = progress.next as usize;
    let mut result = [manifest.roots()[0].reader(), manifest.roots()[1].reader()];
    let mut counts = [0; 2];
    for (root, reader) in result.iter_mut().enumerate() {
        let start = CURSORS + root * CONTINUATION_BYTES;
        *reader = Reader::resume(
            &manifest.roots()[root],
            &bytes[start..start + CONTINUATION_BYTES],
        )
        .map_err(Error::MapCheckpoint)?;
        let count = remaining.min(manifest.roots()[root].page_count());
        remaining -= count;
        if reader
            .next_page()
            .unwrap_or(manifest.roots()[root].page_count())
            != count
        {
            return Err(Error::State);
        }
        counts[root] = count
            .checked_mul(PAGE_RECORDS)
            .ok_or(Error::Framing)?
            .min(manifest.roots()[root].len());
    }
    if remaining != 0 {
        return Err(Error::State);
    }
    Ok((result, counts))
}

/// Capture cursors only after the authorized batch's native writes have succeeded.
/// The adapter must atomically publish this record, exact job metadata and native
/// changes under exclusive custody. This function checks binding/counts, not the
/// semantic correctness of caller-applied writes. No IC transaction is supplied.
pub fn encode(
    job: &PagedMapJob,
    cursors: &[Reader; 2],
    maps: &[Map; 2],
) -> Result<[u8; BYTES], Error> {
    encode_observed(job, cursors, &NativePrefix::from_maps(maps)?)
}

/// Capture admitted observations after native writes and their semantic checks.
/// Observations must belong to this job's exclusively owned target roots; only
/// counts and cursor binding can be checked here. Stored bytes are unchanged.
pub fn encode_observed(
    job: &PagedMapJob,
    cursors: &[Reader; 2],
    prefixes: &[NativePrefix; 2],
) -> Result<[u8; BYTES], Error> {
    let mut bytes = [0; BYTES];
    bytes[..8].copy_from_slice(b"NKPCNT01");
    bytes[8..16].copy_from_slice(&1u64.to_le_bytes());
    bytes[16..120].copy_from_slice(&job.encode_split()?.0);
    for (root, cursor) in cursors.iter().enumerate() {
        let start = CURSORS + root * CONTINUATION_BYTES;
        bytes[start..start + CONTINUATION_BYTES].copy_from_slice(&cursor.checkpoint());
        let start = PREFIXES + root * 32;
        bytes[start..start + 32].copy_from_slice(&prefixes[root].fingerprint);
    }
    if readers(&bytes, job)?.1 != prefixes.each_ref().map(|prefix| prefix.count) {
        return Err(Error::State);
    }
    bytes[SEAL..SEAL + 8].copy_from_slice(b"CONTDONE");
    let digest = Sha256::digest(&bytes[..SEAL + 8]);
    bytes[SEAL + 8..].copy_from_slice(&digest);
    Ok(bytes)
}

/// Reopen trusted storage without rereading consumed checkpoint pages. Refuses
/// corrupt/mixed/stale records and changed private native prefixes. Native scans
/// are O(prefix capacity), with constant hash scratch; this is not bounded CPU.
/// A completed exact retry may preserve later native writes, so consumed jobs
/// validate cursor binding but deliberately do not compare their old fingerprints.
pub fn decode(bytes: &[u8], job: &PagedMapJob, maps: &[Map; 2]) -> Result<[Reader; 2], Error> {
    decode_observed(bytes, job, || NativePrefix::from_maps(maps))
}

/// Validate binding before observing native records. Calls `observe` once for an
/// importing job and never for a completed retry or invalid framing/binding.
/// The callback must freshly admit and scan the exclusively owned target roots.
pub fn decode_observed(
    bytes: &[u8],
    job: &PagedMapJob,
    observe: impl FnOnce() -> Result<[NativePrefix; 2], Error>,
) -> Result<[Reader; 2], Error> {
    if bytes.len() != BYTES
        || &bytes[..8] != b"NKPCNT01"
        || bytes[8..16] != 1u64.to_le_bytes()
        || &bytes[SEAL..SEAL + 8] != b"CONTDONE"
    {
        return Err(Error::Framing);
    }
    if Sha256::digest(&bytes[..SEAL + 8]).as_slice() != &bytes[SEAL + 8..] {
        return Err(Error::Integrity);
    }
    if bytes[16..120] != job.encode_split()?.0 {
        return Err(Error::State);
    }
    let (cursors, counts) = readers(bytes, job)?;
    if job.phase == Phase::Importing {
        let prefixes = observe()?;
        if counts != prefixes.each_ref().map(|prefix| prefix.count) {
            return Err(Error::State);
        }
        for (root, prefix) in prefixes.iter().enumerate() {
            let start = PREFIXES + root * 32;
            if bytes[start..start + 32] != prefix.fingerprint {
                return Err(Error::Integrity);
            }
        }
    }
    Ok(cursors)
}

/// Validate the continuation being retired by a new export. Completed jobs
/// retain their cursor binding but may have newer authoritative native values.
/// Jobs without progress require the canonical empty continuation. An unfinished
/// import cannot be retired; the job's `seal` method still owns epoch admission.
pub fn validate_retirement(bytes: &[u8], job: &PagedMapJob) -> Result<(), Error> {
    match job.phase {
        Phase::Importing => Err(Error::State),
        Phase::Consumed => decode_observed(bytes, job, || Err(Error::State)).map(|_| ()),
        Phase::Ready | Phase::Sealed | Phase::Cancelled => {
            if bytes == [0; BYTES] {
                Ok(())
            } else {
                Err(Error::State)
            }
        }
    }
}

#[cfg(test)]
mod observation_tests;
#[cfg(test)]
mod tests;

//! Sequential portable pages committed by a manifest and a forward hash chain.
//! This codec owns bytes and admission, not durable cursors or publication.

use alloc::vec::Vec;
use core::alloc::Allocator;
use sha2::{Digest, Sha256};

use super::{Error, ExportError, Map};

mod reader;
pub use reader::Reader;

/// Maximum logical records returned by one successful page read.
pub const PAGE_RECORDS: usize = 64;
/// Fixed page bytes, including zero padding after a short final page.
pub const PAGE_BYTES: usize = 80 + PAGE_RECORDS * 16;
/// Fixed manifest size, including its completeness marker and digest.
pub const MANIFEST_BYTES: usize = 128;

const MAGIC: &[u8; 8] = b"NKPAGED1";
const PAGE_MAGIC: &[u8; 8] = b"NKPAGE01";
const SEAL: &[u8; 8] = b"PAGEDONE";

/// Admitted manifest for one logical map and caller-selected epoch/root identity.
/// The digest detects corruption; trusted custody must select this manifest.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Manifest {
    epoch: u64,
    root: u64,
    count: usize,
    first: [u8; 32],
}

impl Manifest {
    /// Validate exact framing, count bounds, identity and manifest integrity.
    /// Page contents remain unverified until the reader reaches them.
    pub fn decode(bytes: &[u8], epoch: u64, root: u64, limit: usize) -> Result<Self, Error> {
        if bytes.len() != MANIFEST_BYTES
            || &bytes[..8] != MAGIC
            || word(&bytes[8..16]) != 1
            || word(&bytes[48..56]) != PAGE_RECORDS as u64
            || &bytes[88..96] != SEAL
            || root == 0
            || word(&bytes[24..32]) != root
        {
            return Err(Error::Checkpoint);
        }
        if word(&bytes[16..24]) != epoch {
            return Err(Error::Epoch);
        }
        let count = usize::try_from(word(&bytes[32..40])).map_err(|_| Error::Limit)?;
        if count > limit {
            return Err(Error::Limit);
        }
        if word(&bytes[40..48]) != count.div_ceil(PAGE_RECORDS) as u64
            || (count == 0 && bytes[56..88] != [0; 32])
        {
            return Err(Error::Checkpoint);
        }
        if Sha256::digest(&bytes[..96]).as_slice() != &bytes[96..] {
            return Err(Error::Integrity);
        }
        Ok(Self {
            epoch,
            root,
            count,
            first: core::array::from_fn(|i| bytes[56 + i]),
        })
    }

    /// Byte-defined manifest; publish only after all referenced pages are stored.
    #[must_use]
    pub fn encode(&self) -> [u8; MANIFEST_BYTES] {
        let mut bytes = [0; MANIFEST_BYTES];
        bytes[..8].copy_from_slice(MAGIC);
        for (index, word) in [
            1,
            self.epoch,
            self.root,
            self.count as u64,
            self.page_count() as u64,
            PAGE_RECORDS as u64,
        ]
        .into_iter()
        .enumerate()
        {
            bytes[8 + index * 8..16 + index * 8].copy_from_slice(&word.to_le_bytes());
        }
        bytes[56..88].copy_from_slice(&self.first);
        bytes[88..96].copy_from_slice(SEAL);
        let digest = Sha256::digest(&bytes[..96]);
        bytes[96..].copy_from_slice(&digest);
        bytes
    }

    /// Complete logical record count bound by the manifest.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.count
    }

    /// Whether this is a complete empty map (with no pages).
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// Exact number of pages required before publishing the recovered map.
    #[must_use]
    pub const fn page_count(&self) -> usize {
        self.count.div_ceil(PAGE_RECORDS)
    }

    /// Start a transient sequential verifier at the committed first page.
    /// This does not reopen a durable import cursor or authorize native writes.
    #[must_use]
    pub fn reader(&self) -> Reader {
        Reader::new(self.clone())
    }
}

impl<A: Allocator + Clone> Map<A> {
    /// Export sorted logical pages, writing **last page first**, then return the
    /// manifest that commits to their complete chain. Root identity must be nonzero.
    ///
    /// The sink receives each logical page index once in descending order. It
    /// owns page placement and must publish the manifest only after success.
    /// Errors/unwinding can leave incomplete sink data but never mutate the map.
    /// This synchronous exporter allocates one pair per source record to sort;
    /// page encoding itself uses one fixed buffer. It is not chunked native export.
    pub fn export_pages<E>(
        &self,
        epoch: u64,
        root: u64,
        mut sink: impl FnMut(usize, &[u8; PAGE_BYTES]) -> Result<(), E>,
    ) -> Result<Manifest, ExportError<E>> {
        self.healthy().map_err(ExportError::Map)?;
        if root == 0 {
            return Err(ExportError::Map(Error::Checkpoint));
        }
        let mut records = Vec::new();
        records
            .try_reserve_exact(self.len)
            .map_err(|_| ExportError::Map(Error::Allocation))?;
        for slot in &self.slots {
            if slot.occupied == 1 {
                records.push((slot.key, slot.value));
            }
        }
        records.sort_unstable_by_key(|record| record.0);
        let mut next = [0; 32];
        for index in (0..records.len().div_ceil(PAGE_RECORDS)).rev() {
            let start = index * PAGE_RECORDS;
            let values = &records[start..records.len().min(start + PAGE_RECORDS)];
            let mut page = [0; PAGE_BYTES];
            page[..8].copy_from_slice(PAGE_MAGIC);
            for (field, word) in [
                epoch,
                root,
                index as u64,
                values.len() as u64,
                self.len as u64,
            ]
            .into_iter()
            .enumerate()
            {
                page[8 + field * 8..16 + field * 8].copy_from_slice(&word.to_le_bytes());
            }
            page[48..80].copy_from_slice(&next);
            for (offset, &(key, value)) in values.iter().enumerate() {
                let offset = 80 + offset * 16;
                page[offset..offset + 8].copy_from_slice(&key.to_le_bytes());
                page[offset + 8..offset + 16].copy_from_slice(&value.to_le_bytes());
            }
            next = page_digest(&page);
            sink(index, &page).map_err(ExportError::Sink)?;
        }
        Ok(Manifest {
            epoch,
            root,
            count: self.len,
            first: next,
        })
    }
}

fn page_digest(bytes: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(b"NKPAGEH1");
    digest.update(bytes);
    digest.finalize().into()
}

fn word(bytes: &[u8]) -> u64 {
    u64::from_le_bytes(core::array::from_fn(|i| bytes[i]))
}

#[cfg(test)]
mod tests;

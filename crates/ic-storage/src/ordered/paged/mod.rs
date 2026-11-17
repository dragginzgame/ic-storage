//! Bounded variable-length records committed by a forward page hash chain.
//! This codec owns integrity and ordering, not job authority or native publication.

use super::{Error, Map, Range};
use alloc::vec::Vec;
use core::{alloc::Allocator, ops::Bound};
use sha2::{Digest, Sha256};

mod reader;
pub use reader::{Page, Reader};
#[cfg(test)]
mod tests;

/// Fixed page size, including framing and canonical zero padding.
pub const PAGE_BYTES: usize = 32 * 1024;
/// Maximum records in a page, bounding import batch preflight work.
pub const PAGE_RECORDS: usize = 64;
/// Byte-defined manifest size, independent of checkpoint population.
pub const MANIFEST_BYTES: usize = 128;
const HEADER: usize = 88;
const PAGE_MAGIC: &[u8; 8] = b"ORDPG001";

/// Admitted checkpoint identity. Pages remain unverified until consumed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Manifest {
    epoch: u64,
    root: u64,
    count: usize,
    pages: usize,
    first: [u8; 32],
}

impl Manifest {
    /// Admit exact framing, expected epoch/root, count bounds and integrity.
    /// Checksums detect corruption; callers must establish trusted custody.
    ///
    /// # Errors
    /// Refuses malformed, incompatible, corrupt or excessive manifests.
    pub fn decode(bytes: &[u8], epoch: u64, root: u64, limit: usize) -> Result<Self, Error> {
        if bytes.len() != MANIFEST_BYTES
            || &bytes[..8] != b"ORDPAG01"
            || word(&bytes[8..16]) != 1
            || root == 0
            || word(&bytes[24..32]) != root
            || word(&bytes[48..56]) != PAGE_BYTES as u64
            || &bytes[88..96] != b"ORDPDONE"
            || Sha256::digest(&bytes[..96]).as_slice() != &bytes[96..]
        {
            return Err(Error::Checkpoint);
        }
        if word(&bytes[16..24]) != epoch {
            return Err(Error::Epoch);
        }
        let count = usize::try_from(word(&bytes[32..40])).map_err(|_| Error::Limit)?;
        let pages = usize::try_from(word(&bytes[40..48])).map_err(|_| Error::Limit)?;
        if count > limit {
            return Err(Error::Limit);
        }
        if pages > count
            || pages < count.div_ceil(PAGE_RECORDS)
            || (count == 0 && bytes[56..88] != [0; 32])
        {
            return Err(Error::Checkpoint);
        }
        Ok(Self {
            epoch,
            root,
            count,
            pages,
            first: core::array::from_fn(|index| bytes[56 + index]),
        })
    }

    /// Encode the manifest. Publish only after every referenced page is durable.
    #[must_use]
    pub fn encode(&self) -> [u8; MANIFEST_BYTES] {
        let mut bytes = [0; MANIFEST_BYTES];
        bytes[..8].copy_from_slice(b"ORDPAG01");
        for (index, value) in [
            1,
            self.epoch,
            self.root,
            self.count as u64,
            self.pages as u64,
            PAGE_BYTES as u64,
        ]
        .into_iter()
        .enumerate()
        {
            bytes[8 + index * 8..16 + index * 8].copy_from_slice(&value.to_le_bytes());
        }
        bytes[56..88].copy_from_slice(&self.first);
        bytes[88..96].copy_from_slice(b"ORDPDONE");
        let seal = Sha256::digest(&bytes[..96]);
        bytes[96..].copy_from_slice(&seal);
        bytes
    }

    /// Complete logical record count.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.count
    }
    /// Whether this checkpoint contains no records or pages.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.count == 0
    }
    /// Number of pages to admit before publishing recovered data.
    #[must_use]
    pub const fn page_count(&self) -> usize {
        self.pages
    }
    /// Start a transient verifier with explicit target key/value bounds.
    #[must_use]
    pub fn reader<const K: usize, const V: usize>(&self) -> Reader<K, V> {
        Reader::new(self.clone())
    }
}

/// Export refusal. A failed sink may contain incomplete, unusable pages.
#[derive(Debug, PartialEq, Eq)]
pub enum ExportError<E> {
    /// Poisoned owner, excessive record size or scratch allocation refusal.
    Map(Error),
    /// Caller-owned page placement failed; no manifest is returned.
    Sink(E),
}

impl<const K: usize, const V: usize, A: Allocator + Clone, P: Allocator + Clone> Map<K, V, A, P> {
    /// Export logical pages in descending page-index order, then return their manifest.
    /// Traverses the ordered tree twice using seeks and one reusable 32-KiB buffer;
    /// it never collects or sorts the complete table. Each page holds at most 64
    /// whole records. A record must fit in `PAGE_BYTES - 88`, including its two
    /// u32 lengths. The sink owns placement, and must publish the manifest last.
    /// This synchronous export is not an incremental export job.
    ///
    /// # Errors
    /// Refuses poisoned state, root zero, oversized records, allocation or sink
    /// failure. Native records remain unchanged, including if the sink unwinds.
    pub fn export_pages<E>(
        &self,
        epoch: u64,
        root: u64,
        mut sink: impl FnMut(usize, &[u8]) -> Result<(), E>,
    ) -> Result<Manifest, ExportError<E>> {
        self.healthy().map_err(ExportError::Map)?;
        if root == 0 {
            return Err(ExportError::Map(Error::Checkpoint));
        }
        let mut pages = 0;
        let (mut used, mut count) = (0, 0);
        for (key, value) in Range::new(self, Bound::Unbounded, Bound::Unbounded).rev() {
            let size = record_size(key.as_slice(), value.as_slice()).map_err(ExportError::Map)?;
            if count == 0 || count == PAGE_RECORDS || used + size > PAGE_BYTES - HEADER {
                pages += 1;
                (used, count) = (0, 0);
            }
            used += size;
            count += 1;
        }
        let mut manifest = Manifest {
            epoch,
            root,
            count: self.image.len,
            pages,
            first: [0; 32],
        };
        if pages == 0 {
            return Ok(manifest);
        }
        let mut page = Vec::new();
        page.try_reserve_exact(PAGE_BYTES)
            .map_err(|_| ExportError::Map(Error::Allocation))?;
        page.resize(PAGE_BYTES, 0);
        let mut records = Range::new(self, Bound::Unbounded, Bound::Unbounded)
            .rev()
            .peekable();
        for index in (0..pages).rev() {
            page.fill(0);
            let (mut start, mut count) = (PAGE_BYTES, 0);
            while let Some(&(key, value)) = records.peek() {
                let (key, value) = (key.as_slice(), value.as_slice());
                let size = 8 + key.len() + value.len();
                if count == PAGE_RECORDS || size > start - HEADER {
                    break;
                }
                start -= size;
                page[start..start + 4].copy_from_slice(&(key.len() as u32).to_le_bytes());
                page[start + 4..start + 8].copy_from_slice(&(value.len() as u32).to_le_bytes());
                page[start + 8..start + 8 + key.len()].copy_from_slice(key);
                page[start + 8 + key.len()..start + size].copy_from_slice(value);
                records.next();
                count += 1;
            }
            let used = PAGE_BYTES - start;
            page.copy_within(start..PAGE_BYTES, HEADER);
            page[HEADER + used..].fill(0);
            page[..8].copy_from_slice(PAGE_MAGIC);
            for (field, value) in [
                epoch,
                root,
                index as u64,
                count as u64,
                self.image.len as u64,
            ]
            .into_iter()
            .enumerate()
            {
                page[8 + field * 8..16 + field * 8].copy_from_slice(&value.to_le_bytes());
            }
            page[48..80].copy_from_slice(&manifest.first);
            page[80..88].copy_from_slice(&(used as u64).to_le_bytes());
            manifest.first = page_digest(&page);
            sink(index, &page).map_err(ExportError::Sink)?;
        }
        Ok(manifest)
    }
}

fn record_size(key: &[u8], value: &[u8]) -> Result<usize, Error> {
    let size = key
        .len()
        .checked_add(value.len())
        .and_then(|n| n.checked_add(8))
        .ok_or(Error::Limit)?;
    if size > PAGE_BYTES - HEADER {
        return Err(Error::Limit);
    }
    Ok(size)
}
fn page_digest(bytes: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(b"ORDPGH01");
    digest.update(bytes);
    digest.finalize().into()
}
fn word(bytes: &[u8]) -> u64 {
    u64::from_le_bytes(bytes.try_into().unwrap())
}

use super::{
    Error, HEADER, Manifest, PAGE_BYTES, PAGE_MAGIC, PAGE_RECORDS, Vec, page_digest, word,
};
use sha2::{Digest, Sha256};

/// Transient sequential verifier. Its Rust representation must not be persisted.
/// Returned records remain private until the entire checkpoint has been admitted.
pub struct Reader<const K: usize, const V: usize> {
    manifest: Manifest,
    next: usize,
    seen: usize,
    digest: [u8; 32],
    last: Vec<u8>,
}

impl<const K: usize, const V: usize> Reader<K, V> {
    pub(super) fn new(manifest: Manifest) -> Self {
        Self {
            digest: manifest.first,
            manifest,
            next: 0,
            seen: 0,
            last: Vec::new(),
        }
    }
    /// Next required page, or `None` when all records have been verified.
    /// Completion does not prove that a caller applied or published native writes.
    #[must_use]
    pub fn next_page(&self) -> Option<usize> {
        (self.next < self.manifest.pages).then_some(self.next)
    }
    /// Number of records admitted so far, independent of their encoded sizes.
    #[must_use]
    pub const fn records_read(&self) -> usize {
        self.seen
    }

    /// Encode progress with manifest binding, last key and a checksum, using
    /// 168 bytes plus the last key's actual length. Store only under trusted
    /// custody, atomically with the owning job and its reconstructed native prefix.
    ///
    /// # Errors
    /// Scratch allocation refusal leaves this verifier unchanged.
    pub fn checkpoint(&self) -> Result<Vec<u8>, Error> {
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(168 + self.last.len())
            .map_err(|_| Error::Allocation)?;
        bytes.extend_from_slice(b"ORDCUR01");
        bytes.extend_from_slice(&1u64.to_le_bytes());
        bytes.extend_from_slice(&self.manifest.encode()[96..]);
        bytes.extend_from_slice(&(self.next as u64).to_le_bytes());
        bytes.extend_from_slice(&(self.seen as u64).to_le_bytes());
        bytes.extend_from_slice(&self.digest);
        bytes.extend_from_slice(&(self.last.len() as u64).to_le_bytes());
        bytes.extend_from_slice(&u64::from(self.seen != 0).to_le_bytes());
        bytes.extend_from_slice(b"ORDCDONE");
        bytes.extend_from_slice(&(K as u64).to_le_bytes());
        bytes.extend_from_slice(&(V as u64).to_le_bytes());
        bytes.extend_from_slice(&self.last);
        let seal = Sha256::digest(&bytes);
        bytes.extend_from_slice(&seal);
        Ok(bytes)
    }

    /// Resume a caller-custodied continuation without rereading consumed pages.
    /// A checksum is not authentication or proof of previous native writes: an
    /// untrusted party can forge progress. The owner must bind these bytes to its
    /// current job and native prefix. Explicit restart revalidates from page zero.
    ///
    /// # Errors
    /// Refuses invalid framing, manifest binding, geometry, key bound or allocation.
    pub fn resume(manifest: &Manifest, bytes: &[u8]) -> Result<Self, Error> {
        if bytes.len() < 168
            || bytes.len() > 168 + K.min(PAGE_BYTES - HEADER - 8)
            || &bytes[..8] != b"ORDCUR01"
            || word(&bytes[8..16]) != 1
            || bytes[16..48] != manifest.encode()[96..]
            || &bytes[112..120] != b"ORDCDONE"
            || word(&bytes[120..128]) != K as u64
            || word(&bytes[128..136]) != V as u64
            || word(&bytes[96..104]) != (bytes.len() - 168) as u64
            || Sha256::digest(&bytes[..bytes.len() - 32]).as_slice() != &bytes[bytes.len() - 32..]
        {
            return Err(Error::Checkpoint);
        }
        let next = usize::try_from(word(&bytes[48..56])).map_err(|_| Error::Limit)?;
        let seen = usize::try_from(word(&bytes[56..64])).map_err(|_| Error::Limit)?;
        let digest = core::array::from_fn(|index| bytes[64 + index]);
        if next > manifest.pages
            || seen > manifest.count
            || seen < next
            || next < seen.div_ceil(PAGE_RECORDS)
            || word(&bytes[104..112]) != u64::from(seen != 0)
            || (next == 0 && (seen != 0 || bytes.len() != 168 || digest != manifest.first))
            || (seen > 1 && bytes.len() == 168)
            || (next == manifest.pages && (seen != manifest.count || digest != [0; 32]))
            || (next < manifest.pages
                && (seen == manifest.count
                    || manifest.count - seen < manifest.pages - next
                    || (manifest.count - seen).div_ceil(PAGE_RECORDS) > manifest.pages - next))
        {
            return Err(Error::Checkpoint);
        }
        let mut last = Vec::new();
        last.try_reserve_exact(bytes.len() - 168)
            .map_err(|_| Error::Allocation)?;
        last.extend_from_slice(&bytes[136..bytes.len() - 32]);
        Ok(Self {
            manifest: manifest.clone(),
            next,
            seen,
            digest,
            last,
        })
    }

    /// Validate one fixed-size page and borrow its records without copying values.
    /// At most 64 records are returned. All errors preserve verifier progress.
    /// Successful verification advances once; repeats and skipped pages refuse.
    /// Caller-owned native writes and durable progress still need one transaction.
    ///
    /// # Errors
    /// Refuses corrupt/mixed/replayed pages, bounds, ordering, padding, counts,
    /// trailing bytes or allocation of the new last-key continuation.
    pub fn read_page<'a>(&mut self, bytes: &'a [u8]) -> Result<Page<'a>, Error> {
        let index = self.next_page().ok_or(Error::Checkpoint)?;
        if bytes.len() != PAGE_BYTES
            || page_digest(bytes) != self.digest
            || &bytes[..8] != PAGE_MAGIC
            || word(&bytes[8..16]) != self.manifest.epoch
            || word(&bytes[16..24]) != self.manifest.root
            || word(&bytes[24..32]) != index as u64
            || word(&bytes[40..48]) != self.manifest.count as u64
        {
            return Err(Error::Checkpoint);
        }
        let count = usize::try_from(word(&bytes[32..40])).map_err(|_| Error::Limit)?;
        let used = usize::try_from(word(&bytes[80..88])).map_err(|_| Error::Limit)?;
        let remaining = self.manifest.count - self.seen;
        let later_pages = self.manifest.pages - index - 1;
        if count == 0
            || count > PAGE_RECORDS
            || count > remaining
            || used > PAGE_BYTES - HEADER
            || count > used / 8
            || remaining - count < later_pages
            || (remaining - count).div_ceil(PAGE_RECORDS) > later_pages
            || (later_pages == 0 && bytes[48..80] != [0; 32])
            || bytes[HEADER + used..].iter().any(|&byte| byte != 0)
        {
            return Err(Error::Checkpoint);
        }
        let data = &bytes[HEADER..HEADER + used];
        let mut input = data;
        let mut last = (self.seen != 0).then_some(self.last.as_slice());
        let mut final_key = &data[..0];
        for _ in 0..count {
            let (key, value) = record(&mut input)?;
            if key.len() > K || value.len() > V {
                return Err(Error::Limit);
            }
            if last.is_some_and(|prior| prior >= key) {
                return Err(Error::Checkpoint);
            }
            last = Some(key);
            final_key = key;
        }
        if !input.is_empty() {
            return Err(Error::Checkpoint);
        }
        self.last
            .try_reserve(final_key.len().saturating_sub(self.last.len()))
            .map_err(|_| Error::Allocation)?;
        self.last.clear();
        self.last.extend_from_slice(final_key);
        self.digest.copy_from_slice(&bytes[48..80]);
        self.next += 1;
        self.seen += count;
        Ok(Page { data, count })
    }
}

/// Fully validated, borrowed logical records from one page. No native owner escapes.
#[derive(Debug)]
pub struct Page<'a> {
    data: &'a [u8],
    count: usize,
}
impl<'a> Page<'a> {
    /// Logical records in strictly increasing key order, with no value allocation.
    pub fn records(&self) -> impl Iterator<Item = (&'a [u8], &'a [u8])> + '_ {
        Records {
            input: self.data,
            remaining: self.count,
        }
    }
    /// Nonzero number of validated records in this page.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.count
    }
    /// Admitted pages are never empty; empty checkpoints contain no pages.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.count == 0
    }
}

// Constructed only from a completely admitted immutable page. Parsing cannot
// fail unless this module's framing validation and iterator disagree.
struct Records<'a> {
    input: &'a [u8],
    remaining: usize,
}
impl<'a> Iterator for Records<'a> {
    type Item = (&'a [u8], &'a [u8]);
    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            return None;
        }
        let item = record(&mut self.input).expect("page framing was admitted");
        self.remaining -= 1;
        Some(item)
    }
}

fn record<'a>(input: &mut &'a [u8]) -> Result<(&'a [u8], &'a [u8]), Error> {
    let (lengths, rest) = input.split_at_checked(8).ok_or(Error::Checkpoint)?;
    let key = u32::from_le_bytes(lengths[..4].try_into().unwrap()) as usize;
    let value = u32::from_le_bytes(lengths[4..].try_into().unwrap()) as usize;
    let (key, rest) = rest.split_at_checked(key).ok_or(Error::Checkpoint)?;
    let (value, rest) = rest.split_at_checked(value).ok_or(Error::Checkpoint)?;
    *input = rest;
    Ok((key, value))
}

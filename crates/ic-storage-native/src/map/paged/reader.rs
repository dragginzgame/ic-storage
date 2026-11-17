use super::{Error, Manifest, PAGE_BYTES, PAGE_MAGIC, PAGE_RECORDS, Vec, page_digest, word};

/// Transient sequential page admission, with no persisted Rust representation.
/// Pages are checked against the manifest's chain and strictly increasing keys.
/// Callers must keep reconstructed data private until every page succeeds.
pub struct Reader {
    manifest: Manifest,
    next: usize,
    digest: [u8; 32],
    last_key: Option<u64>,
}

impl Reader {
    pub(super) fn new(manifest: Manifest) -> Self {
        Self {
            digest: manifest.first,
            manifest,
            next: 0,
            last_key: None,
        }
    }

    /// Required logical page index, or `None` when the entire map is validated.
    /// Empty manifests are already complete. This is not proof of native writes.
    #[must_use]
    pub fn next_page(&self) -> Option<usize> {
        (self.next < self.manifest.page_count()).then_some(self.next)
    }

    /// Validate exactly one page and return at most 64 logical records.
    /// On any error the cursor is unchanged and the same page can be retried.
    /// Successful calls advance once; replaying an earlier page refuses.
    /// Later corruption is detected when its page is reached, never silently skipped.
    pub fn read_page(&mut self, bytes: &[u8]) -> Result<Vec<(u64, u64)>, Error> {
        let index = self.next_page().ok_or(Error::Checkpoint)?;
        if bytes.len() != PAGE_BYTES {
            return Err(Error::Checkpoint);
        }
        if page_digest(bytes) != self.digest {
            return Err(Error::Integrity);
        }
        let count = (self.manifest.count - index * PAGE_RECORDS).min(PAGE_RECORDS);
        if &bytes[..8] != PAGE_MAGIC
            || word(&bytes[8..16]) != self.manifest.epoch
            || word(&bytes[16..24]) != self.manifest.root
            || word(&bytes[24..32]) != index as u64
            || word(&bytes[32..40]) != count as u64
            || word(&bytes[40..48]) != self.manifest.count as u64
            || bytes[80 + count * 16..].iter().any(|&byte| byte != 0)
            || (index + 1 == self.manifest.page_count() && bytes[48..80] != [0; 32])
        {
            return Err(Error::Checkpoint);
        }
        let mut records = Vec::new();
        records
            .try_reserve_exact(count)
            .map_err(|_| Error::Allocation)?;
        let mut last_key = self.last_key;
        for record in bytes[80..80 + count * 16].as_chunks::<16>().0 {
            let key = word(&record[..8]);
            if last_key.is_some_and(|last| last >= key) {
                return Err(Error::Checkpoint);
            }
            records.push((key, word(&record[8..])));
            last_key = Some(key);
        }
        // Publish verifier progress only after every check and allocation succeeds.
        self.digest = core::array::from_fn(|i| bytes[48 + i]);
        self.next += 1;
        self.last_key = last_key;
        Ok(records)
    }
}

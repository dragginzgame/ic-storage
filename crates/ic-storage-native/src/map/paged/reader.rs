use super::{Error, Manifest, PAGE_BYTES, PAGE_MAGIC, PAGE_RECORDS, Vec, page_digest, word};
use sha2::{Digest, Sha256};

/// Byte-defined verifier continuation, including manifest binding and checksum.
pub const CONTINUATION_BYTES: usize = 144;

/// Sequential page admission, with no persisted Rust representation.
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

    /// Encode verified progress without retaining Rust or hash implementation state.
    /// Store these bytes only under trusted custody, atomically with the owning
    /// job's progress and native writes. The caller must bind that transaction;
    /// this codec cannot establish that any returned records were applied.
    #[must_use]
    pub fn checkpoint(&self) -> [u8; CONTINUATION_BYTES] {
        let mut bytes = [0; CONTINUATION_BYTES];
        bytes[..8].copy_from_slice(b"NKPCUR01");
        bytes[8..16].copy_from_slice(&1u64.to_le_bytes());
        bytes[16..48].copy_from_slice(&self.manifest.encode()[96..]);
        bytes[48..56].copy_from_slice(&(self.next as u64).to_le_bytes());
        bytes[56..88].copy_from_slice(&self.digest);
        bytes[88..96].copy_from_slice(&u64::from(self.last_key.is_some()).to_le_bytes());
        bytes[96..104].copy_from_slice(&self.last_key.unwrap_or(0).to_le_bytes());
        bytes[104..112].copy_from_slice(b"CURSDONE");
        let digest = Sha256::digest(&bytes[..112]);
        bytes[112..].copy_from_slice(&digest);
        bytes
    }

    /// Resume bytes previously checkpointed under the caller's trusted custody.
    /// Checks framing, checksum, exact manifest identity and cursor geometry.
    ///
    /// This is **not a proof of the preceding chain** and must not admit a cursor
    /// supplied by an untrusted requester. An actor who can recompute the checksum
    /// can invent intermediate progress. The owner must bind the continuation to
    /// its current job and validate native custody/prefix separately before writes.
    /// Previously consumed pages are not reread; later changes to those bytes are
    /// detected only by a fresh read from the manifest (such as explicit restart).
    pub fn resume(manifest: &Manifest, bytes: &[u8]) -> Result<Self, Error> {
        if bytes.len() != CONTINUATION_BYTES
            || &bytes[..8] != b"NKPCUR01"
            || word(&bytes[8..16]) != 1
            || &bytes[104..112] != b"CURSDONE"
        {
            return Err(Error::Checkpoint);
        }
        if Sha256::digest(&bytes[..112]).as_slice() != &bytes[112..]
            || bytes[16..48] != manifest.encode()[96..]
        {
            return Err(Error::Integrity);
        }
        let next = usize::try_from(word(&bytes[48..56])).map_err(|_| Error::Limit)?;
        let flag = word(&bytes[88..96]);
        if next > manifest.page_count()
            || (next == 0
                && (flag != 0 || bytes[96..104] != [0; 8] || bytes[56..88] != manifest.first))
            || (next != 0 && flag != 1)
            || (next == manifest.page_count() && bytes[56..88] != [0; 32])
        {
            return Err(Error::Checkpoint);
        }
        Ok(Self {
            manifest: manifest.clone(),
            next,
            digest: core::array::from_fn(|i| bytes[56 + i]),
            last_key: (flag == 1).then(|| word(&bytes[96..104])),
        })
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

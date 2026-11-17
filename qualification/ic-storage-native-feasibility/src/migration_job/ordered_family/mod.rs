//! Four-root page projection of the shared migration transitions. This admits
//! declarations/manifests; native custody and IC publication belong to the adapter.
use super::{Error, OrderedFamilyJob, Payload, Phase};
use alloc::vec::Vec;
use ic_storage::ordered::{
    paged::{MANIFEST_BYTES, Manifest, Reader},
    session::Root,
};

/// Closed qualification roster; shared by the installed native and page owners.
pub const ROOTS: [Root; 4] = [
    Root {
        id: 1,
        participant: 1,
        schema: 1,
        entries: 128,
        key_bytes: 512,
        value_bytes: 4096,
    },
    Root {
        id: 2,
        participant: 2,
        schema: 1,
        entries: 32,
        key_bytes: 512,
        value_bytes: 4096,
    },
    Root {
        id: 3,
        participant: 3,
        schema: 1,
        entries: 32,
        key_bytes: 512,
        value_bytes: 4096,
    },
    Root {
        id: 4,
        participant: 4,
        schema: 1,
        entries: 17,
        key_bytes: 1,
        value_bytes: 16384,
    },
];
const PART: usize = 48 + MANIFEST_BYTES;
/// Fixed family payload size; contains no page data or native pointers.
pub const BYTES: usize = 4 * PART;

/// Admitted four-root manifest under the closed qualification declarations.
#[derive(Debug, PartialEq, Eq)]
pub struct Family {
    roots: [Manifest; 4],
}
impl super::payload::sealed::Sealed for Family {}
impl Payload for Family {
    const MAGIC: &'static [u8; 8] = b"ICJOF001";
    const MAX_BYTES: usize = BYTES;
    const DOMAINS: [u64; 2] = [4 * 1024 * 1024, 6 * 1024 * 1024];
    type Decoded = Self;
    fn decode(bytes: &[u8], epoch: u64) -> Result<Self, Error> {
        if bytes.len() != BYTES {
            return Err(Error::Framing);
        }
        let mut roots = Vec::new();
        roots.try_reserve_exact(4).map_err(|_| Error::Allocation)?;
        for (index, root) in ROOTS.iter().enumerate() {
            let part = &bytes[index * PART..(index + 1) * PART];
            if part[..48] != declaration(*root) {
                return Err(Error::Framing);
            }
            roots.push(
                Manifest::decode(&part[48..], epoch, root.id, root.entries)
                    .map_err(Error::OrderedCheckpoint)?,
            );
        }
        Ok(Self {
            roots: roots.try_into().map_err(|_| Error::Framing)?,
        })
    }
    fn len(decoded: &Self) -> usize {
        decoded.page_count()
    }
}
fn declaration(root: Root) -> [u8; 48] {
    let mut bytes = [0; 48];
    for (field, word) in [
        root.id,
        root.participant,
        root.schema,
        root.entries as u64,
        root.key_bytes as u64,
        root.value_bytes as u64,
    ]
    .into_iter()
    .enumerate()
    {
        bytes[field * 8..field * 8 + 8].copy_from_slice(&word.to_le_bytes());
    }
    bytes
}
impl Family {
    /// Validate a complete set of page manifests against the closed declarations.
    pub fn new(roots: [Manifest; 4], epoch: u64) -> Result<Self, Error> {
        Self::decode(&Self { roots }.encode(), epoch)
    }
    /// Encode declarations and manifests; the owning job seals the entire payload.
    #[must_use]
    pub fn encode(&self) -> [u8; BYTES] {
        let mut bytes = [0; BYTES];
        for (index, root) in self.roots.iter().enumerate() {
            let part = &mut bytes[index * PART..(index + 1) * PART];
            part[..48].copy_from_slice(&declaration(ROOTS[index]));
            part[48..].copy_from_slice(&root.encode());
        }
        bytes
    }
    /// Total import pages, skipping empty roots.
    #[must_use]
    pub fn page_count(&self) -> usize {
        self.roots.iter().map(Manifest::page_count).sum()
    }
    /// Locate a global page index without reading any page or native record.
    #[must_use]
    pub fn locate(&self, mut page: usize) -> Option<(usize, usize)> {
        for (root, manifest) in self.roots.iter().enumerate() {
            if page < manifest.page_count() {
                return Some((root, page));
            }
            page = page.checked_sub(manifest.page_count())?;
        }
        None
    }
    fn position(&self, next: usize) -> Result<(usize, usize), Error> {
        if let Some(position) = self.locate(next) {
            return Ok(position);
        }
        if next != self.page_count() {
            return Err(Error::State);
        }
        let root = self
            .roots
            .iter()
            .rposition(|root| !root.is_empty())
            .unwrap_or(0);
        Ok((root, self.roots[root].page_count()))
    }
    /// First verifier, or the first empty root for an entirely empty family.
    pub fn initial_cursor(&self) -> Result<Vec<u8>, Error> {
        self.roots[self.position(0)?.0]
            .reader::<512, 16384>()
            .checkpoint()
            .map_err(Error::OrderedCheckpoint)
    }
    /// Select the following root's fresh verifier at a page-chain boundary.
    /// The adapter still must validate this cursor against the next job state.
    pub fn next_cursor(&self, next: usize, reader: &Reader<512, 16384>) -> Result<Vec<u8>, Error> {
        let (root, local) = self.position(next)?;
        if next < self.page_count() && local == 0 {
            self.roots[root].reader::<512, 16384>().checkpoint()
        } else {
            reader.checkpoint()
        }
        .map_err(Error::OrderedCheckpoint)
    }
}
impl OrderedFamilyJob {
    /// Seal already stored pages under the next shared job epoch.
    pub fn seal(
        &self,
        source: u64,
        target: u64,
        epoch: u64,
        family: &Family,
    ) -> Result<Self, Error> {
        self.export_payload(source, target, epoch, || {
            let bytes = family.encode();
            Family::decode(&bytes, epoch)?;
            Ok(bytes.to_vec())
        })
    }
    /// Bind a trusted persisted verifier to exactly this job's committed offset.
    /// This does not authenticate its custody or revalidate the native candidate.
    pub fn reader(&self, cursor: &[u8]) -> Result<Reader<512, 16384>, Error> {
        if !matches!(self.phase, Phase::Importing | Phase::Consumed) {
            return Err(Error::State);
        }
        let family = Family::decode(&self.checkpoint, self.epoch)?;
        let (root, page) = family.position(self.progress.ok_or(Error::State)?.next as usize)?;
        let reader =
            Reader::resume(&family.roots[root], cursor).map_err(Error::OrderedCheckpoint)?;
        if reader
            .next_page()
            .unwrap_or(family.roots[root].page_count())
            != page
        {
            return Err(Error::State);
        }
        Ok(reader)
    }
}
#[cfg(test)]
mod tests;

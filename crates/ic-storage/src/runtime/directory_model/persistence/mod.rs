//! Reusable snapshot arenas with a fixed protected root.
//! The owner is poisoned before fallible backing IO and must be dropped after
//! a panic. Native partial root writes are detected, not repaired or rolled back.

#[cfg(test)]
mod tests;

use super::{AllocationId, Directory, Error, Image, Prepared, StorageConfig, hash};
use crate::{Memory, WASM_PAGE_SIZE_BYTES as PAGE};
use std::cell::Cell;

const MAGIC: &[u8; 8] = b"ICROOTS2";
const ROOT_BYTES: usize = 64;

///
/// Root
/// The fixed record locating the active directory snapshot and recording its checksum.
///

struct Root {
    generation: u64,
    start: u64,
    length: u64,
    end: u64,
    digest: u64,
}

impl Root {
    fn encode(&self) -> [u8; ROOT_BYTES] {
        let mut bytes = [0; ROOT_BYTES];
        bytes[..8].copy_from_slice(MAGIC);
        for (index, value) in [
            self.generation,
            self.start,
            self.length,
            self.end,
            self.digest,
        ]
        .into_iter()
        .enumerate()
        {
            bytes[8 + index * 8..16 + index * 8].copy_from_slice(&value.to_le_bytes());
        }
        let checksum = digest(&bytes[..48]);
        bytes[48..56].copy_from_slice(&checksum.to_le_bytes());

        bytes
    }

    fn decode(bytes: &[u8; ROOT_BYTES]) -> Result<Self, Error> {
        if &bytes[..8] != MAGIC || bytes[56..].iter().any(|&b| b != 0) {
            return Err(Error::Invalid("root marker or reserved bytes"));
        }
        let word = |offset| u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap());
        if digest(&bytes[..48]) != word(48) {
            return Err(Error::Invalid("root checksum"));
        }

        Ok(Self {
            generation: word(8),
            start: word(16),
            length: word(24),
            end: word(32),
            digest: word(40),
        })
    }
}

fn digest(bytes: &[u8]) -> u64 {
    hash::fnv64(hash::FNV_OFFSET, bytes)
}

pub struct Store<M: Memory> {
    memory: M,
    pub(super) directory: Directory,
    root: [u8; ROOT_BYTES],
    poisoned: Cell<bool>,
}

///
/// Commit
/// The immutable candidate includes the reservation for its own metadata bytes.
///

struct Commit {
    directory: Directory,
    payload: Vec<u8>,
    root: Root,
}

impl Commit {
    fn prepare(image: &Image, budget: StorageConfig, active: Option<u64>) -> Result<Self, Error> {
        // Never overwrite the active snapshot. Reuse only arenas explicitly
        // reserved by the committed directory; abandoned tails remain excluded.
        let mut pages = 1_u64;
        for _ in 0..64 {
            let mut candidate = image.clone();
            let reusable = candidate
                .metadata
                .iter()
                .filter(|arena| Some(arena.start) != active && arena.pages >= pages)
                .min_by_key(|arena| arena.pages)
                .cloned();
            let arena = if let Some(arena) = reusable {
                arena
            } else {
                let arena = super::Extent {
                    start: candidate.next_page,
                    pages,
                };
                candidate.next_page = arena.start.checked_add(pages).ok_or(Error::Budget)?;
                candidate.metadata.push(arena.clone());

                arena
            };
            let directory = Directory::validate(candidate, budget, 1)?;
            let payload = directory.encode()?;
            let length = u64::try_from(payload.len()).map_err(|_| Error::Budget)?;
            let required = length.div_ceil(PAGE);
            if required <= arena.pages {
                let root = Root {
                    generation: directory.image.generation,
                    start: arena.start,
                    length,
                    end: directory.image.next_page,
                    digest: digest(&payload),
                };
                root.end.checked_mul(PAGE).ok_or(Error::Budget)?;

                return Ok(Self {
                    directory,
                    payload,
                    root,
                });
            }
            pages = required.checked_next_power_of_two().ok_or(Error::Budget)?;
        }

        Err(Error::Invalid("metadata reservation did not converge"))
    }
}

impl<M: Memory> Store<M> {
    pub(super) fn create(memory: M, budget: StorageConfig) -> Result<Self, Error> {
        if memory.size() != 0 {
            return Err(Error::Invalid("create requires empty backing"));
        }
        let directory = Directory::empty(budget, 1)?;
        let commit = Commit::prepare(&directory.image, budget, None)?;
        let mut owner = Self {
            memory,
            directory,
            root: [0; ROOT_BYTES],
            poisoned: Cell::new(false),
        };
        owner.install(commit)?;

        Ok(owner)
    }

    pub(super) fn open(memory: M, budget: StorageConfig) -> Result<Self, Error> {
        let (root, directory) = read(&memory, budget)?;

        Ok(Self {
            memory,
            directory,
            root,
            poisoned: Cell::new(false),
        })
    }

    pub(super) fn prepare(
        &self,
        keys: Vec<super::StableKey>,
        growth: &[(super::AllocationId, u64)],
    ) -> Result<Prepared, Error> {
        self.healthy()?;
        // A native interrupted write may leave an uncommitted physical tail.
        // Never reuse those pages as new (supposedly zeroed) application data.
        let mut base = self.directory.image.clone();
        base.next_page = base.next_page.max(self.backing_pages()?);
        let planning = Directory::validate(base, self.directory.budget, 1)?;
        let mut prepared = planning.prepare(keys, growth)?;
        prepared.base = self.directory.image.clone();

        Ok(prepared)
    }

    pub(super) fn commit(&mut self, prepared: Prepared) -> Result<(), Error> {
        self.healthy()?;
        if prepared.base != self.directory.image || prepared.candidate.data_start != 1 {
            return Err(Error::Stale);
        }
        self.verify()?;
        let candidate = prepared.candidate;
        let physical_end = self.backing_pages()?;
        if candidate.image.next_page < physical_end {
            return Err(Error::Stale);
        }
        for entry in &candidate.image.entries {
            let retained = self.directory.by_id.get(&entry.id).map_or(0, |&index| {
                self.directory.image.entries[index].extents.len()
            });

            if entry
                .extents
                .iter()
                .skip(retained)
                .any(|extent| extent.start < physical_end)
            {
                return Err(Error::Invalid("new data would reuse an uncommitted tail"));
            }
        }
        if candidate.image.generation
            != self
                .directory
                .image
                .generation
                .checked_add(1)
                .ok_or(Error::Budget)?
        {
            return Err(Error::Stale);
        }
        // The candidate is private owned state, not an external byte boundary.
        // Final preparation validates and encodes under this owner's budgets,
        // including its metadata reservation, before any backing mutation.
        let commit = Commit::prepare(
            &candidate.image,
            self.directory.budget,
            Some(Root::decode(&self.root)?.start),
        )?;

        self.install(commit)
    }

    pub(super) const fn healthy(&self) -> Result<(), Error> {
        if self.poisoned.get() {
            Err(Error::Poisoned)
        } else {
            Ok(())
        }
    }

    pub(super) fn verify(&self) -> Result<(), Error> {
        self.healthy()?;
        // Metadata checks occur at ownership boundaries, never on each data IO.
        self.poisoned.set(true);
        let (root, observed) = read(&self.memory, self.directory.budget)?;
        if root != self.root || observed.image != self.directory.image {
            return Err(Error::Invalid("directory changed outside its owner"));
        }
        self.poisoned.set(false);

        Ok(())
    }

    fn backing_pages(&self) -> Result<u64, Error> {
        self.healthy()?;
        self.poisoned.set(true);
        let pages = self.memory.size();
        self.poisoned.set(false);

        Ok(pages)
    }

    pub(super) fn pages(&self, id: AllocationId) -> Result<u64, Error> {
        self.healthy()?;
        let index = *self.directory.by_id.get(&id).ok_or(Error::Missing)?;

        // Disjoint extents fit within the admitted physical page budget.
        Ok(self.directory.logical_ends[index]
            .last()
            .copied()
            .unwrap_or(0))
    }

    fn access(
        &self,
        id: AllocationId,
        offset: u64,
        length: usize,
        mut io: impl FnMut(u64, std::ops::Range<usize>),
    ) -> Result<(), Error> {
        let capacity = self.pages(id)?.checked_mul(PAGE).ok_or(Error::Bounds)?;
        let length64 = u64::try_from(length).map_err(|_| Error::Bounds)?;
        let end = offset.checked_add(length64).ok_or(Error::Bounds)?;
        if end > capacity {
            return Err(Error::Bounds);
        }
        // Validate the whole logical range before any backing IO. Unwinding
        // during a partial native read/write poisons every shared lease.
        self.poisoned.set(true);
        let mut done = 0;
        while done < length {
            let logical = offset + u64::try_from(done).unwrap();
            let within = logical % PAGE;
            let (physical, remaining_pages) = self
                .directory
                .page_span(id, logical / PAGE)
                .ok_or(Error::Bounds)?;
            let available = remaining_pages * PAGE - within;
            let count = (length - done).min(usize::try_from(available).unwrap_or(usize::MAX));
            io(physical * PAGE + within, done..done + count);
            done += count;
        }
        self.poisoned.set(false);

        Ok(())
    }

    pub(super) fn read_data(
        &self,
        id: AllocationId,
        offset: u64,
        bytes: &mut [u8],
    ) -> Result<(), Error> {
        self.access(id, offset, bytes.len(), |physical, range| {
            self.memory.read(physical, &mut bytes[range]);
        })
    }

    pub(super) fn write_data(
        &self,
        id: AllocationId,
        offset: u64,
        bytes: &[u8],
    ) -> Result<(), Error> {
        self.access(id, offset, bytes.len(), |physical, range| {
            self.memory.write(physical, &bytes[range]);
        })
    }

    fn install(&mut self, commit: Commit) -> Result<(), Error> {
        self.poisoned.set(true);
        let pages = self.memory.size();
        if commit.root.end > pages && self.memory.grow(commit.root.end - pages) == -1 {
            self.poisoned.set(false);

            return Err(Error::Growth);
        }
        let root = commit.root.encode();
        self.memory.write(commit.root.start * PAGE, &commit.payload);
        // Publication is the final stable write. Never expose tentative state
        // through this owner when either write panics.
        self.memory.write(0, &root);
        self.root = root;
        self.directory = commit.directory;
        self.poisoned.set(false);

        Ok(())
    }
}

fn read(
    memory: &impl Memory,
    budget: StorageConfig,
) -> Result<([u8; ROOT_BYTES], Directory), Error> {
    let physical_pages = memory.size();
    if physical_pages == 0 {
        return Err(Error::Invalid("missing root"));
    }
    if physical_pages > budget.pages || physical_pages.checked_mul(PAGE).is_none() {
        return Err(Error::Budget);
    }
    let mut bytes = [0; ROOT_BYTES];
    memory.read(0, &mut bytes);
    let root = Root::decode(&bytes)?;
    if root.start < 1 || root.length == 0 || root.length > budget.directory_bytes as u64 {
        return Err(Error::Invalid("directory record bounds"));
    }
    let expected_end = root
        .start
        .checked_add(root.length.div_ceil(PAGE))
        .ok_or(Error::Budget)?;
    if expected_end > root.end || root.end > physical_pages {
        return Err(Error::Invalid("directory record extent"));
    }
    let length = usize::try_from(root.length).map_err(|_| Error::Budget)?;
    let mut payload = vec![0; length];
    memory.read(root.start * PAGE, &mut payload);
    if digest(&payload) != root.digest {
        return Err(Error::Invalid("directory checksum"));
    }
    let directory = Directory::decode(&payload, budget, 1)?;
    if directory.image.generation != root.generation || directory.image.next_page != root.end {
        return Err(Error::Invalid("root and directory disagree"));
    }
    // Validation rejects all overlap between application data and every arena,
    // including inactive arenas whose old bytes are deliberately not trusted.
    if !directory
        .image
        .metadata
        .iter()
        .any(|arena| arena.start == root.start && root.length.div_ceil(PAGE) <= arena.pages)
    {
        return Err(Error::Invalid("root is not in a reserved metadata arena"));
    }

    Ok((bytes, directory))
}

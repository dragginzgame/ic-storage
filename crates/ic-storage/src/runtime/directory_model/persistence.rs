//! Reusable snapshot arenas with a fixed protected root.
//! The owner is poisoned before fallible backing IO and must be dropped after
//! a panic. Native partial root writes are detected, not repaired or rolled back.
use super::{AllocationId, Directory, Error, Image, StorageConfig, hash};
use crate::{Memory, WASM_PAGE_SIZE_BYTES as PAGE};
use std::cell::Cell;

const MAGIC: &[u8; 8] = b"ICROOTR1";
const ROOT_BYTES: usize = 64;

struct Root {
    generation: u64,
    start: u64,
    length: u64,
    end: u64,
    digest: u64,
}

fn digest(bytes: &[u8]) -> u64 {
    hash::fnv64(hash::FNV_OFFSET, bytes)
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

pub struct Store<M: Memory> {
    memory: M,
    pub(super) directory: Directory,
    root: [u8; ROOT_BYTES],
    poisoned: Cell<bool>,
    last_span: Cell<Option<Span>>,
}

#[derive(Clone, Copy)]
struct Span {
    id: AllocationId,
    start: u64,
    end: u64,
    physical: u64,
}

// The immutable candidate includes the reservation for its own metadata bytes.
struct Commit {
    directory: Directory,
    payload: Vec<u8>,
    root: Root,
}

impl Commit {
    fn prepare(
        mut image: Image,
        budget: StorageConfig,
        active: Option<u64>,
    ) -> Result<Self, Error> {
        // Never overwrite the active snapshot. Reuse only arenas explicitly
        // reserved by the committed directory; abandoned tails remain excluded.
        let mut pages = 1_u64;
        let next_page = image.next_page;
        let metadata_len = image.metadata.len();
        for _ in 0..64 {
            // Undo only this attempt's metadata reservation before resizing.
            // Carry the owned image forward instead of cloning every entry.
            image.next_page = next_page;
            image.metadata.truncate(metadata_len);
            let reusable = image
                .metadata
                .iter()
                .filter(|arena| Some(arena.start) != active && arena.pages >= pages)
                .min_by_key(|arena| arena.pages)
                .cloned();
            let arena = if let Some(arena) = reusable {
                arena
            } else {
                let arena = super::Extent {
                    start: image.next_page,
                    pages,
                };
                image.next_page = arena.start.checked_add(pages).ok_or(Error::Budget)?;
                image.metadata.push(arena.clone());
                arena
            };
            let payload = image.encode(budget)?;
            let length = u64::try_from(payload.len()).map_err(|_| Error::Budget)?;
            let required = length.div_ceil(PAGE);
            if required <= arena.pages {
                let directory = Directory::validate(image, budget, 1)?;
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
        let commit = Commit::prepare(directory.image.clone(), budget, None)?;
        let mut owner = Self {
            memory,
            directory,
            root: [0; ROOT_BYTES],
            poisoned: Cell::new(false),
            last_span: Cell::new(None),
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
            last_span: Cell::new(None),
        })
    }

    // The exclusive store borrow covers verification, preparation and publication.
    // No prepared candidate escapes to another owner or a later generation.
    pub(super) fn mutate(
        &mut self,
        keys: &std::collections::BTreeSet<super::StableKey>,
        growth: &[(super::AllocationId, u64)],
    ) -> Result<(), Error> {
        self.verify()?;
        // Native interrupted writes may leave nonzero bytes past the committed
        // watermark. Allocate new data beyond that tail, never inside it.
        let image = self
            .directory
            .candidate_image(keys, growth, self.backing_pages()?)?;
        let commit = Commit::prepare(
            image,
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

    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn poison(&self) {
        self.poisoned.set(true);
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

    pub(super) fn diagnostics(&self) -> Result<crate::StorageDiagnostics, Error> {
        self.verify()?;
        let physical_pages = self.backing_pages()?;
        let image = &self.directory.image;
        let metadata_pages: u64 = image.metadata.iter().map(|e| e.pages).sum();
        let data_pages: u64 = image
            .entries
            .iter()
            .flat_map(|e| &e.extents)
            .map(|e| e.pages)
            .sum();
        // Validation proves disjoint extents within the physical watermark.
        Ok(crate::StorageDiagnostics {
            generation: image.generation,
            allocations: image.entries.len(),
            admitted_allocations: 0,
            live_leases: 0,
            extents: image.entries.iter().map(|e| e.extents.len()).sum(),
            data_pages,
            metadata_pages,
            directory_bytes: Root::decode(&self.root)?.length,
            physical_pages,
            unassigned_pages: physical_pages - 1 - metadata_pages - data_pages,
        })
    }

    #[inline]
    pub(super) fn pages(&self, index: usize) -> Result<u64, Error> {
        self.healthy()?;
        // Disjoint extents fit within the admitted physical page budget.
        Ok(self.directory.logical_ends[index]
            .last()
            .copied()
            .unwrap_or(0))
    }

    #[inline]
    fn access(
        &self,
        id: AllocationId,
        offset: u64,
        length: usize,
        mut io: impl FnMut(u64, std::ops::Range<usize>),
    ) -> Result<(), Error> {
        self.healthy()?;
        let length64 = u64::try_from(length).map_err(|_| Error::Bounds)?;
        let end = offset.checked_add(length64).ok_or(Error::Bounds)?;
        if let Some(span) = self.last_span.get()
            && span.id == id
            && offset >= span.start
            && end <= span.end
        {
            self.poisoned.set(true);
            if length != 0 {
                io(span.physical + offset - span.start, 0..length);
            }
            self.poisoned.set(false);
            return Ok(());
        }
        self.access_uncached(id, offset, length, end, io)
    }

    // Keep extent walking and errors out of the span-hit path copied into each
    // collection's tiny IO operations. The complete range is still admitted first.
    // Without this boundary LLVM folds the walk back into the hot read wrapper,
    // preventing small fixed-width callers from inlining its cache-hit path.
    #[inline(never)]
    fn access_uncached(
        &self,
        id: AllocationId,
        offset: u64,
        length: usize,
        end: u64,
        mut io: impl FnMut(u64, std::ops::Range<usize>),
    ) -> Result<(), Error> {
        let index = self.directory.entry_index(id)?;
        let ends = &self.directory.logical_ends[index];
        let capacity = ends
            .last()
            .copied()
            .unwrap_or(0)
            .checked_mul(PAGE)
            .ok_or(Error::Bounds)?;
        if end > capacity {
            return Err(Error::Bounds);
        }
        // Validate the whole logical range before any backing IO. Unwinding
        // during a partial native read/write poisons every shared lease.
        self.poisoned.set(true);
        let mut done = 0;
        while done < length {
            let logical = offset + u64::try_from(done).unwrap();
            let slot = ends.partition_point(|&end| end <= logical / PAGE);
            let extent = &self.directory.image.entries[index].extents[slot];
            let span = Span {
                id,
                start: if slot == 0 { 0 } else { ends[slot - 1] * PAGE },
                end: ends[slot] * PAGE,
                physical: extent.start * PAGE,
            };
            self.last_span.set(Some(span));
            let available = span.end - logical;
            let count = (length - done).min(usize::try_from(available).unwrap_or(usize::MAX));
            io(span.physical + logical - span.start, done..done + count);
            done += count;
        }
        self.poisoned.set(false);
        Ok(())
    }

    #[inline]
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

    /// # Safety
    /// The destination must satisfy `Memory::read_unsafe`'s exclusive range contract.
    #[inline]
    #[expect(
        unsafe_code,
        reason = "Read each admitted extent into a disjoint portion of the caller's destination"
    )]
    pub(super) unsafe fn read_data_unsafe(
        &self,
        id: AllocationId,
        offset: u64,
        dst: *mut u8,
        count: usize,
    ) -> Result<(), Error> {
        self.access(id, offset, count, |physical, range| {
            // SAFETY: access preflights the complete range, then covers each
            // destination byte exactly once. Every chunk lies within count;
            // successful backing reads initialize it before returning.
            unsafe {
                self.memory
                    .read_unsafe(physical, dst.add(range.start), range.len());
            }
        })
    }

    #[inline]
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
        self.last_span.set(None);
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
    if root.start < 1 || root.length == 0 || root.length > budget.wire.bytes as u64 {
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

#[cfg(test)]
mod tests;

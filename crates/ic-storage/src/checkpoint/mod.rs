//! Bounded checkpoint records and pages, independent of the retained native heap.
//!
//! A caller exclusively owns the backing memory for the complete handle lifetime.
//! Cloneable backing handles permit disjoint regions, not concurrent stores or
//! external mutation. Opening validates a fixed caller-declared layout without
//! writing. Creation requires zero allocated pages; unknown bytes never reset.
//! This is checkpoint backing, not a general allocation or native admission API.
//!
//! Writes are not crash-atomic on arbitrary hosts. An enclosing transaction must
//! roll back failed publication, including growth and subsequent native writes.
//! IC message rollback supplies that transaction in the qualification adapter.
//! Logical checkpoint codecs own payload integrity, completeness and job identity.

mod pages;
mod record;
pub use pages::Pages;
pub use record::{Bytes, Record};

use alloc::vec::Vec;

/// Backing page size in bytes.
pub const PAGE_SIZE: u64 = 65_536;
const HEADER: usize = 128;
const MAX_REGIONS: usize = 4;

/// Caller-owned byte backing. Implementations must preserve reads and writes and
/// return the old page count after successful zero-filled growth. IO failure
/// must stop execution; it must never report successful partial IO.
pub trait Memory: Clone {
    /// Allocated 64-KiB pages.
    fn size(&self) -> u64;
    /// Grow by pages, returning the old size, or refuse without mutation.
    fn grow(&self, pages: u64) -> Option<u64>;
    /// Copy a fully allocated range into initialized bytes.
    fn read(&self, offset: u64, destination: &mut [u8]);
    /// Replace a fully allocated range with these exact bytes.
    fn write(&self, offset: u64, source: &[u8]);
}

/// Refusal before a checkpoint operation can be admitted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// Creation requires backing with no allocated pages.
    NotFresh,
    /// Missing, corrupt, foreign or differently declared storage.
    Layout,
    /// Invalid identifier, size, index, or logical byte bound.
    Bounds,
    /// Backing growth refused or reported an inconsistent prior size.
    Growth,
    /// Region was already claimed from this store.
    Claimed,
}

/// A fixed disjoint region. IDs are explicit schema identities, not Rust TypeIds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Specification {
    /// Nonzero owner-assigned identity, increasing in declaration order.
    pub id: u64,
    /// Reserved bytes, including the region's record/page framing.
    pub bytes: u64,
}

/// Admitted layout with one-use issuance of its disjoint regions.
pub struct Store<M: Memory> {
    memory: M,
    regions: Vec<(Specification, u64, bool)>,
    fresh: bool,
}

fn layout(specs: &[Specification]) -> Result<([u8; HEADER], u64), Error> {
    if specs.is_empty() || specs.len() > MAX_REGIONS {
        return Err(Error::Bounds);
    }
    let mut header = [0; HEADER];
    header[..8].copy_from_slice(b"ICKPT001");
    header[8..16].copy_from_slice(&(specs.len() as u64).to_le_bytes());
    let mut end = HEADER as u64;
    let mut prior = 0;
    for (index, spec) in specs.iter().enumerate() {
        if spec.id <= prior || spec.bytes < 16 {
            return Err(Error::Bounds);
        }
        prior = spec.id;
        let start = 16 + index * 24;
        header[start..start + 8].copy_from_slice(&spec.id.to_le_bytes());
        header[start + 8..start + 16].copy_from_slice(&end.to_le_bytes());
        header[start + 16..start + 24].copy_from_slice(&spec.bytes.to_le_bytes());
        end = end.checked_add(spec.bytes).ok_or(Error::Bounds)?;
    }
    if end > u64::MAX / PAGE_SIZE * PAGE_SIZE {
        return Err(Error::Bounds);
    }
    Ok((header, end))
}

impl<M: Memory> Store<M> {
    /// Reserve a fresh bounded layout. No existing allocation is overwritten.
    ///
    /// # Errors
    /// Refuses invalid declarations, existing backing or failed growth. A backing
    /// implementation violating its growth contract may already have mutated it.
    pub fn create(memory: M, specs: &[Specification]) -> Result<Self, Error> {
        let (header, end) = layout(specs)?;
        if memory.size() != 0 {
            return Err(Error::NotFresh);
        }
        let pages = end.div_ceil(PAGE_SIZE);
        if memory.grow(pages) != Some(0) || memory.size() != pages {
            return Err(Error::Growth);
        }
        memory.write(0, &header);
        Ok(Self::admitted(memory, specs, true))
    }

    /// Open only the exact declared layout, with no creation or repair writes.
    ///
    /// # Errors
    /// Refuses malformed declarations, a differing allocated size or any differing
    /// header byte, including version, identity, capacity, offset and reserved bytes.
    pub fn open(memory: M, specs: &[Specification]) -> Result<Self, Error> {
        let (expected, end) = layout(specs)?;
        if memory.size() != end.div_ceil(PAGE_SIZE) {
            return Err(Error::Layout);
        }
        let mut actual = [0; HEADER];
        memory.read(0, &mut actual);
        if actual != expected {
            return Err(Error::Layout);
        }
        Ok(Self::admitted(memory, specs, false))
    }

    fn admitted(memory: M, specs: &[Specification], fresh: bool) -> Self {
        let mut offset = HEADER as u64;
        let regions = specs
            .iter()
            .map(|spec| {
                let start = offset;
                offset += spec.bytes; // layout has checked the complete sum.
                (*spec, start, false)
            })
            .collect();
        Self {
            memory,
            regions,
            fresh,
        }
    }

    /// Issue a region once. Claims remain consumed even if a handle is forgotten.
    ///
    /// # Errors
    /// Refuses an undeclared ID or a second claim.
    pub fn take(&mut self, id: u64) -> Result<Region<M>, Error> {
        let (spec, offset, claimed) = self
            .regions
            .iter_mut()
            .find(|(spec, _, _)| spec.id == id)
            .ok_or(Error::Bounds)?;
        if *claimed {
            return Err(Error::Claimed);
        }
        *claimed = true;
        Ok(Region {
            memory: self.memory.clone(),
            offset: *offset,
            bytes: spec.bytes,
            fresh: self.fresh,
        })
    }
}

/// Exclusive logical region issued by an admitted store; not cloneable.
pub struct Region<M: Memory> {
    memory: M,
    offset: u64,
    bytes: u64,
    fresh: bool,
}

impl<M: Memory> Region<M> {
    fn fits(&self, offset: u64, length: u64) -> Result<(), Error> {
        if offset
            .checked_add(length)
            .is_none_or(|end| end > self.bytes)
        {
            return Err(Error::Bounds);
        }
        Ok(())
    }

    fn read(&self, offset: u64, destination: &mut [u8]) {
        // Every call is bounded by admission or a checked operation first.
        self.memory.read(self.offset + offset, destination);
    }

    fn write(&self, offset: u64, source: &[u8]) {
        self.memory.write(self.offset + offset, source);
    }

    fn header(&self, magic: &[u8; 8]) -> Result<u64, Error> {
        let mut header = [0; 16];
        self.read(0, &mut header);
        if header[..8] != *magic {
            return Err(Error::Layout);
        }
        Ok(u64::from_le_bytes(
            header[8..].try_into().map_err(|_| Error::Layout)?,
        ))
    }

    fn initialize(&self, magic: &[u8; 8], length: u64) -> Result<(), Error> {
        if !self.fresh {
            return Err(Error::NotFresh);
        }
        let mut existing = [0; 16];
        self.read(0, &mut existing);
        if existing != [0; 16] {
            return Err(Error::NotFresh);
        }
        let mut header = [0; 16];
        header[..8].copy_from_slice(magic);
        header[8..].copy_from_slice(&length.to_le_bytes());
        self.write(0, &header);
        Ok(())
    }
}

#[cfg(test)]
mod tests;

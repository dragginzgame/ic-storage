//! Private governed memory views for the independent ordered engine.
use super::region::RegionMemory;
use crate::{Memory, RuntimeMemory, ordered_map};

// One memory type gives ordinary maps and bounded regions the same tree
// specialization. Ordinary maps own their lease; operation-local region trees
// retain the existing Rc without allocating another backing or caching a root.
pub(super) enum TreeMemory<M: Memory> {
    Allocation(RuntimeMemory<M>),
    Region(RegionMemory<M>),
}
impl<M: Memory> TreeMemory<M> {
    pub(super) fn allocation(&self) -> &RuntimeMemory<M> {
        match self {
            Self::Allocation(memory) => memory,
            Self::Region(memory) => memory.allocation(),
        }
    }
}
impl<M: Memory> Memory for TreeMemory<M> {
    fn size(&self) -> u64 {
        match self {
            Self::Allocation(memory) => memory.size(),
            Self::Region(memory) => memory.size(),
        }
    }
    fn grow(&self, pages: u64) -> i64 {
        match self {
            Self::Allocation(memory) => Memory::grow(memory, pages),
            Self::Region(memory) => memory.grow(pages),
        }
    }
    fn read(&self, offset: u64, bytes: &mut [u8]) {
        match self {
            Self::Allocation(memory) => memory.read(offset, bytes),
            Self::Region(memory) => memory.read(offset, bytes),
        }
    }
    fn write(&self, offset: u64, bytes: &[u8]) {
        match self {
            Self::Allocation(memory) => memory.write(offset, bytes),
            Self::Region(memory) => memory.write(offset, bytes),
        }
    }
    #[expect(
        unsafe_code,
        reason = "Forward unchanged destination to the selected checked memory"
    )]
    unsafe fn read_unsafe(&self, offset: u64, dst: *mut u8, count: usize) {
        // SAFETY: each arm retains the caller's exclusive destination and full
        // initialization contract; region translation still checks its bounds.
        unsafe {
            match self {
                Self::Allocation(memory) => memory.read_unsafe(offset, dst, count),
                Self::Region(memory) => memory.read_unsafe(offset, dst, count),
            }
        }
    }
}
pub(super) type Map<K, V, M> = ordered_map::OrderedMap<K, V, TreeMemory<M>>;

pub(super) fn reserve<M: Memory>(memory: &M) -> super::error::Result<()> {
    if memory.size() == 0 && memory.grow(1) == -1 {
        return Err(super::CollectionError::Growth);
    }
    Ok(())
}

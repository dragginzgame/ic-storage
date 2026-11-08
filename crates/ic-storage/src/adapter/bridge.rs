//! Private codec and memory views for the copied collection engines.
use super::Storable;
use super::region::RegionMemory;
use crate::{Memory, RuntimeMemory, engine};
use std::borrow::Cow;

pub(super) struct Backing<M>(pub M);
impl<M: Memory> engine::Memory for Backing<M> {
    fn size(&self) -> u64 {
        self.0.size()
    }
    fn grow(&self, pages: u64) -> i64 {
        self.0.grow(pages)
    }
    fn read(&self, offset: u64, bytes: &mut [u8]) {
        self.0.read(offset, bytes);
    }
    #[inline]
    #[expect(
        unsafe_code,
        reason = "Forward the copied ICSS raw-read contract through governed backing"
    )]
    unsafe fn read_unsafe(&self, offset: u64, dst: *mut u8, count: usize) {
        // SAFETY: both traits require the same exclusive destination and full
        // initialization on success; no address or count changes here.
        unsafe { self.0.read_unsafe(offset, dst, count) }
    }
    fn write(&self, offset: u64, bytes: &[u8]) {
        self.0.write(offset, bytes);
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
#[repr(transparent)]
pub(super) struct Codec<T>(pub T);
impl<T> Codec<T> {
    #[expect(
        unsafe_code,
        reason = "Transparent immutable codec view preserves key identity without a clone"
    )]
    pub(super) const fn borrowed(value: &T) -> &Self {
        // SAFETY: repr(transparent) gives identical layout/alignment and the
        // wrapper adds no invariant. This shared view has the input lifetime.
        unsafe { &*std::ptr::from_ref(value).cast::<Self>() }
    }
}
impl<T: Storable> engine::Storable for Codec<T> {
    const BOUND: engine::storable::Bound = match T::BOUND {
        super::Bound::Unbounded => engine::storable::Bound::Unbounded,
        super::Bound::Bounded {
            max_size,
            is_fixed_size,
        } => engine::storable::Bound::Bounded {
            max_size,
            is_fixed_size,
        },
    };
    fn to_bytes(&self) -> Cow<'_, [u8]> {
        self.0
            .to_bytes_checked()
            .expect("collection encoding failed")
    }
    fn into_bytes(self) -> Vec<u8> {
        self.0
            .into_bytes_checked()
            .expect("collection encoding failed")
    }
    fn from_bytes(bytes: Cow<'_, [u8]>) -> Self {
        T::check_bounds(&bytes).expect("stored collection value exceeds codec bounds");
        Self(T::from_bytes(bytes).expect("stored collection value failed to decode"))
    }
}

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
pub(super) type Map<K, V, M> = engine::BTreeMap<Codec<K>, Codec<V>, Backing<TreeMemory<M>>>;

pub(super) fn reserve<M: Memory>(memory: &M) -> super::error::Result<()> {
    if memory.size() == 0 && memory.grow(1) == -1 {
        return Err(super::CollectionError::Growth);
    }
    Ok(())
}

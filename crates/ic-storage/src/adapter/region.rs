//! Operation-local collection access within a retained allocation region.
use super::{Memory, Storable};
use crate::{
    DefaultMemoryImpl, RuntimeGrowError, RuntimeMemory, WASM_PAGE_SIZE_BYTES as PAGE_BYTES,
    substrate,
};
use std::{
    marker::PhantomData,
    ops::{Range, RangeBounds},
    rc::Rc,
};

const MAX_PAGES: u64 = 4_194_304;

/// Failure to reserve a governed region's virtual extent, before any writes.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum RegionGrowError {
    /// The requested relative extent exceeds the selected region.
    #[error("requested region extent {minimum} exceeds {maximum} pages")]
    LimitExceeded {
        /// Requested relative page count.
        minimum: u64,
        /// Maximum relative page count.
        maximum: u64,
    },
    /// Storage refused to grow the containing allocation.
    #[error("{0}")]
    Runtime(#[source] RuntimeGrowError),
}

/// A B-tree view inside one existing, governed allocation lease.
///
/// The caller retains its `Rc<RuntimeMemory>` for disjoint application frames.
/// Each collection operation loads the current upstream root and allocator;
/// multiple views may interleave operations without retaining stale roots.
/// Key scans return owned snapshots, so no cached tree or iterator escapes.
/// Applications own namespace bounds, codec admission and recovery policy.
///
/// Construction performs no IO. Inspect retained framing with [`Self::read`]
/// before calling collection operations, which retain upstream load/panic
/// contracts. Only [`Self::initialize`] creates a tree, and only in an empty
/// region. No method reinitializes nonempty memory or exports raw backing.
/// As with ordinary collections, callers must not reenter collection access
/// from a codec or mutate the tree's bytes through their retained lease.
pub struct BTreeMapRegion<K: Storable + Ord + Clone, V: Storable, M: Memory = DefaultMemoryImpl> {
    memory: RegionMemory<M>,
    record: PhantomData<(K, V)>,
}

impl<K: Storable + Ord + Clone, V: Storable, M: Memory> BTreeMapRegion<K, V, M> {
    /// Select an absolute, half-open range of 65,536-byte allocation pages.
    ///
    /// Retains the same allocation lease; no allocation or collection is opened.
    /// Other owners of the `Rc` may access disjoint frames in that allocation.
    /// Drop every view and the caller's last `Rc` before reopening the lease.
    ///
    /// # Panics
    /// Panics unless `pages.start < pages.end <= 4_194_304`.
    #[must_use]
    pub fn new(memory: Rc<RuntimeMemory<M>>, pages: Range<u64>) -> Self {
        assert!(pages.start < pages.end && pages.end <= MAX_PAGES);
        Self {
            memory: RegionMemory { memory, pages },
            record: PhantomData,
        }
    }

    /// Return the currently allocated relative extent in Wasm pages.
    #[must_use]
    pub fn allocated_pages(&self) -> u64 {
        (&self.memory).size()
    }

    /// Read relative bytes for the application's retained-framing admission.
    ///
    /// # Panics
    /// Panics before IO if the complete read exceeds the allocated region.
    /// An empty read is valid at the region's end, but not beyond it.
    pub fn read(&self, offset: u64, dst: &mut [u8]) {
        (&self.memory).read(offset, dst);
    }

    /// Create an upstream tree only when the selected region has zero pages.
    ///
    /// Call before reserving extra pages. Retained trees need no initialization:
    /// admit their framing and use the operations directly.
    ///
    /// # Panics
    /// Panics without writing if the region is nonempty. Empty-region creation
    /// retains upstream growth and panic contracts.
    pub fn initialize(&mut self) {
        assert_eq!(self.allocated_pages(), 0, "region is already nonempty");
        substrate::BTreeMap::<K, V, _>::new(&self.memory);
    }

    /// Reserve an absolute relative extent without promising entry capacity.
    ///
    /// # Errors
    /// Returns a region-limit or Storage growth error without changing bytes
    /// or allocation accounting. A satisfied request performs no growth or IO.
    /// Callers own byte/page budgets and must qualify them for their codecs.
    /// Mutations retain upstream failure semantics; this is not a transaction
    /// around an insert that later runs out of space or panics in a codec.
    pub fn reserve_pages(&mut self, minimum: u64) -> Result<(), RegionGrowError> {
        self.memory.reserve_pages(minimum)
    }

    /// Read a record using the current retained root.
    pub fn get(&self, key: &K) -> Option<V> {
        self.current_map().get(key)
    }

    /// Insert using the current retained root and upstream mutation semantics.
    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        self.current_map().insert(key, value)
    }

    /// Remove using the current retained root and upstream mutation semantics.
    pub fn remove(&mut self, key: &K) -> Option<V> {
        self.current_map().remove(key)
    }

    /// Return the current tree's record count across all namespaces.
    #[must_use]
    pub fn len(&self) -> u64 {
        self.current_map().len()
    }

    /// Return whether the current tree has no records.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.current_map().is_empty()
    }

    /// Snapshot ordered keys within the caller's namespace/range bounds.
    ///
    /// The scan completes before returning; the iterator retains owned keys,
    /// with no collection cache, memory handle or borrow across later mutations.
    pub fn keys_range(&self, bounds: impl RangeBounds<K>) -> std::vec::IntoIter<K> {
        self.keys_range_limited(bounds, usize::MAX)
    }

    /// Snapshot at most `limit` ordered keys, stopping iteration at that bound.
    ///
    /// Unlike `keys_range(bounds).take(limit)`, this bounds collection work before
    /// allocating the snapshot. Zero returns without loading the tree. Resume
    /// with an excluded last key; intervening mutations are visible to each call.
    /// The limit bounds key count, not encoded bytes or instruction cost.
    pub fn keys_range_limited(
        &self,
        bounds: impl RangeBounds<K>,
        limit: usize,
    ) -> std::vec::IntoIter<K> {
        if limit == 0 {
            return Vec::new().into_iter();
        }
        self.current_map()
            .keys_range(bounds)
            .take(limit)
            .collect::<Vec<_>>()
            .into_iter()
    }

    fn current_map(&self) -> substrate::BTreeMap<K, V, &RegionMemory<M>> {
        substrate::BTreeMap::load(&self.memory)
    }
}

// Private bounded IO over an admitted lease. Upstream RestrictedMemory does not
// bound reads/writes and may overgrow an absent region; enforce both here.
struct RegionMemory<M: Memory> {
    memory: Rc<RuntimeMemory<M>>,
    pages: Range<u64>,
}

impl<M: Memory> RegionMemory<M> {
    fn reserve_pages(&self, minimum: u64) -> Result<(), RegionGrowError> {
        let maximum = self.pages.end - self.pages.start;
        if minimum > maximum {
            return Err(RegionGrowError::LimitExceeded { minimum, maximum });
        }
        if minimum > self.size() {
            let additional = self.pages.start + minimum - self.memory.size();
            self.memory
                .as_ref()
                .grow(additional)
                .map_err(RegionGrowError::Runtime)?;
        }
        Ok(())
    }

    fn check_io(&self, offset: u64, count: usize) {
        let extent = self.size() * PAGE_BYTES;
        assert!(offset <= extent && count as u64 <= extent - offset);
    }
}

impl<M: Memory> Memory for &RegionMemory<M> {
    fn size(&self) -> u64 {
        self.memory.size().clamp(self.pages.start, self.pages.end) - self.pages.start
    }

    fn grow(&self, delta: u64) -> i64 {
        let previous = self.size();
        let Some(minimum) = previous.checked_add(delta) else {
            return -1;
        };
        if self.reserve_pages(minimum).is_ok() {
            // The selected range is bounded by MAX_PAGES.
            i64::try_from(previous).unwrap()
        } else {
            -1
        }
    }

    fn read(&self, offset: u64, dst: &mut [u8]) {
        self.check_io(offset, dst.len());
        if !dst.is_empty() {
            self.memory
                .read(self.pages.start * PAGE_BYTES + offset, dst);
        }
    }

    fn write(&self, offset: u64, src: &[u8]) {
        self.check_io(offset, src.len());
        if !src.is_empty() {
            self.memory
                .write(self.pages.start * PAGE_BYTES + offset, src);
        }
    }
}

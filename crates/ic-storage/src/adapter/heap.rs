//! Governed heap collection implementation.
use super::{Memory, Storable};
use crate::RuntimeMemory;
use crate::{DefaultMemoryImpl, substrate};

/// Governed upstream `MinHeap`; initialization and failures follow upstream contracts.
pub struct StableMinHeap<T: Storable + Ord, M: Memory = DefaultMemoryImpl> {
    inner: substrate::MinHeap<T, RuntimeMemory<M>>,
}
impl<T: Storable + Ord, M: Memory> StableMinHeap<T, M> {
    /// Create a collection only in zero-page memory.
    ///
    /// # Panics
    /// Refuses nonempty memory before writing. Growth and codec failures retain
    /// upstream behavior.
    #[must_use]
    pub fn create(memory: RuntimeMemory<M>) -> Self {
        assert_eq!(memory.size(), 0, "collection memory is not empty");
        Self::reset(memory)
    }

    /// Deliberately replace any contents in this admitted allocation.
    ///
    /// # Panics
    /// Growth and codec failures retain upstream behavior.
    #[must_use]
    pub fn reset(memory: RuntimeMemory<M>) -> Self {
        Self {
            inner: substrate::MinHeap::new(memory),
        }
    }

    /// Load retained collection state without initializing a replacement.
    ///
    /// # Panics
    /// Refuses empty or foreign memory and incompatible layouts/bounds without
    /// writing. Record decoding and backing failures retain upstream behavior.
    #[must_use]
    pub fn load(memory: RuntimeMemory<M>) -> Self {
        assert!(memory.size() > 0, "collection memory is empty");
        Self {
            inner: substrate::MinHeap::init(memory),
        }
    }
    /// Delegates to upstream `MinHeap::push` with the governed allocation.
    pub fn push(&mut self, item: &T) {
        self.inner.push(item);
    }
    /// Delegates to upstream `MinHeap::pop` with the governed allocation.
    pub fn pop(&mut self) -> Option<T> {
        self.inner.pop()
    }
    /// Delegates to upstream `MinHeap::peek` with the governed allocation.
    #[must_use]
    pub fn peek(&self) -> Option<T> {
        self.inner.peek()
    }
    /// Delegates to upstream `MinHeap::len` with the governed allocation.
    #[must_use]
    pub fn len(&self) -> u64 {
        self.inner.len()
    }
    /// Delegates to upstream `MinHeap::is_empty` with the governed allocation.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
    /// Delegates to upstream `MinHeap::iter` with the governed allocation.
    pub fn iter(&self) -> impl Iterator<Item = T> + '_ {
        self.inner.iter()
    }
}

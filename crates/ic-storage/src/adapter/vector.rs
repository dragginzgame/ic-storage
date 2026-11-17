//! Governed vector collection implementation.
use super::{Memory, Storable};
use crate::RuntimeMemory;
use crate::{DefaultMemoryImpl, substrate};

/// Governed upstream `Vec`; initialization and failures follow upstream contracts.
pub struct StableVec<T: Storable, M: Memory = DefaultMemoryImpl> {
    inner: substrate::Vec<T, RuntimeMemory<M>>,
}
impl<T: Storable, M: Memory> StableVec<T, M> {
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
            inner: substrate::Vec::new(memory),
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
            inner: substrate::Vec::init(memory),
        }
    }
    /// Delegates to upstream `Vec::get` with the governed allocation.
    #[must_use]
    pub fn get(&self, index: u64) -> Option<T> {
        self.inner.get(index)
    }
    /// Delegates to upstream `Vec::set` with the governed allocation.
    pub fn set(&self, index: u64, item: &T) {
        self.inner.set(index, item);
    }
    /// Delegates to upstream `Vec::push` with the governed allocation.
    pub fn push(&self, item: &T) {
        self.inner.push(item);
    }
    /// Delegates to upstream `Vec::pop` with the governed allocation.
    #[must_use]
    pub fn pop(&self) -> Option<T> {
        self.inner.pop()
    }
    /// Delegates to upstream `Vec::len` with the governed allocation.
    #[must_use]
    pub fn len(&self) -> u64 {
        self.inner.len()
    }
    /// Delegates to upstream `Vec::is_empty` with the governed allocation.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
    /// Delegates to upstream `Vec::iter` with the governed allocation.
    pub fn iter(&self) -> impl DoubleEndedIterator<Item = T> + '_ {
        self.inner.iter()
    }
}

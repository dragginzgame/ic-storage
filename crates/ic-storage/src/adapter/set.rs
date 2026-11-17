//! Governed set collection implementation.
use super::{Memory, SetIter, Storable};
use crate::RuntimeMemory;
use crate::{DefaultMemoryImpl, substrate};
use std::ops::RangeBounds;

/// Governed upstream `BTreeSet`; initialization and failures follow upstream contracts.
pub struct StableBTreeSet<K: Storable + Ord + Clone, M: Memory = DefaultMemoryImpl> {
    inner: substrate::BTreeSet<K, RuntimeMemory<M>>,
}
impl<K: Storable + Ord + Clone, M: Memory> StableBTreeSet<K, M> {
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
            inner: substrate::BTreeSet::new(memory),
        }
    }

    /// Load retained collection state without initializing a replacement.
    ///
    /// # Panics
    /// Refuses empty or foreign memory and incompatible layouts/bounds without
    /// writing. Record decoding and backing failures retain upstream behavior.
    #[must_use]
    pub fn load(memory: RuntimeMemory<M>) -> Self {
        Self {
            inner: substrate::BTreeSet::load(memory),
        }
    }
    /// Delegates to upstream `BTreeSet::insert` with the governed allocation.
    pub fn insert(&mut self, key: K) -> bool {
        self.inner.insert(key)
    }
    /// Delegates to upstream `BTreeSet::contains` with the governed allocation.
    pub fn contains(&self, key: &K) -> bool {
        self.inner.contains(key)
    }
    /// Delegates to upstream `BTreeSet::is_empty` with the governed allocation.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
    /// Delegates to upstream `BTreeSet::len` with the governed allocation.
    #[must_use]
    pub fn len(&self) -> u64 {
        self.inner.len()
    }
    /// Delegates to upstream `BTreeSet::remove` with the governed allocation.
    pub fn remove(&mut self, key: &K) -> bool {
        self.inner.remove(key)
    }
    /// Delegates to upstream `BTreeSet::first` with the governed allocation.
    #[must_use]
    pub fn first(&self) -> Option<K> {
        self.inner.first()
    }
    /// Delegates to upstream `BTreeSet::last` with the governed allocation.
    #[must_use]
    pub fn last(&self) -> Option<K> {
        self.inner.last()
    }
    /// Delegates to upstream `BTreeSet::pop_first` with the governed allocation.
    pub fn pop_first(&mut self) -> Option<K> {
        self.inner.pop_first()
    }
    /// Delegates to upstream `BTreeSet::pop_last` with the governed allocation.
    pub fn pop_last(&mut self) -> Option<K> {
        self.inner.pop_last()
    }
    /// Delegates to upstream `BTreeSet::iter` with the governed allocation.
    #[must_use]
    pub fn iter(&self) -> SetIter<'_, K, M> {
        self.inner.iter()
    }
    /// Delegates to upstream `BTreeSet::range` with the governed allocation.
    pub fn range(&self, key_range: impl RangeBounds<K>) -> SetIter<'_, K, M> {
        self.inner.range(key_range)
    }
    /// Delegates to upstream `BTreeSet::clear` with the governed allocation.
    pub fn clear(&mut self) {
        self.inner.clear();
    }
}

impl<'a, K: Storable + Ord + Clone, M: Memory> IntoIterator for &'a StableBTreeSet<K, M> {
    type Item = K;
    type IntoIter = SetIter<'a, K, M>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

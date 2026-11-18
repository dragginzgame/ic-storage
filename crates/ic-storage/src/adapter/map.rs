//! Governed map collection implementation.
use super::{MapIter, Memory, Storable};
use crate::{DefaultMemoryImpl, RuntimeMemory, substrate};
use std::ops::RangeBounds;

/// Governed upstream `BTreeMap`; initialization and failures follow upstream contracts.
pub struct StableBTreeMap<K: Storable + Ord + Clone, V: Storable, M: Memory = DefaultMemoryImpl> {
    inner: substrate::BTreeMap<K, V, RuntimeMemory<M>>,
}

impl<K: Storable + Ord + Clone, V: Storable, M: Memory> StableBTreeMap<K, V, M> {
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
            inner: substrate::BTreeMap::new(memory),
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
            inner: substrate::BTreeMap::load(memory),
        }
    }

    /// Clear entries while retaining this allocation's exclusive lease.
    pub fn clear(&mut self) {
        self.inner.clear_new();
    }

    /// Reserve a minimum virtual extent without exposing or duplicating the lease.
    ///
    /// Returns the map on both success and ordinary growth refusal. Entries are
    /// preserved; reloading the upstream header refreshes its cached allocator.
    /// Backing traps retain the upstream panic contract.
    ///
    /// `minimum` is an absolute virtual extent in 65,536-byte Wasm pages, not an
    /// entry count or additional pages. Existing extents never shrink. A request
    /// already covered performs no growth or writes. This reserves backing only:
    /// it does not promise that any particular number or encoding of entries fits.
    /// Callers own their byte/page budget and must qualify it for their codecs;
    /// upstream node-layout constants are not a supported capacity contract.
    pub fn reserve_pages(self, minimum: u64) -> (Self, Result<(), crate::RuntimeGrowError>) {
        let memory = self.inner.into_memory();
        let additional = minimum.saturating_sub(memory.size());
        let result = memory.grow(additional).map(|_| ());
        (Self::load(memory), result)
    }

    /// Observe the virtual extent in 65,536-byte pages while retaining the lease.
    ///
    /// Consumes and returns the map because upstream exposes memory only through
    /// ownership transfer. Reloading reads the header without writing or growing.
    /// This is allocated extent, not free space or guaranteed entry capacity.
    #[must_use]
    pub fn allocated_pages(self) -> (Self, u64) {
        let memory = self.inner.into_memory();
        let pages = memory.size();
        (Self::load(memory), pages)
    }

    /// Delegates to upstream `BTreeMap::insert` with the governed allocation.
    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        self.inner.insert(key, value)
    }

    /// Delegates to upstream `BTreeMap::get` with the governed allocation.
    pub fn get(&self, key: &K) -> Option<V> {
        self.inner.get(key)
    }

    /// Delegates to upstream `BTreeMap::contains_key` with the governed allocation.
    pub fn contains_key(&self, key: &K) -> bool {
        self.inner.contains_key(key)
    }

    /// Delegates to upstream `BTreeMap::is_empty` with the governed allocation.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Delegates to upstream `BTreeMap::len` with the governed allocation.
    #[must_use]
    pub fn len(&self) -> u64 {
        self.inner.len()
    }

    /// Delegates to upstream `BTreeMap::remove` with the governed allocation.
    pub fn remove(&mut self, key: &K) -> Option<V> {
        self.inner.remove(key)
    }

    /// Delegates to upstream `BTreeMap::first_key_value` with the governed allocation.
    #[must_use]
    pub fn first_key_value(&self) -> Option<(K, V)> {
        self.inner.first_key_value()
    }

    /// Delegates to upstream `BTreeMap::last_key_value` with the governed allocation.
    #[must_use]
    pub fn last_key_value(&self) -> Option<(K, V)> {
        self.inner.last_key_value()
    }

    /// Delegates to upstream `BTreeMap::pop_first` with the governed allocation.
    pub fn pop_first(&mut self) -> Option<(K, V)> {
        self.inner.pop_first()
    }

    /// Delegates to upstream `BTreeMap::pop_last` with the governed allocation.
    pub fn pop_last(&mut self) -> Option<(K, V)> {
        self.inner.pop_last()
    }

    /// Delegates to upstream `BTreeMap::iter` with the governed allocation.
    #[must_use]
    pub fn iter(&self) -> MapIter<'_, K, V, M> {
        self.inner.iter()
    }

    /// Delegates to upstream `BTreeMap::range` with the governed allocation.
    pub fn range(&self, key_range: impl RangeBounds<K>) -> MapIter<'_, K, V, M> {
        self.inner.range(key_range)
    }

    /// Delegates to upstream `BTreeMap::keys` with the governed allocation.
    pub fn keys(&self) -> impl DoubleEndedIterator<Item = K> + '_ {
        self.inner.keys()
    }

    /// Delegates to upstream `BTreeMap::values` with the governed allocation.
    pub fn values(&self) -> impl DoubleEndedIterator<Item = V> + '_ {
        self.inner.values()
    }
}

impl<'a, K: Storable + Ord + Clone, V: Storable, M: Memory> IntoIterator
    for &'a StableBTreeMap<K, V, M>
{
    type Item = <MapIter<'a, K, V, M> as Iterator>::Item;
    type IntoIter = MapIter<'a, K, V, M>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

//! Governed map collection implementation.
use super::tree::Tree;
use super::{MapIter, Memory, Storable};
use crate::{DefaultMemoryImpl, RuntimeMemory};
use std::{collections::BTreeSet, ops::RangeBounds};

/// Allocation policy for one B-tree. The map persists this choice on creation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BTreeMapOptions {
    /// Target reservation for an encoded node. Bounded codecs can use a smaller
    /// target. The default is 1,024 bytes and the minimum is 256.
    pub node_target_bytes: u16,
    /// Smallest allocator block in bytes. Successive block sizes double.
    /// The default and minimum are 256. For framed 1 MiB values, 257 saves
    /// space by allowing the payload and frame to fit in 4,096 units.
    pub allocation_unit_bytes: u16,
}

impl Default for BTreeMapOptions {
    fn default() -> Self {
        Self {
            node_target_bytes: 1024,
            allocation_unit_bytes: 256,
        }
    }
}

impl BTreeMapOptions {
    pub(super) fn valid(self) -> bool {
        self.node_target_bytes >= 256 && self.allocation_unit_bytes >= 256
    }
}

///
/// StableBTreeMap
/// Degree-six ordered B-tree over an exclusive allocation.
///

pub struct StableBTreeMap<K: Storable + Ord + Clone, V: Storable, M: Memory = DefaultMemoryImpl> {
    inner: Tree<K, V, RuntimeMemory<M>>,
}

impl<K: Storable + Ord + Clone, V: Storable, M: Memory> StableBTreeMap<K, V, M> {
    /// Create a collection only in zero-page memory.
    ///
    /// # Panics
    /// Refuses nonempty memory before writing. Growth and codec failures trap.
    #[must_use]
    pub fn create(memory: RuntimeMemory<M>) -> Self {
        Self::create_with_options(memory, BTreeMapOptions::default())
    }

    /// Create an empty B-tree with per-allocation sizing options.
    ///
    /// # Panics
    /// Refuses invalid options or occupied memory before writing.
    #[must_use]
    pub fn create_with_options(memory: RuntimeMemory<M>, options: BTreeMapOptions) -> Self {
        assert_eq!(memory.size(), 0, "collection memory is not empty");

        Self::reset_with_options(memory, options)
    }

    /// Deliberately replace any contents in this admitted allocation.
    ///
    /// # Panics
    /// Growth and codec failures trap.
    #[must_use]
    pub fn reset(memory: RuntimeMemory<M>) -> Self {
        Self::reset_with_options(memory, BTreeMapOptions::default())
    }

    /// Deliberately replace this allocation using the supplied sizing options.
    ///
    /// # Panics
    /// Invalid options are refused before writing. Backing failures trap.
    #[must_use]
    pub fn reset_with_options(memory: RuntimeMemory<M>, options: BTreeMapOptions) -> Self {
        Self {
            inner: Tree::reset(memory, options).expect("cannot create B-tree"),
        }
    }

    /// Load retained collection state without initializing a replacement.
    ///
    /// # Panics
    /// Refuses empty or foreign memory and incompatible layouts/bounds without
    /// writing. Record decoding and backing failures trap.
    #[must_use]
    pub fn load(memory: RuntimeMemory<M>) -> Self {
        Self {
            inner: Tree::load(memory).expect("invalid B-tree"),
        }
    }

    /// Return the allocation policy recovered from this map’s header.
    #[must_use]
    pub fn options(&self) -> BTreeMapOptions {
        self.inner.options()
    }

    /// Clear entries while retaining this allocation's exclusive lease.
    ///
    /// # Panics
    /// Panics on backing, record integrity, or codec failure.
    pub fn clear(&mut self) {
        self.inner.clear().expect("cannot clear B-tree");
    }

    /// Reserve a minimum virtual extent without exposing or duplicating the lease.
    ///
    /// Returns the map on both success and ordinary growth refusal. The reservation preserves all entries.
    ///
    /// `minimum` is an absolute virtual extent in 65,536-byte Wasm pages, not an
    /// entry count or additional pages. Existing extents never shrink. A request
    /// already covered performs no growth or writes. This reserves backing only:
    /// it does not promise that any particular number or encoding of entries fits.
    /// Callers own their byte/page budget and must qualify it for their codecs;
    /// owned node-layout constants are not a supported capacity contract.
    pub fn reserve_pages(self, minimum: u64) -> (Self, Result<(), crate::RuntimeGrowError>) {
        let additional = minimum.saturating_sub(self.inner.memory.size());
        let result = self.inner.memory.grow(additional).map(|_| ());
        (self, result)
    }

    /// Observe the virtual extent in 65,536-byte pages while retaining the lease.
    ///
    /// Consumes and returns the map together with its allocation size.
    /// This is allocated extent, not free space or guaranteed entry capacity.
    #[must_use]
    pub fn allocated_pages(self) -> (Self, u64) {
        let pages = self.inner.memory.size();

        (self, pages)
    }

    /// Insert or replace a value, returning the previous value.
    ///
    /// # Panics
    /// Panics on backing, record integrity, or codec failure.
    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        self.inner.insert(key, value).expect("B-tree insert failed")
    }

    /// Read the value for an exact key.
    ///
    /// # Panics
    /// Panics on backing, record integrity, or codec failure.
    pub fn get(&self, key: &K) -> Option<V> {
        self.inner.get(key).expect("B-tree get failed")
    }

    /// Qualification experiment. Visit sorted distinct probes in order, including misses.
    ///
    /// # Panics
    /// Panics on unordered or duplicate keys, backing, record integrity or codec failure.
    #[cfg(feature = "test-support")]
    pub fn visit_sorted_keys(&self, keys: &[K], visit: impl FnMut(&K, Option<V>)) {
        assert!(keys.windows(2).all(|pair| pair[0] < pair[1]));
        self.inner
            .visit_sorted_keys(keys, visit)
            .expect("B-tree batch read failed");
    }

    /// Check whether the map contains a key.
    ///
    /// # Panics
    /// Panics on backing, record integrity, or codec failure.
    pub fn contains_key(&self, key: &K) -> bool {
        self.inner
            .contains_key(key)
            .expect("B-tree contains_key failed")
    }

    /// Whether the map contains no entries.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Number of committed entries.
    #[must_use]
    pub fn len(&self) -> u64 {
        self.inner.len()
    }

    /// Remove a matching entry and return its value.
    ///
    /// # Panics
    /// Panics on backing, record integrity, or codec failure.
    pub fn remove(&mut self, key: &K) -> Option<V> {
        self.inner.remove(key).expect("B-tree remove failed")
    }

    /// Consume a prepared set of distinct keys and return the number present.
    /// Empty sets, missing keys and retries contribute zero. The caller bounds
    /// the set during admission. Groups of keys share staged writes and root publication.
    /// A caller relying on prior existence checks must trap on a count mismatch
    /// in the same IC update to roll back its writes.
    ///
    /// # Panics
    /// Panics on backing, record integrity, or codec failure.
    pub fn remove_many(&mut self, keys: BTreeSet<K>) -> u64 {
        self.inner
            .remove_many(keys)
            .expect("B-tree remove_many failed")
    }

    /// Read the smallest key and its value.
    #[must_use]
    pub fn first_key_value(&self) -> Option<(K, V)> {
        self.iter().next().map(|e| e.into_pair())
    }

    /// Read the largest key and its value.
    #[must_use]
    pub fn last_key_value(&self) -> Option<(K, V)> {
        self.iter().next_back().map(|e| e.into_pair())
    }

    /// Remove and return the first entry.
    pub fn pop_first(&mut self) -> Option<(K, V)> {
        let key = self.keys().next()?;
        self.remove(&key).map(|value| (key, value))
    }

    /// Remove and return the last entry.
    pub fn pop_last(&mut self) -> Option<(K, V)> {
        let key = self.keys().next_back()?;
        self.remove(&key).map(|value| (key, value))
    }

    /// Iterate over all entries in key order.
    #[must_use]
    pub fn iter(&self) -> MapIter<'_, K, V, M> {
        self.inner.range(..)
    }

    /// Iterate within the supplied key bounds.
    pub fn range(&self, key_range: impl RangeBounds<K>) -> MapIter<'_, K, V, M> {
        self.inner.range(key_range)
    }

    /// Iterate over keys without reading external values.
    pub fn keys(&self) -> impl DoubleEndedIterator<Item = K> + '_ {
        self.keys_range(..)
    }

    /// Iterate over keys within the bounds in either direction, skipping external values.
    pub fn keys_range(
        &self,
        bounds: impl RangeBounds<K>,
    ) -> impl DoubleEndedIterator<Item = K> + '_ {
        self.inner.keys_range(bounds)
    }

    /// Iterate over values in key order.
    pub fn values(&self) -> impl DoubleEndedIterator<Item = V> + '_ {
        self.iter().map(|e| e.into_value())
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

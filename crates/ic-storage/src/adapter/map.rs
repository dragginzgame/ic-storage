//! Governed map operations over the independent ordered engine.
use super::bridge::{self, TreeMemory};
use super::error::{CollectionError as Error, Result, ensure};
use super::{MapIter, Memory, Storable};
use crate::{DefaultMemoryImpl, RuntimeMemory};
use std::ops::RangeBounds;

///
/// `StableBTreeMap`
///
/// Ordered map owning one governed allocation. Stored codec and
/// engine failures trap. Native callers must discard the runtime after a mutation panic.
///
pub struct StableBTreeMap<K: Storable + Ord + Clone, V: Storable, M: Memory = DefaultMemoryImpl> {
    inner: bridge::Map<K, V, M>,
}
impl<K: Storable + Ord + Clone, V: Storable, M: Memory> StableBTreeMap<K, V, M> {
    /// Create only in empty admitted memory.
    /// # Errors
    /// Refuses nonempty memory or initial growth refusal.
    pub fn create(memory: RuntimeMemory<M>) -> Result<Self> {
        ensure(memory.size() == 0, Error::NotEmpty)?;
        Self::reset(memory)
    }
    /// Explicitly replace this allocation's collection image.
    /// # Errors
    /// Refuses initial growth before creating the map.
    pub fn reset(memory: RuntimeMemory<M>) -> Result<Self> {
        bridge::reserve(&memory)?;
        #[cfg(not(target_arch = "wasm32"))]
        let _guard = memory.poison_on_unwind();
        Ok(Self {
            inner: bridge::Map::new(TreeMemory::Allocation(memory)),
        })
    }
    /// Load the current ordered-map format without initialization or migration.
    /// # Errors
    /// Refuses empty or foreign framing.
    pub fn load(memory: RuntimeMemory<M>) -> Result<Self> {
        Ok(Self {
            inner: bridge::Map::load(TreeMemory::Allocation(memory)).ok_or(Error::Format)?,
        })
    }
    /// Clear entries and reuse the map's existing backing capacity.
    pub fn clear(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        let _guard = self.inner.memory().allocation().poison_on_unwind();
        self.inner.clear();
    }
    /// Reserve backing pages without promising entry capacity.
    pub fn reserve_pages(
        self,
        minimum: u64,
    ) -> (Self, std::result::Result<(), crate::RuntimeGrowError>) {
        let additional = minimum.saturating_sub(self.inner.memory().allocation().size());
        let result = self
            .inner
            .memory()
            .allocation()
            .grow(additional)
            .map(|_| ());
        (self, result)
    }
    /// Observe allocated pages while retaining exclusive custody.
    #[must_use]
    pub fn allocated_pages(self) -> (Self, u64) {
        let pages = self.inner.memory().allocation().size();
        (self, pages)
    }
    /// Insert or replace. Invalid codecs and allocation failures trap.
    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        #[cfg(not(target_arch = "wasm32"))]
        let _guard = self.inner.memory().allocation().poison_on_unwind();
        self.inner.insert(key, value)
    }
    /// Read a matching value; malformed stored values trap.
    pub fn get(&self, key: &K) -> Option<V> {
        self.inner.get(key)
    }
    /// Test key membership without decoding values.
    pub fn contains_key(&self, key: &K) -> bool {
        self.inner.contains_key(key)
    }
    /// Whether no entries are committed.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
    /// Number of entries.
    #[must_use]
    pub const fn len(&self) -> u64 {
        self.inner.len()
    }
    /// Remove a matching value; codec and engine failures trap.
    pub fn remove(&mut self, key: &K) -> Option<V> {
        #[cfg(not(target_arch = "wasm32"))]
        let _guard = self.inner.memory().allocation().poison_on_unwind();
        self.inner.remove(key)
    }
    /// First ordered key/value pair.
    #[must_use]
    pub fn first_key_value(&self) -> Option<(K, V)> {
        self.inner.first_key_value()
    }
    /// Last ordered key/value pair.
    #[must_use]
    pub fn last_key_value(&self) -> Option<(K, V)> {
        self.inner.last_key_value()
    }
    /// Iterate in key order.
    #[must_use]
    pub const fn iter(&self) -> MapIter<'_, K, V, M> {
        MapIter(self.inner.iter())
    }
    /// Iterate a bounded key range.
    pub fn range(&self, bounds: impl RangeBounds<K>) -> MapIter<'_, K, V, M> {
        MapIter(self.inner.range(bounds))
    }
    /// Iterate keys without reading value payloads.
    pub fn keys(&self) -> impl DoubleEndedIterator<Item = K> + '_ {
        self.inner.keys()
    }
    /// Iterate bounded keys without reading value payloads.
    pub fn keys_range(
        &self,
        bounds: impl RangeBounds<K>,
    ) -> impl DoubleEndedIterator<Item = K> + '_ {
        self.inner.keys_range(bounds)
    }
    /// Iterate decoded values.
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

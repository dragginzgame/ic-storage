//! Governed set collection implementation.
use super::error::{CollectionError as Error, Result, ensure};
use super::{MapIter, Memory, StableBTreeMap, Storable};
use crate::DefaultMemoryImpl;
use crate::RuntimeMemory;

/// Governed ordered set sharing Storage’s owned map engine with unit values.
pub struct StableBTreeSet<K: Storable + Ord + Clone, M: Memory = DefaultMemoryImpl> {
    inner: StableBTreeMap<K, (), M>,
}
impl<K: Storable + Ord + Clone, M: Memory> StableBTreeSet<K, M> {
    /// Create a collection only in zero-page memory.
    ///
    /// # Errors
    /// Refuses nonempty memory or initial growth refusal; engine failures trap.
    pub fn create(memory: RuntimeMemory<M>) -> Result<Self> {
        ensure(memory.size() == 0, Error::NotEmpty)?;
        Self::reset(memory)
    }

    /// Deliberately replace any contents in this admitted allocation.
    ///
    /// # Errors
    /// Returns initial growth refusal; engine failures trap.
    pub fn reset(memory: RuntimeMemory<M>) -> Result<Self> {
        Ok(Self {
            inner: StableBTreeMap::reset(memory)?,
        })
    }

    /// Load retained collection state without initializing a replacement.
    ///
    /// # Errors
    /// Refuses empty or foreign framing; malformed engine state traps.
    pub fn load(memory: RuntimeMemory<M>) -> Result<Self> {
        Ok(Self {
            inner: StableBTreeMap::load(memory)?,
        })
    }
    /// Perform `insert` on the governed ordered set.
    ///
    /// Engine and codec failures trap.
    pub fn insert(&mut self, key: K) -> bool {
        self.inner.insert(key, ()).is_none()
    }
    /// Perform `contains` on the governed ordered set.
    ///
    /// Engine and codec failures trap.
    pub fn contains(&self, key: &K) -> bool {
        self.inner.contains_key(key)
    }
    /// Perform `is_empty` on the governed ordered set.
    ///
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
    /// Perform `len` on the governed ordered set.
    #[must_use]
    pub const fn len(&self) -> u64 {
        self.inner.len()
    }
    /// Perform `remove` on the governed ordered set.
    ///
    /// Engine and codec failures trap.
    pub fn remove(&mut self, key: &K) -> bool {
        self.inner.remove(key).is_some()
    }
    /// Perform `iter` on the governed ordered set.
    #[must_use]
    pub const fn iter(&self) -> SetIter<'_, K, M> {
        SetIter(self.inner.iter())
    }
}

impl<'a, K: Storable + Ord + Clone, M: Memory> IntoIterator for &'a StableBTreeSet<K, M> {
    type Item = K;
    type IntoIter = SetIter<'a, K, M>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

///
/// `SetIter`
///
/// Lazy bidirectional key traversal over the shared ordered-tree engine.
///
pub struct SetIter<'a, K: Storable + Ord + Clone, M: Memory>(MapIter<'a, K, (), M>);
impl<K: Storable + Ord + Clone, M: Memory> Iterator for SetIter<'_, K, M> {
    type Item = K;
    fn next(&mut self) -> Option<K> {
        self.0.next().map(super::MapEntry::into_key)
    }
    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        self.0.nth(n).map(super::MapEntry::into_key)
    }
    fn count(self) -> usize {
        self.0.count()
    }
    fn last(self) -> Option<Self::Item> {
        self.0.last().map(super::MapEntry::into_key)
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}
impl<K: Storable + Ord + Clone, M: Memory> DoubleEndedIterator for SetIter<'_, K, M> {
    fn next_back(&mut self) -> Option<K> {
        self.0.next_back().map(super::MapEntry::into_key)
    }
    fn nth_back(&mut self, n: usize) -> Option<Self::Item> {
        self.0.nth_back(n).map(super::MapEntry::into_key)
    }
}

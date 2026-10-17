//! Supported upstream collections with governed memory and an explicit API.
//!
//! Constructors preserve upstream initialization, panic and persisted-format
//! contracts. Mutations are not our custom tree's staged transactions. These
//! wrappers expose neither raw memory nor unrestricted upstream collections.
//! Use distinct admitted allocations and drop collections before reopening.
//! IC Memory 0.46 enforces one live lease per allocation within each runtime;
//! hosts retain exclusive custody of the backing across runtimes.
use crate::substrate::{self, DefaultMemoryImpl, Memory};
pub use crate::{BTreeMap, Comparator, Config, Entry, Error, Lexicographic, Range};
use ic_memory::RuntimeMemory;
use std::{borrow::Cow, ops::RangeBounds};
use substrate::storable::Bound;
pub use substrate::{Storable, storable};

/// Governed upstream `BTreeMap`; initialization and failures follow upstream contracts.
pub struct StableBTreeMap<K: Storable + Ord + Clone, V: Storable, M: Memory = DefaultMemoryImpl> {
    inner: substrate::BTreeMap<K, V, RuntimeMemory<M>>,
}
impl<K: Storable + Ord + Clone, V: Storable, M: Memory> StableBTreeMap<K, V, M> {
    /// Delegates to upstream `BTreeMap::new` with the governed allocation.
    pub fn new(memory: RuntimeMemory<M>) -> Self {
        Self {
            inner: substrate::BTreeMap::new(memory),
        }
    }
    /// Delegates to upstream `BTreeMap::init` with the governed allocation.
    pub fn init(memory: RuntimeMemory<M>) -> Self {
        Self {
            inner: substrate::BTreeMap::init(memory),
        }
    }
    /// Delegates to upstream `BTreeMap::load` with the governed allocation.
    pub fn load(memory: RuntimeMemory<M>) -> Self {
        Self {
            inner: substrate::BTreeMap::load(memory),
        }
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
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
    /// Delegates to upstream `BTreeMap::len` with the governed allocation.
    pub fn len(&self) -> u64 {
        self.inner.len()
    }
    /// Delegates to upstream `BTreeMap::remove` with the governed allocation.
    pub fn remove(&mut self, key: &K) -> Option<V> {
        self.inner.remove(key)
    }
    /// Delegates to upstream `BTreeMap::first_key_value` with the governed allocation.
    pub fn first_key_value(&self) -> Option<(K, V)> {
        self.inner.first_key_value()
    }
    /// Delegates to upstream `BTreeMap::last_key_value` with the governed allocation.
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

/// Governed upstream `BTreeSet`; initialization and failures follow upstream contracts.
pub struct StableBTreeSet<K: Storable + Ord + Clone, M: Memory = DefaultMemoryImpl> {
    inner: substrate::BTreeSet<K, RuntimeMemory<M>>,
}
impl<K: Storable + Ord + Clone, M: Memory> StableBTreeSet<K, M> {
    /// Delegates to upstream `BTreeSet::new` with the governed allocation.
    pub fn new(memory: RuntimeMemory<M>) -> Self {
        Self {
            inner: substrate::BTreeSet::new(memory),
        }
    }
    /// Delegates to upstream `BTreeSet::init` with the governed allocation.
    pub fn init(memory: RuntimeMemory<M>) -> Self {
        Self {
            inner: substrate::BTreeSet::init(memory),
        }
    }
    /// Delegates to upstream `BTreeSet::load` with the governed allocation.
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
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
    /// Delegates to upstream `BTreeSet::len` with the governed allocation.
    pub fn len(&self) -> u64 {
        self.inner.len()
    }
    /// Delegates to upstream `BTreeSet::remove` with the governed allocation.
    pub fn remove(&mut self, key: &K) -> bool {
        self.inner.remove(key)
    }
    /// Delegates to upstream `BTreeSet::first` with the governed allocation.
    pub fn first(&self) -> Option<K> {
        self.inner.first()
    }
    /// Delegates to upstream `BTreeSet::last` with the governed allocation.
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

/// Governed upstream `Cell`; initialization and failures follow upstream contracts.
pub struct StableCell<T: Storable, M: Memory = DefaultMemoryImpl> {
    inner: substrate::Cell<T, RuntimeMemory<M>>,
}
impl<T: Storable, M: Memory> StableCell<T, M> {
    /// Delegates to upstream `Cell::new` with the governed allocation.
    pub fn new(memory: RuntimeMemory<M>, value: T) -> Self {
        Self {
            inner: substrate::Cell::new(memory, value),
        }
    }
    /// Delegates to upstream `Cell::init` with the governed allocation.
    pub fn init(memory: RuntimeMemory<M>, default_value: T) -> Self {
        Self {
            inner: substrate::Cell::init(memory, default_value),
        }
    }
    /// Delegates to upstream `Cell::get` with the governed allocation.
    pub fn get(&self) -> &T {
        self.inner.get()
    }
    /// Delegates to upstream `Cell::set` with the governed allocation.
    pub fn set(&mut self, value: T) -> T {
        self.inner.set(value)
    }
}

/// Governed upstream `Vec`; initialization and failures follow upstream contracts.
pub struct StableVec<T: Storable, M: Memory = DefaultMemoryImpl> {
    inner: substrate::Vec<T, RuntimeMemory<M>>,
}
impl<T: Storable, M: Memory> StableVec<T, M> {
    /// Delegates to upstream `Vec::new` with the governed allocation.
    pub fn new(memory: RuntimeMemory<M>) -> Self {
        Self {
            inner: substrate::Vec::new(memory),
        }
    }
    /// Delegates to upstream `Vec::init` with the governed allocation.
    pub fn init(memory: RuntimeMemory<M>) -> Self {
        Self {
            inner: substrate::Vec::init(memory),
        }
    }
    /// Delegates to upstream `Vec::get` with the governed allocation.
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
    pub fn pop(&self) -> Option<T> {
        self.inner.pop()
    }
    /// Delegates to upstream `Vec::len` with the governed allocation.
    pub fn len(&self) -> u64 {
        self.inner.len()
    }
    /// Delegates to upstream `Vec::is_empty` with the governed allocation.
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
    /// Delegates to upstream `Vec::iter` with the governed allocation.
    pub fn iter(&self) -> impl DoubleEndedIterator<Item = T> + '_ {
        self.inner.iter()
    }
}

/// Governed upstream `MinHeap`; initialization and failures follow upstream contracts.
pub struct StableMinHeap<T: Storable + Ord, M: Memory = DefaultMemoryImpl> {
    inner: substrate::MinHeap<T, RuntimeMemory<M>>,
}
impl<T: Storable + Ord, M: Memory> StableMinHeap<T, M> {
    /// Delegates to upstream `MinHeap::new` with the governed allocation.
    pub fn new(memory: RuntimeMemory<M>) -> Self {
        Self {
            inner: substrate::MinHeap::new(memory),
        }
    }
    /// Delegates to upstream `MinHeap::init` with the governed allocation.
    pub fn init(memory: RuntimeMemory<M>) -> Self {
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
    pub fn peek(&self) -> Option<T> {
        self.inner.peek()
    }
    /// Delegates to upstream `MinHeap::len` with the governed allocation.
    pub fn len(&self) -> u64 {
        self.inner.len()
    }
    /// Delegates to upstream `MinHeap::is_empty` with the governed allocation.
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
    /// Delegates to upstream `MinHeap::iter` with the governed allocation.
    pub fn iter(&self) -> impl Iterator<Item = T> + '_ {
        self.inner.iter()
    }
}

/// Governed upstream `Log`; initialization and failures follow upstream contracts.
pub struct StableLog<T: Storable, M: Memory = DefaultMemoryImpl> {
    inner: substrate::Log<T, RuntimeMemory<M>, RuntimeMemory<M>>,
}
impl<T: Storable, M: Memory> StableLog<T, M> {
    /// Delegates to upstream `Log::new` with the governed allocation.
    pub fn new(index_memory: RuntimeMemory<M>, data_memory: RuntimeMemory<M>) -> Self {
        Self {
            inner: substrate::Log::new(index_memory, data_memory),
        }
    }
    /// Delegates to upstream `Log::init` with the governed allocation.
    pub fn init(index_memory: RuntimeMemory<M>, data_memory: RuntimeMemory<M>) -> Self {
        Self {
            inner: substrate::Log::init(index_memory, data_memory),
        }
    }
    /// Delegates to upstream `Log::first` with the governed allocation.
    pub fn first(&self) -> Option<T> {
        self.inner.first()
    }
    /// Delegates to upstream `Log::last` with the governed allocation.
    pub fn last(&self) -> Option<T> {
        self.inner.last()
    }
    /// Delegates to upstream `Log::get` with the governed allocation.
    pub fn get(&self, idx: u64) -> Option<T> {
        self.inner.get(idx)
    }
    /// Delegates to upstream `Log::len` with the governed allocation.
    pub fn len(&self) -> u64 {
        self.inner.len()
    }
    /// Delegates to upstream `Log::is_empty` with the governed allocation.
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
    /// Delegates to upstream `Log::append` with the governed allocation.
    /// # Errors
    /// Returns the upstream write error if memory cannot grow.
    pub fn append(&self, item: &T) -> Result<u64, LogWriteError> {
        self.inner.append(item)
    }
    /// Delegates to upstream `Log::iter` with the governed allocation.
    pub fn iter(&self) -> LogIter<'_, T, M> {
        self.inner.iter()
    }
}

/// Upstream map iterator with governed memory.
pub type MapIter<'a, K, V, M = DefaultMemoryImpl> =
    substrate::btreemap::Iter<'a, K, V, RuntimeMemory<M>>;
/// Upstream set iterator with governed memory.
pub type SetIter<'a, K, M = DefaultMemoryImpl> = substrate::btreeset::Iter<'a, K, RuntimeMemory<M>>;
/// Upstream log iterator with governed index and data memories.
pub type LogIter<'a, T, M = DefaultMemoryImpl> =
    substrate::log::Iter<'a, T, RuntimeMemory<M>, RuntimeMemory<M>>;

pub use substrate::log::WriteError as LogWriteError;

impl<'a, K: Storable + Ord + Clone, V: Storable, M: Memory> IntoIterator
    for &'a StableBTreeMap<K, V, M>
{
    type Item = <MapIter<'a, K, V, M> as Iterator>::Item;
    type IntoIter = MapIter<'a, K, V, M>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, K: Storable + Ord + Clone, M: Memory> IntoIterator for &'a StableBTreeSet<K, M> {
    type Item = K;
    type IntoIter = SetIter<'a, K, M>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, T: Storable, M: Memory> IntoIterator for &'a StableLog<T, M> {
    type Item = T;
    type IntoIter = LogIter<'a, T, M>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

/// A stable cell whose writes enforce the record's declared `Storable` bound.
pub struct BoundedCell<T: Storable, M: Memory = DefaultMemoryImpl> {
    cell: substrate::Cell<BoundedValue<T>, RuntimeMemory<M>>,
}

impl<T: Storable, M: Memory> BoundedCell<T, M> {
    /// Open a current cell or initialize a fresh bounded record.
    ///
    /// # Panics
    /// Panics for unbounded records or encodings that violate the declared bound.
    #[must_use]
    pub fn init(memory: RuntimeMemory<M>, value: T) -> Self {
        assert!(matches!(T::BOUND, Bound::Bounded { .. }));
        Self {
            cell: substrate::Cell::init(memory, BoundedValue(value)),
        }
    }

    /// Read the cached current record without serializing or cloning it.
    #[must_use]
    pub fn get(&self) -> &T {
        &self.cell.get().0
    }

    /// Validate and persist a complete replacement, returning the prior record.
    ///
    /// # Panics
    /// Panics before writing if the encoding violates its declared bound.
    pub fn set(&mut self, value: T) -> T {
        self.cell.set(BoundedValue(value)).0
    }
}

struct BoundedValue<T>(T);

impl<T: Storable> Storable for BoundedValue<T> {
    const BOUND: Bound = T::BOUND;

    fn to_bytes(&self) -> Cow<'_, [u8]> {
        self.0.to_bytes_checked()
    }

    fn into_bytes(self) -> Vec<u8> {
        self.0.into_bytes_checked()
    }

    fn from_bytes(bytes: Cow<'_, [u8]>) -> Self {
        let Bound::Bounded {
            max_size,
            is_fixed_size,
        } = T::BOUND
        else {
            unreachable!("bounded cell record must declare a bound");
        };
        if is_fixed_size {
            assert_eq!(bytes.len(), max_size as usize);
        } else {
            assert!(bytes.len() <= max_size as usize);
        }
        Self(T::from_bytes(bytes))
    }
}

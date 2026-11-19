//! Governed log collection implementation.
use super::{LogIter, LogWriteError, Memory, Storable};
use crate::{DefaultMemoryImpl, RuntimeMemory, substrate};

///
/// StableLog
/// Governed upstream `Log`; initialization and failures follow upstream contracts.
///

pub struct StableLog<T: Storable, M: Memory = DefaultMemoryImpl> {
    inner: substrate::Log<T, RuntimeMemory<M>, RuntimeMemory<M>>,
}

impl<T: Storable, M: Memory> StableLog<T, M> {
    /// Create a log only when both admitted memories have zero pages.
    ///
    /// # Panics
    /// Refuses either nonempty memory before writing. Growth failures retain
    /// upstream behavior.
    #[must_use]
    pub fn create(index_memory: RuntimeMemory<M>, data_memory: RuntimeMemory<M>) -> Self {
        assert_eq!(index_memory.size(), 0, "log index is not empty");
        assert_eq!(data_memory.size(), 0, "log data is not empty");

        Self::reset(index_memory, data_memory)
    }

    /// Deliberately replace both admitted log allocations.
    ///
    /// # Panics
    /// Growth and backing failures retain upstream behavior.
    #[must_use]
    pub fn reset(index_memory: RuntimeMemory<M>, data_memory: RuntimeMemory<M>) -> Self {
        Self {
            inner: substrate::Log::new(index_memory, data_memory),
        }
    }

    /// Load a retained log without initializing either allocation.
    ///
    /// # Panics
    /// Refuses empty/foreign memory and incompatible layouts without writing.
    /// Backing and collection corruption failures retain upstream behavior.
    #[must_use]
    pub fn load(index_memory: RuntimeMemory<M>, data_memory: RuntimeMemory<M>) -> Self {
        assert!(
            index_memory.size() > 0 && data_memory.size() > 0,
            "log memory is empty"
        );
        let mut magic = [0; 3];
        data_memory.read(0, &mut magic);
        assert_eq!(&magic, b"GLD", "foreign log data");

        // With recognized data magic upstream refuses every other incompatible
        // header before writes, including invalid/missing index magic.
        Self {
            inner: substrate::Log::init(index_memory, data_memory),
        }
    }

    /// Delegates to upstream `Log::first` with the governed allocation.
    #[must_use]
    pub fn first(&self) -> Option<T> {
        self.inner.first()
    }

    /// Delegates to upstream `Log::last` with the governed allocation.
    #[must_use]
    pub fn last(&self) -> Option<T> {
        self.inner.last()
    }

    /// Delegates to upstream `Log::get` with the governed allocation.
    #[must_use]
    pub fn get(&self, idx: u64) -> Option<T> {
        self.inner.get(idx)
    }

    /// Delegates to upstream `Log::len` with the governed allocation.
    #[must_use]
    pub fn len(&self) -> u64 {
        self.inner.len()
    }

    /// Delegates to upstream `Log::is_empty` with the governed allocation.
    #[must_use]
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
    #[must_use]
    pub fn iter(&self) -> LogIter<'_, T, M> {
        self.inner.iter()
    }
}

impl<'a, T: Storable, M: Memory> IntoIterator for &'a StableLog<T, M> {
    type Item = T;
    type IntoIter = LogIter<'a, T, M>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

//! Governed vector over the copied ICSS fixed-stride engine.
use super::{
    Memory, Storable,
    bridge::{self, Backing, Codec},
    error::{CollectionError as Error, Result, ensure},
};
use crate::engine::vec::InitError;
use crate::{DefaultMemoryImpl, RuntimeMemory, engine};

///
/// `StableVec`
///
/// ICSS-baseline fixed-stride vector in one governed allocation. There are no
/// custom checksums, cached length, read windows or mutation epochs. Malformed
/// stored values trap; IC execution rollback supplies atomicity.
///
pub struct StableVec<T: Storable, M: Memory = DefaultMemoryImpl> {
    inner: engine::vec::Vec<Codec<T>, Backing<RuntimeMemory<M>>>,
}
impl<T: Storable, M: Memory> StableVec<T, M> {
    /// Create only in empty admitted memory.
    /// # Errors
    /// Refuses nonempty memory or initial growth refusal.
    pub fn create(memory: RuntimeMemory<M>) -> Result<Self> {
        ensure(memory.size() == 0, Error::NotEmpty)?;
        Self::reset(memory)
    }
    /// Explicitly replace this vector image.
    /// # Errors
    /// Refuses initial growth; invalid bounds/codecs trap.
    pub fn reset(memory: RuntimeMemory<M>) -> Result<Self> {
        bridge::reserve(&memory)?;
        #[cfg(not(target_arch = "wasm32"))]
        let _guard = memory.poison_on_unwind();
        Ok(Self {
            inner: engine::vec::Vec::new(Backing(memory)),
        })
    }
    /// Load the current SVC format without initialization.
    /// # Errors
    /// Refuses empty/foreign framing.
    /// # Panics
    /// Traps if the persisted element bounds do not match the selected type.
    pub fn load(memory: RuntimeMemory<M>) -> Result<Self> {
        ensure(memory.size() != 0, Error::Format)?;
        let inner = match engine::vec::Vec::init(Backing(memory)) {
            Ok(vector) => vector,
            Err(InitError::BadMagic { .. } | InitError::IncompatibleVersion(_)) => {
                return Err(Error::Format);
            }
            Err(error) => panic!("Failed to initialize a vector: {error:?}"),
        };
        Ok(Self { inner })
    }
    /// Read one indexed record; malformed stored bytes trap.
    /// # Errors
    /// Engine failures trap instead of returning an ordinary error.
    pub fn get(&self, index: u64) -> Result<Option<T>> {
        Ok(self.inner.get(index).map(|v| v.0))
    }
    /// Replace one record; invalid codecs, bounds or writes trap.
    /// # Errors
    /// Engine failures trap instead of returning an ordinary error.
    pub fn set(&self, index: u64, item: &T) -> Result<()> {
        #[cfg(not(target_arch = "wasm32"))]
        let _guard = self.inner.memory().0.poison_on_unwind();
        self.inner.set(index, Codec::borrowed(item));
        Ok(())
    }
    /// Append one record, publishing its length last.
    /// # Errors
    /// Engine failures trap instead of returning an ordinary error.
    pub fn push(&self, item: &T) -> Result<()> {
        #[cfg(not(target_arch = "wasm32"))]
        let _guard = self.inner.memory().0.poison_on_unwind();
        self.inner.push(Codec::borrowed(item));
        Ok(())
    }
    /// Remove the final record; failures trap.
    /// # Errors
    /// Engine failures trap instead of returning an ordinary error.
    pub fn pop(&self) -> Result<Option<T>> {
        #[cfg(not(target_arch = "wasm32"))]
        let _guard = self.inner.memory().0.poison_on_unwind();
        Ok(self.inner.pop().map(|v| v.0))
    }
    /// Read the current committed length from stable memory.
    #[must_use]
    pub fn len(&self) -> u64 {
        self.inner.len()
    }
    /// Whether there are no records.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
    /// Bidirectional iteration with one reusable buffer. Direct skips and `last()`
    /// decode only the selected record; they do not validate discarded records.
    pub fn iter(&self) -> impl DoubleEndedIterator<Item = Result<T>> + '_ {
        Iter(self.inner.iter())
    }
}

// A general Map adaptor executes its closure for skipped elements. Forward
// skipping/counting directly so the facade retains ICSS's specialized methods.
struct Iter<I>(I);
impl<T, I: Iterator<Item = Codec<T>>> Iterator for Iter<I> {
    type Item = Result<T>;
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|v| Ok(v.0))
    }
    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        self.0.nth(n).map(|v| Ok(v.0))
    }
    fn count(self) -> usize {
        self.0.count()
    }
    fn last(self) -> Option<Self::Item> {
        self.0.last().map(|v| Ok(v.0))
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}
impl<T, I: DoubleEndedIterator<Item = Codec<T>>> DoubleEndedIterator for Iter<I> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.0.next_back().map(|v| Ok(v.0))
    }
    fn nth_back(&mut self, n: usize) -> Option<Self::Item> {
        self.0.nth_back(n).map(|v| Ok(v.0))
    }
}
// This suite meters host IO and injects corrupt bytes through native-only views.
// Installed Wasm qualification exercises the production Vector separately.
#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests;

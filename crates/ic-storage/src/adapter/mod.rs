//! Supported upstream collections with governed memory and an explicit API.
//!
//! Constructors preserve upstream initialization, panic and persisted-format
//! contracts. These wrappers expose neither raw memory nor unrestricted
//! upstream collections.
//! Use distinct admitted allocations for owning collections. Region views retain
//! a shared lease for disjoint application frames and reload roots per operation.
//! Drop collections and all shared lease owners before reopening.
//! Storage enforces one live lease per allocation within each runtime;
//! hosts retain exclusive custody of the backing across runtimes.
/// Exact substrate interfaces for backing bounds and record codecs.
/// Implementing `Memory` grants no allocation authority; constructors require
/// committed `RuntimeMemory` leases.
pub use crate::Memory;
use crate::RuntimeMemory;
use crate::{DefaultMemoryImpl, substrate};
pub use substrate::{Storable, storable, storable::Bound};

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests;

mod region;
pub use region::{BTreeMapRegion, RegionGrowError};

mod map;
pub use map::StableBTreeMap;
mod set;
pub use set::StableBTreeSet;
mod cell;
pub use cell::StableCell;
mod vector;
pub use vector::StableVec;
mod heap;
pub use heap::StableMinHeap;
mod log;
pub use log::StableLog;

/// Upstream map iterator with governed memory.
pub type MapIter<'a, K, V, M = DefaultMemoryImpl> =
    substrate::btreemap::Iter<'a, K, V, RuntimeMemory<M>>;
/// Upstream set iterator with governed memory.
pub type SetIter<'a, K, M = DefaultMemoryImpl> = substrate::btreeset::Iter<'a, K, RuntimeMemory<M>>;
/// Upstream log iterator with governed index and data memories.
pub type LogIter<'a, T, M = DefaultMemoryImpl> =
    substrate::log::Iter<'a, T, RuntimeMemory<M>, RuntimeMemory<M>>;

pub use substrate::log::WriteError as LogWriteError;

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

mod cell;
mod heap;
mod log;
mod map;
mod region;
mod set;
mod vector;

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests;

/// Exact substrate interfaces for backing bounds and record codecs.
/// Implementing `Memory` grants no allocation authority; constructors require
/// committed `RuntimeMemory` leases.
pub use crate::Memory;
use crate::{DefaultMemoryImpl, RuntimeMemory, substrate};
pub use substrate::{
    Storable,
    log::WriteError as LogWriteError,
    storable::{self, Bound},
};

pub use cell::StableCell;
pub use heap::StableMinHeap;
pub use log::StableLog;
pub use map::StableBTreeMap;
pub use region::{BTreeMapRegion, RegionGrowError};
pub use set::StableBTreeSet;
pub use vector::StableVec;

/// Upstream map iterator with governed memory.
pub type MapIter<'a, K, V, M = DefaultMemoryImpl> =
    substrate::btreemap::Iter<'a, K, V, RuntimeMemory<M>>;
/// Upstream set iterator with governed memory.
pub type SetIter<'a, K, M = DefaultMemoryImpl> = substrate::btreeset::Iter<'a, K, RuntimeMemory<M>>;
/// Upstream log iterator with governed index and data memories.
pub type LogIter<'a, T, M = DefaultMemoryImpl> =
    substrate::log::Iter<'a, T, RuntimeMemory<M>, RuntimeMemory<M>>;

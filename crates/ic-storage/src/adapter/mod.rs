//! Governed collections with an explicit API.
//!
//! Storage owns every collection engine and its current format. Constructors
//! enforce explicit creation and loading without exposing raw backing.
//! Use distinct admitted allocations for owning collections. Region views retain
//! a shared lease for disjoint application frames and reload roots per operation.
//! Drop collections and all shared lease owners before reopening.
//! Framing admission and explicit reservation can return typed errors. Collection
//! engine and codec failures trap. Ordinary successful calls return values directly;
//! native callers must discard the runtime after a mutation panic.
//! The allocation runtime enforces one live lease per allocation;
//! hosts retain exclusive custody of the backing across runtimes.
// Collection APIs require public documentation in addition to workspace Clippy.
#![warn(clippy::all, clippy::pedantic, missing_docs)]
use crate::DefaultMemoryImpl;
pub use crate::storable::{Bound, CodecError, Storable};
/// Storage-owned interfaces for backing bounds and record codecs.
/// Implementing `Memory` grants no allocation authority; constructors require
/// committed `RuntimeMemory` leases.
pub use crate::{Memory, storable};

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests;

mod error;
pub use error::CollectionError;
mod bridge;
mod iteration;

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

/// Ordered map iterator retaining the collection borrow.
pub use iteration::{MapEntry, MapIter};
/// Ordered set iterator retaining the collection borrow.
pub type SetIter<'a, K, M = DefaultMemoryImpl> = set::SetIter<'a, K, M>;

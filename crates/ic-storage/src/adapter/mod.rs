//! Storage-owned collections over exclusive named allocations.
mod cell;
mod error;
mod map;
mod record;
#[cfg(test)]
mod tests;
mod tree;
pub use crate::{Memory, storable};
pub use cell::StableCell;
pub use map::{BTreeMapOptions, StableBTreeMap};
pub use storable::{Bound, Storable};
pub use tree::MapEntry;
/// Bidirectional ordered iterator over a governed map.
pub type MapIter<'a, K, V, M = crate::DefaultMemoryImpl> =
    tree::TreeIter<'a, K, V, crate::RuntimeMemory<M>>;

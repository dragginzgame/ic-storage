//! Owned named-allocation directory and durable extent validation.
pub(super) mod leases;
mod persistence;

use crate::StableKey;
use serde::{Deserialize, Serialize};

///
/// `StorageConfig`
///
/// Resource ceilings checked when creating, opening and extending the directory.
/// Physical page accounting includes reserved metadata and bucket slack.
/// Table capacities are fixed when creating an image; reopening may impose
/// smaller live-usage limits but cannot increase the image capacities.
///

#[derive(Clone, Copy, Debug)]
pub struct StorageConfig {
    /// Maximum retained named allocations, including currently omitted names.
    pub allocations: usize,
    /// Maximum allocated eight-page buckets across every allocation.
    pub extents: usize,
    /// Maximum physical backing extent, in 64 KiB Wasm pages.
    pub pages: u64,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            allocations: 4_096,
            extents: 16_384,
            pages: 1_048_576,
        }
    }
}

///
/// `AllocationId`
///
/// Opaque durable identity assigned by the directory. It grants no access.
///

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct AllocationId(u64);
impl AllocationId {
    /// Opaque diagnostic identity; callers cannot choose an allocation ID.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

///
/// `Error`
///
/// Admission, bounds and custody failures from the governed allocation directory.
///

#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Logical page count overflowed before any backing access.
    #[error("allocation page count overflow")]
    ArithmeticOverflow,
    /// The requested allocation exceeds the configured directory resource budget.
    #[error("directory budget exceeded")]
    Budget,
    /// Persisted metadata or allocation geometry failed validation.
    #[error("invalid directory: {0}")]
    Invalid(&'static str),
    /// An exclusive lease for the allocation already exists.
    #[error("allocation already has a live lease")]
    AlreadyOpen,
    /// The requested byte range lies outside the allocation.
    #[error("access exceeds allocation bounds")]
    Bounds,
    /// Another owner retains custody of the backing.
    #[error("backing already has a live owner")]
    AlreadyClaimed,
    /// The requested opaque identity has no directory entry.
    #[error("allocation does not exist")]
    Missing,
    /// Physical backing growth failed before the requested capacity was available.
    #[error("backing growth refused")]
    Growth,
    /// Earlier interrupted backing IO invalidated this owner's in-memory authority.
    #[error("directory owner is poisoned after interrupted backing IO")]
    Poisoned,
}

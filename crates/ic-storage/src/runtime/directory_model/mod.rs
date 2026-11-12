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

#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Logical page count overflowed before any backing access.
    #[error("allocation page count overflow")]
    ArithmeticOverflow,
    #[error("directory budget exceeded")]
    Budget,
    #[error("invalid directory: {0}")]
    Invalid(&'static str),
    #[error("allocation already has a live lease")]
    AlreadyOpen,
    #[error("access exceeds allocation bounds")]
    Bounds,
    #[error("backing already has a live owner")]
    AlreadyClaimed,
    #[error("allocation does not exist")]
    Missing,
    #[error("backing growth refused")]
    Growth,
    #[error("directory owner is poisoned after interrupted backing IO")]
    Poisoned,
}

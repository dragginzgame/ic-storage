//! Owned named-allocation directory and durable extent validation.
pub(super) mod leases;
mod persistence;

use crate::{StableKey, hash};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

const FRAME: &[u8] = b"ICDIRR01";

///
/// `StorageConfig`
///
/// Resource ceilings checked when creating, opening and extending the directory.
/// Physical page accounting includes retained metadata and abandoned tails.
///

#[derive(Clone, Copy, Debug)]
pub struct StorageConfig {
    /// Maximum retained named allocations, including currently omitted names.
    pub allocations: usize,
    /// Maximum total data extents across every allocation.
    pub extents: usize,
    /// Maximum physical backing extent, in 64 KiB Wasm pages.
    pub pages: u64,
    /// Encoding and decoding budgets for each directory snapshot.
    pub wire: ic_wire::Limits,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            allocations: 4_096,
            extents: 16_384,
            pages: 1_048_576,
            wire: ic_wire::Limits {
                bytes: 4 * 1024 * 1024,
                allocation: 16 * 1024 * 1024,
                nodes: 262_144,
                depth: 16,
            },
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

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct Extent {
    start: u64,
    pub pages: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct Entry {
    id: AllocationId,
    key: StableKey,
    // Logical page order is explicit; physical ranges need not be contiguous.
    extents: Vec<Extent>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct Image {
    generation: u64,
    next_id: u64,
    // High-water mark, including space reserved for directory persistence.
    next_page: u64,
    // Reserved snapshot arenas, including the active arena. Never application data.
    metadata: Vec<Extent>,
    entries: Vec<Entry>,
}

impl Image {
    fn encode(&self, budget: StorageConfig) -> Result<Vec<u8>, Error> {
        let mut limits = budget.wire;
        limits.bytes = limits.bytes.checked_sub(FRAME.len()).ok_or(Error::Budget)?;
        let payload = ic_wire::native::encode(self, limits)?;
        let mut bytes = Vec::with_capacity(FRAME.len() + payload.len());
        bytes.extend_from_slice(FRAME);
        bytes.extend_from_slice(&payload);
        Ok(bytes)
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
    #[error("Wire directory record: {0}")]
    Wire(#[from] ic_wire::Error),
}

#[derive(Clone, Debug)]
struct Directory {
    image: Image,
    by_key: BTreeMap<StableKey, AllocationId>,
    // Derived only from validated extents; never persisted as a second authority.
    logical_ends: Vec<Vec<u64>>,
    budget: StorageConfig,
}

impl Directory {
    fn empty(budget: StorageConfig, data_start: u64) -> Result<Self, Error> {
        Self::validate(
            Image {
                generation: 0,
                next_id: 1,
                next_page: data_start,
                metadata: vec![],
                entries: vec![],
            },
            budget,
            data_start,
        )
    }

    fn validate(image: Image, budget: StorageConfig, data_start: u64) -> Result<Self, Error> {
        if image.entries.len() > budget.allocations || image.next_page > budget.pages {
            return Err(Error::Budget);
        }
        if data_start == 0 || image.next_page < data_start || image.next_id == 0 {
            return Err(Error::Invalid("root boundary or identity watermark"));
        }
        let mut by_key = BTreeMap::new();
        if image.metadata.len() > 128 {
            return Err(Error::Budget);
        }
        let mut ranges = Vec::new();
        for extent in &image.metadata {
            let end = extent
                .start
                .checked_add(extent.pages)
                .ok_or(Error::Budget)?;
            if extent.pages == 0 || extent.start < data_start || end > image.next_page {
                return Err(Error::Invalid("metadata arena outside backing"));
            }
            ranges.push((extent.start, end));
        }
        let mut data_extents = 0;
        let mut previous = None;
        for entry in &image.entries {
            if entry.id.0 == 0
                || entry.id.0 >= image.next_id
                || previous.is_some_and(|id| id >= entry.id)
            {
                return Err(Error::Invalid("noncanonical allocation identities"));
            }
            previous = Some(entry.id);
            if by_key.insert(entry.key.clone(), entry.id).is_some() {
                return Err(Error::Invalid("duplicate stable key"));
            }
            if entry.extents.len() > budget.extents.saturating_sub(data_extents) {
                return Err(Error::Budget);
            }
            data_extents += entry.extents.len();
            for extent in &entry.extents {
                let end = extent
                    .start
                    .checked_add(extent.pages)
                    .ok_or(Error::Invalid("extent overflow"))?;
                if extent.pages == 0 || extent.start < data_start || end > image.next_page {
                    return Err(Error::Invalid("extent outside data region"));
                }
                ranges.push((extent.start, end));
            }
        }
        ranges.sort_unstable();
        if ranges.windows(2).any(|pair| pair[0].1 > pair[1].0) {
            return Err(Error::Invalid("overlapping physical extents"));
        }
        let logical_ends = image
            .entries
            .iter()
            .map(|entry| {
                let mut total = 0;
                entry
                    .extents
                    .iter()
                    .map(|extent| {
                        // Disjoint ranges fit within the admitted physical watermark.
                        total += extent.pages;
                        total
                    })
                    .collect()
            })
            .collect();
        Ok(Self {
            image,
            by_key,
            logical_ends,
            budget,
        })
    }

    // Validation admits strictly increasing opaque IDs, including gaps. The
    // canonical entries already provide this index; no second heap tree is needed.
    fn entry_index(&self, id: AllocationId) -> Result<usize, Error> {
        self.image
            .entries
            .binary_search_by_key(&id, |entry| entry.id)
            .map_err(|_| Error::Missing)
    }

    fn decode(bytes: &[u8], budget: StorageConfig, data_start: u64) -> Result<Self, Error> {
        if bytes.len() > budget.wire.bytes {
            return Err(Error::Budget);
        }
        let payload = bytes
            .strip_prefix(FRAME)
            .ok_or(Error::Invalid("directory frame"))?;
        let image = ic_wire::native::decode(payload, budget.wire)?;
        Self::validate(image, budget, data_start)
    }

    // Extend the validated base directly; no second indexed Directory is needed
    // merely to account for a native write's uncommitted physical tail.
    fn candidate_image(
        &self,
        keys: &BTreeSet<StableKey>,
        growth: &[(AllocationId, u64)],
        physical_end: u64,
    ) -> Result<Image, Error> {
        let mut image = self.image.clone();
        image.next_page = image.next_page.max(physical_end);
        for key in keys {
            if self.by_key.contains_key(key) {
                continue;
            }
            if image.entries.len() >= self.budget.allocations {
                return Err(Error::Budget);
            }
            let id = AllocationId(image.next_id);
            image.next_id = image.next_id.checked_add(1).ok_or(Error::Budget)?;
            image.entries.push(Entry {
                id,
                key: key.clone(),
                extents: vec![],
            });
        }
        for &(id, pages) in growth {
            // Growth applies only to bindings already present in the base.
            let index = self.entry_index(id)?;
            if pages == 0 {
                continue;
            }
            let start = image.next_page;
            image.next_page = start.checked_add(pages).ok_or(Error::Budget)?;
            if let Some(last) = image.entries[index].extents.last_mut()
                && last.start + last.pages == start
            {
                last.pages += pages;
            } else {
                image.entries[index].extents.push(Extent { start, pages });
            }
        }
        image.generation = image.generation.checked_add(1).ok_or(Error::Budget)?;
        Ok(image)
    }
}

#[cfg(test)]
mod tests;

//! Owned named-allocation directory and durable extent validation.
mod format;
pub(super) mod leases;
mod persistence;

use crate::{StableKey, hash};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

const FRAME: &[u8] = b"ICDIRS02";

///
/// StorageConfig
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
    pub directory_bytes: usize,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            allocations: 4_096,
            extents: 16_384,
            pages: 1_048_576,
            directory_bytes: 4 * 1024 * 1024,
        }
    }
}

///
/// AllocationId
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

/// Directory validation, custody and backing failures.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Logical page count overflowed before any backing access.
    /// allocation page count overflow.
    #[error("allocation page count overflow")]
    ArithmeticOverflow,
    /// directory budget exceeded.
    #[error("directory budget exceeded")]
    Budget,
    /// invalid directory: {0}.
    #[error("invalid directory: {0}")]
    Invalid(&'static str),
    /// allocation already has a live lease.
    #[error("allocation already has a live lease")]
    AlreadyOpen,
    /// access exceeds allocation bounds.
    #[error("access exceeds allocation bounds")]
    Bounds,
    /// backing already has a live owner.
    #[error("backing already has a live owner")]
    AlreadyClaimed,
    /// allocation does not exist.
    #[error("allocation does not exist")]
    Missing,
    /// prepared directory belongs to a stale generation.
    #[error("prepared directory belongs to a stale generation")]
    Stale,
    /// backing growth refused.
    #[error("backing growth refused")]
    Growth,
    /// directory owner is poisoned after interrupted backing IO.
    #[error("directory owner is poisoned after interrupted backing IO")]
    Poisoned,
}

#[derive(Clone, Debug)]
struct Directory {
    image: Image,
    by_key: BTreeMap<StableKey, AllocationId>,
    by_id: BTreeMap<AllocationId, usize>,
    // Derived only from validated extents; never persisted as a second authority.
    logical_ends: Vec<Vec<u64>>,
    budget: StorageConfig,
    data_start: u64,
}

// A candidate owns all tentative changes. Preparation never mutates its base.
#[derive(Debug)]
struct Prepared {
    base: Image,
    candidate: Directory,
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
        let mut by_id = BTreeMap::new();
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
        for (index, entry) in image.entries.iter().enumerate() {
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
            by_id.insert(entry.id, index);
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
            by_id,
            logical_ends,
            budget,
            data_start,
        })
    }

    fn decode(bytes: &[u8], budget: StorageConfig, data_start: u64) -> Result<Self, Error> {
        if bytes.len() > budget.directory_bytes {
            return Err(Error::Budget);
        }
        let payload = bytes
            .strip_prefix(FRAME)
            .ok_or(Error::Invalid("directory frame"))?;
        let image = format::decode(payload, budget)?;
        Self::validate(image, budget, data_start)
    }

    fn encode(&self) -> Result<Vec<u8>, Error> {
        format::encode(&self.image, self.budget.directory_bytes)
    }

    fn prepare(
        &self,
        keys: Vec<StableKey>,
        growth: &[(AllocationId, u64)],
    ) -> Result<Prepared, Error> {
        let mut image = self.image.clone();
        let keys: BTreeSet<_> = keys.into_iter().collect();
        for key in keys {
            if self.by_key.contains_key(&key) {
                continue;
            }
            if image.entries.len() >= self.budget.allocations {
                return Err(Error::Budget);
            }
            let id = AllocationId(image.next_id);
            image.next_id = image.next_id.checked_add(1).ok_or(Error::Budget)?;
            image.entries.push(Entry {
                id,
                key,
                extents: vec![],
            });
        }
        for &(id, pages) in growth {
            // Growth applies only to bindings already present in the base.
            let index = *self.by_id.get(&id).ok_or(Error::Missing)?;
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
        let candidate = Self::validate(image, self.budget, self.data_start)?;
        Ok(Prepared {
            base: self.image.clone(),
            candidate,
        })
    }

    // Model-only publication; durable publication belongs to Store.
    #[cfg(test)]
    fn publish(&mut self, prepared: Prepared) -> Result<(), Error> {
        if self.image != prepared.base || self.data_start != prepared.candidate.data_start {
            return Err(Error::Stale);
        }
        // Re-admit the candidate under this owner's budgets, even when two
        // owners began from the same image with different local limits.
        let candidate = Self::validate(prepared.candidate.image, self.budget, self.data_start)?;
        candidate.encode()?;
        *self = candidate;
        Ok(())
    }

    fn page_span(&self, id: AllocationId, logical: u64) -> Option<(u64, u64)> {
        let index = *self.by_id.get(&id)?;
        let ends = &self.logical_ends[index];
        let slot = ends.partition_point(|&end| end <= logical);
        let extent = self.image.entries[index].extents.get(slot)?;
        let start = if slot == 0 { 0 } else { ends[slot - 1] };
        Some((extent.start + logical - start, ends[slot] - logical))
    }

    #[cfg(test)]
    fn page(&self, id: AllocationId, logical: u64) -> Option<u64> {
        self.page_span(id, logical).map(|(page, _)| page)
    }
}

#[cfg(test)]
mod tests;

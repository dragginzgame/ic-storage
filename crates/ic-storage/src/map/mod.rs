//! Allocator-owned native hash table for u64 keys and values.
//!
//! Only slots persist: three native u64 words in `repr(C)` order (occupied,
//! key, value). Vec headers, allocator objects and this map's control state are
//! transient. A provider must separately qualify its allocation, root and
//! lifecycle before using [`Map::admit`](crate::map::Map::admit) on retained slots. Admission scans the
//! table; ordinary scopes do not. Portable checkpoints contain logical entries.

use alloc::{alloc::Global, vec::Vec};
use core::alloc::Allocator;

mod checkpoint;
pub mod paged;
mod table;

/// Refusal or unavailable native owner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// The requested entry count exceeds the configured bound.
    Limit,
    /// Allocation or representable table geometry was exhausted.
    Allocation,
    /// Native slots, count or probe placement failed admission.
    NativeImage,
    /// A callback unwound; only explicit replacement may restore availability.
    Poisoned,
    /// Checkpoint version, size or logical entries were invalid.
    Checkpoint,
    /// Checkpoint digest did not match its complete logical payload.
    Integrity,
    /// Checkpoint epoch differed from the caller's expected recovery epoch.
    Epoch,
}

pub use checkpoint::{ExportError, ImportError, export_records};

/// Controlled native slot; no references, owners, padding or executable pointers.
///
/// All three u64 words are initialized, including empty slots. Occupied is 0 or
/// 1; empty slots have zero key/value. This declaration alone does not authorize
/// casting retained bytes. The supported contract uses Rust 1.100.0 and requires
/// an independently valid live allocation and explicit provider custody.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Slot {
    occupied: u64,
    key: u64,
    value: u64,
}

impl Slot {
    /// Fully initialized empty slot, also used to scrub a reclaimed slot.
    pub const EMPTY: Self = Self {
        occupied: 0,
        key: 0,
        value: 0,
    };
}

/// Failed admission preserves the caller's original allocation for settlement.
pub struct AdmissionError<A: Allocator> {
    /// Reason that no map was opened.
    pub error: Error,
    /// Original, unmodified native slots; never silently reset on refusal.
    pub slots: Vec<Slot, A>,
}

/// Runtime owner of a native scalar table, with poisoning on callback unwind.
///
/// `A` must support two simultaneous allocations to grow. An allocator that
/// lends a single fixed region can open a pre-sized table and refuse growth.
/// Failed growth preserves the original table. This owner is transient: never
/// retain its private Rust layout or an allocator value across an upgrade.
pub struct Map<A: Allocator + Clone = Global> {
    slots: Vec<Slot, A>,
    len: usize,
    limit: usize,
    poisoned: bool,
}

impl Map {
    /// Create an empty host-owned table with a maximum live-entry count.
    #[must_use]
    pub fn new(limit: usize) -> Self {
        Self::new_in(limit, Global)
    }
}

impl<A: Allocator + Clone> Map<A> {
    /// Create an empty table under an explicit allocator authority.
    #[must_use]
    pub fn new_in(limit: usize, allocator: A) -> Self {
        Self {
            slots: Vec::new_in(allocator),
            len: 0,
            limit,
            poisoned: false,
        }
    }

    /// Validate a live native slot allocation and rebuild a transient owner.
    ///
    /// Checks every slot, count, load bound, unique key and probe placement.
    /// Failure returns the unchanged allocation. This cannot supply pointer
    /// validity, allocator custody or compatibility admission for raw bytes.
    pub fn admit(slots: Vec<Slot, A>, len: usize, limit: usize) -> Result<Self, AdmissionError<A>> {
        if let Err(error) = table::validate(&slots, len, limit) {
            return Err(AdmissionError { error, slots });
        }
        Ok(Self {
            slots,
            len,
            limit,
            poisoned: false,
        })
    }

    /// Consume a healthy transient owner for provider-controlled root publication.
    ///
    /// Returns the actual allocation and entry count. The provider must release
    /// the Vec, publish its descriptor/count, and settle custody. The returned
    /// Vec header is not a retained ABI. A poisoned owner is destroyed on error.
    pub fn release(self) -> Result<(Vec<Slot, A>, usize), Error> {
        self.healthy()?;
        Ok((self.slots, self.len))
    }

    /// Build a native candidate in another allocator, preserving this source.
    ///
    /// Rehashes copied scalar entries without a checkpoint or retained std
    /// header. The candidate has room for at least `minimum_capacity` entries
    /// and keeps this map's live-entry limit. Refusal leaves the source unchanged.
    /// The runtime owns publication and retiring the source after success; this
    /// method neither changes retained roots nor transfers provider custody.
    pub fn rebuild_in<B: Allocator + Clone>(
        &self,
        minimum_capacity: usize,
        allocator: B,
    ) -> Result<Map<B>, Error> {
        self.healthy()?;
        let mut candidate = Map::new_in(self.limit, allocator);
        candidate.reserve(minimum_capacity.max(self.len))?;
        for slot in &self.slots {
            if slot.occupied == 1 {
                table::place(&mut candidate.slots, *slot);
            }
        }
        candidate.len = self.len;
        Ok(candidate)
    }

    /// Lend scalar operations without exposing the owning allocation or allocator.
    ///
    /// Ordinary returned errors keep prior writes. A panic leaves the map
    /// poisoned, including if a host catches the unwind. Forgetting the view
    /// cannot skip finalization. IC adapters must trap on unexpected failures.
    ///
    /// ```
    /// use ic_storage::map::Map;
    /// let mut map = Map::new(1_000_000);
    /// map.with(|mut view| {
    ///     view.insert(7, 42)?;
    ///     assert_eq!(view.get(7), Some(42));
    ///     assert_eq!(view.remove(7), Some(42));
    ///     Ok::<_, ic_storage::map::Error>(())
    /// }).unwrap().unwrap();
    /// ```
    /// ```compile_fail
    /// use ic_storage::map::Map;
    /// let mut map = Map::new(8);
    /// let escaped = map.with(|view| view);
    /// ```
    /// ```compile_fail
    /// use ic_storage::map::Map;
    /// let mut map = Map::new(8);
    /// let future = map.with(|view| async move { view.get(1) });
    /// ```
    pub fn with<R>(
        &mut self,
        operation: impl for<'scope> FnOnce(View<'scope, A>) -> R,
    ) -> Result<R, Error> {
        self.healthy()?;
        self.poisoned = true;
        let result = operation(View { map: self });
        self.poisoned = false;
        Ok(result)
    }

    /// Current live-entry count, unavailable after an unexpected unwind.
    pub fn len(&self) -> Result<usize, Error> {
        self.healthy()?;
        Ok(self.len)
    }

    /// Whether the healthy table contains no entries.
    pub fn is_empty(&self) -> Result<bool, Error> {
        self.len().map(|len| len == 0)
    }

    /// Maximum live entries without another allocation, capped by the limit.
    pub fn capacity(&self) -> Result<usize, Error> {
        self.healthy()?;
        Ok(table::capacity(self.slots.len()).min(self.limit))
    }

    /// Allocated native slot bytes; excludes allocator metadata and checkpoints.
    pub fn allocated_bytes(&self) -> Result<usize, Error> {
        self.healthy()?;
        Ok(self.slots.capacity() * size_of::<Slot>())
    }

    fn healthy(&self) -> Result<(), Error> {
        if self.poisoned {
            Err(Error::Poisoned)
        } else {
            Ok(())
        }
    }

    fn reserve(&mut self, additional: usize) -> Result<(), Error> {
        let required = self.len.checked_add(additional).ok_or(Error::Limit)?;
        if required > self.limit {
            return Err(Error::Limit);
        }
        if required <= table::capacity(self.slots.len()) {
            return Ok(());
        }
        let buckets = table::buckets_for(required)?;
        // Build privately. Allocation failure cannot change the old table; only
        // a complete rehash replaces the owner. Slot Copy has no user Drop code.
        let mut candidate = Vec::new_in(self.slots.allocator().clone());
        candidate
            .try_reserve_exact(buckets)
            .map_err(|_| Error::Allocation)?;
        candidate.resize(buckets, Slot::EMPTY);
        for slot in &self.slots {
            if slot.occupied == 1 {
                table::place(&mut candidate, *slot);
            }
        }
        self.slots = candidate;
        Ok(())
    }
}

/// Scoped keyed access. Values and keys are copied scalars; no owning iterator.
pub struct View<'scope, A: Allocator + Clone> {
    map: &'scope mut Map<A>,
}

impl<A: Allocator + Clone> View<'_, A> {
    /// Number of live entries in this scope.
    #[must_use]
    pub fn len(&self) -> usize {
        self.map.len
    }

    /// Whether this scope currently contains no entries.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.map.len == 0
    }

    /// Read a copied scalar value by key.
    #[must_use]
    pub fn get(&self, key: u64) -> Option<u64> {
        table::find(&self.map.slots, key).map(|index| self.map.slots[index].value)
    }

    /// Reserve room for additional new keys before a batch, without mutation.
    /// Failure preserves all entries and the original allocation.
    pub fn reserve(&mut self, additional: usize) -> Result<(), Error> {
        self.map.reserve(additional)
    }

    /// Insert or replace a scalar. Returns the previous value if present.
    /// Existing keys can be updated at the entry limit without allocating.
    pub fn insert(&mut self, key: u64, value: u64) -> Result<Option<u64>, Error> {
        if let Some(index) = table::find(&self.map.slots, key) {
            return Ok(Some(core::mem::replace(
                &mut self.map.slots[index].value,
                value,
            )));
        }
        self.map.reserve(1)?;
        table::place(
            &mut self.map.slots,
            Slot {
                occupied: 1,
                key,
                value,
            },
        );
        self.map.len += 1;
        Ok(None)
    }

    /// Delete a key and close its probe-chain gap without persistent tombstones.
    pub fn remove(&mut self, key: u64) -> Option<u64> {
        let index = table::find(&self.map.slots, key)?;
        let value = self.map.slots[index].value;
        table::remove(&mut self.map.slots, index);
        self.map.len -= 1;
        Some(value)
    }

    /// Visit copied entries in unspecified table order; no reference can escape.
    pub fn for_each(&self, mut visit: impl FnMut(u64, u64)) {
        for slot in &self.map.slots {
            if slot.occupied == 1 {
                visit(slot.key, slot.value);
            }
        }
    }

    /// Remove all entries while retaining allocation for reuse.
    pub fn clear(&mut self) {
        self.map.slots.fill(Slot::EMPTY);
        self.map.len = 0;
    }
}

#[cfg(test)]
mod tests;

//! Ordered storage for encoded database keys and rows.
//!
//! Native AVL nodes contain only scalar links and spans into an owned byte arena. The
//! transient owner and allocator are never a retained ABI. Admission accepts an
//! already valid live allocation, not raw bytes or pointers. A runtime must own
//! allocation custody, compatibility and publication. Portable checkpoints carry
//! logical records only. No application field codec or ordering is inferred.

mod arena;
mod batch;
mod bytes;
mod checkpoint;
#[cfg(test)]
mod payload_tests;
mod range;
#[cfg(test)]
mod tests;
mod tree;

pub use batch::Mutation;
pub use bytes::{ByteSlice, Bytes};
pub use range::Range;

use alloc::{alloc::Global, vec::Vec};
use core::{alloc::Allocator, ops::Bound};

/// Refusal at a collection, allocation or recovery boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// A key/value, entry count or arithmetic bound was exceeded.
    Limit,
    /// The backing allocator refused capacity.
    Allocation,
    /// Native links, order, lengths, balance or free-list membership were invalid.
    NativeImage,
    /// A scoped callback unwound; only explicit replacement may recover it.
    Poisoned,
    /// Portable framing, length, ordering or checksum was invalid.
    Checkpoint,
    /// The checkpoint did not match the requested epoch.
    Epoch,
}

/// Controlled native node. All fields are private and initialized;
/// padding is never serialized or used as evidence. Links are one-based indices.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Node<const K: usize, const V: usize> {
    left: u64,
    right: u64,
    height: u64,
    key: arena::Span,
    value: arena::Span,
}

impl<const K: usize, const V: usize> Node<K, V> {
    /// Canonical unused node for fresh provider-owned backing initialization.
    pub const EMPTY: Self = Self {
        left: 0,
        right: 0,
        height: 0,
        key: arena::Span::EMPTY,
        value: arena::Span::EMPTY,
    };
}

/// Live allocations and scalar descriptors for explicit provider settlement.
/// This Rust struct, its Vec headers and allocators must not be retained.
pub struct Image<const K: usize, const V: usize, A: Allocator = Global, P: Allocator = A> {
    /// Initialized node allocation; unused capacity is not part of the image.
    pub nodes: Vec<Node<K, V>, A>,
    /// Initialized byte arena, including its controlled free-list directory.
    /// Its Vec header and allocator remain transient, like the node owner.
    pub payload: Vec<u8, P>,
    /// Root node, one-based; zero is empty.
    pub root: u64,
    /// First reclaimed node, one-based; zero ends the free list.
    pub free: u64,
    /// Number of live entries.
    pub len: usize,
}

/// Failed admission returns both original allocations unchanged.
pub struct AdmissionError<const K: usize, const V: usize, A: Allocator, P: Allocator> {
    /// Why the image refused.
    pub error: Error,
    /// Original allocation and descriptors for owner-controlled disposition.
    pub image: Image<K, V, A, P>,
}

/// Current node and payload allocation sizes, without traversing records.
/// Provider reservations, growth peaks and transient scratch are separate costs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AllocationUsage {
    /// Initialized node and payload bytes, including reusable slots and slack.
    pub initialized_bytes: usize,
    /// Combined capacities of the two live allocations.
    pub allocated_bytes: usize,
}

/// Transient ordered owner over caller-controlled native allocation.
/// Nodes and payloads have separate allocator authorities and scalar custody.
pub struct Map<
    const K: usize,
    const V: usize,
    A: Allocator + Clone = Global,
    P: Allocator + Clone = A,
> {
    image: Image<K, V, A, P>,
    limit: usize,
    poisoned: bool,
}

impl<const K: usize, const V: usize> Map<K, V> {
    /// Empty host-owned collection bounded by live entry count.
    #[must_use]
    pub fn new(limit: usize) -> Self {
        Self::new_in(limit, Global)
    }
}

impl<const K: usize, const V: usize, A: Allocator + Clone> Map<K, V, A> {
    /// Empty collection using one allocator authority for both allocations.
    #[must_use]
    pub fn new_in(limit: usize, allocator: A) -> Self {
        Self::new_in_allocators(limit, allocator.clone(), allocator)
    }
}

impl<const K: usize, const V: usize, A: Allocator + Clone, P: Allocator + Clone> Map<K, V, A, P> {
    /// Observe allocation sizes without closing the owner or scanning the tree.
    ///
    /// # Errors
    /// A poisoned owner refuses observation.
    pub fn allocation_usage(&self) -> Result<AllocationUsage, Error> {
        self.healthy()?;
        Ok(AllocationUsage {
            initialized_bytes: self.image.nodes.len() * size_of::<Node<K, V>>()
                + self.image.payload.len(),
            allocated_bytes: self.image.nodes.capacity() * size_of::<Node<K, V>>()
                + self.image.payload.capacity(),
        })
    }
    /// Empty collection with explicit node and byte allocator authorities.
    #[must_use]
    pub fn new_in_allocators(limit: usize, allocator: A, payload_allocator: P) -> Self {
        Self {
            image: Image {
                nodes: Vec::new_in(allocator),
                payload: Vec::new_in(payload_allocator),
                root: 0,
                free: 0,
                len: 0,
            },
            limit,
            poisoned: false,
        }
    }

    /// Validate every live/free node and payload block, then rebuild an owner.
    /// Pointer validity and allocator custody must already be established.
    /// Failure preserves the original image; no reset or repair is attempted.
    pub fn admit(
        image: Image<K, V, A, P>,
        limit: usize,
    ) -> Result<Self, AdmissionError<K, V, A, P>> {
        if let Err(error) = tree::validate(&image, limit) {
            return Err(AdmissionError { error, image });
        }
        Ok(Self {
            image,
            limit,
            poisoned: false,
        })
    }

    /// Return healthy allocations for provider-controlled settlement.
    ///
    /// # Errors
    /// A poisoned owner refuses publication and is dropped.
    pub fn release(self) -> Result<Image<K, V, A, P>, Error> {
        self.healthy()?;
        Ok(self.image)
    }

    /// Lend access without exposing owners or allowing references to escape.
    /// Ordinary callback errors preserve prior mutations. An unwind poisons the
    /// owner even if caught by a host; IC transactions supply their own rollback.
    ///
    /// # Errors
    /// Refuses a poisoned owner before calling the callback.
    ///
    /// ```compile_fail
    /// let mut map = ic_storage::ordered::Map::<512, 4096>::new(100);
    /// let escaped = map.with(|view| view);
    /// ```
    /// ```compile_fail
    /// let mut map = ic_storage::ordered::Map::<512, 4096>::new(100);
    /// let escaped = map.with(|view| view.get(b"key"));
    /// ```
    pub fn with<R>(
        &mut self,
        operation: impl for<'a> FnOnce(View<'a, K, V, A, P>) -> R,
    ) -> Result<R, Error> {
        self.healthy()?;
        self.poisoned = true;
        let result = operation(View { map: self });
        self.poisoned = false;
        Ok(result)
    }

    fn healthy(&self) -> Result<(), Error> {
        if self.poisoned {
            Err(Error::Poisoned)
        } else {
            Ok(())
        }
    }
}

/// Scoped operations; no allocation owner or mutable element reference escapes.
pub struct View<
    'a,
    const K: usize,
    const V: usize,
    A: Allocator + Clone = Global,
    P: Allocator + Clone = A,
> {
    map: &'a mut Map<K, V, A, P>,
}

impl<const K: usize, const V: usize, A: Allocator + Clone, P: Allocator + Clone>
    View<'_, K, V, A, P>
{
    /// Number of live keys.
    #[must_use]
    pub fn len(&self) -> usize {
        self.map.image.len
    }
    /// Whether no keys are present.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    /// Exact lookup, without scanning other records.
    #[must_use]
    pub fn get(&self, key: &[u8]) -> Option<ByteSlice<'_>> {
        let id = self.map.find(key);
        (id != 0).then(|| ByteSlice(self.map.value(id)))
    }
    /// Insert or replace one key, returning its previous value.
    ///
    /// # Errors
    /// Bounds, live-entry limit or allocation refusal leave all entries unchanged.
    /// Replacement needs no free entry, but can require new payload capacity.
    /// Both byte bounds are validated first.
    pub fn insert(&mut self, key: &[u8], value: &[u8]) -> Result<Option<Bytes<V>>, Error> {
        if key.len() > K || value.len() > V {
            return Err(Error::Limit);
        }
        let id = self.map.find(key);
        if id != 0 {
            let previous = Bytes::new(self.map.value(id))?;
            let old = self.map.node(id).value;
            let span = arena::replace(&mut self.map.image.payload, old, value)?;
            self.map.node_mut(id).value = span;
            return Ok(Some(previous));
        }
        if self.len() >= self.map.limit {
            return Err(Error::Limit);
        }
        // Reserve both authorities before changing records or free lists.
        if self.map.image.free == 0 {
            self.map
                .image
                .nodes
                .try_reserve(1)
                .map_err(|_| Error::Allocation)?;
        }
        arena::reserve_pair(&mut self.map.image.payload, key.len(), value.len())?;
        let key = arena::allocate(&mut self.map.image.payload, key);
        let value = arena::allocate(&mut self.map.image.payload, value);
        let id = self.map.allocate(Node {
            key,
            value,
            height: 1,
            ..Node::EMPTY
        });
        self.map.image.root = self.map.insert_node(self.map.image.root, id);
        self.map.image.len += 1;
        Ok(None)
    }
    /// Delete a key and scrub/recycle its node without allocating or scanning.
    pub fn remove(&mut self, key: &[u8]) -> Option<Bytes<V>> {
        let id = self.map.find(key);
        if id == 0 {
            return None;
        }
        let value = Bytes::copy_admitted(self.map.value(id));
        self.map.image.root = self.map.remove_node(self.map.image.root, key);
        self.map.image.len -= 1;
        Some(value)
    }
    /// Lexicographic byte range. Reversed or empty bounds yield an empty iterator.
    /// Each next/next_back seeks from the root in logarithmic time. Bounds and
    /// iterator borrows prevent mutation while traversing a page. Between scopes,
    /// resume with Excluded(last_key): newly inserted keys beyond it are visible,
    /// deletion of it is harmless, and prior keys are not replayed. This is not a snapshot.
    #[must_use]
    pub fn range(&self, lower: Bound<&[u8]>, upper: Bound<&[u8]>) -> Range<'_, K, V, A, P> {
        Range::new(self.map, lower, upper)
    }
    /// All keys starting with these exact bytes, forward or reverse via `rev()`.
    #[must_use]
    pub fn prefix(&self, prefix: &[u8]) -> Range<'_, K, V, A, P> {
        Range::prefix(self.map, prefix)
    }
}

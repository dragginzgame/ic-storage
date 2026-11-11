//! Governed indexed storage with an independent bounded-slot format.
//!
//! The exclusive lease owns the committed-length cache. Allocation, backing
//! custody, and transaction rollback remain the runtime and host's responsibility.

use std::{borrow::Cow, cell::Cell, marker::PhantomData};

use crate::{
    DefaultMemoryImpl, Memory, RuntimeMemory,
    storable::{Bound, Storable},
};

use super::error::{CollectionError as Error, Result};

const HEADER_BYTES: usize = 32;
const DATA_OFFSET: u64 = 32;
const LENGTH_OFFSET: u64 = 24;
const PAGE_BYTES: u64 = 65_536;
const MAGIC: &[u8; 8] = b"STORVECT";
const VERSION: u32 = 1;
const STACK_BYTES: usize = 64;

///
/// `StableVec`
///
/// A persistent vector over one exclusive, admitted allocation lease.
///
/// Elements require a finite codec bound. Length is cached by this owner after
/// read-only admission and updated only after successful persistent mutations;
/// observing length checks owner health without reading backing bytes.
///
pub struct StableVec<T: Storable, M: Memory = DefaultMemoryImpl> {
    memory: RuntimeMemory<M>,
    layout: Layout,
    length: Cell<u64>,
    element: PhantomData<T>,
}

impl<T: Storable, M: Memory> StableVec<T, M> {
    /// Creates an empty vector in an empty allocation.
    ///
    /// # Errors
    /// Returns `NotEmpty` for a nonempty allocation or `Growth` if the initial
    /// header reservation is refused.
    ///
    /// # Panics
    /// Panics for an unbounded codec, a poisoned owner, or a backing IO failure.
    pub fn create(memory: RuntimeMemory<M>) -> Result<Self> {
        if memory.size() != 0 {
            return Err(Error::NotEmpty);
        }
        Self::reset(memory)
    }

    /// Explicitly replaces the allocation's image with an empty vector.
    /// Unreachable previous payload bytes need not be erased.
    ///
    /// # Errors
    /// Returns `Growth` if the initial header reservation is refused.
    ///
    /// # Panics
    /// Panics for an unbounded codec, a poisoned owner, or a backing IO failure.
    pub fn reset(memory: RuntimeMemory<M>) -> Result<Self> {
        #[cfg(not(target_arch = "wasm32"))]
        let _guard = memory.poison_on_unwind();
        let layout = Layout::for_bound(T::BOUND);
        if !reserve(&memory, DATA_OFFSET) {
            return Err(Error::Growth);
        }
        memory.write(0, &layout.header(0));
        Ok(Self {
            memory,
            layout,
            length: Cell::new(0),
            element: PhantomData,
        })
    }

    /// Admits the current vector image without modifying its bytes.
    /// Admission validates framing, codec bounds and the complete live extent,
    /// without decoding or scanning records.
    ///
    /// # Errors
    /// Returns `Format` for empty, foreign, unsupported or truncated framing,
    /// or a length whose occupied extent cannot be represented or is absent.
    ///
    /// # Panics
    /// Panics for an unbounded or incompatible codec, a poisoned owner, or a
    /// backing IO failure.
    pub fn load(memory: RuntimeMemory<M>) -> Result<Self> {
        let layout = Layout::for_bound(T::BOUND);
        if memory.size() == 0 {
            return Err(Error::Format);
        }
        let mut header = [0; HEADER_BYTES];
        memory.read(0, &mut header);
        let length = layout.admit_header(&header)?;
        let end = layout.extent(length).ok_or(Error::Format)?;
        if !contains(&memory, end) {
            return Err(Error::Format);
        }
        Ok(Self {
            memory,
            layout,
            length: Cell::new(length),
            element: PhantomData,
        })
    }

    /// Decodes one live record, or returns `None` when the index is absent.
    ///
    /// # Panics
    /// Panics for a poisoned owner, corrupt selected record, codec failure or
    /// backing IO failure. Unselected records are not decoded.
    #[must_use]
    pub fn get(&self, index: u64) -> Option<T> {
        // Selected reads check owner health while admitting the complete slot.
        if index >= self.length.get() {
            let _ = self.memory.size();
            return None;
        }
        Some(read_value(&self.memory, self.layout, index))
    }

    /// Replaces one existing record without changing the length.
    ///
    /// # Panics
    /// Panics for an absent index, a poisoned owner, codec failure, declared-bound
    /// violation or backing IO failure. Native mutation unwinding poisons the owner.
    pub fn set(&self, index: u64, item: &T) {
        #[cfg(not(target_arch = "wasm32"))]
        let _guard = self.memory.poison_on_unwind();
        assert!(index < self.len(), "vector index out of bounds");
        let bytes = encode(item, self.layout);
        write_value(&self.memory, self.layout, index, &bytes);
    }

    /// Appends one record and commits its new length after the record is written.
    ///
    /// # Panics
    /// Panics for a poisoned owner, codec failure, declared-bound violation,
    /// arithmetic overflow, growth refusal or backing IO failure. Native mutation
    /// unwinding poisons the owner.
    pub fn push(&self, item: &T) {
        #[cfg(not(target_arch = "wasm32"))]
        let _guard = self.memory.poison_on_unwind();
        let length = self.len();
        let next = length.checked_add(1).expect("vector length overflow");
        let end = self.layout.extent(next).expect("vector extent overflow");
        let bytes = encode(item, self.layout);
        assert!(reserve(&self.memory, end), "vector growth refused");
        write_value(&self.memory, self.layout, length, &bytes);
        self.commit_length(next);
    }

    /// Removes and returns the final record, or returns `None` when empty.
    ///
    /// # Panics
    /// Panics for a poisoned owner, corrupt selected record, codec failure or
    /// backing IO failure. Native mutation unwinding poisons the owner.
    #[must_use]
    pub fn pop(&self) -> Option<T> {
        #[cfg(not(target_arch = "wasm32"))]
        let _guard = self.memory.poison_on_unwind();
        let Some(index) = self.length.get().checked_sub(1) else {
            let _ = self.memory.size();
            return None;
        };
        let value = read_value(&self.memory, self.layout, index);
        self.commit_length(index);
        Some(value)
    }

    /// Returns the locally cached committed length after checking owner health.
    ///
    /// # Panics
    /// Panics if the owner has been poisoned.
    #[must_use]
    pub fn len(&self) -> u64 {
        let _ = self.memory.size();
        self.length.get()
    }

    /// Returns whether the committed length is zero.
    ///
    /// # Panics
    /// Panics if the owner has been poisoned.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Iterates within the length observed at creation, reading current values.
    /// Advancing, skipping or counting permanently truncates the remaining
    /// endpoint to the current length. Removed records are never read; replacements or
    /// re-appends inside an unconsumed endpoint may be observed. Appends beyond
    /// the original endpoint are excluded, and exhaustion is permanent.
    /// Skipping and counting do not decode unselected records.
    ///
    /// # Panics
    /// Panics for a poisoned owner. Selecting a record may panic for corrupt
    /// data, codec failure or backing IO failure. Counting panics if the remaining
    /// length cannot fit in `usize`.
    pub fn iter(&self) -> impl DoubleEndedIterator<Item = T> + '_ {
        Iter {
            vector: self,
            front: 0,
            back: self.len(),
        }
    }

    fn commit_length(&self, length: u64) {
        self.memory.write(LENGTH_OFFSET, &length.to_le_bytes());
        self.length.set(length);
    }
}

// The format is deliberately uniform: fixed values occupy `maximum` bytes;
// variable values prepend a u32 length within a `maximum + 4` byte slot.
// Header bytes 17..24 are reserved and must remain zero in version 1.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Layout {
    maximum: u32,
    fixed: bool,
}

impl Layout {
    const fn for_bound(bound: Bound) -> Self {
        match bound {
            Bound::Bounded {
                max_size,
                is_fixed_size,
            } => Self {
                maximum: max_size,
                fixed: is_fixed_size,
            },
            Bound::Unbounded => panic!("vector requires a finite codec bound"),
        }
    }

    fn stride(self) -> u64 {
        u64::from(self.maximum) + if self.fixed { 0 } else { 4 }
    }

    fn extent(self, length: u64) -> Option<u64> {
        length.checked_mul(self.stride())?.checked_add(DATA_OFFSET)
    }

    fn header(self, length: u64) -> [u8; HEADER_BYTES] {
        let mut header = [0; HEADER_BYTES];
        header[..8].copy_from_slice(MAGIC);
        header[8..12].copy_from_slice(&VERSION.to_le_bytes());
        header[12..16].copy_from_slice(&self.maximum.to_le_bytes());
        header[16] = u8::from(self.fixed);
        header[24..32].copy_from_slice(&length.to_le_bytes());
        header
    }

    fn admit_header(self, header: &[u8; HEADER_BYTES]) -> Result<u64> {
        if &header[..8] != MAGIC
            || header[8..12] != VERSION.to_le_bytes()
            || header[16] > 1
            || header[17..24] != [0; 7]
        {
            return Err(Error::Format);
        }
        assert!(
            header[12..16] == self.maximum.to_le_bytes() && header[16] == u8::from(self.fixed),
            "vector codec bound does not match its admitted image"
        );
        let mut length = [0; 8];
        length.copy_from_slice(&header[24..32]);
        Ok(u64::from_le_bytes(length))
    }

    fn check_length(self, length: usize) {
        let length = u64::try_from(length).expect("vector record length overflow");
        assert!(
            length <= u64::from(self.maximum) && (!self.fixed || length == u64::from(self.maximum)),
            "vector record violates its codec bound"
        );
    }
}

fn contains(memory: &impl Memory, end: u64) -> bool {
    memory.size() >= end.div_ceil(PAGE_BYTES)
}

fn reserve(memory: &impl Memory, end: u64) -> bool {
    let pages = end.div_ceil(PAGE_BYTES);
    let current = memory.size();
    pages <= current || Memory::grow(memory, pages - current) >= 0
}

fn encode<T: Storable>(item: &T, layout: Layout) -> Cow<'_, [u8]> {
    let bytes = item
        .to_bytes()
        .unwrap_or_else(|_| panic!("vector encoding failed"));
    layout.check_length(bytes.len());
    bytes
}

fn write_value(memory: &impl Memory, layout: Layout, index: u64, bytes: &[u8]) {
    let end = layout
        .extent(index.checked_add(1).expect("vector index overflow"))
        .expect("vector extent overflow");
    assert!(contains(memory, end), "vector record extends beyond memory");
    // The checked complete extent also proves the slot's starting offset.
    let mut offset = end - layout.stride();
    if !layout.fixed {
        let length = u32::try_from(bytes.len()).expect("vector record length overflow");
        memory.write(offset, &length.to_le_bytes());
        offset += 4;
    }
    if !bytes.is_empty() {
        memory.write(offset, bytes);
    }
}

fn read_value<T: Storable>(memory: &impl Memory, layout: Layout, index: u64) -> T {
    let end = layout
        .extent(index.checked_add(1).expect("vector index overflow"))
        .expect("vector extent overflow");
    assert!(contains(memory, end), "vector record extends beyond memory");
    // The checked complete extent also proves the slot's starting offset.
    let mut offset = end - layout.stride();
    let length = if layout.fixed {
        layout.maximum
    } else {
        let mut bytes = [0; 4];
        memory.read(offset, &mut bytes);
        offset += 4;
        u32::from_le_bytes(bytes)
    };
    assert!(
        length <= layout.maximum,
        "vector record exceeds its codec bound"
    );
    let length = usize::try_from(length).expect("vector record length overflow");
    if length <= STACK_BYTES {
        let mut buffer = [0; STACK_BYTES];
        if length != 0 {
            memory.read(offset, &mut buffer[..length]);
        }
        T::from_bytes(Cow::Borrowed(&buffer[..length]))
            .unwrap_or_else(|_| panic!("vector decoding failed"))
    } else {
        let mut buffer = Vec::with_capacity(length);
        // SAFETY: the complete slot was admitted above, and the bounded payload
        // lies within it. The fresh allocation has at least `length` writable
        // bytes and cannot overlap backing. Successful read_unsafe initializes
        // every byte; an unwinding read leaves the vector length at zero.
        #[expect(
            unsafe_code,
            reason = "Publish only bytes initialized by the admitted backing read"
        )]
        unsafe {
            memory.read_unsafe(offset, buffer.as_mut_ptr(), length);
            buffer.set_len(length);
        }
        T::from_bytes(Cow::Owned(buffer)).unwrap_or_else(|_| panic!("vector decoding failed"))
    }
}

struct Iter<'a, T: Storable, M: Memory> {
    vector: &'a StableVec<T, M>,
    front: u64,
    back: u64,
}

impl<T: Storable, M: Memory> Iter<'_, T, M> {
    // Callers observe owner health either directly or through selected-slot admission.
    fn remaining(&mut self) -> u64 {
        self.back = self.back.min(self.vector.length.get());
        self.front = self.front.min(self.back);
        self.back - self.front
    }
}

impl<T: Storable, M: Memory> Iterator for Iter<'_, T, M> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        self.nth(0)
    }

    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        let n = u64::try_from(n).unwrap_or(u64::MAX);
        if n >= self.remaining() {
            let _ = self.vector.memory.size();
            self.front = self.back;
            return None;
        }
        let index = self.front + n;
        self.front = index + 1;
        Some(read_value(&self.vector.memory, self.vector.layout, index))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let length = self.back.min(self.vector.len()).saturating_sub(self.front);
        usize::try_from(length).map_or((usize::MAX, None), |length| (length, Some(length)))
    }

    fn count(mut self) -> usize {
        let _ = self.vector.memory.size();
        usize::try_from(self.remaining()).expect("vector iterator count exceeds usize")
    }

    fn last(mut self) -> Option<Self::Item> {
        self.next_back()
    }
}

impl<T: Storable, M: Memory> DoubleEndedIterator for Iter<'_, T, M> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.nth_back(0)
    }

    fn nth_back(&mut self, n: usize) -> Option<Self::Item> {
        let n = u64::try_from(n).unwrap_or(u64::MAX);
        if n >= self.remaining() {
            let _ = self.vector.memory.size();
            self.back = self.front;
            return None;
        }
        self.back -= n + 1;
        Some(read_value(
            &self.vector.memory,
            self.vector.layout,
            self.back,
        ))
    }
}

#[cfg(test)]
mod format_tests;

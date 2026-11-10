//! One cached application value persisted under an exclusive governed lease.

use super::error::{CollectionError as Error, Result};
use crate::{
    DefaultMemoryImpl, Memory, RuntimeMemory,
    stable_cell::{read_payload, stable_cell_payload_length, write_payload},
    storable::{Bound, Storable},
};
use std::{borrow::Cow, mem};

///
/// `StableCell`
///
/// An application value cached in memory and persisted in an exclusive Storage
/// allocation. Dropping the Cell releases its lease without changing its bytes.
///
pub struct StableCell<T: Storable, M: Memory = DefaultMemoryImpl> {
    memory: RuntimeMemory<M>,
    value: T,
    encoded_length: usize,
}

impl<T: Storable, M: Memory> StableCell<T, M> {
    /// Create a Cell only in an empty allocation.
    ///
    /// # Errors
    /// Returns `NotEmpty` without mutation if the allocation is nonempty, or
    /// `Growth` if its initial reservation cannot accommodate the encoding.
    ///
    /// # Panics
    /// Panics if encoding fails, violates the declared bound, or backing IO
    /// fails. A native mutation panic poisons the backing owner.
    pub fn create(memory: RuntimeMemory<M>, value: T) -> Result<Self> {
        if Memory::size(&memory) != 0 {
            return Err(Error::NotEmpty);
        }
        Self::reset(memory, value)
    }

    /// Explicitly replace an allocation's contents with a new Cell.
    ///
    /// # Errors
    /// Returns `Growth` if the allocation cannot accommodate the encoding.
    ///
    /// # Panics
    /// Panics if encoding fails, violates the declared bound, or backing IO
    /// fails. A native mutation panic poisons the backing owner.
    pub fn reset(memory: RuntimeMemory<M>, value: T) -> Result<Self> {
        #[cfg(not(target_arch = "wasm32"))]
        let _guard = memory.poison_on_unwind();
        let bytes = encode(&value);
        let encoded_length = bytes.len();
        if !write_payload(&memory, &bytes, None) {
            return Err(Error::Growth);
        }
        drop(bytes);
        Ok(Self {
            memory,
            value,
            encoded_length,
        })
    }

    /// Open a Cell without modifying its allocation.
    ///
    /// The smaller of `maximum` and the type's declared ceiling is checked
    /// before allocating or reading the application payload.
    ///
    /// # Errors
    /// Returns a Cell framing error if framing, extent or size admission fails.
    ///
    /// # Panics
    /// Panics if decoding fails, the length violates a fixed-size codec bound,
    /// or backing IO fails. Decode failure releases the lease without poisoning
    /// the owner or changing the persisted image.
    pub fn load(memory: RuntimeMemory<M>, maximum: usize) -> Result<Self> {
        let maximum = match T::BOUND {
            Bound::Unbounded => maximum,
            Bound::Bounded { max_size, .. } => maximum.min(max_size as usize),
        };
        let length = stable_cell_payload_length(&memory, maximum)?;
        check_bound::<T>(length);
        let bytes = read_payload(&memory, length);
        let value = T::from_bytes(Cow::Owned(bytes))
            .unwrap_or_else(|error| panic!("Cell decoding failed: {error}"));
        Ok(Self {
            memory,
            value,
            encoded_length: length,
        })
    }

    /// Borrow the cached value without accessing backing memory or its codec.
    #[must_use]
    pub const fn get(&self) -> &T {
        &self.value
    }

    /// Persist a replacement and return the previous cached value.
    ///
    /// # Panics
    /// Panics on encoding or bound failure, refused growth, or backing IO
    /// failure. A native mutation panic poisons the backing owner; subsequent
    /// backing access and new leases are refused.
    pub fn set(&mut self, value: T) -> T {
        // RuntimeMemory checks owner health from its admitted heap state here,
        // even when the replacement will need no backing IO.
        let _ = Memory::size(&self.memory);
        #[cfg(not(target_arch = "wasm32"))]
        let _guard = self.memory.poison_on_unwind();
        let bytes = encode(&value);
        let encoded_length = bytes.len();
        assert!(
            write_payload(&self.memory, &bytes, Some(self.encoded_length)),
            "Cell replacement reservation failed"
        );
        drop(bytes);
        self.encoded_length = encoded_length;
        mem::replace(&mut self.value, value)
    }
}

fn encode<T: Storable>(value: &T) -> Cow<'_, [u8]> {
    let bytes = value
        .to_bytes()
        .unwrap_or_else(|error| panic!("Cell encoding failed: {error}"));
    check_bound::<T>(bytes.len());
    bytes
}

fn check_bound<T: Storable>(length: usize) {
    if let Bound::Bounded {
        max_size,
        is_fixed_size,
    } = T::BOUND
    {
        let maximum = max_size as usize;
        assert!(
            length <= maximum && (!is_fixed_size || length == maximum),
            "Cell codec length violates its declared bound"
        );
    }
}

//! One cached application value persisted under an exclusive governed lease.

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
    /// # Panics
    /// Panics on encoding, backing or growth failure.
    pub fn create(memory: RuntimeMemory<M>, value: T) -> Self {
        if Memory::size(&memory) != 0 {
            panic!("collection memory is not empty");
        }
        Self::reset(memory, value)
    }

    /// Explicitly replace an allocation's contents with a new Cell.
    ///
    /// # Panics
    /// Panics on encoding, backing or growth failure.
    pub fn reset(memory: RuntimeMemory<M>, value: T) -> Self {
        let bytes = encode(&value);
        let encoded_length = bytes.len();
        if !write_payload(&memory, &bytes, None) {
            panic!("cell growth failed");
        }
        drop(bytes);
        Self {
            memory,
            value,
            encoded_length,
        }
    }

    /// Open a Cell without modifying its allocation.
    ///
    /// The type's declared ceiling is checked
    /// before allocating or reading the application payload.
    ///
    /// # Errors
    /// Returns a Cell framing error if framing, extent or size admission fails.
    ///
    /// # Panics
    /// Panics if decoding fails, the length violates a fixed-size codec bound,
    /// or backing IO fails. Decode failure releases the lease without writing.
    pub fn load(memory: RuntimeMemory<M>) -> Result<Self, crate::StableCellPayloadError> {
        let maximum = match T::BOUND {
            Bound::Unbounded => usize::MAX,
            Bound::Bounded { max_size, .. } => max_size as usize,
        };
        let length = stable_cell_payload_length(&memory, maximum)?;
        check_bound::<T>(length);
        let bytes = read_payload(&memory, length);
        let value = T::from_bytes(Cow::Owned(bytes));
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
    /// failure.
    pub fn set(&mut self, value: T) -> T {
        // RuntimeMemory checks owner health from its admitted heap state here,
        // even when the replacement will need no backing IO.
        let _ = Memory::size(&self.memory);
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
    let bytes = value.to_bytes();
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

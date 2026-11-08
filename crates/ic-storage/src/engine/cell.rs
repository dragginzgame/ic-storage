// Adapted from ic-stable-structures 0.7.2, DFINITY Foundation.
// SPDX-License-Identifier: Apache-2.0
// See the package LICENSE-APACHE and docs/storage/icss-source.json.
//! A serializable value stored in the stable memory.
use crate::engine::storable::Storable;
use crate::engine::{Memory, WASM_PAGE_SIZE, read_to_vec};
use std::borrow::Cow;

const MAGIC: &[u8; 3] = b"SCL"; // short for "stable cell"
const HEADER_SIZE: u64 = 8;
const LAYOUT_VERSION: u8 = 1;

// NOTE: the size of this structure should be equal to [HEADER_SIZE].
// Changing these fields requires an explicit persisted-format decision.
//
// # Memory layout
//
// -------------------------------
// Magic "SCL"         ↕ 3 bytes
// -------------------------------
// Layout version      ↕ 1 byte
// -------------------------------
// Value length = N    ↕ 4 bytes
// -------------------------------
// <encoded value>     ↕ N bytes
// -------------------------------
#[derive(Debug, PartialEq, Eq)]
pub enum ValueError {
    ValueTooLarge { value_size: u64 },
}

pub struct Cell<T: Storable, M: Memory> {
    memory: M,
    value: T,
}

impl<T: Storable, M: Memory> Cell<T, M> {
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) const fn memory(&self) -> &M {
        &self.memory
    }
    pub fn new(memory: M, value: T) -> Self {
        Self::flush_value(&memory, &value).expect("Failed to write initial value to the memory");
        Self { memory, value }
    }

    pub(crate) fn load_with_length(memory: M, length: u32) -> Self {
        let value = Self::read_value(&memory, length);
        Self { memory, value }
    }

    fn read_value(memory: &M, len: u32) -> T {
        let mut buf = vec![];
        read_to_vec(memory, HEADER_SIZE.into(), &mut buf, len as usize);
        T::from_bytes(Cow::Owned(buf))
    }

    pub const fn get(&self) -> &T {
        &self.value
    }

    pub fn set(&mut self, value: T) -> T {
        Self::flush_value(&self.memory, &value).expect("Failed to write value to the memory");
        std::mem::replace(&mut self.value, value)
    }
    fn flush_value(memory: &M, value: &T) -> Result<(), ValueError> {
        let encoded = value.to_bytes();
        flush_bytes(memory, encoded.as_ref())
    }
}

// Keep memory IO shared across value codecs instead of repeating it for every T.
#[inline(never)]
fn flush_bytes<M: Memory>(memory: &M, bytes: &[u8]) -> Result<(), ValueError> {
    let len = bytes.len();
    if len > u32::MAX as usize {
        return Err(ValueError::ValueTooLarge {
            value_size: len as u64,
        });
    }
    let size = memory.size();
    let available_space = size * WASM_PAGE_SIZE;
    if len as u64 > available_space.saturating_sub(HEADER_SIZE) || size == 0 {
        let grow_by = (len as u64 + HEADER_SIZE + WASM_PAGE_SIZE - size * WASM_PAGE_SIZE - 1)
            / WASM_PAGE_SIZE;
        if memory.grow(grow_by) < 0 {
            return Err(ValueError::ValueTooLarge {
                value_size: len as u64,
            });
        }
    }

    debug_assert!(memory.size() * WASM_PAGE_SIZE >= len as u64 + HEADER_SIZE);

    let mut header = [0; 8];
    header[..3].copy_from_slice(MAGIC);
    header[3] = LAYOUT_VERSION;
    header[4..].copy_from_slice(
        &u32::try_from(len)
            .expect("admitted cell length")
            .to_le_bytes(),
    );
    memory.write(0, &header);
    memory.write(HEADER_SIZE, bytes);
    Ok(())
}

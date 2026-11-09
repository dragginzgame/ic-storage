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
    // Length of the successfully persisted payload, not a second encoded copy.
    encoded_len: u32,
}

impl<T: Storable, M: Memory> Cell<T, M> {
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) const fn memory(&self) -> &M {
        &self.memory
    }
    pub fn new(memory: M, value: T) -> Self {
        let encoded_len = Self::flush_value(&memory, &value, None)
            .expect("Failed to write initial value to the memory");
        Self {
            memory,
            value,
            encoded_len,
        }
    }

    pub(crate) fn load_with_length(memory: M, length: u32) -> Self {
        let value = Self::read_value(&memory, length);
        Self {
            memory,
            value,
            encoded_len: length,
        }
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
        let encoded_len = Self::flush_value(&self.memory, &value, Some(self.encoded_len))
            .expect("Failed to write value to the memory");
        self.encoded_len = encoded_len;
        std::mem::replace(&mut self.value, value)
    }
    fn flush_value(memory: &M, value: &T, previous_len: Option<u32>) -> Result<u32, ValueError> {
        let encoded = value.to_bytes();
        flush_bytes(memory, encoded.as_ref(), previous_len)
    }
}

// Keep memory IO shared across value codecs instead of repeating it for every T.
#[inline(never)]
fn flush_bytes<M: Memory>(
    memory: &M,
    bytes: &[u8],
    previous_len: Option<u32>,
) -> Result<u32, ValueError> {
    let len = bytes.len();
    if len > u32::MAX as usize {
        return Err(ValueError::ValueTooLarge {
            value_size: len as u64,
        });
    }
    let encoded_len = u32::try_from(len).expect("admitted cell length");
    // An admitted or successfully written same-length payload already fits.
    // The header is unchanged; keep encoding and the actual payload write.
    if previous_len == Some(encoded_len) {
        if !bytes.is_empty() {
            memory.write(HEADER_SIZE, bytes);
        }
        return Ok(encoded_len);
    }
    // A shorter payload fits in the previously admitted exclusive allocation.
    // Avoid loading allocation controls solely to rediscover that capacity.
    if previous_len.is_none_or(|previous| encoded_len > previous) {
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
    Ok(encoded_len)
}

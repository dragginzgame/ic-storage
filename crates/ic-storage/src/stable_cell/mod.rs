//! Cell framing and read-only payload admission.
//!
//! This module owns the persisted envelope, not application codecs or leases.

use crate::{Memory, WASM_PAGE_SIZE_BYTES};

/// Number of bytes preceding a Cell payload in the current format.
pub const STABLE_CELL_HEADER_SIZE: usize = 24;

const DISCRIMINATOR: [u8; 8] = *b"STORCELL";
const VERSION: u32 = 1;
const PAYLOAD_OFFSET: u64 = STABLE_CELL_HEADER_SIZE as u64;

///
/// `StableCellPayloadError`
///
/// A framing or size refusal detected before reading the application payload.
///

#[derive(Debug, Clone, Copy, Eq, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum StableCellPayloadError {
    /// The payload exceeds the caller's ceiling or the host's buffer limit.
    #[error("Cell payload length {value_len} exceeds the admitted limit")]
    TooLarge {
        /// Declared payload length in bytes, excluding the Cell header.
        value_len: u64,
    },
    /// The allocation does not contain supported Cell framing.
    #[error("allocation does not contain supported Cell framing")]
    NotStableCell,
    /// The declared payload extends beyond the allocation.
    #[error("Cell payload length {value_len} exceeds available bytes {available_bytes}")]
    InvalidLength {
        /// Declared payload length in bytes, excluding the Cell header.
        value_len: u64,
        /// Allocation bytes available after the Cell header.
        available_bytes: u64,
    },
}

/// Admit the current Cell framing and return its payload length without reading
/// or decoding the payload.
///
/// # Errors
/// Returns an error for unsupported framing, an impossible extent, or a payload
/// above `maximum` or the host's addressable buffer limit.
///
/// # Panics
/// Panics if the supplied memory implementation refuses a size query or read.
pub fn stable_cell_payload_length<M: Memory>(
    memory: &M,
    maximum: usize,
) -> Result<usize, StableCellPayloadError> {
    let capacity = memory
        .size()
        .checked_mul(WASM_PAGE_SIZE_BYTES)
        .filter(|capacity| *capacity >= PAYLOAD_OFFSET)
        .ok_or(StableCellPayloadError::NotStableCell)?;
    let mut header = [0; STABLE_CELL_HEADER_SIZE];
    memory.read(0, &mut header);
    if header[..8] != DISCRIMINATOR
        || header[8..12] != VERSION.to_le_bytes()
        || header[12..16] != [0; 4]
    {
        return Err(StableCellPayloadError::NotStableCell);
    }
    let mut length = [0; 8];
    length.copy_from_slice(&header[16..]);
    let value_len = u64::from_le_bytes(length);
    let available_bytes = capacity - PAYLOAD_OFFSET;
    if value_len > available_bytes {
        return Err(StableCellPayloadError::InvalidLength {
            value_len,
            available_bytes,
        });
    }
    let length =
        usize::try_from(value_len).map_err(|_| StableCellPayloadError::TooLarge { value_len })?;
    if length > maximum || length > isize::MAX as usize {
        return Err(StableCellPayloadError::TooLarge { value_len });
    }
    Ok(length)
}

fn header(value_len: u64) -> [u8; STABLE_CELL_HEADER_SIZE] {
    let mut bytes = [0; STABLE_CELL_HEADER_SIZE];
    bytes[..8].copy_from_slice(&DISCRIMINATOR);
    bytes[8..12].copy_from_slice(&VERSION.to_le_bytes());
    bytes[16..].copy_from_slice(&value_len.to_le_bytes());
    bytes
}

pub fn read_payload<M: Memory>(memory: &M, length: usize) -> Vec<u8> {
    // The caller has admitted this complete extent before allocating the buffer.
    let mut bytes = Vec::with_capacity(length);
    if length != 0 {
        // SAFETY: Fresh capacity provides exclusive, correctly aligned storage
        // for at least `length` bytes, disjoint from the backing and its state.
        // A successful read initializes the entire destination before set_len
        // publishes it. If reading unwinds after a partial copy, the Vec still
        // has length zero and drops without exposing uninitialized bytes.
        #[expect(
            unsafe_code,
            reason = "The admitted read initializes fresh Vec capacity."
        )]
        unsafe {
            memory.read_unsafe(PAYLOAD_OFFSET, bytes.as_mut_ptr(), length);
            bytes.set_len(length);
        }
    }
    bytes
}

pub fn write_payload<M: Memory>(memory: &M, bytes: &[u8], previous_length: Option<usize>) -> bool {
    let Ok(length) = u64::try_from(bytes.len()) else {
        return false;
    };
    let Some(end) = PAYLOAD_OFFSET.checked_add(length) else {
        return false;
    };
    // The exclusive lease keeps the previously admitted extent valid. A
    // replacement inside that extent requires neither reservation nor a query.
    if previous_length.is_none_or(|previous| bytes.len() > previous) {
        let needed_pages = end.div_ceil(WASM_PAGE_SIZE_BYTES);
        if needed_pages.checked_mul(WASM_PAGE_SIZE_BYTES).is_none() {
            return false;
        }
        let current_pages = memory.size();
        if current_pages.checked_mul(WASM_PAGE_SIZE_BYTES).is_none() {
            return false;
        }
        if needed_pages > current_pages {
            if memory.grow(needed_pages - current_pages) < 0 {
                return false;
            }
            // Check a successful growth before publishing any bytes.
            if memory
                .size()
                .checked_mul(WASM_PAGE_SIZE_BYTES)
                .is_none_or(|capacity| capacity < end)
            {
                return false;
            }
        }
    }
    if !bytes.is_empty() {
        memory.write(PAYLOAD_OFFSET, bytes);
    }
    // Publishing the header last does not promise native atomicity. The owner
    // health guard covers every write; IC executions supply their own rollback.
    if previous_length != Some(bytes.len()) {
        memory.write(0, &header(length));
    }
    true
}

#[cfg(test)]
mod tests;

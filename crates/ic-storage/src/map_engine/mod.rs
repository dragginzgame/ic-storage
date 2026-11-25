//! Packaged stable B-tree derived from ic-stable-structures 0.7.2.
pub(crate) mod btreemap;
mod types;
use crate::WASM_PAGE_SIZE_BYTES as WASM_PAGE_SIZE;
use crate::substrate::{Memory, Storable, storable};
use std::{
    error,
    fmt::{Display, Formatter},
};
use types::Address;

fn read_to_vec<M: Memory>(memory: &M, addr: Address, dst: &mut Vec<u8>, count: usize) {
    dst.resize(count, 0);
    memory.read(addr.get(), dst);
}
/// A helper function that reads a single 32bit integer encoded as
/// little-endian from the specified memory at the specified offset.
fn read_u32<M: Memory>(m: &M, addr: Address) -> u32 {
    let mut buf: [u8; 4] = [0; 4];
    m.read(addr.get(), &mut buf);
    u32::from_le_bytes(buf)
}

/// A helper function that reads a single 64bit integer encoded as
/// little-endian from the specified memory at the specified offset.
fn read_u64<M: Memory>(m: &M, addr: Address) -> u64 {
    let mut buf: [u8; 8] = [0; 8];
    m.read(addr.get(), &mut buf);
    u64::from_le_bytes(buf)
}

/// Reads `count` Address values in a single read.
fn read_address_vec<M: Memory>(m: &M, addr: Address, count: usize) -> std::vec::Vec<Address> {
    let chunk_size = Address::size().get() as usize;
    assert_eq!(chunk_size, 8);
    let byte_len = count * chunk_size;
    let mut buf = std::vec::Vec::with_capacity(byte_len);
    read_to_vec(m, addr, &mut buf, byte_len);

    buf.chunks_exact(chunk_size)
        .map(|chunk| {
            let bytes: [u8; 8] = chunk.try_into().expect("chunk must be 8 bytes");
            Address::from(u64::from_le_bytes(bytes))
        })
        .collect()
}

/// Writes a single 32-bit integer encoded as little-endian.
fn write_u32<M: Memory>(m: &M, addr: Address, val: u32) {
    write(m, addr.get(), &val.to_le_bytes());
}

/// Writes a single 64-bit integer encoded as little-endian.
fn write_u64<M: Memory>(m: &M, addr: Address, val: u64) {
    write(m, addr.get(), &val.to_le_bytes());
}

#[derive(Debug, PartialEq, Eq)]
pub struct GrowFailed {
    current_size: u64,
    delta: u64,
}

impl Display for GrowFailed {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Failed to grow memory: current size={}, delta={}",
            self.current_size, self.delta
        )
    }
}

impl error::Error for GrowFailed {}

/// Writes the bytes at the specified offset, growing the memory size if needed.
fn safe_write<M: Memory>(memory: &M, offset: u64, bytes: &[u8]) -> Result<(), GrowFailed> {
    let last_byte = offset
        .checked_add(bytes.len() as u64)
        .expect("Address space overflow");

    let size_pages = memory.size();
    let size_bytes = size_pages
        .checked_mul(WASM_PAGE_SIZE)
        .expect("Address space overflow");

    if size_bytes < last_byte {
        let diff_bytes = last_byte - size_bytes;
        let diff_pages = diff_bytes
            .checked_add(WASM_PAGE_SIZE - 1)
            .expect("Address space overflow")
            / WASM_PAGE_SIZE;
        if memory.grow(diff_pages) == -1 {
            return Err(GrowFailed {
                current_size: size_pages,
                delta: diff_pages,
            });
        }
    }
    memory.write(offset, bytes);
    Ok(())
}

/// Like [safe_write], but panics if the memory.grow fails.
fn write<M: Memory>(memory: &M, offset: u64, bytes: &[u8]) {
    if let Err(GrowFailed {
        current_size,
        delta,
    }) = safe_write(memory, offset, bytes)
    {
        panic!(
            "Failed to grow memory from {} pages to {} pages (delta = {} pages).",
            current_size,
            current_size + delta,
            delta
        );
    }
}

#[cfg(test)]
mod tests {
    use super::btreemap::BTreeMap;
    use crate::{VectorMemory, substrate};

    #[test]
    fn bulk_removal_round_trips_through_released_engine() {
        let memory = VectorMemory::default();
        let mut old = substrate::BTreeMap::new(memory.clone());
        for key in 0_u64..256 {
            old.insert(key, vec![key as u8; if key % 13 == 0 { 8192 } else { 15 }]);
        }
        drop(old);
        let mut current = BTreeMap::<u64, Vec<u8>, _>::load(memory.clone());
        assert_eq!(current.remove_many((0..256).step_by(2).collect()), 128);
        drop(current);
        let old = substrate::BTreeMap::<u64, Vec<u8>, _>::load(memory);
        assert_eq!(old.len(), 128);
        for (entry, key) in old.iter().zip((1..256).step_by(2)) {
            assert_eq!(*entry.key(), key);
            assert_eq!(
                entry.value(),
                vec![key as u8; if key % 13 == 0 { 8192 } else { 15 }]
            );
        }
    }
}

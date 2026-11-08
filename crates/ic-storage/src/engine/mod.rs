// Adapted from ic-stable-structures 0.7.2, DFINITY Foundation.
// SPDX-License-Identifier: Apache-2.0
// See the package LICENSE-APACHE and docs/storage/icss-source.json.
//! Copied ICSS 0.7.2 collection engines, private behind governed adapters.
#![expect(
    unsafe_code,
    reason = "Copied ICSS packed-header and initialized-buffer operations are confined to this private engine"
)]
#![expect(
    clippy::inline_always,
    reason = "Retain ICSS baseline inlining choices for the initial comparison"
)]
mod base_vec;
pub mod btreemap;
pub mod cell;
pub mod storable;
mod types;
pub mod vec;
pub use btreemap::BTreeMap;
use std::{
    error,
    fmt::{Display, Formatter},
    mem::MaybeUninit,
};
pub use storable::Storable;
use types::Address;
const WASM_PAGE_SIZE: u64 = crate::WASM_PAGE_SIZE_BYTES;
pub trait Memory {
    fn size(&self) -> u64;
    fn grow(&self, pages: u64) -> i64;
    fn read(&self, offset: u64, dst: &mut [u8]);
    // Every private view must explicitly forward initialization. Backing's
    // former inherited zeroing fallback silently lost ICSS's fast-read path.
    unsafe fn read_unsafe(&self, offset: u64, dst: *mut u8, count: usize);
    fn write(&self, offset: u64, src: &[u8]);
}
fn read_to_vec<M: Memory>(m: &M, addr: Address, dst: &mut std::vec::Vec<u8>, count: usize) {
    dst.clear();
    dst.reserve_exact(count);
    unsafe {
        m.read_unsafe(addr.get(), dst.as_mut_ptr(), count);
        // SAFETY: read_unsafe guarantees to initialize the first `count` bytes
        dst.set_len(count);
    }
}
fn read_u32<M: Memory>(m: &M, addr: Address) -> u32 {
    let mut buf: [u8; 4] = [0; 4];
    m.read(addr.get(), &mut buf);
    u32::from_le_bytes(buf)
}
fn read_u64<M: Memory>(m: &M, addr: Address) -> u64 {
    let mut buf: [u8; 8] = [0; 8];
    m.read(addr.get(), &mut buf);
    u64::from_le_bytes(buf)
}
fn read_address_vec<M: Memory>(m: &M, addr: Address, count: usize) -> std::vec::Vec<Address> {
    const { assert!(size_of::<Address>() == 8) };
    let byte_len = count.checked_mul(8).expect("address vector byte length");
    let mut addresses = Vec::<Address>::with_capacity(count);
    // SAFETY: Address is a packed u64 with no padding or invalid bit patterns.
    // Capacity covers the complete destination, separate from backing memory.
    // Publish the length only after the read initializes every byte; a panic
    // drops an empty vector. Decode little-endian words in place afterwards.
    unsafe {
        m.read_unsafe(addr.get(), addresses.as_mut_ptr().cast(), byte_len);
        addresses.set_len(count);
    }
    for address in &mut addresses {
        *address = Address::from(u64::from_le(address.get()));
    }
    addresses
}
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
fn read_struct<T, M: Memory>(addr: Address, memory: &M) -> T {
    let mut value = MaybeUninit::<T>::uninit();
    unsafe {
        memory.read_unsafe(
            addr.get(),
            value.as_mut_ptr().cast::<u8>(),
            core::mem::size_of::<T>(),
        );
        value.assume_init()
    }
}
fn write_struct<T, M: Memory>(t: &T, addr: Address, memory: &M) {
    let slice = unsafe {
        core::slice::from_raw_parts(
            std::ptr::from_ref(t).cast::<u8>(),
            core::mem::size_of::<T>(),
        )
    };

    write(memory, addr.get(), slice);
}

#[cfg(test)]
mod tests;

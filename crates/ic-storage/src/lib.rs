#![deny(unsafe_code, unsafe_op_in_unsafe_fn)]
#![deny(rustdoc::broken_intra_doc_links)]
#![doc = include_str!("../README.md")]

/// Governed collections over checked named allocations and upstream codecs.
pub mod adapter;
mod hash;
mod key;
mod memory;
mod runtime;
mod stable_cell;
#[cfg(feature = "test-support")]
pub mod testing;

use ic_platform::stable_structures as substrate;
pub use substrate::storable;

pub use key::{StableKey, StableKeyError};
pub use memory::{BackingIdentity, DefaultMemoryImpl, Memory, VectorMemory};
pub use runtime::*;
pub use stable_cell::{
    STABLE_CELL_HEADER_SIZE, StableCellPayloadError, stable_cell_payload_length,
};

/// WebAssembly stable-memory page size in bytes.
pub const WASM_PAGE_SIZE_BYTES: u64 = 65_536;

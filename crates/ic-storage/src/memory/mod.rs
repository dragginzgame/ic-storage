//! Upstream memory contract and Platform-backed canister storage.
#[cfg(target_arch = "wasm32")]
mod wasm;

pub use crate::substrate::{Memory, VectorMemory};

#[cfg(target_arch = "wasm32")]
pub use wasm::DefaultMemoryImpl;

/// Native default backing for fixture and host integration.
#[cfg(not(target_arch = "wasm32"))]
pub type DefaultMemoryImpl = VectorMemory;

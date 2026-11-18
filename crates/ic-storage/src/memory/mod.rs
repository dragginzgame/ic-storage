//! Upstream memory contract and Platform-backed canister storage.
pub use crate::substrate::{Memory, VectorMemory};

/// Native default backing for fixture and host integration.
#[cfg(not(target_arch = "wasm32"))]
pub type DefaultMemoryImpl = VectorMemory;

#[cfg(target_arch = "wasm32")]
mod wasm;
#[cfg(target_arch = "wasm32")]
pub use wasm::DefaultMemoryImpl;

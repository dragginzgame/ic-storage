//! Upstream memory contract and Platform-backed canister storage.
pub use crate::substrate::{Memory, VectorMemory};

/// Native default backing for fixture and host integration.
#[cfg(not(target_arch = "wasm32"))]
pub type DefaultMemoryImpl = VectorMemory;

/// Canister stable memory through Platform's system-call boundary.
#[cfg(target_arch = "wasm32")]
#[derive(Clone, Copy, Default)]
pub struct DefaultMemoryImpl {
    _private: (),
}
#[cfg(target_arch = "wasm32")]
impl Memory for DefaultMemoryImpl {
    fn size(&self) -> u64 {
        ic_platform_runtime::raw::stable64_size()
    }
    fn grow(&self, pages: u64) -> i64 {
        ic_platform_runtime::raw::stable64_grow(pages).map_or(-1, |previous| {
            i64::try_from(previous).expect("IC page count")
        })
    }
    fn read(&self, offset: u64, bytes: &mut [u8]) {
        ic_platform_runtime::raw::stable64_read(offset, bytes);
    }
    fn write(&self, offset: u64, bytes: &[u8]) {
        ic_platform_runtime::raw::stable64_write(offset, bytes);
    }
}

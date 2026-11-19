use super::{BackingIdentity, Memory};
use crate::MemoryBacking;

///
/// DefaultMemoryImpl
/// Canister stable memory through Platform's system-call boundary.
///

#[derive(Clone, Copy, Default)]
pub struct DefaultMemoryImpl {
    _private: (),
}

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

impl MemoryBacking for DefaultMemoryImpl {
    fn backing_identity(&self) -> BackingIdentity {
        // Every default handle addresses the same canister stable memory.
        BackingIdentity(0)
    }
}

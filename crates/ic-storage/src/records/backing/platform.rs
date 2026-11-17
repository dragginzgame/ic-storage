//! One Storage-owned lease of the entire stable address space per Wasm generation.
use super::{Backing, Error, sealed};
use core::sync::atomic::{AtomicBool, Ordering};
use ic_platform_runtime::raw as ic0;

static CLAIMED: AtomicBool = AtomicBool::new(false);

/// Exclusive Platform stable-memory binding for ordinary heap-discarding upgrades.
/// Applications must not also use a raw memory manager or legacy Storage owner.
/// This does not qualify enhanced orthogonal persistence or native heap admission.
pub struct StableMemory {
    _private: (),
}
impl StableMemory {
    /// Claim the one application backing. This performs no stable IO and never
    /// creates records; pass the result to explicit `Store::create` or `Store::open`.
    ///
    /// # Errors
    /// Refuses while another binding (including one owned by participants) is live.
    pub fn take() -> Result<Self, Error> {
        CLAIMED
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .map_err(|_| Error::Claimed)?;
        Ok(Self { _private: () })
    }
}
impl Drop for StableMemory {
    fn drop(&mut self) {
        CLAIMED.store(false, Ordering::SeqCst);
    }
}
impl sealed::Sealed for StableMemory {}
impl Backing for StableMemory {
    fn size(&self) -> u64 {
        ic0::stable64_size()
    }
    fn grow(&mut self, pages: u64) -> Option<u64> {
        ic0::stable64_grow(pages)
    }
    fn read(&self, offset: u64, bytes: &mut [u8]) {
        ic0::stable64_read(offset, bytes);
    }
    fn write(&mut self, offset: u64, bytes: &[u8]) {
        ic0::stable64_write(offset, bytes);
    }
}

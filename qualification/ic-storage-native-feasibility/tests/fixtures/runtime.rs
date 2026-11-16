//! No default-heap allocation or unwinding in the closed compiler fixtures.

use core::alloc::{GlobalAlloc, Layout};

struct RejectGlobal;

#[global_allocator]
static GLOBAL: RejectGlobal = RejectGlobal;

// SAFETY: no block is ever issued; a valid deallocation cannot reach this impl.
#[expect(
    unsafe_code,
    reason = "fail-closed Global allocator for no_std fixtures"
)]
unsafe impl GlobalAlloc for RejectGlobal {
    unsafe fn alloc(&self, _: Layout) -> *mut u8 {
        core::ptr::null_mut()
    }

    unsafe fn dealloc(&self, _: *mut u8, _: Layout) {
        abort();
    }
}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo<'_>) -> ! {
    abort()
}

pub(super) fn abort() -> ! {
    core::arch::wasm32::unreachable()
}

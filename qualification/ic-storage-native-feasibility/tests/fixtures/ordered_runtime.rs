//! Closed wasm32 fixture memory contract: transient allocations never reach roots.
use core::{
    alloc::{GlobalAlloc, Layout},
    ptr,
    sync::atomic::{AtomicUsize, Ordering},
};
use ic_platform_runtime::raw as ic0;
use ic_storage::checkpoint::Memory;

pub(super) const HEADER: usize = 1_572_864;
pub(super) const A: usize = 4_194_304;
pub(super) const B: usize = 6_291_456;
const SCRATCH: usize = 2_097_152;
const END: usize = A;
static NEXT: AtomicUsize = AtomicUsize::new(SCRATCH);
struct Transient;
#[global_allocator]
static GLOBAL: Transient = Transient;

// SAFETY: this closed executable owns the admitted [SCRATCH, END) wasm32 window.
// Every entry resets only after the former entry's owners have ended. Allocation
// is monotonic, aligned and disjoint; no pointers survive an entry. Deallocation
// delays reclamation until that boundary. Native nodes use the separate Pool.
#[expect(unsafe_code, reason = "closed transient-window allocation contract")]
unsafe impl GlobalAlloc for Transient {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let previous = NEXT.load(Ordering::Relaxed);
        let Some(start) = previous
            .checked_add(layout.align() - 1)
            .map(|n| n & !(layout.align() - 1))
        else {
            return ptr::null_mut();
        };
        let Some(end) = start.checked_add(layout.size()).filter(|end| *end <= END) else {
            return ptr::null_mut();
        };
        NEXT.store(end, Ordering::Relaxed);
        ptr::with_exposed_provenance_mut(start)
    }
    unsafe fn dealloc(&self, _: *mut u8, _: Layout) {}
}

pub(super) fn enter() {
    NEXT.store(SCRATCH, Ordering::Relaxed);
}
pub(super) fn scratch_bytes() -> u64 {
    (NEXT.load(Ordering::Relaxed) - SCRATCH) as u64
}

// Platform Runtime links std's aborting panic implementation. No unwind or
// successful-frame panic state is retained by this closed fixture.

// SAFETY: admission fixes memory at eight MiB and keeps compiler data/stack below
// HEADER. This 112-byte initialized scalar header is exclusively owned here. No
// borrowed header reference crosses a native loan or escapes an entry.
#[expect(unsafe_code, reason = "closed scalar descriptor memory boundary")]
pub(super) fn header() -> [u64; 14] {
    unsafe { ptr::read(ptr::with_exposed_provenance(HEADER)) }
}
#[expect(unsafe_code, reason = "closed scalar descriptor publication boundary")]
pub(super) fn publish(words: [u64; 14], fail: bool) {
    let pointer: *mut u64 = ptr::with_exposed_provenance_mut(HEADER);
    for (index, word) in words.into_iter().enumerate() {
        // SAFETY: same fixed header contract; descriptor publication happens only
        // after native owners end. An injected partial write traps the message.
        unsafe {
            pointer.add(index).write(word);
        }
        assert!(!(fail && index == 4));
    }
}

#[expect(
    unsafe_code,
    reason = "byte evidence with no active native mutation frame"
)]
pub(super) fn reply_native(base: usize, bytes: usize) {
    assert!(matches!(base, A | B) && bytes <= 2_097_152);
    // SAFETY: the complete fixed Wasm window is initialized, and the concrete
    // node has no padding. No mutable native borrow exists during this diagnostic read.
    unsafe {
        ic0::msg_reply_data_append(core::slice::from_raw_parts(
            ptr::with_exposed_provenance(base),
            bytes,
        ));
    }
}

#[derive(Clone, Copy)]
pub(super) struct Stable;
impl Memory for Stable {
    fn size(&self) -> u64 {
        ic0::stable64_size()
    }
    fn grow(&self, pages: u64) -> Option<u64> {
        ic0::stable64_grow(pages)
    }
    fn read(&self, offset: u64, bytes: &mut [u8]) {
        ic0::stable64_read(offset, bytes);
    }
    fn write(&self, offset: u64, bytes: &[u8]) {
        ic0::stable64_write(offset, bytes);
    }
}

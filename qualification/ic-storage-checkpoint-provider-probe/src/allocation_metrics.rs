//! Opt-in checkpoint-heap requested-byte accounting. Never retained-map admission.
//! System allocator metadata, rounding, fragmentation and native pool loans are
//! outside these counts. Realloc's internal overlap is bounded, not observed.
use ic_platform_runtime::raw as ic0;
use std::{
    alloc::{GlobalAlloc, Layout, System},
    sync::atomic::{AtomicUsize, Ordering::Relaxed},
};

struct Measured;
#[global_allocator]
static ALLOCATOR: Measured = Measured;
static START: AtomicUsize = AtomicUsize::new(0);
static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);
static UPPER: AtomicUsize = AtomicUsize::new(0);
static TOTAL: AtomicUsize = AtomicUsize::new(0);
static ALLOCS: AtomicUsize = AtomicUsize::new(0);
static FREES: AtomicUsize = AtomicUsize::new(0);
static FAILED: AtomicUsize = AtomicUsize::new(0);

fn refused() -> ! {
    ic0::trap(b"allocation metric arithmetic refused")
}
fn add(counter: &AtomicUsize, value: usize) -> usize {
    counter
        .try_update(Relaxed, Relaxed, |old| old.checked_add(value))
        .unwrap_or_else(|_| refused())
        .checked_add(value)
        .unwrap_or_else(|| refused())
}
fn freed(size: usize) {
    LIVE.try_update(Relaxed, Relaxed, |old| old.checked_sub(size))
        .unwrap_or_else(|_| refused());
    add(&FREES, 1);
}
fn allocated(pointer: *mut u8, size: usize) {
    if pointer.is_null() {
        add(&FAILED, 1);
        return;
    }
    let live = add(&LIVE, size);
    PEAK.fetch_max(live, Relaxed);
    UPPER.fetch_max(live, Relaxed);
    add(&TOTAL, size);
    add(&ALLOCS, 1);
}

// SAFETY: every request is forwarded unchanged to System. Counters allocate
// nothing, cannot unwind and never inspect or extend allocation lifetimes.
// This qualification target is single-threaded Wasm; reset is an idle endpoint.
#[expect(
    unsafe_code,
    reason = "transparent qualification-only System allocator wrapper"
)]
unsafe impl GlobalAlloc for Measured {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: forward the caller's valid GlobalAlloc request unchanged.
        let pointer = unsafe { System.alloc(layout) };
        allocated(pointer, layout.size());
        pointer
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        // SAFETY: preserve System's zero-initialization contract.
        let pointer = unsafe { System.alloc_zeroed(layout) };
        allocated(pointer, layout.size());
        pointer
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        // SAFETY: pointer and layout retain the caller's original contract.
        unsafe { System.dealloc(pointer, layout) };
        freed(layout.size());
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        // SAFETY: System owns the old block and the caller supplies a valid size.
        let new = unsafe { System.realloc(pointer, layout, new_size) };
        if new.is_null() {
            add(&FAILED, 1); // Failed realloc preserves the old live allocation.
            return new;
        }
        // System may grow in place or allocate-copy-free internally. Account
        // logical replacement exactly, and retain the conservative coexistence
        // bound separately instead of claiming the internal peak was observed.
        let upper = LIVE
            .load(Relaxed)
            .checked_add(new_size)
            .unwrap_or_else(|| refused());
        UPPER.fetch_max(upper, Relaxed);
        freed(layout.size());
        allocated(new, new_size);
        new
    }
}

fn reset() {
    let live = LIVE.load(Relaxed);
    START.store(live, Relaxed);
    PEAK.store(live, Relaxed);
    UPPER.store(live, Relaxed);
    for counter in [&TOTAL, &ALLOCS, &FREES, &FAILED] {
        counter.store(0, Relaxed);
    }
}

#[expect(unsafe_code, reason = "private idle allocation-meter control")]
#[unsafe(no_mangle)]
extern "C" fn checkpoint_alloc_reset() {
    crate::job_control::controller();
    reset();
    ic0::msg_reply();
}

#[expect(unsafe_code, reason = "private allocation evidence query")]
#[unsafe(no_mangle)]
extern "C" fn checkpoint_alloc_read() {
    let mut bytes = [0u8; 64];
    for (index, counter) in [
        &START, &LIVE, &PEAK, &UPPER, &TOTAL, &ALLOCS, &FREES, &FAILED,
    ]
    .into_iter()
    .enumerate()
    {
        bytes[index * 8..index * 8 + 8]
            .copy_from_slice(&(counter.load(Relaxed) as u64).to_le_bytes());
    }
    ic0::msg_reply_data_append(&bytes);
    ic0::msg_reply();
}

#[expect(
    unsafe_code,
    reason = "installed calibration owns and frees every System allocation"
)]
#[unsafe(no_mangle)]
extern "C" fn checkpoint_alloc_calibrate() {
    crate::job_control::controller();
    let [fail, reserved_a, reserved_b] = crate::job_control::args();
    assert!(fail <= 2 && reserved_a == 0 && reserved_b == 0);
    let layout = |size| Layout::from_size_align(size, 8).unwrap();
    // SAFETY: all layouts are valid and nonzero; accesses stay within successful
    // allocations; failed realloc retains its original block. No pointers escape.
    unsafe {
        let a = ALLOCATOR.alloc(layout(32));
        let b = ALLOCATOR.alloc_zeroed(layout(64));
        assert!(!a.is_null() && !b.is_null());
        if fail == 2 {
            reset(); // Existing allocations remain live across a meter reset.
        }
        a.write_bytes(0x5a, 32);
        assert!((0..64).all(|index| b.add(index).read() == 0));
        let a = ALLOCATOR.realloc(a, layout(32), 96);
        assert!(!a.is_null());
        assert!((0..32).all(|index| a.add(index).read() == 0x5a));
        let a = ALLOCATOR.realloc(a, layout(96), 16);
        assert!(!a.is_null());
        assert!((0..16).all(|index| a.add(index).read() == 0x5a));
        // The admitted module maximum is 16 MiB, so these 1 GiB requests fail.
        assert!(ALLOCATOR.alloc(layout(1 << 30)).is_null());
        assert!(ALLOCATOR.realloc(a, layout(16), 1 << 30).is_null());
        assert!((0..16).all(|index| a.add(index).read() == 0x5a));
        ALLOCATOR.dealloc(a, layout(16));
        ALLOCATOR.dealloc(b, layout(64));
    }
    if fail == 1 {
        ic0::trap(b"allocation calibration rollback");
    }
    ic0::msg_reply();
}

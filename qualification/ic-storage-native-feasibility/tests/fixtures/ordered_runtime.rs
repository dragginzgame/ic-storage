//! Closed wasm32 fixture memory contract: transient allocations never reach roots.
use core::{
    alloc::{GlobalAlloc, Layout},
    ptr,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};
use ic_platform_runtime::raw as ic0;
use ic_storage::checkpoint::Memory;
use sha2::{Digest, Sha256};

pub(super) const HEADER: usize = 1_572_864;
pub(super) const A: usize = 4_194_304;
pub(super) const B: usize = 6_291_456;
const SCRATCH: usize = 2_097_152;
const END: usize = A;
static NEXT: AtomicUsize = AtomicUsize::new(SCRATCH);
const METADATA: usize = HEADER + 65_536;
const METADATA_END: usize = METADATA + 65_536;
const _: () = assert!(METADATA_END <= SCRATCH);
static META_NEXT: AtomicUsize = AtomicUsize::new(METADATA);
static META_LIMIT: AtomicUsize = AtomicUsize::new(METADATA_END);
static BUILDING_OWNER: AtomicBool = AtomicBool::new(false);
struct Transient;
#[global_allocator]
static GLOBAL: Transient = Transient;

// SAFETY: this closed executable owns disjoint metadata and scratch windows.
// Only Owner construction selects metadata; it outlives ordinary calls and is
// reset only after the public grant, Rc and directory have ended. Scratch contains
// only per-entry values and resets after the previous entry returns. Native maps
// use separate Pool allocations. All regions are admitted within fixed memory.
#[expect(unsafe_code, reason = "closed transient-window allocation contract")]
unsafe impl GlobalAlloc for Transient {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let (next, limit) = if BUILDING_OWNER.load(Ordering::Relaxed) {
            (&META_NEXT, META_LIMIT.load(Ordering::Relaxed))
        } else {
            (&NEXT, END)
        };
        let previous = next.load(Ordering::Relaxed);
        let Some(start) = previous
            .checked_add(layout.align() - 1)
            .map(|n| n & !(layout.align() - 1))
        else {
            return ptr::null_mut();
        };
        let Some(end) = start.checked_add(layout.size()).filter(|end| *end <= limit) else {
            return ptr::null_mut();
        };
        next.store(end, Ordering::Relaxed);
        ptr::with_exposed_provenance_mut(start)
    }
    unsafe fn dealloc(&self, _: *mut u8, _: Layout) {}
}

pub(super) fn enter() {
    assert!(!BUILDING_OWNER.load(Ordering::Relaxed));
    NEXT.store(SCRATCH, Ordering::Relaxed);
}
// Called only for the public owner's nonescaping metadata allocations. A trap
// rolls back this selector together with metadata/native state; no host unwind.
pub(super) fn metadata<R>(build: impl FnOnce() -> R) -> R {
    assert!(!BUILDING_OWNER.swap(true, Ordering::Relaxed));
    assert_eq!(META_NEXT.load(Ordering::Relaxed), METADATA);
    let result = build();
    BUILDING_OWNER.store(false, Ordering::Relaxed);
    result
}
// Owner::end first destroys all Rc/grant/directory carriers. Native allocations
// remain live in their original pools and never reside in this window.
pub(super) fn release_metadata() {
    assert!(!BUILDING_OWNER.load(Ordering::Relaxed));
    META_NEXT.store(METADATA, Ordering::Relaxed);
}
pub(super) fn metadata_bytes() -> u64 {
    (META_NEXT.load(Ordering::Relaxed) - METADATA) as u64
}
pub(super) fn refuse_metadata() {
    assert_eq!(metadata_bytes(), 0);
    META_LIMIT.store(METADATA, Ordering::Relaxed);
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

// Three additional root descriptors, each containing node span, tree scalars,
// and byte span. The closed format's magic fixes their identities and bounds.
const EXTRA: usize = HEADER + 128;
const SEAL: usize = EXTRA + 3 * 9 * 8;
#[expect(
    unsafe_code,
    reason = "closed initialized auxiliary descriptor interval"
)]
pub(super) fn auxiliary() -> [[u64; 9]; 3] {
    unsafe { ptr::read(ptr::with_exposed_provenance(EXTRA)) }
}
#[expect(
    unsafe_code,
    reason = "settled auxiliary descriptors before sealed publication"
)]
pub(super) fn publish_auxiliary(roots: [[u64; 9]; 3]) {
    unsafe { ptr::write(ptr::with_exposed_provenance_mut(EXTRA), roots) };
}
fn directory_seal(words: &[u64; 14]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"ORDROOT4");
    for word in words.iter().chain(auxiliary().iter().flatten()) {
        hash.update(word.to_le_bytes());
    }
    hash.finalize().into()
}
#[expect(
    unsafe_code,
    reason = "read the initialized byte-defined directory seal"
)]
pub(super) fn seal() -> [u8; 32] {
    unsafe { ptr::read(ptr::with_exposed_provenance(SEAL)) }
}
pub(super) fn verify_directory(words: &[u64; 14]) {
    assert_eq!(seal(), directory_seal(words), "native root directory seal");
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
    // SAFETY: the fixed seal interval is disjoint from headers, metadata and pools.
    unsafe {
        ptr::write(
            ptr::with_exposed_provenance_mut(SEAL),
            directory_seal(&words),
        )
    };
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

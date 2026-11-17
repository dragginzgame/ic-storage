//! Fixture-only custody for the fixed retained interval, never the live stack.
//!
//! Admission binds the reviewed raw module, one nonshared wasm32 memory, a
//! stack-first 1 MiB stack, data/heap below 8 MiB, and the controlled bootstrap.
//! No source path allocates, spawns callbacks or runs an executor. This module
//! alone creates retained slices and grows memory. A busy guard forbids nesting;
//! slices end before growth and before replies. Traps rely on IC message rollback.

use std::{
    arch::wasm32,
    sync::atomic::{AtomicBool, Ordering},
};

use ic_platform_runtime::raw as ic0;
use ic_storage_m0::m0::{
    image::{Error, Image, Session},
    inspect_retained,
};

const BASE: usize = 8 * 1024 * 1024;
const PAGE: usize = 65536;
static BUSY: AtomicBool = AtomicBool::new(false);
static READY: AtomicBool = AtomicBool::new(false);

// std's panic machinery must not obtain persistent pages through dlmalloc.
// No normal fixture path allocates; an unexpected allocation/deallocation traps
// before touching memory. This is a refusal policy, not a transient allocator.
struct DenyAllocator;
#[global_allocator]
static ALLOCATOR: DenyAllocator = DenyAllocator;

// SAFETY: these methods never return and never dereference a supplied pointer,
// so they cannot supply invalid storage or invoke a foreign deallocator.
#[expect(
    unsafe_code,
    reason = "fixture allocator refuses all heap access by trapping"
)]
unsafe impl std::alloc::GlobalAlloc for DenyAllocator {
    unsafe fn alloc(&self, _: std::alloc::Layout) -> *mut u8 {
        wasm32::unreachable()
    }
    unsafe fn dealloc(&self, _: *mut u8, _: std::alloc::Layout) {
        wasm32::unreachable()
    }
}

struct Guard;
impl Guard {
    fn acquire() -> Self {
        if BUSY.swap(true, Ordering::Relaxed) {
            wasm32::unreachable();
        }
        Self
    }
}
impl Drop for Guard {
    fn drop(&mut self) {
        BUSY.store(false, Ordering::Relaxed);
    }
}

fn require<T, E>(result: Result<T, E>) -> T {
    match result {
        Ok(value) => value,
        Err(_) => wasm32::unreachable(),
    }
}

fn pages() -> u32 {
    let pages = wasm32::memory_size::<0>();
    if !(129..=256).contains(&pages) {
        wasm32::unreachable();
    }
    pages as u32
}

// Guard possession is private and unique. The higher-ranked callback cannot
// return a borrowed slice. Caller-selected pages never manufacture extra memory.
fn retained<R>(_guard: &mut Guard, f: impl FnOnce(&mut [u8], u32) -> R) -> R {
    let pages = pages();
    let length = pages as usize * PAGE - BASE;
    // SAFETY: module admission reserves [BASE, memory.size*PAGE) exclusively for
    // this kernel, disjoint from stack/data and all allocator use. Wasm memory is
    // initialized and contiguous, BASE is nonzero, and length <= 8 MiB < isize::MAX.
    // The unique guard and synchronous callback exclude aliasing/reentry. No
    // memory.grow occurs during this borrow; callbacks here are private and
    // reviewed. The slice lifetime cannot escape into the return value.
    #[expect(unsafe_code, reason = "reviewed fixture-only retained memory custody")]
    let bytes = unsafe { std::slice::from_raw_parts_mut(BASE as *mut u8, length) };
    f(bytes, pages)
}

pub(super) fn initialize() {
    let mut guard = Guard::acquire();
    if READY.load(Ordering::Relaxed) || pages() != 129 {
        wasm32::unreachable();
    }
    retained(&mut guard, |bytes, pages| {
        require(Image::initialize_fresh(bytes, pages));
    });
    READY.store(true, Ordering::Relaxed);
}

pub(super) fn admit() {
    let mut guard = Guard::acquire();
    retained(&mut guard, |bytes, _| {
        if require(inspect_retained(bytes)).root.is_none() {
            wasm32::unreachable();
        }
    });
    READY.store(true, Ordering::Relaxed);
}

pub(super) fn update<R>(f: impl for<'id> FnOnce(&mut Session<'id>) -> Result<R, Error>) -> R {
    let mut guard = Guard::acquire();
    if !READY.load(Ordering::Relaxed) {
        wasm32::unreachable();
    }
    retained(&mut guard, |bytes, pages| {
        let mut image = require(Image::open(bytes, pages));
        require(require(image.with_update(f)))
    })
}

pub(super) fn grow() -> bool {
    let mut guard = Guard::acquire();
    if !READY.load(Ordering::Relaxed) {
        wasm32::unreachable();
    }
    let old = retained(&mut guard, |bytes, pages| {
        require(inspect_retained(bytes));
        pages
    });
    if old == 256 {
        return false;
    }
    // No retained slice or session exists across the memory.grow instruction.
    let previous = wasm32::memory_grow::<0>(1);
    if previous == usize::MAX {
        return false;
    }
    if previous != old as usize {
        wasm32::unreachable();
    }
    retained(&mut guard, |bytes, new| {
        if new != old + 1 {
            wasm32::unreachable();
        }
        // Open the previously admitted prefix with the now-physical extra page
        // as capacity, then publish its extent and digest through the M0 writer.
        let mut image = require(Image::open(bytes, old));
        require(require(image.with_update(|s| s.grow(1))));
    });
    true
}

pub(super) fn evidence() {
    let mut guard = Guard::acquire();
    if !READY.load(Ordering::Relaxed) {
        wasm32::unreachable();
    }
    retained(&mut guard, |bytes, _| {
        require(inspect_retained(bytes));
        if bytes.len() > 3 * PAGE {
            wasm32::unreachable();
        }
        for chunk in bytes.chunks(256) {
            let mut copy = [0; 256];
            copy[..chunk.len()].copy_from_slice(chunk);
            ic0::msg_reply_data_append(&copy[..chunk.len()]);
        }
    });
}

//! Compile-only vector/provider diagnostic. Never admitted by the scalar linker.
//! The foreign loan and initialized prefix are preconditions, not implemented here.
#![no_std]
#![deny(unsafe_code, unsafe_op_in_unsafe_fn, missing_docs, dead_code)]

extern crate alloc;

mod region;

use alloc::vec::Vec;
use core::{
    alloc::{GlobalAlloc, Layout},
    ptr::NonNull,
};
use region::{Descriptor, Region};

// Linking alloc requires a global allocator even though this client selects
// Region explicitly. Refuse every Global request; never borrow provider memory.
struct RejectGlobal;

#[global_allocator]
static GLOBAL: RejectGlobal = RejectGlobal;

// SAFETY: alloc always reports failure and issues no block. Thus no valid
// deallocation can reach this implementation; violations trap without unwinding.
#[expect(
    unsafe_code,
    reason = "fail-closed Global allocator for a no_std compiler fixture"
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

fn abort() -> ! {
    core::arch::wasm32::unreachable()
}

#[expect(
    unsafe_code,
    reason = "hypothetical loan contract for compiler diagnostics only"
)]
#[link(wasm_import_module = "native_vector_diagnostic")]
unsafe extern "C" {
    fn claim_region() -> *mut u8;
}

/// Rebuilds a transient vector, performs one operation and releases its header.
/// Returns a diagnostic packed length/capacity, never a pointer or std header.
///
/// # Safety
/// Requires a real exclusive, aligned, live 4096-byte provider loan with the
/// admitted initialized prefix described by length/capacity. All source owners
/// must have ended. A trusted caller must commit the returned descriptor and
/// settle only after return; traps must abort the entire transaction. Neither
/// this source nor its compiler output supplies that provider/admission contract.
#[expect(
    unsafe_code,
    reason = "compile-only raw entrypoint under explicit provider preconditions"
)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn provider_vector_step(
    length: u32,
    capacity: u32,
    operation: u32,
    value: u64,
) -> u64 {
    if operation > 5 {
        abort();
    }
    // SAFETY: the diagnostic caller's contract supplies one exclusive initialized
    // provider loan. The controller and all clients end before this call returns.
    let controller = unsafe {
        let base = NonNull::new(claim_region()).unwrap_or_else(|| abort());
        Region::claim(
            base,
            512,
            Descriptor {
                length: length as usize,
                capacity: capacity as usize,
            },
        )
        .unwrap_or_else(|_| abort())
    };
    let mut values = controller.restore().unwrap_or_else(|_| abort());
    match operation {
        0 => {
            if let Some(first) = values.first_mut() {
                *first = first.wrapping_add(value);
            }
        }
        1 => {
            values.try_reserve_exact(1).unwrap_or_else(|_| abort());
            values.push(value);
        }
        2 => {
            if !values.is_empty() {
                values.remove(0);
            }
        }
        3 => values.shrink_to_fit(),
        4 => {
            values.clear();
            values.shrink_to_fit();
        }
        5 => {
            drop(values);
            values = Vec::new_in(&controller);
            values.try_reserve_exact(1).unwrap_or_else(|_| abort());
            values.push(value);
        }
        _ => abort(),
    }
    let root = controller.release(values).unwrap_or_else(|_| abort());
    ((root.length as u64) << 32) | root.capacity as u64
}

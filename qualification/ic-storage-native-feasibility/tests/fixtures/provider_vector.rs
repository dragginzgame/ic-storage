//! Bounded vector/provider fixture. Never admitted by the scalar linker.
//! Installed use requires vector-provider-contract.md and the final link checks.
#![no_std]
#![deny(unsafe_code, unsafe_op_in_unsafe_fn, missing_docs, dead_code)]

extern crate alloc;

mod region;
mod runtime;

use alloc::vec::Vec;
use core::ptr::NonNull;
use ic_storage::with_vector;
use region::{Descriptor, Region};
use runtime::abort;

#[expect(
    unsafe_code,
    reason = "exclusive foreign backing under the bounded vector provider contract"
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
    reason = "private raw entrypoint under the bounded vector provider contract"
)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn provider_vector_step(
    length: u32,
    capacity: u32,
    operation: u32,
    value: u64,
) -> u64 {
    if operation > 7 {
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
    if operation == 5 {
        drop(values);
        values = Vec::new_in(&controller);
    }
    let outcome = with_vector(&mut values, 512, |mut view| {
        match operation {
            0 | 7 => {
                if let Some(first) = view.values_mut().first_mut() {
                    *first = first.wrapping_add(value);
                }
                if operation == 7 {
                    #[expect(
                        clippy::forget_non_drop,
                        reason = "qualify forgotten scoped views on IC"
                    )]
                    core::mem::forget(view);
                }
            }
            1 | 5 => {
                view.push(value).unwrap_or_else(|_| abort());
            }
            2 => {
                let _ = view.remove_first();
            }
            3 => view.shrink_to_fit(),
            4 => {
                view.clear();
                view.shrink_to_fit();
            }
            6 => {
                if let Some(first) = view.values_mut().first_mut() {
                    *first = first.wrapping_add(value);
                }
                // A normal callback error preserves this mutation and any successful
                // append. Allocation/limit refusal also leaves the preceding write.
                view.push(value).map_err(|_| ())?;
                return Err(());
            }
            _ => abort(),
        }
        Ok(())
    })
    .unwrap_or_else(|_| abort());
    if outcome.is_err() != (operation == 6) {
        abort();
    }
    // Finalize even for a normal callback error or a forgotten non-owning view.
    let root = controller.release(values).unwrap_or_else(|_| abort());
    ((root.length as u64) << 32) | root.capacity as u64
}

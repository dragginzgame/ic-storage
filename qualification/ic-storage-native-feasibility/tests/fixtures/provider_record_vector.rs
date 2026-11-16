//! Closed V3 compiler fixture; installed use still requires its own admission.
#![no_std]
#![deny(unsafe_code, unsafe_op_in_unsafe_fn, missing_docs, dead_code)]

extern crate alloc;

mod region;
mod runtime;

use alloc::vec::Vec;
use core::ptr::NonNull;
use ic_storage_native_feasibility::record::narrow::V3;
use region::{Descriptor, Region};
use runtime::abort;

impl region::sealed::Sealed for V3 {}
impl region::Element for V3 {}

// This is the actual shared derive declaration compiled for wasm32, not a
// matching host size or a second hand-written representation.
const _: () = {
    let mut index = 0;
    while index < 8 {
        assert!(V3::STORE_ID[index] == b"RECU64V1"[index]);
        index += 1;
    }
    assert!(V3::STORE_VERSION == 3);
    assert!(V3::STORE_SIZE == 16 && V3::STORE_ALIGN == 8 && V3::STORE_BYTES == 12);
    assert!(V3::STORE_FIELDS[0].id == 1 && V3::STORE_FIELDS[0].offset == 0);
    assert!(V3::STORE_FIELDS[1].id == 2 && V3::STORE_FIELDS[1].offset == 8);
};

#[expect(
    unsafe_code,
    reason = "exclusive foreign record backing in compiler fixture"
)]
#[link(wasm_import_module = "native_record_vector_diagnostic")]
unsafe extern "C" {
    fn claim_region() -> *mut u8;
}

fn element(value: u64) -> V3 {
    let counter = u32::try_from(value).unwrap_or_else(|_| abort());
    V3::new(counter, 1000).unwrap_or_else(|_| abort())
}

fn add_first(values: &mut [V3], value: u64) {
    let delta = u32::try_from(value).unwrap_or_else(|_| abort());
    if let Some(first) = values.first_mut() {
        first
            .store_with(|mut view| view.add(delta))
            .unwrap_or_else(|_| abort());
    }
}

/// Reconstruct one bounded record vector and return only its new length/capacity.
///
/// # Safety
/// Requires one exclusive live aligned 256-byte provider allocation. Its first
/// `length` elements must be initialized valid V3 values, with admitted schema
/// and `length <= capacity <= 16`. Prior owners must have ended. The trusted
/// wrapper settles after return; any trap must abort the entire transaction.
#[expect(
    unsafe_code,
    reason = "private entrypoint with explicit foreign loan contract"
)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn provider_record_vector_step(
    length: u32,
    capacity: u32,
    operation: u32,
    value: u64,
) -> u64 {
    if operation > 7 {
        abort();
    }
    // SAFETY: the caller supplies the admitted initialized prefix and exclusive
    // foreign loan. No owner/header survives this entrypoint or its controller.
    let controller = unsafe {
        let base = NonNull::new(claim_region()).unwrap_or_else(|| abort());
        Region::<V3>::claim(
            base,
            16,
            Descriptor {
                length: length as usize,
                capacity: capacity as usize,
            },
        )
        .unwrap_or_else(|_| abort())
    };
    let mut values = controller.restore().unwrap_or_else(|_| abort());
    match operation {
        0 => add_first(&mut values, value),
        1 | 5 => {
            if operation == 5 {
                drop(values);
                values = Vec::new_in(&controller);
            }
            let next = element(value);
            values.try_reserve(1).unwrap_or_else(|_| abort());
            values.push(next);
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
        6 => {
            // Business refusal retains earlier writes and still releases the
            // real owner. Failed reserve leaves the prior vector intact.
            add_first(&mut values, value);
            let next = element(value);
            let outcome: Result<(), ()> = (|| {
                values.try_reserve(1).map_err(|_| ())?;
                values.push(next);
                Err(())
            })();
            if outcome.is_ok() {
                abort();
            }
        }
        7 => {
            let delta = u32::try_from(value).unwrap_or_else(|_| abort());
            if let Some(first) = values.first_mut() {
                first.store_with(|mut view| {
                    view.add(delta).unwrap_or_else(|_| abort());
                    #[expect(clippy::forget_non_drop, reason = "qualify a forgotten scalar view")]
                    core::mem::forget(view);
                });
            }
        }
        _ => abort(),
    }
    let root = controller.release(values).unwrap_or_else(|_| abort());
    ((root.length as u64) << 32) | root.capacity as u64
}

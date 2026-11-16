//! Closed native map entrypoint; the foreign provider owns admission and custody.
#![no_std]
#![deny(unsafe_code, unsafe_op_in_unsafe_fn, missing_docs, dead_code)]

mod region;
mod runtime;

use core::ptr::NonNull;
use ic_storage_native::map::{Error, Map, Slot};
use region::{Descriptor, Region};
use runtime::abort;

impl region::sealed::Sealed for Slot {}
impl region::Element for Slot {}

const _: () = assert!(size_of::<Slot>() == 24 && align_of::<Slot>() == 8);

#[expect(
    unsafe_code,
    reason = "exclusive foreign loan for the closed map schema"
)]
#[link(wasm_import_module = "native_map_diagnostic")]
unsafe extern "C" {
    fn claim_region() -> *mut u8;
}

/// Reopen a native map, optionally rebuild in the other bank, and release owners.
///
/// # Safety
/// Requires the closed map provider's exclusive live aligned 24 KiB allocation,
/// split into two disjoint 512-Slot banks. `root` encodes buckets in its low 16
/// bits and the active bank in its upper 16 bits. The active prefix is initialized
/// and its schema/root admitted; the inactive bank has no owning client. All
/// previous owners have ended. The provider publishes the returned entry count
/// and bucket descriptor only after return; unexpected failures trap the message.
#[expect(
    unsafe_code,
    reason = "private entrypoint under the foreign map loan contract"
)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn provider_map_step(
    count: u32,
    root: u32,
    operation: u32,
    argument: u64,
) -> u64 {
    if operation > 10 || root >> 16 > 1 {
        abort();
    }
    let buckets = root & 0xffff;
    let active = root >> 16;
    // SAFETY: pointer custody and initialized native Slot representation are
    // supplied by the trusted foreign provider, before any Rust interpretation.
    let (controller, spare) = unsafe {
        let base = NonNull::new(claim_region()).unwrap_or_else(|| abort());
        let source = Region::<Slot>::claim(
            NonNull::new_unchecked(base.as_ptr().add(active as usize * 12288)),
            512,
            Descriptor {
                length: buckets as usize,
                capacity: buckets as usize,
            },
        )
        .unwrap_or_else(|_| abort());
        let spare = Region::<Slot>::claim(
            NonNull::new_unchecked(base.as_ptr().add((1 - active) as usize * 12288)),
            512,
            Descriptor {
                length: 0,
                capacity: 0,
            },
        )
        .unwrap_or_else(|_| abort());
        (source, spare)
    };
    let slots = controller.restore().unwrap_or_else(|_| abort());
    let mut map = Map::admit(slots, count as usize, 512).unwrap_or_else(|_| abort());
    // The closed operation wire uses two u32 fields, independently of the
    // product's full-width u64 key/value representation.
    let key = argument & 0xffff_ffff;
    let value = argument >> 32;
    let mut bank = active;
    if matches!(operation, 1 | 5 | 6 | 7 | 9 | 10)
        && map
            .with(|view| view.get(key).is_none())
            .unwrap_or_else(|_| abort())
        && count as usize == map.capacity().unwrap_or_else(|_| abort())
    {
        // The source remains live until every native entry has been copied.
        // A zero-capacity restoration opens the disjoint destination allocator.
        drop(spare.restore().unwrap_or_else(|_| abort()));
        match map.rebuild_in(count as usize + 1, &spare) {
            Ok(candidate) => {
                if operation == 10 {
                    abort();
                } // complete candidate, old owner live
                map = candidate; // retire the old header only after full construction
                bank = 1 - active;
            }
            Err(Error::Allocation) if operation == 7 => {}
            _ => abort(),
        }
    }
    let outcome = map
        .with(|mut view| {
            match operation {
                0 => {}
                1 | 5 | 6 | 9 => {
                    view.insert(key, value).unwrap_or_else(|_| abort());
                    if operation == 5 {
                        return Err(());
                    }
                    if operation == 6 {
                        #[expect(
                            clippy::forget_non_drop,
                            reason = "installed forgotten-view settlement"
                        )]
                        core::mem::forget(view);
                    }
                }
                2 => {
                    if view.remove(key) != Some(value) {
                        abort();
                    }
                }
                3 => {
                    if view.get(key) != Some(value) {
                        abort();
                    }
                }
                4 => {
                    if view.get(key).is_some() {
                        abort();
                    }
                }
                7 => {
                    if view.insert(key, value) != Err(Error::Allocation) {
                        abort();
                    }
                }
                8 => view.clear(),
                _ => abort(),
            }
            Ok(())
        })
        .unwrap_or_else(|_| abort());
    if outcome.is_err() != (operation == 5) {
        abort();
    }
    let (slots, count) = map.release().unwrap_or_else(|_| abort());
    let owner = if bank == active { &controller } else { &spare };
    let root = owner.release(slots).unwrap_or_else(|_| abort());
    if root.length != root.capacity {
        abort();
    }
    ((count as u64) << 32) | ((bank as u64) << 16) | root.capacity as u64
}

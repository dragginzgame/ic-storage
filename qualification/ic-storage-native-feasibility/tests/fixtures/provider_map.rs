//! Closed two-root map entrypoint under one exclusive foreign pool loan.
#![no_std]
#![deny(unsafe_code, unsafe_op_in_unsafe_fn, missing_docs, dead_code)]

#[expect(
    dead_code,
    reason = "shared region module also owns the separate single-vector fixture"
)]
mod region;
mod runtime;

use core::ptr::NonNull;
use ic_storage::map::{Error, Map, Slot};
use region::pool::{Pool, Root};
use runtime::abort;

impl region::sealed::Sealed for Slot {}
impl region::Element for Slot {}
const _: () = assert!(size_of::<Slot>() == 24 && align_of::<Slot>() == 8);

#[expect(unsafe_code, reason = "exclusive foreign pool loan")]
#[link(wasm_import_module = "native_map_diagnostic")]
unsafe extern "C" {
    fn claim_region() -> *mut u8;
    // Scalar-only, non-reentrant provider callback. Fields 0/1 read the next
    // staged key/value; field 2 acknowledges the completed operation.
    safe fn staged_word(index: u32, field: u32) -> u64;
}

fn decode(word: u32) -> (Root, usize) {
    let exponent = (word >> 12) & 15;
    if !(3..=12).contains(&exponent) {
        abort();
    }
    let buckets = 1usize << exponent;
    let count = (word & 4095) as usize;
    if count > buckets - buckets / 4 {
        abort();
    }
    (
        Root {
            start: (word >> 16) as usize,
            length: buckets,
            capacity: buckets,
        },
        count,
    )
}

fn encode(root: Root, count: usize) -> u32 {
    if root.length != root.capacity
        || !(8..=4096).contains(&root.capacity)
        || !root.capacity.is_power_of_two()
        || root.start >= 8192
        || count > root.capacity - root.capacity / 4
    {
        abort();
    }
    (root.start as u32) << 16 | root.capacity.trailing_zeros() << 12 | count as u32
}

/// Reopen both maps, perform one closed operation and release the complete pool.
///
/// # Safety
/// The provider supplies one exclusive live aligned allocation containing the
/// admitted number of initialized Slots (encoded in operation's upper 16 bits).
/// Provider growth occurs only between loans, with all former owners ended;
/// no pointer or allocator survives it. The two roots must have admitted representation,
/// counts and disjoint spans. Previous owners have ended. No provider access
/// to the loan is allowed during native ownership. The scalar staging callback
/// reads only disjoint provider staging, cannot re-enter Rust and transfers no
/// pointer or reference. The returned roots are published only after return;
/// every unexpected failure traps the complete IC message.
#[expect(
    unsafe_code,
    reason = "closed foreign allocation and ownership boundary"
)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn provider_map_step(
    first: u32,
    second: u32,
    operation: u32,
    key: u64,
    value: u64,
) -> u64 {
    let elements = (operation >> 16) as usize;
    let operation = operation & 65535;
    let selected = (operation >> 8) as usize;
    let command = operation & 255;
    if selected > 1 || command > 13 || !matches!(elements, 1024 | 2048 | 4096 | 8192) {
        abort();
    }
    let (a, count_a) = decode(first);
    let (b, count_b) = decode(second);
    // SAFETY: the foreign claim creates the live loan; Pool separately checks
    // complete-span bounds and nonoverlap before issuing either allocation.
    let pool = unsafe {
        Pool::<Slot>::claim(
            NonNull::new(claim_region()).unwrap_or_else(|| abort()),
            elements,
            &[a, b],
        )
    }
    .unwrap_or_else(|_| abort());
    let mut maps = [
        Map::admit(pool.restore(0).unwrap_or_else(|_| abort()), count_a, 6144)
            .unwrap_or_else(|_| abort()),
        Map::admit(pool.restore(1).unwrap_or_else(|_| abort()), count_b, 6144)
            .unwrap_or_else(|_| abort()),
    ];
    if command == 4 {
        // Explicit destructive reset retires this root first. Any subsequent
        // failure traps the IC message and restores the entire prior pool.
        maps[selected] = Map::new_in(6144, &pool);
        maps[selected]
            .with(|mut view| view.reserve(6))
            .unwrap_or_else(|_| abort())
            .unwrap_or_else(|_| abort());
    } else {
        let result = maps[selected]
            .with(|mut view| {
                match command {
                    0 => {}
                    1 | 5 | 6 | 8 | 9 | 10 => {
                        view.insert(key, value).unwrap_or_else(|_| abort());
                        if command == 5 {
                            return Err(());
                        }
                        if command == 6 {
                            #[expect(
                                clippy::forget_non_drop,
                                reason = "forgotten view must not skip settlement"
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
                    7 => {
                        if view.insert(key, value) != Err(Error::Allocation) {
                            abort();
                        }
                    }
                    11 => {
                        if key > value || value > 3072 {
                            abort();
                        }
                        for key in key..value {
                            view.insert(key, key + 1000).unwrap_or_else(|_| abort());
                        }
                    }
                    12 => {
                        // Reserve one staged page without inserting records. An
                        // allocation refusal preserves the old root; after both
                        // owners return, the provider may expand and retry.
                        if key > 64 || value != 0 {
                            abort();
                        }
                        match view.reserve(key as usize) {
                            Ok(()) | Err(Error::Allocation) => {}
                            Err(_) => abort(),
                        }
                    }
                    13 => {
                        // One admitted scope for a bounded batch. Mode 0 is the
                        // recovery insert-only contract; other modes are ordinary
                        // lookup, replacement and deletion against existing keys.
                        if key == 0 || key > 64 || value > 3 {
                            abort();
                        }
                        for index in 0..key as u32 {
                            let key = staged_word(index, 0);
                            let expected = staged_word(index, 1);
                            let valid = match value {
                                0 => view
                                    .insert(key, expected)
                                    .unwrap_or_else(|_| abort())
                                    .is_none(),
                                1 => view.get(key) == Some(expected),
                                2 => view
                                    .insert(key, expected)
                                    .unwrap_or_else(|_| abort())
                                    .is_some(),
                                3 => view.remove(key) == Some(expected),
                                _ => abort(),
                            };
                            if !valid || staged_word(index, 2) != 0 {
                                abort();
                            }
                        }
                    }
                    _ => abort(),
                }
                Ok(())
            })
            .unwrap_or_else(|_| abort());
        if result.is_err() != (command == 5) {
            abort();
        }
    }
    if command == 9 || command == 10 {
        maps[1 - selected]
            .with(|mut view| view.insert(key, value + 1))
            .unwrap_or_else(|_| abort())
            .unwrap_or_else(|_| abort());
    }
    let [a, b] = maps;
    let (slots, count_a) = a.release().unwrap_or_else(|_| abort());
    let a = pool
        .release(slots)
        .unwrap_or_else(|_| abort())
        .unwrap_or_else(|| abort());
    if command == 8 || command == 10 {
        abort();
    }
    let (slots, count_b) = b.release().unwrap_or_else(|_| abort());
    let b = pool
        .release(slots)
        .unwrap_or_else(|_| abort())
        .unwrap_or_else(|| abort());
    pool.finish().unwrap_or_else(|_| abort());
    u64::from(encode(a, count_a)) | (u64::from(encode(b, count_b)) << 32)
}

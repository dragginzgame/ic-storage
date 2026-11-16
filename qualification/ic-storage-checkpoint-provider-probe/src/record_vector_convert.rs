//! Retained conversion uses scalar callbacks, never old native Rust owners.

use crate::record_vector::args;
use ic_platform_runtime::raw as ic0;
use ic_storage_native_feasibility::{
    checkpoint::record_vector::LIMIT,
    record::{V2, narrow::V3},
};

#[expect(unsafe_code, reason = "closed retained-conversion scalar callbacks")]
#[link(wasm_import_module = "native_checkpoint")]
unsafe extern "C" {
    safe fn convert_count() -> u32;
    safe fn convert_value(index: u32, field: u32) -> u64;
    safe fn convert_begin();
    safe fn convert_put(index: u32, counter: u32, ceiling: u64);
    safe fn convert_finish();
}

#[expect(
    unsafe_code,
    reason = "private versioned conversion bound by the composer"
)]
#[unsafe(no_mangle)]
extern "C" fn checkpoint_vector_convert() {
    let [reserved, fault] = args();
    assert_eq!(reserved, 0);
    assert!(fault <= 3);
    let count = convert_count();
    assert!(count <= LIMIT as u32);
    // Read and validate the entire predecessor before any narrowing. Nothing
    // escapes as an old pointer, reference, allocator or owning Vec.
    let mut source = [None; LIMIT];
    for index in 0..count {
        source[index as usize] =
            Some(V2::new(convert_value(index, 0), convert_value(index, 1)).unwrap());
    }
    let mut candidate = [None; LIMIT];
    for index in 0..count as usize {
        candidate[index] = Some(
            V3::try_from(source[index].unwrap())
                .unwrap_or_else(|_| ic0::trap(b"retained vector narrowing out of range")),
        );
    }
    if fault == 1 {
        ic0::trap(b"retained vector before replacement");
    }
    convert_begin();
    for (index, record) in candidate.into_iter().take(count as usize).enumerate() {
        let record = record.unwrap();
        convert_put(index as u32, record.counter(), record.ceiling());
        if fault == 2 {
            ic0::trap(b"retained vector during replacement");
        }
    }
    convert_finish();
    if fault == 3 {
        ic0::trap(b"retained vector after publication");
    }
}

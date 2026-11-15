//! Diagnostic input for compiler layout metadata, never a retained canister.
//!
//! The outer C representation does not establish the private Vec representation.
//! Keeping the record visible to the debugger requests recursive type metadata.

use std::{hint::black_box, mem};

#[repr(C)]
#[cfg(not(feature = "layout-swapped"))]
struct LayoutProbeRecord {
    counter: u64,
    values: Vec<u64>,
}

// Same total size/alignment as the first record on the selected targets, but
// incompatible field offsets. This is a diagnostic negative fixture.
#[repr(C)]
#[cfg(feature = "layout-swapped")]
struct LayoutProbeRecord {
    values: Vec<u64>,
    counter: u64,
}

fn main() {
    let mut record = LayoutProbeRecord {
        counter: 17,
        values: vec![3, 5, 8],
    };
    black_box(&mut record);
    println!(
        "record_size={} record_align={} counter_offset={} values_offset={} vector_size={} vector_align={} element_size={} element_align={}",
        mem::size_of::<LayoutProbeRecord>(),
        mem::align_of::<LayoutProbeRecord>(),
        mem::offset_of!(LayoutProbeRecord, counter),
        mem::offset_of!(LayoutProbeRecord, values),
        mem::size_of::<Vec<u64>>(),
        mem::align_of::<Vec<u64>>(),
        mem::size_of::<u64>(),
        mem::align_of::<u64>(),
    );
    black_box(record);
}

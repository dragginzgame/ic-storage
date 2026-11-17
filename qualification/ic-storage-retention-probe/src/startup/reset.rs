//! Shared controlled startup: discard transient owners, then replay compiler data.
use wasm_encoder::{DataSection, Function, Instruction as I};

pub(super) fn build(end: i32, segments: &[(i32, &[u8])]) -> (Function, DataSection) {
    let mut start = Function::new([]);
    for op in [
        I::I32Const(0),
        I::I32Const(0),
        I::I32Const(end),
        I::MemoryFill(0),
    ] {
        start.instruction(&op);
    }
    let mut data = DataSection::new();
    for (i, &(offset, bytes)) in segments.iter().enumerate() {
        data.passive(bytes.iter().copied());
        for op in [
            I::I32Const(offset),
            I::I32Const(0),
            I::I32Const(bytes.len() as i32),
            I::MemoryInit {
                mem: 0,
                data_index: i as u32,
            },
        ] {
            start.instruction(&op);
        }
    }
    start.instruction(&I::End);
    (start, data)
}

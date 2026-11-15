//! Fixed-policy Wasm growth probes; no general allocator or Rust memory pointers.

use wasm_encoder::{BlockType, Function, Instruction as I, MemArg, MemArgTarget, ValType};

use super::{BOUNDARY, SENTINEL};

const WORD: MemArg = MemArg {
    offset: 0,
    align: 3,
    target: MemArgTarget::Memory(0),
    ordering: None,
};

#[derive(Clone, Copy)]
pub(super) enum Mode {
    One,
    Limit,
    Trap,
}

/// Returns the old page count as one LE u64, or u64::MAX on growth refusal.
/// Each successful growth checks and marks the first/last words of the new range.
pub(super) fn grow(mode: Mode, append: u32, reply: u32) -> Function {
    // Locals: previous page count; first new address; last new address.
    let mut f = Function::new([(3, ValType::I32)]);
    match mode {
        Mode::One | Mode::Trap => {
            f.instruction(&I::I32Const(1));
        }
        Mode::Limit => {
            for op in [I::I32Const(256), I::MemorySize(0), I::I32Sub] {
                f.instruction(&op);
            }
        }
    }
    for op in [
        I::MemoryGrow(0),
        I::LocalSet(0),
        I::LocalGet(0),
        I::I32Const(-1),
        I::I32Ne,
        I::MemorySize(0),
        I::LocalGet(0),
        I::I32GtU,
        I::I32And,
        I::If(BlockType::Empty),
        I::LocalGet(0),
        I::I32Const(16),
        I::I32Shl,
        I::LocalSet(1),
        I::MemorySize(0),
        I::I32Const(16),
        I::I32Shl,
        I::I32Const(8),
        I::I32Sub,
        I::LocalSet(2),
        I::LocalGet(1),
        I::I64Load(WORD),
        I::I64Eqz,
        I::I32Eqz,
        I::If(BlockType::Empty),
        I::Unreachable,
        I::End,
        I::LocalGet(2),
        I::I64Load(WORD),
        I::I64Eqz,
        I::I32Eqz,
        I::If(BlockType::Empty),
        I::Unreachable,
        I::End,
        I::LocalGet(1),
        I::I64Const(SENTINEL),
        I::I64Store(WORD),
        I::LocalGet(2),
        I::I64Const(SENTINEL),
        I::I64Store(WORD),
        I::I32Const(BOUNDARY + 16),
        I::MemorySize(0),
        I::I64ExtendI32U,
        I::I64Store(WORD),
        I::End,
    ] {
        f.instruction(&op);
    }
    if matches!(mode, Mode::Trap) {
        // Refused growth replies normally, so a trap proves growth succeeded.
        for op in [
            I::LocalGet(0),
            I::I32Const(-1),
            I::I32Ne,
            I::If(BlockType::Empty),
            I::Unreachable,
            I::End,
        ] {
            f.instruction(&op);
        }
    }
    for op in [
        I::I32Const(0),
        I::LocalGet(0),
        I::I64ExtendI32S,
        I::I64Store(WORD),
        I::I32Const(0),
        I::I32Const(8),
        I::Call(append),
        I::Call(reply),
        I::End,
    ] {
        f.instruction(&op);
    }
    f
}

/// Reports physical pages, committed extent, and sentinels at pages 129/130 and
/// the last word of memory. Missing probe pages report zero without reading OOB.
pub(super) fn status(append: u32, reply: u32) -> Function {
    let mut f = Function::new([]);
    for op in [
        I::I32Const(0),
        I::MemorySize(0),
        I::I64ExtendI32U,
        I::I64Store(WORD),
        I::I32Const(8),
        I::I32Const(BOUNDARY + 16),
        I::I64Load(WORD),
        I::I64Store(WORD),
    ] {
        f.instruction(&op);
    }
    for (slot, page, offset) in [
        (16, 129, 0),
        (24, 129, 65528),
        (32, 130, 0),
        (40, 130, 65528),
    ] {
        for op in [
            I::I32Const(slot),
            I::I64Const(0),
            I::I64Store(WORD),
            I::MemorySize(0),
            I::I32Const(page),
            I::I32GtU,
            I::If(BlockType::Empty),
            I::I32Const(slot),
            I::I32Const(page * 65536 + offset),
            I::I64Load(WORD),
            I::I64Store(WORD),
            I::End,
        ] {
            f.instruction(&op);
        }
    }
    for op in [
        I::I32Const(48),
        I::MemorySize(0),
        I::I32Const(16),
        I::I32Shl,
        I::I32Const(8),
        I::I32Sub,
        I::I64Load(WORD),
        I::I64Store(WORD),
        I::I32Const(0),
        I::I32Const(56),
        I::Call(append),
        I::Call(reply),
        I::End,
    ] {
        f.instruction(&op);
    }
    f
}

/// Commit an extent that cannot fit the physical memory, to test upgrade refusal.
pub(super) fn corrupt_extent(reply: u32) -> Function {
    let mut f = Function::new([]);
    for op in [
        I::I32Const(BOUNDARY + 16),
        I::I64Const(257),
        I::I64Store(WORD),
        I::Call(reply),
        I::End,
    ] {
        f.instruction(&op);
    }
    f
}

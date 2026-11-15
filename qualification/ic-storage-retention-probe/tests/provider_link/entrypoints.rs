//! Trusted scalar lifecycle glue, emitted outside LLVM. No pointer escapes Rust.

use wasm_encoder::{
    CodeSection, ExportKind, ExportSection, Function, FunctionSection, Instruction as I, MemArg,
    MemArgTarget, TypeSection, ValType,
};

use super::artifact::Profile;

pub(super) struct Calls {
    pub(super) read: u32,
    pub(super) write: u32,
    pub(super) settle: u32,
    pub(super) post: u32,
    pub(super) append: u32,
    pub(super) reply: u32,
}

pub(super) fn append(
    types: &mut TypeSection,
    functions: &mut FunctionSection,
    exports: &mut ExportSection,
    code: &mut CodeSection,
    imports: u32,
    calls: &Calls,
    profile: Profile,
) {
    let unit = types.len();
    types.ty().function([], []);
    let mut endpoint = |name: &str, mut body: Function| {
        body.instruction(&I::End);
        exports.export(name, ExportKind::Func, imports + functions.len());
        functions.function(unit);
        code.function(&body);
    };

    let mut read = Function::new([(1, ValType::I64)]);
    read.instruction(&I::Call(calls.read));
    read.instruction(&I::LocalSet(0));
    // The admitted Rust body has returned only a scalar. No Rust pointer or
    // frame is live when settlement returns custody to the provider.
    read.instruction(&I::Call(calls.settle));
    read.instruction(&I::I32Const(0));
    read.instruction(&I::LocalGet(0));
    read.instruction(&I::I64Store(MemArg {
        offset: 0,
        align: 3,
        target: MemArgTarget::Memory(0),
        ordering: None,
    }));
    read.instruction(&I::I32Const(0));
    read.instruction(&I::I32Const(8));
    read.instruction(&I::Call(calls.append));
    read.instruction(&I::Call(calls.reply));
    endpoint("canister_query native_read", read);

    let mut write = Function::new([]);
    write.instruction(&I::I64Const(if matches!(profile, Profile::A) {
        42
    } else {
        97
    }));
    write.instruction(&I::Call(calls.write));
    write.instruction(&I::Call(calls.settle));
    write.instruction(&I::Call(calls.reply));
    endpoint("canister_update native_write", write);

    let mut trap = Function::new([]);
    trap.instruction(&I::I64Const(999));
    trap.instruction(&I::Call(calls.write));
    // Traps propagate to the IC transaction boundary; no local recovery.
    trap.instruction(&I::Unreachable);
    endpoint("canister_update native_trap", trap);

    let mut abandon = Function::new([]);
    abandon.instruction(&I::Call(calls.read));
    abandon.instruction(&I::Drop);
    // Deliberate test-only loss of custody. No Rust pointer survives, but the
    // retained unsealed marker still prevents another claim or upgrade.
    abandon.instruction(&I::Call(calls.reply));
    endpoint("canister_update native_abandon", abandon);

    let mut post = Function::new([]);
    post.instruction(&I::Call(calls.post));
    // Source header/geometry admission precedes the first native read in B.
    post.instruction(&I::Call(calls.read));
    post.instruction(&I::Drop);
    post.instruction(&I::Call(calls.settle));
    if matches!(profile, Profile::Trapping) {
        post.instruction(&I::I64Const(999));
        post.instruction(&I::Call(calls.write));
        post.instruction(&I::Unreachable);
    }
    endpoint("canister_post_upgrade", post);
}

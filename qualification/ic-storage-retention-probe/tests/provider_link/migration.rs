//! Native/governed bridge composition under a separate memory and custody contract.

use super::vector_link;
use ic_host_artifacts::{wasm, wasm_text};

mod artifact;
mod tests;
mod transient;

const LIMIT: usize = 1024 * 1024;

fn native(raw: &[u8], relocated: bool, root: (u64, u64), job: bool) -> Vec<u8> {
    let mut source = vector_link::source(vector_link::Profile { root, ..vector_link::Profile::A }).unwrap()
        .replace("(memory (export \"memory\") 3 3)", "(memory (export \"memory\") 32 256)")
        .replace("(i32.ne (memory.size) (i32.const 3))", "(i32.lt_u (memory.size) (i32.const 32))")
        .replace("(func (export \"canister_init\")", "(func $native_init (export \"__bridge_init\")")
        .replace("(export \"canister_post_upgrade\")", "(export \"__bridge_post\")")
        .replace("(call $provider_pre)", "(if (i32.eq (global.get $phase) (i32.const 3)) (then (call $frozen)) (else (call $provider_pre)))")
        .replace("(i64.load (i32.const 65544)) (i64.const 1)", "(i64.load (i32.const 65544)) (i64.const 2)")
        .replace("(i64.store (i32.const 65544) (i64.const 1))", "(i64.store (i32.const 65544) (i64.const 2))\n(i64.store (i32.const 65600) (i64.const 41))");
    source.truncate(source.rfind(')').unwrap());
    source.push_str(include_str!("migration/callbacks.wat"));
    if job {
        source = source.replace("(else (call $provider_pre))", "(else (if (i32.eq (global.get $phase) (i32.const 5)) (then (call $staged)) (else (call $provider_pre))))");
        source.push_str(include_str!("migration/job-callbacks.wat"));
        source.push_str(include_str!("migration/staged-callbacks.wat"));
    }
    source.push(')');
    if relocated {
        source = source
            .replace("131088", "147472")
            .replace("131080", "147464")
            .replace("131072", "147456");
    }
    let provider = wasm_text::compile(
        &source,
        wasm_text::CompileLimits {
            source_bytes: LIMIT,
            module_bytes: LIMIT,
        },
    )
    .unwrap();
    vector_link::compose(raw, &provider).unwrap()
}

fn prepare(
    raw: &[u8],
    bridge: &[u8],
    relocated: bool,
    root: (u64, u64),
    recovery: bool,
) -> Vec<u8> {
    let post = if recovery {
        artifact::Post::Recover
    } else {
        artifact::Post::Refuse
    };
    let bytes = artifact::combine(&native(raw, relocated, root, false), bridge, post);
    wasm::validate(&bytes, LIMIT).unwrap();
    bytes
}

fn prepare_reopen(raw: &[u8], bridge: &[u8], relocated: bool, root: (u64, u64)) -> Vec<u8> {
    let bytes = artifact::combine(
        &native(raw, relocated, root, false),
        bridge,
        artifact::Post::Reopen,
    );
    wasm::validate(&bytes, LIMIT).unwrap();
    bytes
}

fn prepare_job(
    raw: &[u8],
    bridge: &[u8],
    relocated: bool,
    root: (u64, u64),
    recovery: bool,
) -> Vec<u8> {
    let post = if recovery {
        artifact::Post::Recover
    } else {
        artifact::Post::Reopen
    };
    let bytes = artifact::combine(&native(raw, relocated, root, true), bridge, post);
    wasm::validate(&bytes, LIMIT).unwrap();
    bytes
}

fn prepare_frozen_job(raw: &[u8], bridge: &[u8], relocated: bool) -> Vec<u8> {
    let bytes = artifact::combine(
        &native(raw, relocated, (0, 0), true),
        bridge,
        artifact::Post::Frozen,
    );
    wasm::validate(&bytes, LIMIT).unwrap();
    bytes
}

fn prepare_staged_job(raw: &[u8], bridge: &[u8], relocated: bool, resume: bool) -> Vec<u8> {
    let post = if resume {
        artifact::Post::Importing
    } else {
        artifact::Post::Stage
    };
    let bytes = artifact::combine(&native(raw, relocated, (0, 0), true), bridge, post);
    wasm::validate(&bytes, LIMIT).unwrap();
    bytes
}

fn admit_final(
    bytes: &[u8],
    raw: &[u8],
    bridge: &[u8],
    relocated: bool,
    root: (u64, u64),
    recovery: bool,
) -> bool {
    bytes == prepare(raw, bridge, relocated, root, recovery)
}

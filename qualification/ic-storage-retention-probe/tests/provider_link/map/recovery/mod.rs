//! Bounded map recovery through the shared governed checkpoint composer.
use super::{inputs, link};
use crate::{
    migration::artifact::{self, Post},
    vector_link,
};
use ic_storage_retention_probe::artifact::{compile_wat, validate_wasm};

mod installed_tests;
mod job_installed_tests;
mod paged_installed_tests;
mod read_tests;
const LIMIT: usize = 1024 * 1024;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Profile {
    Checkpoint,
    Job,
    Paged,
}

fn replace(source: &mut String, from: &str, to: &str) {
    assert_eq!(
        source.matches(from).count(),
        1,
        "map checkpoint anchor: {from}"
    );
    *source = source.replace(from, to);
}
fn prepare(raw: &[u8], bridge: &[u8], relocated: bool, recover: bool) -> Vec<u8> {
    prepare_mode(
        raw,
        bridge,
        relocated,
        if recover { Post::Recover } else { Post::Reopen },
        Profile::Checkpoint,
    )
}
fn prepare_mode(
    raw: &[u8],
    bridge: &[u8],
    relocated: bool,
    post: Post,
    profile: Profile,
) -> Vec<u8> {
    let mut source = link::source(link::Profile::A).unwrap();
    replace(
        &mut source,
        "(memory (export \"memory\") 3 5)",
        "(memory (export \"memory\") 32 256)",
    );
    if profile != Profile::Paged {
        replace(
            &mut source,
            "(func $static_header",
            "(func $static_header\n(if (i64.ne (i64.load (i32.const 65560)) (i64.const 24576)) (then unreachable))",
        );
    }
    replace(
        &mut source,
        "(if (i64.ne (i64.extend_i32_u (memory.size))\n      (i64.shr_u (i64.add (i64.const 196607) (i64.load (i32.const 65560))) (i64.const 16))) (then unreachable))",
        "(if (i32.lt_u (memory.size) (i32.const 32)) (then unreachable))",
    );
    replace(
        &mut source,
        "(i64.load (i32.const 65544)) (i64.const 4)",
        "(i64.load (i32.const 65544)) (i64.const 5)",
    );
    replace(
        &mut source,
        "(i64.store (i32.const 65544) (i64.const 4))",
        "(i64.store (i32.const 65544) (i64.const 5))",
    );
    replace(
        &mut source,
        "(func (export \"canister_init\")",
        "(func $native_init (export \"__bridge_init\")",
    );
    replace(
        &mut source,
        "(func (export \"canister_pre_upgrade\") (call $run (i32.const 0) (i64.const 0) (i32.const 0)))",
        "(func (export \"canister_pre_upgrade\") (call $boundary))",
    );
    let start = source
        .find("  (func (export \"canister_post_upgrade\")")
        .unwrap();
    if profile == Profile::Paged {
        let end = source.find("  ;; No Rust owner").unwrap();
        source.replace_range(start..end, "");
        // Growth uses the native provider's idle-only admission. Both windows
        // fit below checkpoint data, inside the preallocated 32 Wasm pages.
        if relocated {
            replace(
                &mut source,
                "(i32.const 196607) (local.get $new)",
                "(i32.const 458751) (local.get $new)",
            );
        }
    } else {
        let end = source
            .find("  (func (export \"canister_update pool_grow\")")
            .unwrap();
        source.replace_range(
            start..end,
            "  (func $expand_pool (param i32) (result i32) unreachable)\n",
        );
    }
    replace(
        &mut source,
        "(call $reply_data (i32.const 131072) (i32.sub (i32.shl (memory.size) (i32.const 16)) (i32.const 131072)))",
        "(call $reply_data (i32.const 131072) (i32.wrap_i64 (i64.load (i32.const 65560))))",
    );
    source.truncate(source.rfind(')').unwrap());
    source.push_str(include_str!("callbacks.wat"));
    source.push_str(include_str!("read_tests.wat"));
    source.push(')');
    if profile != Profile::Checkpoint {
        replace(
            &mut source,
            "(i64.load (i32.const 65544)) (i64.const 5)",
            "(i64.load (i32.const 65544)) (i64.const 8)",
        );
        replace(
            &mut source,
            "(i64.store (i32.const 65544) (i64.const 5))",
            "(i64.store (i32.const 65544) (i64.const 8))",
        );
        replace(
            &mut source,
            "(else (call $boundary))))",
            "(else (if (i64.ne (i64.load (i32.const 65584)) (i64.const 1)) (then unreachable)) (call $boundary))))",
        );
        let start = source.find("(func $boundary (local $frozen i32)").unwrap();
        let end = source.find("(func (export \"__bridge_export\")").unwrap();
        source.replace_range(start..end, "");
        replace(
            &mut source,
            "(func $read_root (param $id i32) (result i64)\n  (call $frozen)",
            "(func $read_root (param $id i32) (result i64)\n  (call $readable)",
        );
        replace(
            &mut source,
            "(func (export \"__bridge_export\")\n  (call $boundary)",
            "(func (export \"__bridge_export\")\n  (call $boundary)\n  (if (i32.eq (global.get $phase) (i32.const 6)) (then unreachable))",
        );
        let end = source.rfind(')').unwrap();
        source.insert_str(end, include_str!("job.wat"));
    }
    if profile == Profile::Paged {
        // A distinct admitted native format owns the larger retained roots.
        // Scalar staging remains 64 records: only cumulative root bounds grow.
        replace(
            &mut source,
            "(i64.load (i32.const 65544)) (i64.const 8)",
            "(i64.load (i32.const 65544)) (i64.const 10)",
        );
        replace(
            &mut source,
            "(i64.store (i32.const 65544) (i64.const 8))",
            "(i64.store (i32.const 65544) (i64.const 10))",
        );
        for root in [65568, 65576] {
            replace(
                &mut source,
                &format!("(call $count (i64.load (i32.const {root}))) (i64.const 64)"),
                &format!("(call $count (i64.load (i32.const {root}))) (i64.const 192)"),
            );
        }
        for name in ["expected_a", "expected_b"] {
            replace(
                &mut source,
                &format!("(global.get ${name}) (i32.const 64)"),
                &format!("(global.get ${name}) (i32.const 192)"),
            );
        }
        let end = source.rfind(')').unwrap();
        source.insert_str(end, include_str!("paged_tests.wat"));
    }
    let destination = if profile == Profile::Paged {
        393216
    } else {
        163840
    };
    if relocated {
        source = source.replace("131072", &destination.to_string());
    }
    let end = source.rfind(')').unwrap();
    source.insert_str(end, &format!("(func (export \"canister_query discarded_payload\") (call $reply_data (i32.const {}) (i32.const {})) (call $reply))", if relocated { 131072 } else { destination }, if profile == Profile::Paged { 196608 } else { 24576 }));
    let provider = compile_wat(&source, LIMIT, LIMIT).unwrap();
    let native = vector_link::compose_map(raw, &provider).unwrap();
    let bytes = artifact::combine(&native, bridge, post);
    validate_wasm(&bytes, LIMIT).unwrap();
    bytes
}

fn admit_final(bytes: &[u8], raw: &[u8], bridge: &[u8], relocated: bool, recover: bool) -> bool {
    bytes == prepare(raw, bridge, relocated, recover)
}

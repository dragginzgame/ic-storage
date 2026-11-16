//! Portable V3 vector recovery through the existing governed checkpoint domain.

use super::{
    LIMIT,
    artifact::{self, Post},
};
use crate::{record_vector::link, vector_link};
use ic_storage_retention_probe::artifact::{compile_wat, validate_wasm};

mod tests;

fn replace(source: &mut String, from: &str, to: &str, count: usize) {
    assert_eq!(
        source.matches(from).count(),
        count,
        "record recovery anchor: {from}"
    );
    *source = source.replace(from, to);
}

fn prepare(raw: &[u8], bridge: &[u8], relocated: bool, root: (u64, u64), post: Post) -> Vec<u8> {
    assert!(matches!(
        post,
        Post::Reopen | Post::Recover | Post::NarrowVector
    ));
    assert!(post == Post::Reopen || root == (0, 0));
    let mut source = link::source(link::Profile {
        root,
        ..link::Profile::A
    })
    .unwrap();
    replace(
        &mut source,
        "(memory (export \"memory\") 3 3)",
        "(memory (export \"memory\") 32 256)",
        1,
    );
    replace(
        &mut source,
        "(i32.ne (memory.size) (i32.const 3))",
        "(i32.lt_u (memory.size) (i32.const 32))",
        1,
    );
    replace(
        &mut source,
        "(func (export \"canister_init\")",
        "(func $native_init (export \"__bridge_init\")",
        1,
    );
    replace(
        &mut source,
        "(export \"canister_post_upgrade\")",
        "(export \"__bridge_post\")",
        1,
    );
    replace(
        &mut source,
        "(call $provider_pre)",
        "(if (i32.eq (global.get $phase) (i32.const 3)) (then (call $frozen)) (else (call $provider_pre)))",
        1,
    );
    // A separate native format prevents a bare record provider accepting a heap
    // whose checkpoint domain has not been rebuilt by the bridge lifecycle.
    replace(
        &mut source,
        "(i64.const 0x4e5650524f563033)",
        "(i64.const 0x4e5650524f563034)",
        2,
    );
    source.truncate(source.rfind(')').unwrap());
    source.push_str(include_str!("callbacks.wat"));
    source.push(')');
    if relocated {
        // Source-level reviewed constants only; the raw Rust obtains its address
        // from claim. Both the provider and every access guard move together.
        source = source
            .replace("131080", "147464")
            .replace("131072", "147456");
    }
    let end = source.rfind(')').unwrap();
    source.insert_str(end, &format!(
        "(func (export \"canister_query discarded_payload\") (call $reply_data (i32.const {}) (i32.const 256)) (call $reply))",
        if relocated { 131072 } else { 147456 }
    ));
    let provider = compile_wat(&source, LIMIT, LIMIT).unwrap();
    let native = vector_link::compose_record(raw, &provider).unwrap();
    let bytes = artifact::combine(&native, bridge, post);
    validate_wasm(&bytes, LIMIT).unwrap();
    bytes
}

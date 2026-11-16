//! Portable V3 vector recovery through the existing governed checkpoint domain.

use super::{
    LIMIT,
    artifact::{self, Post},
};
use crate::{record_vector::link, vector_link};
use ic_storage_retention_probe::artifact::{compile_wat, validate_wasm};

mod tests;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    V3,
    V2,
    Job,
}

fn prepare_job(
    raw: &[u8],
    bridge: &[u8],
    relocated: bool,
    root: (u64, u64),
    post: Post,
) -> Vec<u8> {
    prepare_schema(
        raw,
        bridge,
        relocated,
        link::Profile {
            root,
            ..link::Profile::A
        },
        post,
        Mode::Job,
        None,
    )
}

fn replace(source: &mut String, from: &str, to: &str, count: usize) {
    assert_eq!(
        source.matches(from).count(),
        count,
        "record recovery anchor: {from}"
    );
    *source = source.replace(from, to);
}

fn prepare(raw: &[u8], bridge: &[u8], relocated: bool, root: (u64, u64), post: Post) -> Vec<u8> {
    prepare_schema(
        raw,
        bridge,
        relocated,
        link::Profile {
            root,
            ..link::Profile::A
        },
        post,
        Mode::V3,
        None,
    )
}

fn prepare_v2(raw: &[u8], bridge: &[u8], relocated: bool, root: (u64, u64), post: Post) -> Vec<u8> {
    assert!(matches!(
        post,
        Post::Reopen | Post::Recover | Post::WidenVector
    ));
    prepare_schema(
        raw,
        bridge,
        relocated,
        link::Profile {
            root,
            ..link::Profile::A
        },
        post,
        Mode::V2,
        None,
    )
}

fn prepare_v2_boundary(raw: &[u8], bridge: &[u8], index: usize, counter: u64) -> Vec<u8> {
    prepare_schema(
        raw,
        bridge,
        false,
        link::Profile {
            root: (16, 16),
            ..link::Profile::A
        },
        Post::Reopen,
        Mode::V2,
        Some((index, counter)),
    )
}

fn prepare_v2_corrupt(raw: &[u8], bridge: &[u8], fault: (u32, u64)) -> Vec<u8> {
    prepare_schema(
        raw,
        bridge,
        false,
        link::Profile {
            fault: Some(fault),
            source_guard: false,
            ..link::Profile::A
        },
        Post::Reopen,
        Mode::V2,
        None,
    )
}

fn prepare_schema(
    raw: &[u8],
    bridge: &[u8],
    relocated: bool,
    profile: link::Profile,
    post: Post,
    mode: Mode,
    boundary: Option<(usize, u64)>,
) -> Vec<u8> {
    let v2 = mode == Mode::V2;
    let job = mode == Mode::Job;
    assert!(if job {
        matches!(
            post,
            Post::Reopen
                | Post::Recover
                | Post::Frozen
                | Post::Stage
                | Post::Restart
                | Post::Importing
        )
    } else {
        matches!(
            post,
            Post::Reopen
                | Post::Recover
                | Post::NarrowVector
                | Post::WidenVector
                | Post::ConvertVector
        )
    });
    assert!(post != Post::WidenVector || v2);
    assert!(post != Post::NarrowVector || !v2);
    let root = profile.root;
    assert!(
        matches!(
            post,
            Post::Reopen | Post::ConvertVector | Post::Frozen | Post::Importing
        ) || root == (0, 0)
    );
    let mut source = if v2 {
        link::source_v2(profile)
    } else {
        link::source(profile)
    }
    .unwrap();
    if let Some((index, counter)) = boundary {
        // A V3 boundary value also leaves its explicit reserved word zero.
        assert!((v2 || u32::try_from(counter).is_ok()) && index < root.0 as usize);
        replace(
            &mut source,
            "(call $element_init)",
            &format!(
                "(call $element_init) (i64.store (i32.const {}) (i64.const {counter})) (i64.store (i32.const {}) (i64.const -1))",
                131072 + index * 16,
                131080 + index * 16
            ),
            1,
        );
    }
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
        if job {
            "(i64.const 0x4e5650524f563035)"
        } else {
            "(i64.const 0x4e5650524f563034)"
        },
        2,
    );
    source.truncate(source.rfind(')').unwrap());
    let callbacks = include_str!("callbacks.wat");
    if v2 {
        source.push_str(
            &callbacks
                .replace(
                    "(i64.extend_i32_u (i32.load (local.get $address)))",
                    "(i64.load (local.get $address))",
                )
                .replace("(param $counter i32)", "(param $counter i64)")
                .replace(
                    "(i64.extend_i32_u (local.get $counter))",
                    "(local.get $counter)",
                )
                .replace(
                    "(i32.store (local.get $address) (local.get $counter))",
                    "(i64.store (local.get $address) (local.get $counter))",
                ),
        );
        source.push_str("(export \"__bridge_record_v2\" (func $provider_pre))");
    } else {
        source.push_str(callbacks);
    }
    if job {
        replace(
            &mut source,
            "(else (call $provider_pre))",
            "(else (if (i32.eq (global.get $phase) (i32.const 5)) (then (call $staged)) (else (call $provider_pre))))",
            1,
        );
        source.push_str(include_str!("../job-callbacks.wat"));
        source.push_str(&include_str!("../staged-callbacks.wat").replace(
            "__STAGE_SCHEMA__",
            "(call $element_header) (call $elements_admit (i64.load (i32.const 65568)))",
        ));
        source.push_str(include_str!("staged.wat"));
    }
    if post == Post::ConvertVector {
        let checks: String = ic_storage_retention_probe::vector_provider::records::SCHEMA_V2.iter().enumerate().map(|(i, value)| format!("(if (i64.ne (i64.load (i32.const {})) (i64.const {value})) (then unreachable))", 65592 + i * 8)).collect();
        source.push_str(&include_str!("convert.wat").replace("__V2_SCHEMA_CHECKS__", &checks));
    }
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
    let native = if v2 {
        vector_link::compose_record_v2(raw, &provider)
    } else {
        vector_link::compose_record(raw, &provider)
    }
    .unwrap();
    let bytes = artifact::combine(&native, bridge, post);
    validate_wasm(&bytes, LIMIT).unwrap();
    bytes
}

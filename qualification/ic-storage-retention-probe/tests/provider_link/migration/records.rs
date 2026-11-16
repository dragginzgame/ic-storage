//! Explicit native record profile; legacy native-format-2 fixtures stay unchanged.
use super::{
    LIMIT,
    artifact::{self, Post},
    compile_native, native_source,
};
use ic_storage_retention_probe::artifact::validate_wasm;

pub(super) fn prepare(
    raw: &[u8],
    bridge: &[u8],
    relocated: bool,
    root: (u64, u64),
    schema: u64,
    post: Post,
) -> Vec<u8> {
    assert!(matches!(schema, 1..=3));
    assert!(matches!(
        post,
        Post::Reopen | Post::Convert | Post::Narrow | Post::Recover
    ));
    assert!(post != Post::Convert || schema == 2);
    assert!(post != Post::Narrow || schema == 3);
    let source_schema = match post {
        Post::Convert => 1,
        Post::Narrow => 2,
        _ => 0,
    };
    let mut source = native_source(relocated, root, false)
        .replace("(i64.load (i32.const 65544)) (i64.const 2)", "(i64.load (i32.const 65544)) (i64.const 3)")
        .replace("(i64.store (i32.const 65544) (i64.const 2))", &format!("(i64.store (i32.const 65544) (i64.const 3))\n(i64.store (i32.const 65592) (i64.const {schema}))\n(i64.store (i32.const 65608) (i64.const {}))", if schema == 1 { 0 } else { u64::MAX }))
        .replace("(func $static_header", "(func $static_header\n  (call $record_shape)")
        .replace("(i32.le_u (local.get $offset) (i32.const 65600))", "(i32.le_u (local.get $offset) (i32.const 65608))");
    source.truncate(source.rfind(')').unwrap());
    source.push_str(
        &include_str!("records.wat")
            .replace("__SCHEMA__", &schema.to_string())
            .replace("__SOURCE__", &source_schema.to_string()),
    );
    source.push(')');
    let bytes = artifact::combine(&compile_native(raw, &source), bridge, post);
    validate_wasm(&bytes, LIMIT).unwrap();
    bytes
}

//! Closed V3 record provider built on the shared vector custody transitions.

/// Byte-defined ID/version/stride/alignment/count and (ID, offset, width) fields.
/// Width tags belong to this fixture, not Rust enum discriminants.
pub const SCHEMA: [u64; 11] = [
    u64::from_le_bytes(*b"RECU64V1"),
    3,
    16,
    8,
    2,
    1,
    0,
    4,
    2,
    8,
    8,
];
/// Complete retained metadata size in u64 words.
pub const HEADER_WORDS: usize = 18;
/// Complete native record backing extent.
pub const PAYLOAD_BYTES: usize = 256;

fn replace(source: &mut String, from: &str, to: &str, count: usize) {
    assert_eq!(
        source.matches(from).count(),
        count,
        "profile anchor: {from}"
    );
    *source = source.replace(from, to);
}

/// Build the closed record provider, including explicit test fault injection.
///
/// # Panics
/// Panics for invalid root geometry or changed internal fixture anchors.
#[must_use]
pub fn source(version: u64, root: (u64, u64), trap: bool, fault: &str, guard: bool) -> String {
    assert!(root.0 <= root.1 && root.1 <= 16);
    let mut wat = super::source(version, root, 256, trap, "__RECORD_FAULT__", guard);
    // Retain the existing claim/settle and upgrade ordering. Only this closed
    // profile changes element geometry, field writes and schema admission.
    replace(&mut wat, "(i64.const 512)", "(i64.const 16)", 2);
    replace(&mut wat, "(i32.const 4096)", "(i32.const 256)", 2);
    replace(&mut wat, "(i64.const 4096)", "(i64.const 256)", 1);
    replace(
        &mut wat,
        "(i64.const 0x4e5650524f563031)",
        "(i64.const 0x4e5650524f563033)",
        2,
    );
    replace(&mut wat, "(i32.const 56)", "(i32.const 144)", 1);
    replace(
        &mut wat,
        "(i32.mul (i32.wrap_i64 (local.get $length)) (i32.const 8))",
        "(i32.mul (i32.wrap_i64 (local.get $length)) (i32.const 16))",
        2,
    );
    replace(
        &mut wat,
        "(i32.add (local.get $base) (i32.const 8))",
        "(i32.add (local.get $base) (i32.const 16))",
        1,
    );
    replace(
        &mut wat,
        "(i64.store\n      (i32.add",
        "(call $write_element\n      (i32.add",
        1,
    );
    replace(
        &mut wat,
        "(call $static_header)",
        "(call $static_header) (call $element_header)",
        2,
    );
    replace(
        &mut wat,
        "(call $shape (i64.load (i32.const 65568)) (i64.load (i32.const 65576))))",
        "(call $shape (i64.load (i32.const 65568)) (i64.load (i32.const 65576)))\n    (call $elements_admit (i64.load (i32.const 65568))))",
        1,
    );
    replace(
        &mut wat,
        "(call $shape (local.get $length) (local.get $capacity))",
        "(call $shape (local.get $length) (local.get $capacity))\n    (call $elements_admit (local.get $length))",
        1,
    );
    replace(
        &mut wat,
        "(memory.fill (i32.const 131072) (i32.const 0) (i32.const 256))\n    (i64.store (i32.const 131072) (i64.const 17))\n    (i64.store (i32.const 131080) (i64.const 23))\n    (i64.store (i32.const 131088) (i64.const 41))",
        "(call $element_init)",
        1,
    );
    let writes: String = SCHEMA
        .iter()
        .enumerate()
        .map(|(i, value)| {
            format!(
                "(i64.store (i32.const {}) (i64.const {value}))\n",
                65592 + i * 8
            )
        })
        .collect();
    let checks: String = SCHEMA
        .iter()
        .enumerate()
        .map(|(i, value)| {
            format!(
                "(if (i64.ne (i64.load (i32.const {})) (i64.const {value})) (then unreachable))\n",
                65592 + i * 8
            )
        })
        .collect();
    let elements = include_str!("elements.wat")
        .replace("__SCHEMA_WRITES__", &writes)
        .replace("__SCHEMA_CHECKS__", &checks)
        .replace("__LENGTH__", &root.0.to_string());
    let end = wat.rfind(')').unwrap();
    wat.insert_str(end, &elements);
    replace(&mut wat, "__RECORD_FAULT__", fault, 1);
    wat
}

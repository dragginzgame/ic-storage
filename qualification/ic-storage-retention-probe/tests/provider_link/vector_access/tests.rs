//! Binding checks and valid-Wasm refusals for the access instrumentation boundary.

use ic_storage_retention_probe::artifact::{compile_wat, validate_wasm};
use wasmparser::{Operator as Op, Parser, Payload};

use super::{Error, HELPERS, prepare};

fn fixture(body: &str, extra: &str) -> Vec<u8> {
    fixture_kind(body, extra, false)
}

fn fixture_kind(body: &str, extra: &str, record: bool) -> Vec<u8> {
    let mut source = format!(
        r#"(module
        (import "native_vector_diagnostic" "claim_region" (func (result i32)))
        (memory (export "memory") 3 3)
        (global (export "__stack_pointer") (mut i32) (i32.const 32768))
        (global (export "__data_end") i32 (i32.const 32768))
        (global (export "__heap_base") i32 (i32.const 32768))
        (func (param i32 i32 i32))
        (func (export "provider_vector_step") (param i32 i32 i32 i64) (result i64) (local i32)
            global.get 0 i32.const 64 i32.sub local.tee 4 global.set 0
            block
                {body}
                local.get 4 i32.const 64 i32.add global.set 0
                i64.const 7 return
            end
            unreachable)
        {extra})"#
    );
    if record {
        source = source
            .replace(
                "native_vector_diagnostic",
                "native_record_vector_diagnostic",
            )
            .replace("provider_vector_step", "provider_record_vector_step");
    }
    let bytes = compile_wat(&source, 8192, 8192).unwrap();
    validate_wasm(&bytes, 8192).unwrap();
    bytes
}

#[test]
fn record_profile_keeps_numeric_checks_but_refuses_other_element_and_memory_profiles() {
    let raw = fixture_kind(
        "i64.const 4294967296 i32.wrap_i64 local.set 0
         i32.const 0 i32.const 1 i32.lt_u local.set 0
         i64.const 1 i64.const 2 i64.lt_u local.set 0
         i64.const 1 i64.const 2 i64.gt_u local.set 0
         i64.const 1 i64.const 2 i64.ge_u local.set 0
         i32.const 131072 i32.load local.set 0",
        "",
        true,
    );
    assert_eq!(prepare(&raw), Err(Error::Profile));
    let guarded = super::prepare_record(&raw).unwrap();
    assert_eq!(verify_coverage(&raw, &guarded), [1, 0, 0, 0, 0, 0, 0]);
    // Numeric instructions are passed through exactly, not replaced by guards.
    let numeric = |bytes: &[u8]| {
        let mut ops = Vec::new();
        for payload in Parser::new(0).parse_all(bytes) {
            if let Payload::CodeSectionEntry(body) = payload.unwrap() {
                for op in body.get_operators_reader().unwrap() {
                    let op = op.unwrap();
                    if matches!(
                        op,
                        Op::I32WrapI64 | Op::I32LtU | Op::I64LtU | Op::I64GtU | Op::I64GeU
                    ) {
                        ops.push(format!("{op:?}"));
                    }
                }
            }
        }
        ops
    };
    assert_eq!(numeric(&raw).len(), 5);
    assert_eq!(numeric(&raw), numeric(&guarded));
    assert_eq!(super::prepare_record(&fixture("", "")), Err(Error::Profile));
    for (body, extra) in [
        ("loop end", ""),
        ("i32.const 0 memory.grow local.set 0", ""),
        ("i32.const 131072 i32.load16_u local.set 0", ""),
        ("", "(data (i32.const 131072) \"bad\")"),
    ] {
        assert!(super::prepare_record(&fixture_kind(body, extra, true)).is_err());
    }
}

#[test]
fn preserves_full_offsets_and_store_values_while_remapping_calls() {
    let raw = fixture(
        r#"
        call 0 local.set 0
        i32.const 0 i32.const 0 i32.const 0 call 1
        i32.const 32704 i32.load offset=4 local.set 0
        i32.const 32704 i32.load8_u offset=7 local.set 0
        i32.const 32704 i64.load offset=8 local.set 3
        i32.const 32704 i32.const 123 i32.store offset=12
        i32.const 32704 i32.const 255 i32.store8 offset=16
        i32.const 32704 i64.const 456 i64.store offset=4294967295
        i32.const 131072 i32.const 131080 i32.const 8 memory.copy
    "#,
        "",
    );
    let guarded = prepare(&raw).unwrap();
    assert_eq!(verify_coverage(&raw, &guarded), [1; 7]);
    let mut imports = Vec::new();
    let mut functions = Vec::new();
    let mut export = None;
    for payload in Parser::new(0).parse_all(&guarded) {
        match payload.unwrap() {
            Payload::ImportSection(r) => {
                for import in r.into_imports() {
                    let import = import.unwrap();
                    imports.push((import.module, import.name));
                }
            }
            Payload::ExportSection(r) => {
                for entry in r {
                    let entry = entry.unwrap();
                    if entry.name == "provider_vector_step" {
                        export = Some(entry.index);
                    }
                }
            }
            Payload::CodeSectionEntry(body) => functions.push(body),
            _ => {}
        }
    }
    assert_eq!(imports[0], ("native_vector_diagnostic", "claim_region"));
    assert_eq!(
        imports[1..],
        HELPERS.map(|name| ("native_vector_access", name))
    );
    assert_eq!(export, Some(9));
    let ops = functions[1]
        .get_operators_reader()
        .unwrap()
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert!(
        ops.iter()
            .any(|op| matches!(op, Op::Call { function_index: 0 }))
    );
    assert!(
        ops.iter()
            .any(|op| matches!(op, Op::Call { function_index: 8 }))
    );
    assert!(ops.windows(3).any(|ops| matches!(
        ops,
        [
            Op::I64Const { value: 456 },
            Op::I64Const { value: 4294967295 },
            Op::Call { function_index: 6 }
        ]
    )));
    assert!(ops.windows(3).any(|ops| matches!(
        ops,
        [
            Op::I32Const { value: 255 },
            Op::I64Const { value: 16 },
            Op::Call { function_index: 5 }
        ]
    )));
}

#[test]
fn unsupported_memory_operators_and_initialization_refuse() {
    for body in [
        "i32.const 32704 i32.load16_u local.set 0",
        "i32.const 32704 i32.const 0 i32.const 4 memory.fill",
        "i32.const 0 memory.grow local.set 0",
    ] {
        assert_eq!(prepare(&fixture(body, "")), Err(Error::Stack));
    }
    assert_eq!(
        prepare(&fixture("", "(data (i32.const 131072) \"bad\")")),
        Err(Error::Profile)
    );
    assert_eq!(
        prepare(&fixture("", "(table 1 funcref)")),
        Err(Error::Profile)
    );
}

// Independent inventory of source access kinds and rewritten import call sites.
// Every rewritten body must be free of direct memory operations from this profile.
pub(crate) fn verify_coverage(raw: &[u8], guarded: &[u8]) -> [usize; 7] {
    verify_counts(raw, guarded)
}

pub(crate) fn verify_map_coverage(raw: &[u8], guarded: &[u8]) -> [usize; 8] {
    verify_counts(raw, guarded)
}

fn verify_counts<const N: usize>(raw: &[u8], guarded: &[u8]) -> [usize; N] {
    let mut expected = [0; N];
    let mut observed = [0; N];
    for (bytes, rewritten) in [(raw, false), (guarded, true)] {
        for payload in Parser::new(0).parse_all(bytes) {
            if let Payload::CodeSectionEntry(body) = payload.unwrap() {
                for op in body.get_operators_reader().unwrap() {
                    let op = op.unwrap();
                    let kind = match op {
                        Op::I32Load { .. } => Some(0),
                        Op::I64Load32U { .. } if N == 8 => Some(0),
                        Op::I32Load8U { .. } => Some(1),
                        Op::I64Load { .. } => Some(2),
                        Op::I32Store { .. } => Some(3),
                        Op::I32Store8 { .. } => Some(4),
                        Op::I64Store { .. } => Some(5),
                        Op::MemoryCopy { .. } => Some(6),
                        Op::MemoryFill { .. } if N == 8 => Some(7),
                        _ => None,
                    };
                    if let Some(kind) = kind {
                        assert!(!rewritten, "unguarded memory access");
                        expected[kind] += 1;
                    }
                    if rewritten
                        && let Op::Call {
                            function_index: index,
                        } = op
                        && index > 0
                        && index as usize <= N
                    {
                        observed[index as usize - 1] += 1;
                    }
                }
            }
        }
    }
    assert_eq!(observed, expected);
    observed
}

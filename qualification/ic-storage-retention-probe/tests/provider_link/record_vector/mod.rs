//! Compile, bind and qualify the closed V3 record-vector fixture.

use ic_storage_retention_probe::artifact::{sha256_hex, validate_wasm};
use std::{collections::BTreeMap, fmt::Write, fs};
use wasmparser::{ExternalKind, Operator, Parser, Payload, TypeRef, ValType};

mod installed_tests;
pub(super) mod link;

#[test]
#[ignore = "make native-provider-probe supplies fresh record compiler output"]
fn record_link_keeps_native_entry_private_and_settles_only_after_return() {
    use Operator as Op;
    let (directory, _) = super::input();
    let raw = fs::read(directory.join("vector/provider_record_vector.wasm")).unwrap();
    let bytes = link::prepare(&raw, link::Profile::A).unwrap();
    link::admit_final(&bytes, &raw, link::Profile::A).unwrap();
    assert!(link::admit_final(&bytes, &raw, link::Profile::B).is_err());
    assert!(super::vector_link::compose(&raw, &bytes).is_err());
    let mut imports = Vec::new();
    let mut exports = BTreeMap::new();
    let mut bodies = Vec::new();
    for payload in Parser::new(0).parse_all(&bytes) {
        match payload.unwrap() {
            Payload::ImportSection(r) => {
                for import in r.into_imports() {
                    let import = import.unwrap();
                    imports.push((import.module, import.name));
                }
            }
            Payload::ExportSection(r) => {
                for export in r {
                    let export = export.unwrap();
                    assert!(
                        !export.name.starts_with("__link_")
                            && !export.name.starts_with("provider_")
                    );
                    exports.insert(export.name, (export.kind, export.index));
                }
            }
            Payload::CodeSectionEntry(body) => bodies.push(body),
            Payload::StartSection { .. } | Payload::DataSection(_) | Payload::TableSection(_) => {
                panic!("unexpected initialization")
            }
            _ => {}
        }
    }
    assert_eq!(
        imports,
        [("ic0", "msg_reply_data_append"), ("ic0", "msg_reply")]
    );
    let first_rust = bodies.len() as u32; // Two IC imports, two native bodies.
    assert!(
        exports
            .values()
            .all(|(kind, index)| *kind != ExternalKind::Func || *index < first_rust)
    );
    let ops = |index: u32| {
        bodies[index as usize - 2]
            .get_operators_reader()
            .unwrap()
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
    };
    let calls = |index| {
        ops(index)
            .into_iter()
            .filter_map(|op| match op {
                Op::Call { function_index } => Some(function_index),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    let wat_append = calls(exports["canister_update append"].1)[0];
    let wat_calls = calls(wat_append);
    let claim = wat_calls[0];
    let settle = *wat_calls.last().unwrap();
    let admit = calls(claim)[0];
    let run = calls(exports["canister_update vector_mutate"].1)[0];
    let run_with_fault = calls(run)[0];
    use Op::*;
    assert!(matches!(&ops(run_with_fault)[..], [
        GlobalGet { global_index: 2 }, I32Const { value: -1 }, I32Ne,
        If { .. }, Unreachable, End, Call { function_index: a },
        LocalGet { local_index: 0 }, GlobalSet { global_index: 2 },
        I32Const { value: 65568 }, I64Load { .. }, I32WrapI64,
        I32Const { value: 65576 }, I64Load { .. }, I32WrapI64,
        LocalGet { local_index: 0 }, LocalGet { local_index: 1 },
        Call { function_index: r }, LocalSet { local_index: 3 },
        LocalGet { local_index: 2 }, If { .. }, Unreachable, End,
        LocalGet { local_index: 3 }, I64Const { value: 32 }, I64ShrU,
        LocalGet { local_index: 3 }, I64Const { value: 0xffffffff }, I64And,
        Call { function_index: s }, I32Const { value: -1 }, GlobalSet { global_index: 2 }, End,
    ] if *a == admit && *r == first_rust + 1 && *s == settle));
    let mut guarded_calls = 0;
    for index in first_rust..first_rust + 2 {
        for op in ops(index) {
            match op {
                Call { function_index }
                    if function_index < first_rust && function_index != claim =>
                {
                    guarded_calls += 1
                }
                I32Load { .. }
                | I64Load { .. }
                | I32Load8U { .. }
                | I32Store { .. }
                | I64Store { .. }
                | I32Store8 { .. }
                | MemoryCopy { .. } => panic!("unbound native access"),
                _ => {}
            }
        }
    }
    assert_eq!(guarded_calls, 58);
    fs::write(directory.join("vector/record-linked.wasm"), &bytes).unwrap();
    println!(
        "linked record vector bytes={} sha256={}; artifact admission only",
        bytes.len(),
        sha256_hex(&bytes)
    );
}

#[test]
#[ignore = "make native-provider-probe supplies fresh record compiler output"]
fn compiled_record_vector_uses_the_shared_schema_and_refuses_u64_admission() {
    let (directory, _) = super::input();
    let raw = ic_host_artifacts::read(
        fs::File::open(directory.join("vector/provider_record_vector.wasm")).unwrap(),
        ic_host_artifacts::Length::AtMost(super::artifact::LIMIT as u64),
    )
    .unwrap()
    .0;
    validate_wasm(&raw, super::artifact::LIMIT).unwrap();
    assert!(super::artifact::admit_raw(&raw).is_err());
    assert!(super::vector_access::inspect(&raw).is_err());
    let mut imports = Vec::new();
    let mut exports = BTreeMap::new();
    let mut types = Vec::new();
    let mut functions = Vec::new();
    let mut bodies = Vec::new();
    let mut globals = Vec::new();
    let mut memories = 0;
    let mut listing = String::new();
    let mut memory_sites = BTreeMap::<&str, usize>::new();
    for payload in Parser::new(0).parse_all(&raw) {
        match payload.unwrap() {
            Payload::TypeSection(r) => {
                for ty in r.into_iter_err_on_gc_types() {
                    types.push(ty.unwrap());
                }
            }
            Payload::ImportSection(r) => {
                for import in r.into_imports() {
                    let import = import.unwrap();
                    let TypeRef::Func(ty) = import.ty else {
                        panic!("non-function import")
                    };
                    imports.push((import.module, import.name, ty));
                }
            }
            Payload::FunctionSection(r) => {
                for ty in r {
                    functions.push(ty.unwrap());
                }
            }
            Payload::MemorySection(r) => {
                for memory in r {
                    let memory = memory.unwrap();
                    assert_eq!((memory.initial, memory.maximum), (3, Some(3)));
                    assert!(!memory.memory64 && !memory.shared && memory.page_size_log2.is_none());
                    memories += 1;
                }
            }
            Payload::GlobalSection(r) => {
                for global in r {
                    let global = global.unwrap();
                    assert_eq!(global.ty.content_type, ValType::I32);
                    assert_eq!(global.ty.mutable, globals.is_empty());
                    assert!(!global.ty.shared);
                    let mut ops = global.init_expr.get_operators_reader();
                    assert!(matches!(
                        ops.read().unwrap(),
                        Operator::I32Const { value: 32768 }
                    ));
                    assert!(matches!(ops.read().unwrap(), Operator::End) && ops.eof());
                    globals.push(global.ty);
                }
            }
            Payload::ExportSection(r) => {
                for export in r {
                    let export = export.unwrap();
                    assert!(
                        exports
                            .insert(export.name, (export.kind, export.index))
                            .is_none()
                    );
                }
            }
            Payload::CodeSectionEntry(body) => {
                writeln!(listing, "function {}", bodies.len() + 1).unwrap();
                for op in body.get_operators_reader().unwrap() {
                    let op = op.unwrap();
                    writeln!(listing, "{op:?}").unwrap();
                    let site = match op {
                        Operator::I32Load { .. } => Some("i32.load"),
                        Operator::I64Load { .. } => Some("i64.load"),
                        Operator::I32Load8U { .. } => Some("i32.load8_u"),
                        Operator::I32Store { .. } => Some("i32.store"),
                        Operator::I64Store { .. } => Some("i64.store"),
                        Operator::I32Store8 { .. } => Some("i32.store8"),
                        Operator::MemoryCopy { .. } => Some("memory.copy"),
                        Operator::MemoryGrow { .. } => panic!("growth outside allocation contract"),
                        _ => None,
                    };
                    if let Some(site) = site {
                        *memory_sites.entry(site).or_default() += 1;
                    }
                }
                bodies.push(body);
            }
            Payload::Version { .. }
            | Payload::CodeSectionStart { .. }
            | Payload::CustomSection(_)
            | Payload::End(_) => {}
            _ => panic!("unsupported section, initialization or table"),
        }
    }
    assert_eq!(memories, 1);
    assert_eq!(globals.len(), 3);
    assert_eq!(imports.len(), 1);
    assert_eq!(
        (imports[0].0, imports[0].1),
        ("native_record_vector_diagnostic", "claim_region")
    );
    let claim = &types[imports[0].2 as usize];
    assert!(claim.params().is_empty());
    assert_eq!(claim.results(), [ValType::I32]);
    let &(ExternalKind::Func, entry) = exports.get("provider_record_vector_step").unwrap() else {
        panic!("record entrypoint must be a function")
    };
    let entry_type = &types[functions[entry as usize - 1] as usize];
    assert_eq!(
        entry_type.params(),
        [ValType::I32, ValType::I32, ValType::I32, ValType::I64]
    );
    assert_eq!(entry_type.results(), [ValType::I64]);
    assert_eq!(
        exports,
        BTreeMap::from([
            ("__data_end", (ExternalKind::Global, 1)),
            ("__heap_base", (ExternalKind::Global, 2)),
            ("__stack_pointer", (ExternalKind::Global, 0)),
            ("memory", (ExternalKind::Memory, 0)),
            ("provider_record_vector_step", (ExternalKind::Func, entry)),
        ])
    );
    fs::write(directory.join("vector/record-operators.txt"), listing).unwrap();
    let stack = super::vector_stack::inspect(&bodies, entry, 0, 32768).unwrap();
    assert_eq!(stack.frame_bytes, 64);
    assert_eq!(stack.lowest_stack_pointer, 32704);
    let profile = super::vector_access::inspect_record(&raw).unwrap();
    assert_eq!((profile.functions, profile.entry), (2, 2));
    let guarded = super::vector_access::prepare_record(&raw).unwrap();
    assert_eq!(
        super::vector_access::tests::verify_coverage(&raw, &guarded),
        [22, 1, 3, 26, 2, 3, 1]
    );
    fs::write(directory.join("vector/record-guarded.wasm"), &guarded).unwrap();
    println!(
        "record vector sha256={} bytes={} functions={} entry={entry} sites={memory_sites:?} stack={stack:?} guarded_sha256={}; NOT linked or executed",
        sha256_hex(&raw),
        raw.len(),
        functions.len(),
        sha256_hex(&guarded)
    );
}

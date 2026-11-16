//! Inspect actual compiler composition without installing or executing it.

use ic_storage_retention_probe::artifact::{sha256_hex, validate_wasm};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
};

use wasm_encoder::{
    CodeSection, Module, RawSection,
    reencode::{Reencode, RoundtripReencoder},
};
use wasmparser::{ExternalKind, Operator as Op, Parser, Payload};

use super::{Profile, admit_final, prepare};

#[test]
#[ignore = "make native-provider-probe supplies fresh vector compiler output"]
fn linked_vector_binds_guards_and_orders_custody_without_execution() {
    let (directory, _) = super::super::input();
    let raw = fs::read(directory.join("vector/provider_vector.wasm")).unwrap();
    let bytes = prepare(&raw, Profile::A).unwrap();
    admit_final(&bytes, &raw, Profile::A).unwrap();
    let mut imports = Vec::new();
    let mut exports = BTreeMap::new();
    let mut bodies = Vec::new();
    let mut memories = 0;
    let mut globals = 0;
    let mut retention_markers = 0;
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
                    assert!(!export.name.starts_with("__link_"));
                    assert!(!export.name.starts_with("provider_"));
                    assert!(export.kind == ExternalKind::Func || export.name == "memory");
                    exports.insert(export.name, export.index);
                }
            }
            Payload::MemorySection(r) => {
                for memory in r {
                    let memory = memory.unwrap();
                    assert_eq!((memory.initial, memory.maximum), (3, Some(3)));
                    memories += 1;
                }
            }
            Payload::GlobalSection(r) => globals = r.count(),
            Payload::CodeSectionEntry(body) => bodies.push(body),
            Payload::CustomSection(section) => {
                assert_eq!(
                    section.name(),
                    "icp:private enhanced-orthogonal-persistence"
                );
                assert!(section.data().is_empty());
                retention_markers += 1;
            }
            Payload::DataSection(_) | Payload::StartSection { .. } | Payload::TableSection(_) => {
                panic!("unexpected initialization")
            }
            _ => {}
        }
    }
    assert_eq!(
        imports,
        [("ic0", "msg_reply_data_append"), ("ic0", "msg_reply")]
    );
    assert_eq!((memories, globals), (1, 6));
    assert_eq!(retention_markers, 1);
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
    let profile = super::super::vector_access::inspect(&raw).unwrap();
    let first_rust = bodies.len() as u32 + 2 - profile.functions;
    let root = first_rust + profile.entry - 1;
    assert!(!exports.values().any(|index| *index >= first_rust));
    let append = calls(exports["canister_update append"])[0];
    let wat_calls = calls(append);
    let claim = wat_calls[0];
    let settle = wat_calls[1];
    let admit = calls(claim)[0];
    let run = calls(exports["canister_update vector_mutate"])[0];
    let run_with_fault = calls(run)[0];
    for (operation, name) in ["mutate", "append", "remove", "shrink", "clear", "reuse"]
        .into_iter()
        .enumerate()
    {
        assert!(
            matches!(ops(exports[format!("canister_update vector_{name}").as_str()])[0],
            Op::I32Const { value } if value == operation as i32)
        );
        assert_eq!(
            calls(exports[format!("canister_update vector_{name}").as_str()]),
            [run, 1]
        );
    }
    for (operation, name, expected_calls) in [
        (6, "vector_business_error", vec![run, 0, 1]),
        (7, "vector_forget", vec![run, 1]),
        (6, "vector_error_trap", vec![run]),
    ] {
        let entry = exports[format!("canister_update {name}").as_str()];
        assert!(matches!(ops(entry)[0], Op::I32Const { value } if value == operation));
        assert_eq!(calls(entry), expected_calls);
    }
    // Full instruction sequence: admission precedes narrowing and native entry;
    // the injected trap follows native return but precedes settlement and idle.
    use Op::*;
    assert!(matches!(&ops(run)[..], [
        LocalGet { local_index: 0 }, LocalGet { local_index: 1 },
        I32Const { value: 0 }, Call { function_index }, End,
    ] if *function_index == run_with_fault));
    assert!(
        matches!(&ops(exports["canister_update vector_error_finalize_trap"])[..], [
        I32Const { value: 6 }, GlobalGet { global_index: 0 },
        I32Const { value: 1 }, Call { function_index }, End,
    ] if *function_index == run_with_fault)
    );
    let run_ops = ops(run_with_fault);
    assert!(matches!(&run_ops[..], [
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
    ] if *a == admit && *r == root && *s == settle));
    let post = calls(exports["canister_post_upgrade"]);
    assert_eq!(post.len(), 2);
    assert_eq!(post[1], run);
    assert_eq!(calls(post[0])[0], admit);
    assert!(matches!(
        &ops(exports["canister_pre_upgrade"])[..],
        [
            GlobalGet { global_index: 2 },
            I32Const { value: -1 },
            I32Ne,
            If { .. },
            Unreachable,
            End,
            Call { .. },
            End,
        ]
    ));

    let mut guard_counts = BTreeMap::new();
    for index in first_rust..first_rust + profile.functions {
        for op in ops(index) {
            match op {
                Call { function_index }
                    if function_index != claim && function_index < first_rust =>
                {
                    *guard_counts.entry(function_index).or_insert(0) += 1;
                }
                I32Load { .. }
                | I32Load8U { .. }
                | I64Load { .. }
                | I32Store { .. }
                | I32Store8 { .. }
                | I64Store { .. }
                | MemoryCopy { .. } => panic!("unguarded Rust access"),
                GlobalGet { global_index } | GlobalSet { global_index } => {
                    assert_eq!(global_index, 3)
                }
                _ => {}
            }
        }
    }
    assert_eq!(guard_counts.len(), 7);
    assert_eq!(guard_counts.values().sum::<u32>(), 85);
    let guards: Vec<_> = guard_counts.keys().copied().collect();
    assert_eq!(
        guard_counts.values().copied().collect::<Vec<_>>(),
        [36, 1, 2, 37, 2, 6, 1]
    );
    let access = calls(guards[0])[0];
    for private in guards
        .iter()
        .copied()
        .chain([claim, settle, access, run, run_with_fault])
    {
        assert!(!exports.values().any(|index| *index == private));
    }
    assert_eq!(
        calls(root)
            .iter()
            .filter(|&&target| target == claim)
            .count(),
        1
    );
    assert_eq!(
        calls(root)
            .iter()
            .filter(|&&target| target >= first_rust)
            .count(),
        4
    );
    for (kind, guard) in guards.iter().enumerate() {
        assert_eq!(
            calls(*guard),
            if kind == 6 {
                vec![access, access]
            } else {
                vec![access]
            }
        );
        assert!(ops(*guard).iter().any(|op| matches!(
            (kind, op),
            (0, I32Load { .. })
                | (1, I32Load8U { .. })
                | (2, I64Load { .. })
                | (3, I32Store { .. })
                | (4, I32Store8 { .. })
                | (5, I64Store { .. })
                | (6, MemoryCopy { .. })
        )));
    }
    // The entire foreign closure is non-reentrant: no provider/helper path can
    // call any Rust body, even transitively or through a host callback.
    let mut pending = guards.clone();
    pending.push(claim);
    let mut seen = BTreeSet::new();
    while let Some(index) = pending.pop() {
        assert!(index >= 2 && index < first_rust);
        if seen.insert(index) {
            pending.extend(calls(index));
        }
    }

    // Swap two same-signature guards. This remains valid Wasm and therefore must
    // be rejected by exact composition, not accidentally by Wasm validation.
    let changed = super::super::rewrite(&bytes, |module: &mut Module, id, data| {
        if id != 10 {
            module.section(&RawSection { id, data });
            return;
        }
        let mut code = CodeSection::new();
        for body in &bodies {
            let mut copy = RoundtripReencoder;
            let mut function = copy.new_function_with_parsed_locals(body).unwrap();
            for op in body.get_operators_reader().unwrap() {
                let mut op = op.unwrap();
                if let Call { function_index } = &mut op {
                    if *function_index == guards[0] {
                        *function_index = guards[1];
                    } else if *function_index == guards[1] {
                        *function_index = guards[0];
                    }
                }
                function.instruction(&copy.instruction(op).unwrap());
            }
            code.function(&function);
        }
        module.section(&code);
    });
    validate_wasm(&changed, super::LIMIT).unwrap();
    assert_eq!(
        admit_final(&changed, &raw, Profile::A),
        Err("final vector mismatch")
    );
    let version_b = prepare(&raw, Profile::B).unwrap();
    assert_eq!(
        admit_final(&version_b, &raw, Profile::A),
        Err("final vector mismatch")
    );
    admit_final(&version_b, &raw, Profile::B).unwrap();
    fs::write(directory.join("vector/linked.wasm"), &bytes).unwrap();
    println!(
        "linked vector bytes={} sha256={}; NOT executed",
        bytes.len(),
        sha256_hex(&bytes)
    );
}

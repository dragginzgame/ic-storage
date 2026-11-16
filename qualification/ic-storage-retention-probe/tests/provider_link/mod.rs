//! Bounded scalar-provider artifact and installed lifecycle qualification.
#![forbid(unsafe_code)]

mod artifact;
mod entrypoints;
mod installed_tests;
mod migration;
mod vector_access;
mod vector_diagnostic;
mod vector_link;
mod vector_stack;

use artifact::Profile;
use ic_storage_retention_probe::artifact::{sha256_hex, validate_wasm};

use std::{collections::BTreeMap, fs, path::PathBuf};
use wasm_encoder::{
    CodeSection, ConstExpr, DataSection, EntityType, GlobalSection, GlobalType, ImportSection,
    MemorySection, MemoryType, Module, RawSection, ValType,
    reencode::{Reencode, RoundtripReencoder},
};
use wasmparser::{Operator, Parser, Payload};

fn input() -> (PathBuf, Vec<u8>) {
    let directory = PathBuf::from(
        std::env::var_os("NATIVE_PROVIDER_INPUTS")
            .expect("make native-provider-probe supplies fresh inputs"),
    );
    assert!(directory.is_absolute());
    let raw = ic_host_artifacts::read(
        fs::File::open(directory.join("provider_abi.wasm")).unwrap(),
        ic_host_artifacts::Length::AtMost((artifact::LIMIT) as u64),
    )
    .unwrap()
    .0;
    (directory, raw)
}

fn rewrite(raw: &[u8], mut section: impl FnMut(&mut Module, u8, &[u8])) -> Vec<u8> {
    let mut module = Module::new();
    for payload in Parser::new(0).parse_all(raw) {
        if let Some((id, range)) = payload.unwrap().as_section() {
            section(
                &mut module,
                id,
                &raw[range.start as usize..range.end as usize],
            );
        }
    }
    module.finish()
}

#[test]
#[ignore = "make native-provider-probe supplies fresh compiler output"]
fn compiled_provider_boundary_resolves_the_actual_claim_without_execution() {
    let (directory, raw) = input();
    artifact::admit_raw(&raw).unwrap();
    let combined = artifact::prepare(&raw, Profile::A).unwrap();
    artifact::admit_final(&combined, &raw, Profile::A).unwrap();
    let mut imports = Vec::new();
    let mut exports = BTreeMap::new();
    let mut bodies = Vec::new();
    let mut memories = 0;
    for payload in Parser::new(0).parse_all(&combined) {
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
            Payload::CodeSectionEntry(body) => bodies.push(body),
            _ => {}
        }
    }
    assert_eq!(memories, 1);
    assert_eq!(
        imports,
        [("ic0", "msg_reply_data_append"), ("ic0", "msg_reply")]
    );
    for name in [
        "__link_claim",
        "__link_settle",
        "provider_probe_read",
        "provider_probe_write",
    ] {
        assert!(!exports.contains_key(name));
    }
    let body = |index: u32| &bodies[index as usize - imports.len()];
    let calls = |index| {
        body(index)
            .get_operators_reader()
            .unwrap()
            .into_iter()
            .filter_map(|op| match op.unwrap() {
                Operator::Call { function_index } => Some(function_index),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    let wat = calls(exports["canister_update advance"]);
    let read = calls(exports["canister_query native_read"]);
    let write = calls(exports["canister_update native_write"]);
    let post = calls(exports["canister_post_upgrade"]);
    assert_eq!(read.len(), 4);
    assert_eq!(write.len(), 3);
    assert_eq!(post.len(), 3);
    // Follow the existing WAT client's claim and settlement calls independently
    // of the combiner's map. Native reads/writes must return before settlement.
    assert_eq!(
        artifact::normalized(body(read[0])).unwrap(),
        artifact::scalar_body(false, wat[0])
    );
    assert_eq!(
        artifact::normalized(body(write[0])).unwrap(),
        artifact::scalar_body(true, wat[0])
    );
    assert_eq!(read[1], wat[1]);
    assert_eq!(write[1], wat[1]);
    assert_eq!(&post[1..], &[read[0], wat[1]]);
    assert_ne!(post[0], read[0]);
    fs::write(directory.join("combined.wasm"), &combined).unwrap();
    println!("raw sha256={} bytes={}", sha256_hex(&raw), raw.len());
    println!(
        "combined sha256={} bytes={}",
        sha256_hex(&combined),
        combined.len()
    );
}

#[test]
#[ignore = "make native-provider-probe supplies fresh compiler output"]
fn raw_profile_refuses_overlap_initialization_imports_and_extra_code() {
    let (_, raw) = input();
    let memory = rewrite(&raw, |module, id, bytes| {
        if id == 5 {
            let mut section = MemorySection::new();
            section.memory(MemoryType {
                minimum: 3,
                maximum: Some(4),
                memory64: false,
                shared: false,
                page_size_log2: None,
            });
            module.section(&section);
        } else {
            module.section(&RawSection { id, data: bytes });
        }
    });
    assert_eq!(artifact::admit_raw(&memory), Err("memory"));
    let overlap = rewrite(&raw, |module, id, bytes| {
        if id == 6 {
            let mut section = GlobalSection::new();
            for (index, value) in [65536, 131080, 131080].into_iter().enumerate() {
                section.global(
                    GlobalType {
                        val_type: ValType::I32,
                        mutable: index == 0,
                        shared: false,
                    },
                    &ConstExpr::i32_const(value),
                );
            }
            module.section(&section);
        } else {
            module.section(&RawSection { id, data: bytes });
        }
    });
    assert_eq!(artifact::admit_raw(&overlap), Err("transient bounds"));
    let initialized = rewrite(&raw, |module, id, bytes| {
        module.section(&RawSection { id, data: bytes });
        if id == 10 {
            let mut data = DataSection::new();
            data.active(0, &ConstExpr::i32_const(131072), [99]);
            module.section(&data);
        }
    });
    assert_eq!(
        artifact::admit_raw(&initialized),
        Err("unsupported section")
    );
    let wrong_import = rewrite(&raw, |module, id, bytes| {
        if id == 2 {
            let mut imports = ImportSection::new();
            imports.import("unexpected", "claim_region", EntityType::Function(0));
            module.section(&imports);
        } else {
            module.section(&RawSection { id, data: bytes });
        }
    });
    assert_eq!(artifact::admit_raw(&wrong_import), Err("roster"));
    let growth = rewrite(&raw, |module, id, bytes| {
        if id == 10 {
            let mut code = CodeSection::new();
            let mut read = artifact::scalar_body(false, 0);
            read.pop();
            // Valid additional zero-page memory.grow; the size need not change.
            read.extend_from_slice(&[0x41, 0, 0x40, 0, 0x1a, 0x0b]);
            code.raw(&read).raw(&artifact::scalar_body(true, 0));
            module.section(&code);
        } else {
            module.section(&RawSection { id, data: bytes });
        }
    });
    validate_wasm(&growth, artifact::LIMIT).unwrap();
    assert_eq!(artifact::admit_raw(&growth), Err("scalar code"));
}

#[test]
#[ignore = "make native-provider-probe supplies fresh compiler output"]
fn final_admission_rejects_unresolved_imports_and_a_different_provider() {
    let (_, raw) = input();
    assert_eq!(
        artifact::admit_final(&raw, &raw, Profile::A),
        Err("final mismatch")
    );
    let combined = artifact::prepare(&raw, Profile::A).unwrap();
    let changed = rewrite(&combined, |module, id, bytes| {
        if id == 6 {
            let mut section = GlobalSection::new();
            for payload in Parser::new(0).parse_all(&combined) {
                if let Payload::GlobalSection(r) = payload.unwrap() {
                    for (index, global) in r.into_iter().enumerate() {
                        if index == 0 {
                            section.global(
                                GlobalType {
                                    val_type: ValType::I64,
                                    mutable: false,
                                    shared: false,
                                },
                                &ConstExpr::i64_const(99),
                            );
                        } else {
                            RoundtripReencoder
                                .parse_global(&mut section, global.unwrap())
                                .unwrap();
                        }
                    }
                }
            }
            module.section(&section);
        } else {
            module.section(&RawSection { id, data: bytes });
        }
    });
    validate_wasm(&changed, artifact::LIMIT).unwrap();
    assert_eq!(
        artifact::admit_final(&changed, &raw, Profile::A),
        Err("final mismatch")
    );
}

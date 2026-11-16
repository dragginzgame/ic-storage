use super::*;
use ic_storage_retention_probe::artifact::{sha256_hex, validate_wasm};
use std::{fs, path::PathBuf};
use wasm_encoder::{
    CodeSection, ConstExpr, DataSection, Instruction as I, Module, RawSection,
    reencode::{Reencode, RoundtripReencoder},
};
use wasmparser::{Operator as Op, Parser, Payload};

fn inputs() -> (PathBuf, Vec<u8>, Vec<u8>) {
    let (directory, _) = super::super::input();
    let native = fs::read(directory.join("vector/provider_vector.wasm")).unwrap();
    let bridge = fs::read(directory.join("checkpoint-bridge.wasm")).unwrap();
    (directory, native, bridge)
}

#[test]
#[ignore = "native-checkpoint-test supplies fresh compiler inputs"]
fn bridge_artifact_has_disjoint_initialization_and_resolved_calls() {
    let (directory, raw, bridge) = inputs();
    let bytes = prepare(&raw, &bridge, false, (3, 4), false);
    let recovery = prepare(&raw, &bridge, true, (0, 0), true);
    let mut exports = std::collections::BTreeMap::new();
    let mut bodies = Vec::new();
    let mut imports = 0;
    for payload in Parser::new(0).parse_all(&recovery) {
        match payload.unwrap() {
            Payload::ImportSection(r) => {
                for import in r.into_imports() {
                    assert_eq!(import.unwrap().module, "ic0");
                    imports += 1;
                }
            }
            Payload::ExportSection(r) => {
                for export in r {
                    let export = export.unwrap();
                    assert!(export.name == "memory" || export.name.starts_with("canister_"));
                    exports.insert(export.name, export.index);
                }
            }
            Payload::CodeSectionEntry(body) => bodies.push(body),
            _ => {}
        }
    }
    let calls = |index: u32| {
        bodies[(index - imports) as usize]
            .get_operators_reader()
            .unwrap()
            .into_iter()
            .filter_map(|op| {
                if let Op::Call { function_index } = op.unwrap() {
                    Some(function_index)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
    };
    let init = calls(exports["canister_init"]);
    let post = calls(exports["canister_post_upgrade"]);
    assert_eq!((init.len(), post.len()), (2, 3));
    assert!(
        post[0] > post[1] && post[2] > post[1],
        "WAT fresh check, Rust restore, then native entry"
    );
    // Swap same-signature create/restore calls. It remains valid Wasm; exact
    // final admission must reject the changed lifecycle rather than validation.
    let changed = crate::rewrite(&recovery, |module: &mut Module, id, data| {
        if id != 10 {
            module.section(&RawSection { id, data });
            return;
        }
        let mut code = CodeSection::new();
        for body in &bodies {
            let mut copy = RoundtripReencoder;
            let mut function = copy.new_function_with_parsed_locals(body).unwrap();
            for op in body.get_operators_reader().unwrap() {
                let op = op.unwrap();
                if matches!(op, Op::Call { function_index } if function_index == post[1]) {
                    function.instruction(&I::Call(init[1]));
                } else {
                    function.instruction(&copy.instruction(op).unwrap());
                }
            }
            code.function(&function);
        }
        module.section(&code);
    });
    validate_wasm(&changed, LIMIT).unwrap();
    assert!(!admit_final(&changed, &raw, &bridge, true, (0, 0), true));
    assert!(!admit_final(&recovery, &raw, &bridge, false, (0, 0), true));
    let overlap = crate::rewrite(&bridge, |module, id, bytes| {
        if id != 11 {
            module.section(&RawSection { id, data: bytes });
            return;
        }
        let mut data = DataSection::new();
        for payload in Parser::new(0).parse_all(&bridge) {
            if let Payload::DataSection(r) = payload.unwrap() {
                for (index, item) in r.into_iter().enumerate() {
                    let item = item.unwrap();
                    if index == 0 {
                        data.active(0, &ConstExpr::i32_const(131072), item.data.to_vec());
                    } else {
                        RoundtripReencoder.parse_data(&mut data, item).unwrap();
                    }
                }
            }
        }
        module.section(&data);
    });
    validate_wasm(&overlap, LIMIT).unwrap();
    assert!(std::panic::catch_unwind(|| prepare(&raw, &overlap, false, (3, 4), false)).is_err());
    fs::write(directory.join("native-checkpoint.wasm"), &bytes).unwrap();
    println!(
        "native checkpoint bytes={} sha256={}",
        bytes.len(),
        sha256_hex(&bytes)
    );
}

pub(super) mod installed;
mod job;
mod records;
mod reopen;

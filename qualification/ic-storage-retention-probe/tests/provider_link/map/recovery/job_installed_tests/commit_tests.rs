//! The native apply frame ends before checkpoint verification and publication.
use super::*;
use wasm_encoder::{
    CodeSection, Instruction as I, RawSection,
    reencode::{Reencode, RoundtripReencoder},
};
use wasmparser::{Operator as Op, Parser, Payload};

#[test]
#[ignore = "map job collector supplies compiled native and checkpoint domains"]
fn post_apply_verification_precedes_publication_and_reply() {
    let (directory, raw) = inputs();
    let bridge = fs::read(directory.join("checkpoint-map-job.wasm")).unwrap();
    for post in [Post::Stage, Post::Restart] {
        let bytes = prepare_mode(&raw, &bridge, true, post, true);
        let mut imports = 0;
        let mut exports = std::collections::BTreeMap::new();
        let mut bodies = Vec::new();
        for payload in Parser::new(0).parse_all(&bytes) {
            match payload.unwrap() {
                Payload::ImportSection(reader) => imports = reader.into_imports().count(),
                Payload::ExportSection(reader) => {
                    for export in reader {
                        let export = export.unwrap();
                        assert!(export.name == "memory" || export.name.starts_with("canister_"));
                        exports.insert(export.name, export.index);
                    }
                }
                Payload::CodeSectionEntry(body) => bodies.push(body),
                _ => {}
            }
        }
        let calls = |entry: &str| {
            bodies[exports[entry] as usize - imports]
                .get_operators_reader()
                .unwrap()
                .into_iter()
                .filter_map(|op| match op.unwrap() {
                    Op::Call { function_index } => Some(function_index),
                    _ => None,
                })
                .collect::<Vec<_>>()
        };
        let init = calls("canister_init");
        let upgrade = calls("canister_post_upgrade");
        let batch = calls("canister_update import_batch");
        assert_eq!((upgrade.len(), batch.len()), (5, 6));
        assert_eq!(&upgrade[2..], &batch[2..5]);
        assert!(batch[0] >= init[0] && batch[1] < init[0]);
        assert!(batch[2] >= init[0] && batch[3] < init[0] && batch[4] >= init[0]);
        assert_ne!(batch[1], batch[3]);
        assert!(batch[5] >= init[0]);
        // Reordering same-signature native publication before verification is
        // valid Wasm but differs from the exact admitted composition.
        let changed = crate::rewrite(&bytes, |module, id, data| {
            if id != 10 {
                module.section(&RawSection { id, data });
                return;
            }
            let mut code = CodeSection::new();
            for (index, body) in bodies.iter().enumerate() {
                let mut copy = RoundtripReencoder;
                let mut function = copy.new_function_with_parsed_locals(body).unwrap();
                for op in body.get_operators_reader().unwrap() {
                    let op = op.unwrap();
                    if index + imports == exports["canister_update import_batch"] as usize {
                        match op {
                            Op::Call { function_index } if function_index == batch[3] => {
                                function.instruction(&I::Call(batch[4]));
                                continue;
                            }
                            Op::Call { function_index } if function_index == batch[4] => {
                                function.instruction(&I::Call(batch[3]));
                                continue;
                            }
                            _ => {}
                        }
                    }
                    function.instruction(&copy.instruction(op).unwrap());
                }
                code.function(&function);
            }
            module.section(&code);
        });
        validate_wasm(&changed, LIMIT).unwrap();
        assert_ne!(changed, prepare_mode(&raw, &bridge, true, post, true));
    }
}

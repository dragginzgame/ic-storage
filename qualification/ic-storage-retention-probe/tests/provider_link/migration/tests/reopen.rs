use super::*;
use wasm_encoder::{GlobalSection, GlobalType, ValType};

#[test]
#[ignore = "native-checkpoint-test supplies fresh compiler inputs"]
fn reopening_rebuilds_only_the_admitted_transient_domain() {
    let (directory, raw, bridge) = inputs();
    let bytes = prepare_reopen(&raw, &bridge, false, (3, 4));
    let mut data_count = None;
    let mut active = Vec::new();
    let mut passive = Vec::new();
    let mut bodies = Vec::new();
    let mut imported = 0;
    let mut post = None;
    for payload in Parser::new(0).parse_all(&bytes) {
        match payload.unwrap() {
            Payload::ImportSection(r) => imported = r.into_imports().count(),
            Payload::DataCountSection { count, .. } => data_count = Some(count),
            Payload::DataSection(r) => {
                for segment in r {
                    let segment = segment.unwrap();
                    match segment.kind {
                        wasmparser::DataKind::Active { .. } => active.push(segment.data),
                        wasmparser::DataKind::Passive => passive.push(segment.data),
                    }
                }
            }
            Payload::ExportSection(r) => {
                for export in r {
                    let export = export.unwrap();
                    if export.name == "canister_post_upgrade" {
                        post = Some(export.index);
                    }
                }
            }
            Payload::CodeSectionEntry(body) => bodies.push(body),
            _ => {}
        }
    }
    assert_eq!(
        active, passive,
        "reset restores exactly the target initialization"
    );
    assert_eq!(data_count, Some((active.len() * 2) as u32));
    let ops: Vec<_> = bodies[post.unwrap() as usize - imported]
        .get_operators_reader()
        .unwrap()
        .into_iter()
        .map(Result::unwrap)
        .collect();
    let [
        Op::Call {
            function_index: native,
        },
        Op::Call {
            function_index: reset,
        },
        Op::End,
    ] = ops[..]
    else {
        panic!("native admission/return must precede transient reset")
    };
    assert!(native < reset);
    let reset_body = &bodies[reset as usize - imported];
    let reset_ops: Vec<_> = reset_body
        .get_operators_reader()
        .unwrap()
        .into_iter()
        .map(Result::unwrap)
        .collect();
    assert_eq!(
        reset_ops
            .iter()
            .filter(|op| matches!(op, Op::MemoryFill { .. }))
            .count(),
        2
    );
    assert_eq!(
        reset_ops
            .iter()
            .filter(|op| matches!(op, Op::MemoryInit { .. }))
            .count(),
        active.len()
    );
    assert!(
        !reset_ops
            .iter()
            .any(|op| matches!(op, Op::Call { .. } | Op::CallIndirect { .. }))
    );

    // Equal types and valid Wasm do not admit changing the reset lower bound.
    let changed = crate::rewrite(&bytes, |module, id, data| {
        if id != 10 {
            module.section(&RawSection { id, data });
            return;
        }
        let mut code = CodeSection::new();
        for (index, body) in bodies.iter().enumerate() {
            let mut copy = RoundtripReencoder;
            let mut f = copy.new_function_with_parsed_locals(body).unwrap();
            for op in body.get_operators_reader().unwrap() {
                let op = op.unwrap();
                if index == reset as usize - imported
                    && matches!(op, Op::I32Const { value: 196608 })
                {
                    f.instruction(&I::I32Const(131072));
                } else {
                    f.instruction(&copy.instruction(op).unwrap());
                }
            }
            code.function(&f);
        }
        module.section(&code);
    });
    wasm::validate(&changed, LIMIT).unwrap();
    assert_ne!(changed, prepare_reopen(&raw, &bridge, false, (3, 4)));

    // A second private mutable global has no admitted reset contract.
    let mutable = crate::rewrite(&bridge, |module, id, data| {
        if id != 6 {
            module.section(&RawSection { id, data });
            return;
        }
        let mut globals = GlobalSection::new();
        for payload in Parser::new(0).parse_all(&bridge) {
            if let Payload::GlobalSection(r) = payload.unwrap() {
                RoundtripReencoder
                    .parse_global_section(&mut globals, r)
                    .unwrap();
            }
        }
        globals.global(
            GlobalType {
                val_type: ValType::I32,
                mutable: true,
                shared: false,
            },
            &ConstExpr::i32_const(65536),
        );
        module.section(&globals);
    });
    wasm::validate(&mutable, LIMIT).unwrap();
    assert!(std::panic::catch_unwind(|| prepare_reopen(&raw, &mutable, false, (3, 4))).is_err());
    // Even a valid, zero-length table write exceeds this profile's contract.
    let table_write = crate::rewrite(&bridge, |module, id, data| {
        if id != 10 {
            module.section(&RawSection { id, data });
            return;
        }
        let mut code = CodeSection::new();
        for payload in Parser::new(0).parse_all(&bridge) {
            if let Payload::CodeSectionEntry(body) = payload.unwrap() {
                let mut copy = RoundtripReencoder;
                let mut f = copy.new_function_with_parsed_locals(&body).unwrap();
                if code.is_empty() {
                    f.instruction(&I::I32Const(0));
                    f.instruction(&I::RefNull(wasm_encoder::HeapType::FUNC));
                    f.instruction(&I::I32Const(0));
                    f.instruction(&I::TableFill(0));
                }
                for op in body.get_operators_reader().unwrap() {
                    f.instruction(&copy.instruction(op.unwrap()).unwrap());
                }
                code.function(&f);
            }
        }
        module.section(&code);
    });
    wasm::validate(&table_write, LIMIT).unwrap();
    assert!(
        std::panic::catch_unwind(|| prepare_reopen(&raw, &table_write, false, (3, 4))).is_err()
    );
    let digest = Sha256Digest::compute(&bytes);
    fs::write(directory.join(format!("reopen-{digest}.wasm")), &bytes).unwrap();
    println!("reopen bytes={} sha256={digest}", bytes.len());
}

mod installed;

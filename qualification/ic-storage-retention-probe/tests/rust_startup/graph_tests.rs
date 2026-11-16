//! Installed graph custody, retaining upgrades and message rollback.

use super::artifact::{admit_graph, prepare_graph};
use super::*;
use ic_storage_retention_probe::artifact::{sha256, sha256_hex, validate_wasm};
use wasm_encoder::{
    CodeSection, Encode, Instruction as I, MemArg,
    reencode::{Reencode, RoundtripReencoder},
};

fn input_graph(variant: &str) -> Vec<u8> {
    let path = root().join(format!(
        "target/qualification/graph-{variant}/wasm32-unknown-unknown/release/ic_storage_rust_startup_probe.wasm"
    ));
    ic_host_artifacts::read(
        fs::File::open(path).expect("make managed-graph-build"),
        ic_host_artifacts::Length::AtMost((1024 * 1024) as u64),
    )
    .expect("bounded graph artifact")
    .0
}

#[test]
fn graph_artifacts_have_a_separate_exact_profile() {
    let raw = input_graph("a");
    let a = prepare_graph(&raw).expect("graph admission");
    admit_graph(&a.bytes, &raw).expect("final graph admission");
    assert!(prepare(&raw).is_err());
    assert!(prepare_graph(&input("a")).is_err());
    assert_eq!(
        admit_graph(&a.bytes, &input_graph("b")),
        Err(Refusal::FinalMismatch)
    );
    for removed in [0, 8] {
        let altered = rewrite(&a.bytes, |m, id, bytes| {
            if id != removed {
                m.section(&RawSection { id, data: bytes });
            }
        });
        assert_eq!(admit_graph(&altered, &raw), Err(Refusal::FinalMismatch));
    }
    for mutate_table in [false, true] {
        let mut code = CodeSection::new();
        let mut first = true;
        for payload in Parser::new(0).parse_all(&raw) {
            if let Payload::CodeSectionEntry(body) = payload.expect("source") {
                if !first {
                    code.raw(body.as_bytes());
                    continue;
                }
                first = false;
                let body = body.as_bytes();
                let mut changed = body[..body.len() - 1].to_vec();
                I::I32Const(1).encode(&mut changed);
                if mutate_table {
                    I::RefNull(wasm_encoder::HeapType::FUNC).encode(&mut changed);
                    I::TableSet(0).encode(&mut changed);
                } else {
                    I::MemoryGrow(0).encode(&mut changed);
                    I::Drop.encode(&mut changed);
                }
                I::End.encode(&mut changed);
                code.raw(&changed);
            }
        }
        let altered = rewrite(&raw, |m, id, data| {
            if id == 10 {
                m.section(&code);
            } else {
                m.section(&RawSection { id, data });
            }
        });
        validate_wasm(&altered, 1024 * 1024).expect("valid negative source");
        assert_eq!(
            prepare_graph(&altered).err(),
            Some(if mutate_table {
                Refusal::IndirectCall
            } else {
                Refusal::GrowthCount
            })
        );
    }
}

async fn retained_bytes(pic: &PocketIc, id: CanisterId) -> Vec<u8> {
    pic.query_call(id, id, "graph_bytes", vec![])
        .await
        .expect("complete retained bytes")
}

// Two deliberately unadmitted destinations: inject an incompatible version
// before normal post-upgrade admission, or invoke a growth/mutation trap after
// normal admission. Neither is accepted by the positive artifact validator.
fn negative_upgrade(source: &[u8], incompatible: bool) -> Vec<u8> {
    let mut post = None;
    let mut trap = None;
    let mut bodies = Vec::new();
    for payload in Parser::new(0).parse_all(source) {
        match payload.expect("admitted graph") {
            Payload::ExportSection(exports) => {
                for export in exports {
                    let export = export.expect("export");
                    match export.name {
                        "canister_post_upgrade" => post = Some(export.index as usize - 2),
                        "canister_update graph_trap" => trap = Some(export.index),
                        _ => {}
                    }
                }
            }
            Payload::CodeSectionEntry(body) => bodies.push(body),
            _ => {}
        }
    }
    let mut code = CodeSection::new();
    for (index, body) in bodies.iter().enumerate() {
        if Some(index) != post {
            code.raw(body.as_bytes());
            continue;
        }
        let raw = body.as_bytes();
        let ops = body
            .get_operators_reader()
            .expect("operators")
            .original_position()
            - body.range().start;
        let ops = usize::try_from(ops).expect("bounded fixture body");
        let mut changed = raw[..ops].to_vec();
        if incompatible {
            I::I32Const(BOUNDARY + 8).encode(&mut changed);
            I::I32Const(2).encode(&mut changed);
            I::I32Store(MemArg {
                offset: 0,
                align: 2,
                target: wasm_encoder::MemArgTarget::Memory(0),
                ordering: None,
            })
            .encode(&mut changed);
        }
        let mut reader = body.get_operators_reader().expect("operators");
        while !reader.eof() {
            let op = reader.read().expect("instruction");
            // LLVM may return explicitly, leaving an appended tail unreachable.
            // Inject at every normal exit, including the implicit final end.
            if !incompatible && (matches!(op, wasmparser::Operator::Return) || reader.eof()) {
                I::Call(trap.expect("trap export")).encode(&mut changed);
            }
            RoundtripReencoder
                .instruction(op)
                .expect("instruction encoding")
                .encode(&mut changed);
        }
        code.raw(&changed);
    }
    let negative = rewrite(source, |m, id, data| {
        if id == 10 {
            m.section(&code);
        } else {
            m.section(&RawSection { id, data });
        }
    });
    validate_wasm(&negative, 1024 * 1024).expect("negative Wasm");
    negative
}

async fn refused_upgrade(pic: &PocketIc, id: CanisterId, source: &[u8], destination: &[u8]) {
    let graph = words::<12>(pic, id, "graph").await;
    let transient = words::<4>(pic, id, "transient").await;
    let bytes = retained_bytes(pic, id).await;
    pic.stop_canister(id, None).await.expect("stop");
    let rejection = pic
        .upgrade_eop_canister(id, destination.to_vec(), vec![], None)
        .await
        .expect_err("negative upgrade");
    assert_eq!(rejection.error_code, ErrorCode::CanisterTrapped);
    assert_eq!(
        pic.canister_status(id, None)
            .await
            .expect("status")
            .module_hash
            .expect("module hash"),
        &sha256(source)
    );
    pic.start_canister(id, None)
        .await
        .expect("restart old code");
    assert_eq!(words::<12>(pic, id, "graph").await, graph);
    assert_eq!(words::<4>(pic, id, "transient").await, transient);
    assert_eq!(retained_bytes(pic, id).await, bytes);
}

#[tokio::test(flavor = "current_thread")]
async fn managed_graph_survives_installed_upgrades_and_rolls_back_trapped_updates() {
    let raw_a = input_graph("a");
    let raw_b = input_graph("b");
    let a = prepare_graph(&raw_a).expect("graph A");
    let b = prepare_graph(&raw_b).expect("graph B");
    admit_graph(&a.bytes, &raw_a).expect("final A");
    admit_graph(&b.bytes, &raw_b).expect("final B");
    assert_ne!(a.bytes, b.bytes);
    let evidence = root().join("target/evidence/managed-graph");
    fs::create_dir_all(&evidence).expect("evidence directory");
    for (name, artifact) in [("a", &a), ("b", &b)] {
        fs::write(evidence.join(format!("{name}.wasm")), &artifact.bytes).expect("artifact");
        fs::write(evidence.join(format!("{name}.txt")), &artifact.report).expect("report");
        println!("graph-{name}:\n{}", artifact.report);
    }
    let server = std::env::var("IC_TESTKIT_POCKET_IC_URL").expect(
        "run with the prepared server via make retention-setup and the selected test target",
    );
    let instance = PocketIcBuilder::new()
        .with_application_subnet()
        .with_max_request_time_ms(Some(30_000))
        .with_server_url(server.parse().expect("valid prepared server URL"))
        .build_async()
        .await;
    let pic = &instance;
    let id = pic.create_canister().await;
    pic.install_canister(id, a.bytes.clone(), vec![], None)
        .await;
    let initial = [2, 160, 3, 65536, 72, 2, 41, 7, 7, 1, 1, 1];
    assert_eq!(words::<12>(pic, id, "graph").await, initial);
    let bytes_a = retained_bytes(pic, id).await;
    update(pic, id, "dirty").await;
    for incompatible in [true, false] {
        let negative = negative_upgrade(&b.bytes, incompatible);
        assert_eq!(admit_graph(&negative, &raw_b), Err(Refusal::FinalMismatch));
        let name = if incompatible {
            "incompatible-post"
        } else {
            "mutating-post-trap"
        };
        fs::write(evidence.join(format!("{name}.wasm")), &negative).expect("negative artifact");
        println!("{name}={}", sha256_hex(&negative));
        refused_upgrade(pic, id, &a.bytes, &negative).await;
    }
    upgrade(pic, id, &b.bytes).await;
    assert_eq!(words::<4>(pic, id, "transient").await, [202, 0, 2, 2560]);
    assert_eq!(words::<12>(pic, id, "graph").await, initial);
    assert_eq!(retained_bytes(pic, id).await, bytes_a);
    update(pic, id, "graph_change").await;
    let changed = [3, 232, 4, 65536, 72, 2, 42, 99, 99, 1, 1, 1];
    assert_eq!(words::<12>(pic, id, "graph").await, changed);
    let bytes_b = retained_bytes(pic, id).await;
    let trap = pic
        .update_call(id, id, "graph_trap", vec![])
        .await
        .expect_err("growth and writes trap");
    assert_eq!(trap.error_code, ErrorCode::CanisterTrapped);
    assert_eq!(words::<12>(pic, id, "graph").await, changed);
    assert_eq!(retained_bytes(pic, id).await, bytes_b);
    upgrade(pic, id, &a.bytes).await;
    assert_eq!(words::<4>(pic, id, "transient").await, [101, 0, 2, 2560]);
    assert_eq!(retained_bytes(pic, id).await, bytes_b);
    // Fill to within one object of the page boundary, then upgrade and continue.
    update(pic, id, "graph_fill").await;
    assert_eq!(
        &words::<12>(pic, id, "graph").await[..4],
        &[909, 65464, 910, 65536]
    );
    upgrade(pic, id, &b.bytes).await;
    update(pic, id, "graph_change").await;
    let full = retained_bytes(pic, id).await;
    let refused = pic
        .update_call(id, id, "graph_change", vec![])
        .await
        .expect_err("arena full");
    assert_eq!(refused.error_code, ErrorCode::CanisterTrapped);
    assert_eq!(retained_bytes(pic, id).await, full);
    update(pic, id, "graph_grow").await;
    update(pic, id, "graph_change").await;
    assert_eq!(
        &words::<12>(pic, id, "graph").await[..4],
        &[911, 65608, 912, 131072]
    );
    let grown = retained_bytes(pic, id).await;
    upgrade(pic, id, &a.bytes).await;
    assert_eq!(retained_bytes(pic, id).await, grown);
    instance.drop().await;
}

use super::super::installed::{export, logical, query, recover, refused, words};
use super::*;
use crate::installed_tests::{stopped_hash, update};
use ic_testkit::{pic::PocketIcStartupConfig, pocket_ic::PocketIcBuilder};
use std::time::Duration;
use wasm_encoder::{BlockType, ExportKind, ExportSection, Function, FunctionSection, MemArg};

// Deliberate source corruption, outside both Rust runtimes. After poisoning,
// only WAT observation/pre-upgrade may run until a successful target reset.
// These two diagnostic exports are added only to installed-test artifacts.
fn diagnostics(bytes: &[u8]) -> Vec<u8> {
    let mut imports = Vec::new();
    let mut types = 0;
    let mut count = 0;
    for payload in Parser::new(0).parse_all(bytes) {
        match payload.unwrap() {
            Payload::ImportSection(r) => {
                for import in r.into_imports() {
                    imports.push(import.unwrap().name);
                }
            }
            Payload::TypeSection(r) => types = r.count(),
            Payload::FunctionSection(r) => count = r.count(),
            _ => {}
        }
    }
    let reply = imports.iter().position(|n| *n == "msg_reply").unwrap() as u32;
    let append = imports
        .iter()
        .position(|n| *n == "msg_reply_data_append")
        .unwrap() as u32;
    let mut poison = Function::new([]);
    for op in [
        I::I32Const(1),
        I::MemoryGrow(0),
        I::I32Const(-1),
        I::I32Eq,
        I::If(BlockType::Empty),
        I::Unreachable,
        I::End,
        I::I32Const(196608),
        I::I32Const(0xa5),
        I::MemorySize(0),
        I::I32Const(65536),
        I::I32Mul,
        I::I32Const(196608),
        I::I32Sub,
        I::MemoryFill(0),
        I::I32Const(0),
        I::I32Const(0xa5),
        I::I32Const(65536),
        I::MemoryFill(0),
        I::Call(reply),
        I::End,
    ] {
        poison.instruction(&op);
    }
    let mem = MemArg {
        offset: 0,
        align: 3,
        target: wasm_encoder::MemArgTarget::Memory(0),
        ordering: None,
    };
    let mut probe = Function::new([]);
    for op in [
        I::I32Const(0),
        I::MemorySize(0),
        I::I64ExtendI32U,
        I::I64Store(mem),
        I::I32Const(8),
        I::MemorySize(0),
        I::I32Const(65536),
        I::I32Mul,
        I::I32Const(1),
        I::I32Sub,
        I::I64Load8U(MemArg { align: 0, ..mem }),
        I::I64Store(mem),
        I::I32Const(16),
        I::I32Const(196608),
        I::I64Load(mem),
        I::I64Store(mem),
        I::I32Const(0),
        I::I32Const(24),
        I::Call(append),
        I::Call(reply),
        I::End,
    ] {
        probe.instruction(&op);
    }
    let result = crate::rewrite(bytes, |module, id, data| {
        if !matches!(id, 3 | 7 | 10) {
            module.section(&RawSection { id, data });
            return;
        }
        let mut functions = FunctionSection::new();
        let mut exports = ExportSection::new();
        let mut code = CodeSection::new();
        for payload in Parser::new(0).parse_all(bytes) {
            match payload.unwrap() {
                Payload::FunctionSection(r) if id == 3 => {
                    RoundtripReencoder
                        .parse_function_section(&mut functions, r)
                        .unwrap();
                }
                Payload::ExportSection(r) if id == 7 => {
                    RoundtripReencoder
                        .parse_export_section(&mut exports, r)
                        .unwrap();
                }
                Payload::CodeSectionEntry(body) if id == 10 => {
                    RoundtripReencoder
                        .parse_function_body(&mut code, body)
                        .unwrap();
                }
                _ => {}
            }
        }
        match id {
            3 => {
                functions.function(types - 1).function(types - 1);
                module.section(&functions);
            }
            7 => {
                let base = imports.len() as u32 + count;
                exports
                    .export("canister_update poison_transient", ExportKind::Func, base)
                    .export("canister_query transient_probe", ExportKind::Func, base + 1);
                module.section(&exports);
            }
            10 => {
                code.function(&poison).function(&probe);
                module.section(&code);
            }
            _ => unreachable!(),
        }
    });
    wasm::validate(&result, LIMIT).unwrap();
    result
}

// Fault after both native admission and transient clearing have completed.
// Never enters checkpoint Rust with deliberately damaged allocator state.
fn trap_after_reset(bytes: &[u8]) -> Vec<u8> {
    let mut imported = 0;
    let mut post = 0;
    for payload in Parser::new(0).parse_all(bytes) {
        match payload.unwrap() {
            Payload::ImportSection(r) => imported = r.into_imports().count(),
            Payload::ExportSection(r) => {
                for export in r {
                    let export = export.unwrap();
                    if export.name == "canister_post_upgrade" {
                        post = export.index as usize;
                    }
                }
            }
            _ => {}
        }
    }
    let result = crate::rewrite(bytes, |module, id, data| {
        if id != 10 {
            module.section(&RawSection { id, data });
            return;
        }
        let mut code = CodeSection::new();
        for payload in Parser::new(0).parse_all(bytes) {
            if let Payload::CodeSectionEntry(body) = payload.unwrap() {
                let mut copy = RoundtripReencoder;
                let mut f = copy.new_function_with_parsed_locals(&body).unwrap();
                for op in body.get_operators_reader().unwrap() {
                    let op = op.unwrap();
                    if code.len() as usize + imported == post && matches!(op, Op::End) {
                        f.instruction(&I::Unreachable);
                    }
                    f.instruction(&copy.instruction(op).unwrap());
                }
                code.function(&f);
            }
        }
        module.section(&code);
    });
    wasm::validate(&result, LIMIT).unwrap();
    result
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "native-checkpoint-test supplies fresh inputs and the prepared server"]
async fn compatible_reopening_discards_transients_then_can_export_and_recover() {
    let (directory, raw, bridge) = inputs();
    let candidate = |relocated, root| {
        let clean = prepare_reopen(&raw, &bridge, relocated, root);
        assert_eq!(clean, prepare_reopen(&raw, &bridge, relocated, root));
        let bytes = diagnostics(&clean);
        let digest = Sha256Digest::compute(&bytes);
        fs::write(
            directory.join(format!("reopen-diagnostic-{digest}.wasm")),
            &bytes,
        )
        .unwrap();
        println!(
            "reopen diagnostic relocated={relocated} root={root:?} bytes={} sha256={digest}",
            bytes.len()
        );
        bytes
    };
    let owner = PocketIcStartupConfig::from_env(Duration::from_secs(30))
        .unwrap()
        .build_instance(
            PocketIcBuilder::new()
                .with_application_subnet()
                .with_max_request_time_ms(Some(30_000)),
        )
        .await
        .unwrap();
    let pic = owner.instance();
    let reopened = candidate(false, (0, 0));
    let failed = trap_after_reset(&reopened);
    let digest = Sha256Digest::compute(&failed);
    fs::write(
        directory.join(format!("reopen-post-trap-{digest}.wasm")),
        &failed,
    )
    .unwrap();
    let relocated = candidate(true, (0, 0));
    let recovery = prepare(&raw, &bridge, true, (0, 0), true);
    for root in [(3, 4), (0, 0), (512, 512)] {
        let id = pic.create_canister().await;
        pic.install_canister(id, candidate(false, root), vec![], None)
            .await;
        let pristine = words(&query(pic, id, "transient_probe").await)[2];
        for pages in [33, 34] {
            let before = query(pic, id, "observe").await;
            let stable = pic.get_stable_memory(id).await;
            update(pic, id, "poison_transient").await;
            let poisoned = words(&query(pic, id, "transient_probe").await);
            assert_eq!(poisoned, [pages, 0xa5, 0xa5a5a5a5a5a5a5a5]);
            // Native refusal must not invoke the poisoned checkpoint runtime.
            refused(pic, id, &relocated, 0, 0, true).await;
            assert_eq!(words(&query(pic, id, "transient_probe").await), poisoned);
            refused(pic, id, &failed, 0, 0, true).await;
            assert_eq!(words(&query(pic, id, "transient_probe").await), poisoned);
            pic.upgrade_eop_canister(id, reopened.clone(), vec![], None)
                .await
                .unwrap();
            assert_eq!(
                stopped_hash(pic, id).await,
                Sha256Digest::compute(&reopened).as_bytes()
            );
            pic.start_canister(id, None).await.unwrap();
            assert_eq!(query(pic, id, "observe").await, before);
            assert_eq!(pic.get_stable_memory(id).await, stable);
            assert_eq!(
                words(&query(pic, id, "transient_probe").await),
                [pages, 0, pristine]
            );
            // First checkpoint Rust call after rebuilding the transient domain.
            assert!(query(pic, id, "checkpoint_bytes").await.is_empty());
        }
        update(pic, id, "vector_mutate").await;
        export(pic, id, 11).await;
        refused(pic, id, &reopened, 11, 0, true).await; // Frozen is not ready.
        recover(pic, id, &recovery, 11, 147456).await;
        // A fresh-heap recovery destination can itself reopen compatibly.
        let before = query(pic, id, "observe").await;
        let checkpoint = query(pic, id, "checkpoint_bytes").await;
        pic.upgrade_eop_canister(id, relocated.clone(), vec![], None)
            .await
            .unwrap();
        assert_eq!(query(pic, id, "observe").await, before);
        assert_eq!(query(pic, id, "checkpoint_bytes").await, checkpoint);
        update(pic, id, "vector_reuse").await;
        assert_eq!(logical(&query(pic, id, "observe").await), [1]);
        export(pic, id, 12).await;
    }
    owner.close().await;
}

//! Installed memory extent, growth rollback and retention-mode boundaries.

use super::*;
use ic_storage_retention_probe::artifact::{sha256, sha256_hex, validate_wasm};
use wasm_encoder::{CodeSection, Function, Instruction as I};

async fn grow(pic: &PocketIc, id: CanisterId, method: &str) -> u64 {
    let bytes = pic
        .update_call(id, id, method, vec![])
        .await
        .expect("growth reply");
    u64::from_le_bytes(bytes.try_into().expect("one word"))
}

async fn refused(
    pic: &PocketIc,
    id: CanisterId,
    destination: &[u8],
    keep: bool,
    source: &[u8],
    expected: ErrorCode,
) {
    let transient = words::<4>(pic, id, "transient").await;
    let retained = words::<2>(pic, id, "retained").await;
    let growth = words::<7>(pic, id, "growth").await;
    pic.stop_canister(id, None).await.expect("stop");
    let result = if keep {
        pic.upgrade_eop_canister(id, destination.to_vec(), vec![], None)
            .await
    } else {
        pic.upgrade_canister(id, destination.to_vec(), vec![], None)
            .await
    };
    let rejection = result.expect_err("unsafe upgrade must be refused");
    println!("keep={keep}: {:?}", rejection.error_code);
    assert_eq!(rejection.error_code, expected);
    assert_eq!(
        pic.canister_status(id, None)
            .await
            .expect("status")
            .module_hash
            .expect("hash"),
        &sha256(source)
    );
    pic.start_canister(id, None)
        .await
        .expect("restart old code");
    assert_eq!(words::<4>(pic, id, "transient").await, transient);
    assert_eq!(words::<2>(pic, id, "retained").await, retained);
    assert_eq!(words::<7>(pic, id, "growth").await, growth);
}

fn upgrade_growth_trap(bytes: &[u8]) -> Vec<u8> {
    let mut post = None;
    let mut bodies = Vec::new();
    for payload in Parser::new(0).parse_all(bytes) {
        match payload.expect("admitted module") {
            Payload::ExportSection(exports) => {
                for export in exports {
                    let export = export.expect("export");
                    if export.name == "canister_post_upgrade" {
                        post = Some(export.index);
                    }
                }
            }
            Payload::CodeSectionEntry(body) => bodies.push(body.as_bytes()),
            _ => {}
        }
    }
    // Exactly two function imports are required by the source admission policy.
    let index = post.expect("post-upgrade export") as usize - 2;
    let mut hook = Function::new([]);
    for op in [
        I::I32Const(1),
        I::MemoryGrow(0),
        I::I32Const(-1),
        I::I32Eq,
        I::If(wasm_encoder::BlockType::Empty),
        // Refusal would make the upgrade succeed and fail the test. Only a
        // successful growth reaches the deliberate trap below.
        I::Return,
        I::End,
        I::I32Const(BOUNDARY + 16),
        I::I32Const(0),
        I::I32Const(8),
        I::MemoryFill(0),
        I::Unreachable,
        I::End,
    ] {
        hook.instruction(&op);
    }
    let mut code = CodeSection::new();
    for (i, body) in bodies.into_iter().enumerate() {
        if i == index {
            code.function(&hook);
        } else {
            code.raw(body);
        }
    }
    let negative = rewrite(bytes, |m, id, data| {
        if id == 10 {
            m.section(&code);
        } else {
            m.section(&RawSection { id, data });
        }
    });
    validate_wasm(&negative, 1024 * 1024).expect("valid negative hook");
    negative
}

#[tokio::test(flavor = "current_thread")]
async fn grown_memory_survives_upgrades_and_refusals_preserve_state() {
    let raw_a = input("a");
    let raw_b = input("b");
    let a = prepare(&raw_a).expect("A");
    let b = prepare(&raw_b).expect("B");
    admit(&a.bytes, &raw_a).expect("final A");
    admit(&b.bytes, &raw_b).expect("final B");
    let owner = PocketIcStartupConfig::from_env(Duration::from_secs(30))
        .expect("server setup")
        .build_instance(
            PocketIcBuilder::new()
                .with_application_subnet()
                .with_max_request_time_ms(Some(30_000)),
        )
        .await
        .expect("instance");
    let pic = owner.instance();
    let id = pic.create_canister().await;
    pic.install_canister(id, a.bytes.clone(), vec![], None)
        .await;
    update(pic, id, "retain").await;
    assert_eq!(
        words::<7>(pic, id, "growth").await,
        [129, 129, 0, 0, 0, 0, 0]
    );
    assert_eq!(grow(pic, id, "grow").await, 129);
    let s = SENTINEL as u64;
    assert_eq!(
        words::<7>(pic, id, "growth").await,
        [130, 130, s, s, 0, 0, s]
    );

    // A message trap must roll back memory extent, writes and cursor together.
    let rejection = pic
        .update_call(id, id, "grow_then_trap", vec![])
        .await
        .expect_err("trap after growth");
    assert_eq!(rejection.error_code, ErrorCode::CanisterTrapped);
    assert_eq!(
        words::<7>(pic, id, "growth").await,
        [130, 130, s, s, 0, 0, s]
    );

    update(pic, id, "dirty").await;
    let trapping = upgrade_growth_trap(&b.bytes);
    let evidence = root().join("target/evidence/rust-startup");
    fs::create_dir_all(&evidence).expect("evidence directory");
    fs::write(evidence.join("upgrade-growth-trap.wasm"), &trapping).expect("negative artifact");
    println!("upgrade-growth-trap={}", sha256_hex(&trapping));
    assert_eq!(admit(&trapping, &raw_b), Err(Refusal::FinalMismatch));
    refused(
        pic,
        id,
        &trapping,
        true,
        &a.bytes,
        ErrorCode::CanisterTrapped,
    )
    .await;
    refused(
        pic,
        id,
        &b.bytes,
        false,
        &a.bytes,
        ErrorCode::CanisterContractViolation,
    )
    .await;
    let stripped = rewrite(&b.bytes, |module, id, data| {
        if id != 0 {
            module.section(&RawSection { id, data });
        }
    });
    fs::write(evidence.join("stripped-marker.wasm"), &stripped).expect("negative artifact");
    println!("stripped-marker={}", sha256_hex(&stripped));
    assert_eq!(admit(&stripped, &raw_b), Err(Refusal::FinalMismatch));
    // Deliberately submit the unadmitted negative fixture to verify IC's own
    // marker requirement, separately from the host's earlier artifact refusal.
    refused(
        pic,
        id,
        &stripped,
        true,
        &a.bytes,
        ErrorCode::CanisterContractViolation,
    )
    .await;

    upgrade(pic, id, &b.bytes).await;
    assert_eq!(
        words::<7>(pic, id, "growth").await,
        [130, 130, s, s, 0, 0, s]
    );
    assert_eq!(grow(pic, id, "grow").await, 130);
    assert_eq!(
        words::<7>(pic, id, "growth").await,
        [131, 131, s, s, s, s, s]
    );
    assert_eq!(grow(pic, id, "grow_limit").await, 131);
    let full = [256, 256, s, s, s, s, s];
    assert_eq!(words::<7>(pic, id, "growth").await, full);
    assert_eq!(grow(pic, id, "grow").await, u64::MAX);
    assert_eq!(words::<7>(pic, id, "growth").await, full);
    // Zero-page growth at the maximum must not probe beyond physical memory.
    assert_eq!(grow(pic, id, "grow_limit").await, 256);
    assert_eq!(words::<7>(pic, id, "growth").await, full);

    upgrade(pic, id, &a.bytes).await;
    assert_eq!(words::<7>(pic, id, "growth").await, full);
    assert_eq!(words::<2>(pic, id, "retained").await, [MAGIC as u64, s]);
    update(pic, id, "corrupt_extent").await;
    refused(
        pic,
        id,
        &b.bytes,
        true,
        &a.bytes,
        ErrorCode::CanisterTrapped,
    )
    .await;
    assert_eq!(
        words::<7>(pic, id, "growth").await,
        [256, 257, s, s, s, s, s]
    );
    owner.close().await;
}

//! Independent wire observations and installed transaction/authority evidence.
use super::{
    installed_tests::{checkpoint, query, records, word},
    *,
};
use crate::{
    installed_tests::{stopped_hash, update},
    migration::tests::installed::{replace_heap, replace_heap_with_options},
};
use ic_storage_retention_probe::artifact::{sha256, sha256_hex};
use ic_testkit::pocket_ic::{CanisterId, ErrorCode, PocketIcBuilder, nonblocking::PocketIc};
use std::fs;
mod staged;
const A: u64 = 131072;
const B: u64 = 163840;
fn args(epoch: u64, domain: u64, fault: u64) -> Vec<u8> {
    [epoch, domain, fault]
        .into_iter()
        .flat_map(u64::to_le_bytes)
        .collect()
}
fn envelope(bytes: &[u8], phase: u16, epoch: u64, source: u64, target: u64) -> &[u8] {
    assert!((104..=2360).contains(&bytes.len()));
    assert_eq!(&bytes[..8], b"ICJMP001");
    assert_eq!(&bytes[8..10], &1u16.to_le_bytes());
    assert_eq!(&bytes[10..12], &phase.to_le_bytes());
    assert_eq!(&bytes[12..16], &64u32.to_le_bytes());
    assert_eq!(
        [word(bytes, 16), word(bytes, 24), word(bytes, 32)],
        [epoch, source, target]
    );
    let len = u32::from_le_bytes(bytes[40..44].try_into().unwrap()) as usize;
    assert_eq!(bytes.len(), len + 104);
    assert_eq!(&bytes[44..64], &[0; 20]);
    assert_eq!(&bytes[64 + len..72 + len], b"JOBDONE1");
    assert_eq!(&bytes[72 + len..], &sha256(&bytes[..72 + len]));
    &bytes[64..64 + len]
}
fn trapped(error: &ic_testkit::pocket_ic::RejectResponse) {
    assert!(
        matches!(
            error.error_code,
            ErrorCode::CanisterTrapped | ErrorCode::CanisterCalledTrap
        ),
        "{error:?}"
    );
}
async fn refused_update(pic: &PocketIc, id: CanisterId, method: &str, arguments: Vec<u8>) {
    let before = query(pic, id, "observe").await;
    let stable = pic.get_stable_memory(id).await;
    trapped(
        &pic.update_call(id, id, method, arguments)
            .await
            .unwrap_err(),
    );
    assert_eq!(query(pic, id, "observe").await, before);
    assert_eq!(pic.get_stable_memory(id).await, stable);
}
async fn refused_upgrade(
    pic: &PocketIc,
    id: CanisterId,
    wasm: &[u8],
    arguments: Vec<u8>,
    keep: bool,
) {
    let before = query(pic, id, "observe").await;
    let stable = pic.get_stable_memory(id).await;
    let hash = stopped_hash(pic, id).await;
    let result = if keep {
        pic.upgrade_eop_canister(id, wasm.to_vec(), arguments, None)
            .await
    } else {
        replace_heap(pic, id, wasm, arguments).await
    };
    trapped(&result.unwrap_err());
    assert_eq!(stopped_hash(pic, id).await, hash);
    pic.start_canister(id, None).await.unwrap();
    assert_eq!(query(pic, id, "observe").await, before);
    assert_eq!(pic.get_stable_memory(id).await, stable);
}
async fn publish(pic: &PocketIc, id: CanisterId, epoch: u64, source: u64, target: u64) -> Vec<u8> {
    let native = query(pic, id, "observe").await;
    pic.update_call(id, id, "export_checkpoint", args(epoch, target, 0))
        .await
        .unwrap();
    let bytes = query(pic, id, "checkpoint_bytes").await;
    assert_eq!(
        checkpoint(envelope(&bytes, 1, epoch, source, target), epoch),
        [records(&native, 0), records(&native, 1)]
    );
    bytes
}
async fn install(pic: &PocketIc, source: &[u8]) -> CanisterId {
    let id = pic.create_canister().await;
    pic.install_canister(id, source.to_vec(), vec![], None)
        .await;
    id
}
async fn authorize(pic: &PocketIc, id: CanisterId) {
    pic.set_controllers(id, None, vec![CanisterId::anonymous(), id])
        .await
        .unwrap();
}
#[tokio::test(flavor = "current_thread")]
#[ignore = "map job collector and prepared PocketIC server"]
async fn durable_map_jobs_authorize_cancel_resume_and_consume_atomically() {
    let (directory, raw) = inputs();
    let bridge = fs::read(directory.join("checkpoint-map-job.wasm")).unwrap();
    let source = prepare_mode(&raw, &bridge, false, Post::Reopen, true);
    let target = prepare_mode(&raw, &bridge, true, Post::Recover, true);
    let reopen = prepare_mode(&raw, &bridge, true, Post::Reopen, true);
    let frozen = prepare_mode(&raw, &bridge, false, Post::Frozen, true);
    let reverse = prepare_mode(&raw, &bridge, false, Post::Recover, true);
    assert_ne!(target, reopen);
    for (name, bytes) in [
        ("job-source", &source),
        ("job-recovery", &target),
        ("job-reopen", &reopen),
        ("job-frozen", &frozen),
        ("job-reverse", &reverse),
    ] {
        fs::write(directory.join(format!("{name}.wasm")), bytes).unwrap();
        println!(
            "map {name} sha256={} bytes={}",
            sha256_hex(bytes),
            bytes.len()
        );
    }
    let instance = PocketIcBuilder::new()
        .with_max_request_time_ms(Some(30_000))
        .with_application_subnet()
        .with_server_url(
            std::env::var("IC_TESTKIT_POCKET_IC_URL")
                .unwrap()
                .parse()
                .unwrap(),
        )
        .build_async()
        .await;
    let pic = &instance;
    let id = install(pic, &source).await;
    envelope(&query(pic, id, "checkpoint_bytes").await, 0, 0, A, A);
    update(pic, id, "fill_checkpoint").await;
    update(pic, id, "full_width").await;
    refused_update(pic, id, "export_checkpoint", args(1, B, 0)).await;
    authorize(pic, id).await;
    for arguments in [
        args(0, B, 0),
        args(2, B, 0),
        args(1, A, 0),
        args(1, 147456, 0),
        args(1, B, 1),
        args(1, B, 2),
        vec![],
    ] {
        refused_update(pic, id, "export_checkpoint", arguments).await;
    }
    update(pic, id, "grow_a").await;
    refused_update(pic, id, "export_checkpoint", args(1, B, 0)).await;
    update(pic, id, "remove_growth_a").await;
    let saved = publish(pic, id, 1, A, B).await;
    assert_eq!(saved.len(), 2360);
    assert_eq!(publish(pic, id, 1, A, B).await, saved);
    refused_update(pic, id, "insert_a", vec![]).await;
    refused_update(pic, id, "export_checkpoint", args(2, B, 0)).await;
    refused_upgrade(pic, id, &source, vec![], true).await;
    for arguments in [args(2, B, 0), args(1, A, 0), args(1, B, 1)] {
        refused_upgrade(pic, id, &frozen, arguments, true).await;
    }
    pic.upgrade_eop_canister(id, frozen.clone(), args(1, B, 0), None)
        .await
        .unwrap();
    assert_eq!(query(pic, id, "checkpoint_bytes").await, saved);
    // Cancellation authorization and both publication rollback points.
    pic.set_controllers(id, None, vec![CanisterId::anonymous()])
        .await
        .unwrap();
    refused_update(pic, id, "cancel_checkpoint", args(1, B, 0)).await;
    refused_update(pic, id, "damage_checkpoint", args(0, 0, 0)).await;
    authorize(pic, id).await;
    for arguments in [args(2, B, 0), args(1, A, 0), args(1, B, 1), args(1, B, 2)] {
        refused_update(pic, id, "cancel_checkpoint", arguments).await;
    }
    pic.update_call(id, id, "cancel_checkpoint", args(1, B, 0))
        .await
        .unwrap();
    envelope(&query(pic, id, "checkpoint_bytes").await, 3, 1, A, B);
    update(pic, id, "delete_full_a").await;
    update(pic, id, "grow_a").await;
    let newer = query(pic, id, "observe").await;
    pic.update_call(id, id, "cancel_checkpoint", args(1, B, 0))
        .await
        .unwrap();
    assert_eq!(query(pic, id, "observe").await, newer);
    refused_upgrade(pic, id, &target, args(1, A, 0), false).await;
    refused_update(pic, id, "export_checkpoint", args(1, B, 0)).await;
    let saved = publish(pic, id, 2, A, B).await;
    refused_upgrade(pic, id, &target, args(2, A, 0), true).await;
    for arguments in [
        args(1, A, 0),
        args(2, B, 0),
        args(2, A, 1),
        args(2, A, 2),
        args(2, A, 3),
        args(2, A, 4),
    ] {
        refused_upgrade(pic, id, &target, arguments, false).await;
        assert_eq!(query(pic, id, "checkpoint_bytes").await, saved);
    }
    replace_heap(pic, id, &target, args(2, A, 0)).await.unwrap();
    let recovered = query(pic, id, "observe").await;
    assert_eq!(word(&recovered, 40), B);
    for index in 0..2 {
        assert_eq!(records(&recovered, index), records(&newer, index));
    }
    let consumed = query(pic, id, "checkpoint_bytes").await;
    assert_eq!(
        envelope(&consumed, 2, 2, A, B),
        envelope(&saved, 1, 2, A, B)
    );
    assert!(
        query(pic, id, "discarded_payload")
            .await
            .iter()
            .all(|byte| *byte == 0)
    );
    pic.upgrade_eop_canister(id, reopen, vec![], None)
        .await
        .unwrap();
    update(pic, id, "delete_full_b").await;
    let written = query(pic, id, "observe").await;
    refused_upgrade(pic, id, &target, args(2, A, 0), false).await;
    refused_update(pic, id, "cancel_checkpoint", args(2, A, 0)).await;
    refused_update(pic, id, "export_checkpoint", args(2, A, 0)).await;
    publish(pic, id, 3, B, A).await;
    replace_heap(pic, id, &reverse, args(3, B, 0))
        .await
        .unwrap();
    for index in 0..2 {
        assert_eq!(
            records(&query(pic, id, "observe").await, index),
            records(&written, index)
        );
    }
    envelope(&query(pic, id, "checkpoint_bytes").await, 2, 3, B, A);
    // Independent locator survives native-header loss; source-hook bypass is explicit.
    let lost = install(pic, &source).await;
    authorize(pic, lost).await;
    update(pic, lost, "full_width").await;
    let before = query(pic, lost, "observe").await;
    publish(pic, lost, 1, A, B).await;
    update(pic, lost, "damage_native").await;
    refused_upgrade(pic, lost, &target, args(1, A, 0), false).await;
    replace_heap_with_options(pic, lost, &target, args(1, A, 0), true, None)
        .await
        .unwrap();
    for index in 0..2 {
        assert_eq!(
            records(&query(pic, lost, "observe").await, index),
            records(&before, index)
        );
    }
    // A correctly checksummed but changed payload cannot resume a frozen source.
    for mode in 0..=2 {
        let damaged = install(pic, &source).await;
        authorize(pic, damaged).await;
        update(pic, damaged, "full_width").await;
        publish(pic, damaged, 1, A, B).await;
        pic.update_call(
            damaged,
            damaged,
            "damage_checkpoint",
            if mode == 1 {
                args(1, 1, B)
            } else {
                args(mode, 0, 0)
            },
        )
        .await
        .unwrap();
        refused_upgrade(pic, damaged, &frozen, args(1, B, 0), true).await;
        if mode != 1 {
            refused_upgrade(pic, damaged, &target, args(1, A, 0), false).await;
        }
    }
    // Empty is a real sealed payload, not a missing/Ready job.
    let empty = install(pic, &source).await;
    authorize(pic, empty).await;
    refused_upgrade(pic, empty, &target, args(1, A, 0), false).await;
    publish(pic, empty, 1, A, B).await;
    replace_heap(pic, empty, &target, args(1, A, 0))
        .await
        .unwrap();
    assert!(records(&query(pic, empty, "observe").await, 0).is_empty());
    instance.drop().await;
    fs::write(directory.join("JOB_RECOVERED"), "durable two-map jobs: authority, epochs, cancellation, frozen resumption, consumption and rollback passed\n").unwrap();
}

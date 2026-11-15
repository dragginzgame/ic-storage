//! Installed ordinary-upgrade checkpoint continuity and rollback.
#![forbid(unsafe_code)]

mod artifacts;
mod v2;

use std::time::Duration;

use ic_host_artifacts::artifact::Sha256Digest;
use ic_testkit::{
    pic::PocketIcStartupConfig,
    pocket_ic::{
        CanisterId, ErrorCode, PocketIcBuilder, common::rest::BlobCompression,
        nonblocking::PocketIc,
    },
};

fn encode(words: &[u64]) -> Vec<u8> {
    words.iter().flat_map(|word| word.to_le_bytes()).collect()
}

async fn query(pic: &PocketIc, id: CanisterId, method: &str) -> Vec<u8> {
    pic.query_call(id, id, method, vec![]).await.expect("query")
}

async fn update(pic: &PocketIc, id: CanisterId, method: &str, words: &[u64]) {
    assert!(
        pic.update_call(id, id, method, encode(words))
            .await
            .expect("update")
            .is_empty()
    );
}

async fn state(pic: &PocketIc, id: CanisterId, expected: &[u64]) {
    assert_eq!(query(pic, id, "state").await, encode(expected));
}

async fn module_hash(pic: &PocketIc, id: CanisterId) -> Vec<u8> {
    pic.canister_status(id, None)
        .await
        .expect("status")
        .module_hash
        .expect("module hash")
}

async fn upgrade(pic: &PocketIc, id: CanisterId, target: &[u8], epoch: u64) {
    pic.stop_canister(id, None).await.expect("stop");
    // Ordinary upgrade intentionally discards the source Wasm heap.
    pic.upgrade_canister(id, target.to_vec(), encode(&[epoch, 0]), None)
        .await
        .expect("import");
    assert_eq!(
        module_hash(pic, id).await,
        Sha256Digest::compute(target).as_bytes()
    );
    pic.start_canister(id, None).await.expect("start");
}

async fn refused(pic: &PocketIc, id: CanisterId, target: &[u8], args: &[u64]) {
    let old_state = query(pic, id, "state").await;
    let old_checkpoint = query(pic, id, "checkpoint_bytes").await;
    let old_stable = pic.get_stable_memory(id).await;
    let old_hash = module_hash(pic, id).await;
    pic.stop_canister(id, None).await.expect("stop");
    let error = pic
        .upgrade_canister(id, target.to_vec(), encode(args), None)
        .await
        .expect_err("refused import");
    if args[1] == 1 {
        assert_eq!(error.error_code, ErrorCode::CanisterCalledTrap);
        assert!(
            error
                .reject_message
                .contains("injected failure after stable checkpoint mutation")
        );
    } else {
        assert_eq!(error.error_code, ErrorCode::CanisterTrapped);
    }
    assert_eq!(module_hash(pic, id).await, old_hash);
    assert_eq!(pic.get_stable_memory(id).await, old_stable);
    pic.start_canister(id, None).await.expect("restart source");
    assert_eq!(query(pic, id, "state").await, old_state);
    assert_eq!(query(pic, id, "checkpoint_bytes").await, old_checkpoint);
}

async fn install(pic: &PocketIc, wasm: &[u8]) -> CanisterId {
    let id = pic.create_canister().await;
    pic.install_canister(id, wasm.to_vec(), vec![], None).await;
    assert_eq!(
        module_hash(pic, id).await,
        Sha256Digest::compute(wasm).as_bytes()
    );
    id
}

#[tokio::test(flavor = "current_thread")]
async fn fresh_heap_import_and_failed_upgrade_rollback() {
    let a = artifacts::input("a");
    let b = artifacts::input("b");
    assert_ne!(a, b);
    let owner = PocketIcStartupConfig::from_env(Duration::from_secs(30))
        .expect("prepare server with make retention-setup")
        .build_instance(
            PocketIcBuilder::new()
                .with_application_subnet()
                .with_max_request_time_ms(Some(30_000)),
        )
        .await
        .expect("start admitted server");
    let pic = owner.instance();
    let id = install(pic, &a).await;
    state(pic, id, &[101, 101, 0, 0, 41, 7, 3, 3, 5, 8]).await;
    refused(pic, id, &b, &[7, 0]).await; // No complete export yet.
    update(pic, id, "append", &[13]).await;
    update(pic, id, "export_checkpoint", &[]).await;
    state(pic, id, &[101, 999, 0, 1, 42, 8, 4, 3, 5, 8, 13]).await;
    let checkpoint = query(pic, id, "checkpoint_bytes").await;
    // Independent wire oracle; do not use the canister's codec to decode evidence.
    assert_eq!(checkpoint.len(), 152);
    assert_eq!(&checkpoint[..8], b"ICNCP001");
    assert_eq!(&checkpoint[32..40], &8_u64.to_le_bytes());
    assert_eq!(&checkpoint[56..64], &42_u64.to_le_bytes());
    assert_eq!(&checkpoint[80..112], encode(&[3, 5, 8, 13]));
    assert_eq!(&checkpoint[112..120], b"SEALED01");
    assert_eq!(
        &checkpoint[120..],
        Sha256Digest::compute(&checkpoint[..120]).as_bytes()
    );
    let error = pic
        .update_call(id, id, "append", encode(&[99]))
        .await
        .expect_err("frozen source");
    assert_eq!(error.error_code, ErrorCode::CanisterTrapped);
    state(pic, id, &[101, 999, 0, 1, 42, 8, 4, 3, 5, 8, 13]).await;
    refused(pic, id, &b, &[9, 0]).await; // Wrong source epoch.
    refused(pic, id, &b, &[8, 1]).await; // Trap after a stable write.
    upgrade(pic, id, &b, 8).await;
    state(pic, id, &[202, 202, 1, 0, 42, 8, 4, 3, 5, 8, 13]).await;
    assert_eq!(query(pic, id, "checkpoint_bytes").await, checkpoint);
    update(pic, id, "append", &[21]).await;
    update(pic, id, "export_checkpoint", &[]).await;
    upgrade(pic, id, &a, 9).await;
    state(pic, id, &[101, 101, 1, 0, 43, 9, 5, 3, 5, 8, 13, 21]).await;

    for mode in 0..3 {
        let id = install(pic, &a).await;
        update(pic, id, "export_checkpoint", &[]).await;
        update(pic, id, "damage_checkpoint", &[mode]).await;
        refused(pic, id, &b, &[7, 0]).await;
    }

    // Infrastructure-only fault injection: corrupt provider framing without
    // teaching the adapter raw allocation IDs or bypassing governed handles.
    let id = install(pic, &a).await;
    update(pic, id, "export_checkpoint", &[]).await;
    let mut stable = pic.get_stable_memory(id).await;
    let offsets: Vec<_> = stable
        .windows(8)
        .enumerate()
        .filter_map(|(i, bytes)| (bytes == b"STORCELL").then_some(i))
        .collect();
    assert_eq!(offsets.len(), 1, "one provider cell in isolated fixture");
    stable[offsets[0]] ^= 1;
    pic.set_stable_memory(id, stable, BlobCompression::NoCompression)
        .await;
    refused(pic, id, &b, &[7, 0]).await;
    // The cached source root remains observable, but the target has no locator.
    // Its constructor must not turn empty stable memory into a fresh success.
    let id = install(pic, &a).await;
    pic.set_stable_memory(id, vec![], BlobCompression::NoCompression)
        .await;
    refused(pic, id, &b, &[7, 0]).await;
    owner.close().await;
}

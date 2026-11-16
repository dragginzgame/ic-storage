//! Installed Wasm retention contract; this does not qualify Rust startup.
#![forbid(unsafe_code)]

use ic_storage_retention_probe::artifact::{compile_wat, validate_wasm};
use std::time::Duration;

use ic_testkit::pic::PocketIcStartupConfig;
use ic_testkit::pocket_ic::{CanisterId, ErrorCode, PocketIcBuilder, nonblocking::PocketIc};

const MAGIC: u64 = 0x4f50_524f_4245_3031;
const A: &str = include_str!("fixtures/a.wat");
const B: &str = include_str!("fixtures/b.wat");
const INCOMPATIBLE: &str = include_str!("fixtures/incompatible.wat");
const TRAPPING: &str = include_str!("fixtures/trapping.wat");

mod provider_custody;

fn module(source: &str) -> Vec<u8> {
    let bytes = compile_wat(source, 8192, 8192).expect("compile the bounded protocol fixture");
    validate_wasm(&bytes, 8192).expect("validate fixture instructions");
    bytes
}

async fn read(pic: &PocketIc, id: CanisterId) -> [u64; 4] {
    let bytes = pic.query_call(id, id, "read", vec![]).await.expect("read");
    assert_eq!(bytes.len(), 32, "exact probe response length");
    std::array::from_fn(|i| u64::from_le_bytes(bytes[i * 8..(i + 1) * 8].try_into().expect("word")))
}

async fn advance(pic: &PocketIc, id: CanisterId) {
    let reply = pic
        .update_call(id, id, "advance", vec![])
        .await
        .expect("advance");
    assert!(reply.is_empty());
}

async fn stopped_hash(pic: &PocketIc, id: CanisterId) -> Vec<u8> {
    // These fixtures have no outgoing calls, timers or asynchronous work.
    // The management stop request completes only once the canister is stopped.
    pic.stop_canister(id, None).await.expect("stop");
    let status = pic.canister_status(id, None).await.expect("status");
    status.module_hash.expect("installed module hash")
}

async fn rejected_upgrade(pic: &PocketIc, id: CanisterId, source: &str) {
    let before = read(pic, id).await;
    let hash = stopped_hash(pic, id).await;
    // Testkit's EOP helper explicitly sends wasm_memory_persistence = keep.
    let rejection = pic
        .upgrade_eop_canister(id, module(source), vec![], None)
        .await
        .expect_err("incompatible/trapping upgrade must fail");
    assert_eq!(rejection.error_code, ErrorCode::CanisterTrapped);
    assert_eq!(stopped_hash(pic, id).await, hash, "old code survives");
    pic.start_canister(id, None)
        .await
        .expect("restart old module");
    assert_eq!(read(pic, id).await, before, "old observed state survives");
}

#[tokio::test(flavor = "current_thread")]
async fn retained_scalar_survives_upgrade_and_failed_upgrade() {
    let config = PocketIcStartupConfig::from_env(Duration::from_secs(30))
        .expect("prepare the server explicitly with make retention-setup");
    let owner = config
        .build_instance(
            PocketIcBuilder::new()
                .with_max_request_time_ms(Some(30_000))
                .with_application_subnet(),
        )
        .await
        .expect("start admitted PocketIC server");
    let pic = owner.instance();
    let id = pic.create_canister().await;
    pic.install_canister(id, module(A), vec![], None).await;
    assert_eq!(read(pic, id).await, [MAGIC, 1, 10, 11]);
    advance(pic, id).await;
    assert_eq!(read(pic, id).await, [MAGIC, 2, 99, 88]);

    rejected_upgrade(pic, id, INCOMPATIBLE).await;
    rejected_upgrade(pic, id, TRAPPING).await;
    advance(pic, id).await;
    assert_eq!(read(pic, id).await, [MAGIC, 3, 99, 88]);

    let a_hash = stopped_hash(pic, id).await;
    pic.upgrade_eop_canister(id, module(B), vec![], None)
        .await
        .expect("keep upgrade A to B");
    assert_ne!(stopped_hash(pic, id).await, a_hash, "new code installed");
    pic.start_canister(id, None).await.expect("start B");
    assert_eq!(read(pic, id).await, [MAGIC, 3, 20, 21]);
    advance(pic, id).await;
    assert_eq!(read(pic, id).await, [MAGIC, 4, 20, 21]);

    // Downgrade is another retaining upgrade, never a destructive reinstall.
    stopped_hash(pic, id).await;
    pic.upgrade_eop_canister(id, module(A), vec![], None)
        .await
        .expect("keep downgrade B to A");
    assert_eq!(stopped_hash(pic, id).await, a_hash);
    pic.start_canister(id, None).await.expect("start A");
    assert_eq!(read(pic, id).await, [MAGIC, 4, 10, 11]);

    // Precommitted corruption is preserved on rejection, not repaired/reset.
    pic.update_call(id, id, "corrupt_magic", vec![])
        .await
        .expect("commit corrupt marker");
    assert_eq!(read(pic, id).await, [0, 4, 10, 11]);
    rejected_upgrade(pic, id, B).await;
    owner.close().await;
}

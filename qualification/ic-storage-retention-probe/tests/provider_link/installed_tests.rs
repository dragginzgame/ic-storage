//! Execute only the admitted fixed-scalar foreign-allocation contract.

use std::{fs, time::Duration};

use ic_host_artifacts::artifact::Sha256Digest;
use ic_testkit::{
    pic::PocketIcStartupConfig,
    pocket_ic::{CanisterId, ErrorCode, PocketIcBuilder, nonblocking::PocketIc},
};

use super::{
    artifact::{self, Profile},
    input,
};

async fn observe(pic: &PocketIc, id: CanisterId) -> [u64; 5] {
    let bytes = pic.query_call(id, id, "observe", vec![]).await.unwrap();
    assert_eq!(bytes.len(), 40);
    std::array::from_fn(|i| u64::from_le_bytes(bytes[i * 8..i * 8 + 8].try_into().unwrap()))
}

async fn read(pic: &PocketIc, id: CanisterId) -> u64 {
    let bytes = pic.query_call(id, id, "native_read", vec![]).await.unwrap();
    u64::from_le_bytes(bytes.try_into().expect("one native u64"))
}

pub(super) async fn update(pic: &PocketIc, id: CanisterId, method: &str) {
    assert!(
        pic.update_call(id, id, method, vec![])
            .await
            .unwrap()
            .is_empty()
    );
}

pub(super) async fn stopped_hash(pic: &PocketIc, id: CanisterId) -> Vec<u8> {
    pic.stop_canister(id, None).await.unwrap();
    pic.canister_status(id, None)
        .await
        .unwrap()
        .module_hash
        .unwrap()
}

async fn rejected_upgrade(pic: &PocketIc, id: CanisterId, target: Vec<u8>) {
    let before = observe(pic, id).await;
    let hash = stopped_hash(pic, id).await;
    let error = pic
        .upgrade_eop_canister(id, target, vec![], None)
        .await
        .unwrap_err();
    assert_eq!(error.error_code, ErrorCode::CanisterTrapped);
    assert_eq!(stopped_hash(pic, id).await, hash);
    pic.start_canister(id, None).await.unwrap();
    assert_eq!(observe(pic, id).await, before);
}

async fn rejected_update(pic: &PocketIc, id: CanisterId, method: &str) {
    let before = observe(pic, id).await;
    let error = pic.update_call(id, id, method, vec![]).await.unwrap_err();
    assert_eq!(error.error_code, ErrorCode::CanisterTrapped);
    assert_eq!(observe(pic, id).await, before);
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "make native-provider-test supplies fresh artifacts and the prepared server"]
async fn scalar_access_settlement_upgrade_and_rollback() {
    let (directory, raw) = input();
    let candidate = |profile| {
        let bytes = artifact::prepare(&raw, profile).unwrap();
        artifact::admit_final(&bytes, &raw, profile).unwrap();
        let digest = Sha256Digest::compute(&bytes);
        fs::write(directory.join(format!("{digest}.wasm")), &bytes).unwrap();
        println!(
            "native scalar {profile:?}: sha256={digest} bytes={}",
            bytes.len()
        );
        bytes
    };
    let config = PocketIcStartupConfig::from_env(Duration::from_secs(30))
        .expect("prepare the server explicitly with make retention-setup");
    let owner = config
        .build_instance(
            PocketIcBuilder::new()
                .with_max_request_time_ms(Some(30_000))
                .with_application_subnet(),
        )
        .await
        .unwrap();
    let pic = owner.instance();
    let id = pic.create_canister().await;
    pic.install_canister(id, candidate(Profile::A), vec![], None)
        .await;
    assert_eq!(read(pic, id).await, 17);
    assert_eq!(observe(pic, id).await, [1, 1, 1, 17, 4096]);
    update(pic, id, "native_write").await;
    assert_eq!(read(pic, id).await, 42);
    assert_eq!(observe(pic, id).await, [1, 1, 1, 42, 4096]);
    // WAT advances the same payload, independently confirming Rust's address.
    update(pic, id, "advance").await;
    assert_eq!(read(pic, id).await, 43);
    rejected_update(pic, id, "native_trap").await;
    assert_eq!(read(pic, id).await, 43);
    rejected_upgrade(pic, id, candidate(Profile::Incompatible)).await;
    rejected_upgrade(pic, id, candidate(Profile::Trapping)).await;
    assert_eq!(read(pic, id).await, 43);

    let a_hash = stopped_hash(pic, id).await;
    pic.upgrade_eop_canister(id, candidate(Profile::B), vec![], None)
        .await
        .unwrap();
    assert_ne!(stopped_hash(pic, id).await, a_hash);
    pic.start_canister(id, None).await.unwrap();
    assert_eq!(read(pic, id).await, 43);
    assert_eq!(observe(pic, id).await, [2, 1, 1, 43, 4096]);
    update(pic, id, "native_write").await;
    assert_eq!(read(pic, id).await, 97);
    stopped_hash(pic, id).await;
    pic.upgrade_eop_canister(id, candidate(Profile::A), vec![], None)
        .await
        .unwrap();
    assert_eq!(stopped_hash(pic, id).await, a_hash);
    pic.start_canister(id, None).await.unwrap();
    assert_eq!(read(pic, id).await, 97);
    update(pic, id, "native_write").await;
    assert_eq!(read(pic, id).await, 42);

    update(pic, id, "native_abandon").await;
    assert_eq!(observe(pic, id).await, [1, 2, 0, 42, 4096]);
    let error = pic
        .query_call(id, id, "native_read", vec![])
        .await
        .unwrap_err();
    assert_eq!(error.error_code, ErrorCode::CanisterTrapped);
    rejected_update(pic, id, "native_write").await;
    rejected_upgrade(pic, id, candidate(Profile::B)).await;

    let corrupt = pic.create_canister().await;
    pic.install_canister(corrupt, candidate(Profile::A), vec![], None)
        .await;
    update(pic, corrupt, "corrupt_extent").await;
    rejected_update(pic, corrupt, "native_write").await;
    rejected_upgrade(pic, corrupt, candidate(Profile::B)).await;
    assert_eq!(observe(pic, corrupt).await, [1, 1, 1, 17, 0]);
    owner.close().await;
}

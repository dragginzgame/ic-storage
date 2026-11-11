//! Installed format-cut qualification with caller-supplied old/new single-collection consumers.
use ic_testkit::pocket_ic::{ErrorCode, PocketIcBuilder};
use std::{env, fs};

#[tokio::test(flavor = "current_thread")]
#[ignore = "requires prepared PocketIC server and old/new Cell-only size-consumer Wasm"]
async fn incompatible_cell_upgrade_traps_and_preserves_previous_application() {
    reject_upgrade("CELL", 0, 42, 0).await;
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "requires prepared PocketIC server and old/new Vector-only size-consumer Wasm"]
async fn incompatible_vector_upgrade_traps_and_preserves_previous_application() {
    reject_upgrade("VECTOR", 1, 2, u64::MAX).await;
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "requires prepared PocketIC server and old/new Map-only consumers across the backing format cut"]
async fn incompatible_backing_upgrade_traps_and_preserves_previous_application() {
    reject_upgrade("BACKING", u64::MAX, 42, u64::MAX).await;
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "requires prepared PocketIC server and old/new Map-only consumers across the ordered format cut"]
async fn incompatible_ordered_upgrade_traps_and_preserves_previous_application() {
    reject_upgrade("ORDERED", u64::MAX, 42, u64::MAX).await;
}

async fn reject_upgrade(prefix: &str, first_reply: u64, second_reply: u64, empty_value: u64) {
    let previous = fs::read(env::var(format!("{prefix}_PREVIOUS_WASM")).unwrap()).unwrap();
    let replacement = fs::read(env::var(format!("{prefix}_REPLACEMENT_WASM")).unwrap()).unwrap();
    let url = env::var("IC_TESTKIT_POCKET_IC_URL").unwrap();
    let pic = PocketIcBuilder::new()
        .with_max_request_time_ms(Some(300_000))
        .with_server_url(url.parse().unwrap())
        .with_application_subnet()
        .build_async()
        .await;
    let old = pic.create_canister().await;
    pic.add_cycles(old, 100_000_000_000_000).await;
    pic.install_canister(old, previous, vec![], None).await;
    let value = 42_u64;
    let reply = pic
        .update_call(
            old,
            old,
            "put",
            [0_u64.to_le_bytes(), value.to_le_bytes()].concat(),
        )
        .await
        .unwrap();
    assert_eq!(reply, first_reply.to_le_bytes());
    let failure = pic
        .upgrade_canister(old, replacement.clone(), vec![], None)
        .await
        .unwrap_err();
    assert!(matches!(
        failure.error_code,
        ErrorCode::CanisterTrapped | ErrorCode::CanisterCalledTrap
    ));
    let reply = pic
        .query_call(old, old, "get", 0_u64.to_le_bytes().to_vec())
        .await
        .unwrap();
    assert_eq!(reply, value.to_le_bytes());
    let reply = pic
        .update_call(
            old,
            old,
            "put",
            [0_u64.to_le_bytes(), 43_u64.to_le_bytes()].concat(),
        )
        .await
        .unwrap();
    assert_eq!(reply, second_reply.to_le_bytes());
    let fresh = pic.create_canister().await;
    pic.add_cycles(fresh, 100_000_000_000_000).await;
    pic.install_canister(fresh, replacement, vec![], None).await;
    let reply = pic
        .query_call(fresh, fresh, "get", 0_u64.to_le_bytes().to_vec())
        .await
        .unwrap();
    assert_eq!(reply, empty_value.to_le_bytes());
    pic.drop().await;
}

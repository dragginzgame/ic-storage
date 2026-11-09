//! Installed Storage adapters under the same caller-owned server as Memory.
use ic_storage_test_fixtures::storage as protocol;
use ic_testkit::pocket_ic::{CanisterId, ClientError, PocketIc};
use ic_wire::native;
fn frame(id: u64, value: &impl serde::Serialize) -> Vec<u8> {
    native::encode_request(id, value, protocol::REQUEST).unwrap()
}
use std::{env, fs, time::Duration};

async fn update(
    pic: &PocketIc,
    canister: CanisterId,
    method: &str,
    id: u64,
    mode: u8,
) -> Result<(), ClientError> {
    pic.update_call(canister, canister, method, frame(id, &mode))
        .await
        .map(|bytes| {
            native::decode_response::<()>(id, &bytes, protocol::RESPONSE).unwrap();
        })
}

pub(super) async fn qualify(pic: &PocketIc) {
    let wasm =
        fs::read(env::var("IC_STORAGE_QUALIFICATION_WASM").expect("Storage fixture path")).unwrap();
    let canister = super::install_fixture(pic, &wasm, frame(protocol::INIT_ID, &())).await;
    let snapshot = async || {
        let bytes = pic
            .query_call(
                canister,
                canister,
                "snapshot",
                frame(protocol::SNAPSHOT_ID, &()),
            )
            .await
            .unwrap();
        native::decode_response::<protocol::Snapshot>(
            protocol::SNAPSHOT_ID,
            &bytes,
            protocol::RESPONSE,
        )
        .unwrap()
    };
    let expected = |generation, values| protocol::Snapshot { generation, values };
    let initial = snapshot().await;
    assert!(
        initial.generation > 1,
        "initial collection growth commits extents"
    );
    assert_eq!(initial.values, [10, 20, 30]);
    let mut trailing = frame(protocol::SNAPSHOT_ID, &());
    trailing.push(0);
    for bytes in [
        Vec::new(),
        trailing,
        frame(protocol::MUTATE_ID, &()),
        vec![0; protocol::REQUEST.bytes + 1],
    ] {
        super::assert_trap(
            pic.query_call(canister, canister, "snapshot", bytes)
                .await
                .unwrap_err(),
        );
        assert_eq!(
            snapshot().await,
            initial,
            "refused Wire snapshot request must preserve state"
        );
    }
    println!("installed Storage Wire: four malformed/foreign/oversized snapshot frames refuse");
    for mode in 0..4_u8 {
        update(
            pic,
            canister,
            "cell_load_failure",
            protocol::CELL_FAILURE_ID,
            mode,
        )
        .await
        .unwrap();
        assert_eq!(
            snapshot().await,
            initial,
            "rejected cell load preserves live collections"
        );
    }
    super::assert_trap(
        update(
            pic,
            canister,
            "cell_load_failure",
            protocol::CELL_FAILURE_ID,
            4_u8,
        )
        .await
        .unwrap_err(),
    );
    assert_eq!(
        snapshot().await,
        initial,
        "cell decoder trap preserves live collections"
    );
    // The failed call's lease is rolled back with the Wasm heap and can reopen.
    update(
        pic,
        canister,
        "cell_load_failure",
        protocol::CELL_FAILURE_ID,
        3_u8,
    )
    .await
    .unwrap();
    super::assert_trap(
        update(pic, canister, "mutate", protocol::MUTATE_ID, 1_u8)
            .await
            .unwrap_err(),
    );
    assert_eq!(
        snapshot().await,
        initial,
        "trapped update rolls back all collection writes"
    );
    update(pic, canister, "mutate", protocol::MUTATE_ID, 0_u8)
        .await
        .unwrap();
    let committed = snapshot().await;
    assert_eq!(committed, expected(initial.generation, [100, 101, 102]));
    pic.advance_time(Duration::from_secs(300))
        .await
        .expect("advance PocketIC time");
    super::assert_trap(
        pic.upgrade_canister(
            canister,
            wasm.clone(),
            frame(protocol::UPGRADE_ID, &1_u8),
            None,
        )
        .await
        .unwrap_err(),
    );
    assert_eq!(
        snapshot().await,
        committed,
        "failed upgrade preserves collections and generation"
    );
    pic.advance_time(Duration::from_secs(300))
        .await
        .expect("advance PocketIC time");
    pic.upgrade_canister(canister, wasm, frame(protocol::UPGRADE_ID, &0_u8), None)
        .await
        .unwrap();
    assert_eq!(
        snapshot().await,
        committed,
        "reopening unchanged names does not commit a generation"
    );
    println!(
        "installed Storage: non-initializing cell admission preserves bytes/pages and leases; cells, map, set, vector and region survive upgrade across 1,031 named allocations; decoder/update/upgrade traps roll back"
    );
}

#[cfg(test)]
mod tests;

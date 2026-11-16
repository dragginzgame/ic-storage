//! Installed full-width vector descriptor admission; no Rust vector executes.

use ic_testkit::pocket_ic::{CanisterId, ErrorCode, PocketIcBuilder, nonblocking::PocketIc};

use super::{admit, record, update};
use crate::{module, stopped_hash};

mod access;
mod records;

use ic_storage_retention_probe::vector_provider::source;

fn candidate(
    version: u64,
    root: (u64, u64),
    extent: u64,
    trap: bool,
    fault: &str,
    source_guard: bool,
) -> Vec<u8> {
    let bytes = module(&source(version, root, extent, trap, fault, source_guard));
    admit(&bytes).expect("fixed-domain vector metadata fixture");
    record("vector metadata", &bytes);
    bytes
}

#[derive(Debug, PartialEq, Eq)]
struct Snapshot {
    version: u64,
    phase: u64,
    header: [u64; 7],
    payload: Vec<u8>,
}

impl Snapshot {
    fn root(&self) -> (u64, u64) {
        (self.header[4], self.header[5])
    }

    fn values(&self) -> Vec<u64> {
        // Test assertions call this only after observing an admitted descriptor.
        let length = usize::try_from(self.header[4]).unwrap();
        assert!(length <= 512);
        self.payload[..length * 8]
            .as_chunks::<8>()
            .0
            .iter()
            .map(|word| u64::from_le_bytes(*word))
            .collect()
    }
}

async fn observe(pic: &PocketIc, id: CanisterId) -> Snapshot {
    let bytes = pic.query_call(id, id, "observe", vec![]).await.unwrap();
    assert_eq!(bytes.len(), 16 + 56 + 4096);
    let word = |offset| u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap());
    Snapshot {
        version: word(0),
        phase: word(8),
        header: std::array::from_fn(|i| word(16 + i * 8)),
        payload: bytes[72..].to_vec(),
    }
}

async fn rejected_update(pic: &PocketIc, id: CanisterId, method: &str) {
    let before = observe(pic, id).await;
    let error = pic.update_call(id, id, method, vec![]).await.unwrap_err();
    assert_eq!(error.error_code, ErrorCode::CanisterTrapped);
    assert_eq!(observe(pic, id).await, before);
}

async fn rejected_upgrade(pic: &PocketIc, id: CanisterId, target: &[u8]) {
    let before = observe(pic, id).await;
    let hash = stopped_hash(pic, id).await;
    let error = pic
        .upgrade_eop_canister(id, target.to_vec(), vec![], None)
        .await
        .unwrap_err();
    assert_eq!(error.error_code, ErrorCode::CanisterTrapped);
    assert_eq!(stopped_hash(pic, id).await, hash);
    pic.start_canister(id, None).await.unwrap();
    assert_eq!(observe(pic, id).await, before);
}

async fn install(pic: &PocketIc, bytes: Vec<u8>) -> CanisterId {
    let id = pic.create_canister().await;
    pic.install_canister(id, bytes, vec![], None).await;
    id
}

#[tokio::test(flavor = "current_thread")]
async fn descriptors_are_admitted_before_access_and_published_after_settlement() {
    let server = std::env::var("IC_TESTKIT_POCKET_IC_URL").expect(
        "run with the prepared server via make retention-setup and the selected test target",
    );
    let instance = PocketIcBuilder::new()
        .with_max_request_time_ms(Some(30_000))
        .with_application_subnet()
        .with_server_url(server.parse().expect("valid prepared server URL"))
        .build_async()
        .await;
    let pic = &instance;
    let a = candidate(1, (3, 4), 4096, false, "nop", true);
    let b = candidate(2, (3, 4), 4096, false, "nop", true);
    let id = install(pic, a.clone()).await;
    assert_eq!(observe(pic, id).await.values(), [17, 23, 41]);
    update(pic, id, "append").await;
    assert_eq!(observe(pic, id).await.values(), [17, 23, 41, 101]);
    for method in [
        "duplicate_claim",
        "settle_without_claim",
        "invalid_settlement",
        "trap_after_settlement",
    ] {
        rejected_update(pic, id, method).await;
    }
    rejected_upgrade(pic, id, &candidate(2, (3, 4), 8192, false, "nop", true)).await;
    rejected_upgrade(pic, id, &candidate(2, (3, 4), 4096, true, "nop", true)).await;

    let before = observe(pic, id).await;
    let a_hash = stopped_hash(pic, id).await;
    pic.upgrade_eop_canister(id, b.clone(), vec![], None)
        .await
        .unwrap();
    assert_ne!(stopped_hash(pic, id).await, a_hash);
    pic.start_canister(id, None).await.unwrap();
    let after = observe(pic, id).await;
    assert_eq!(after.version, 2);
    assert_eq!(after.phase, 1);
    assert_eq!(after.header, before.header);
    assert_eq!(after.payload, before.payload);
    update(pic, id, "append").await;
    assert_eq!(observe(pic, id).await.root(), (5, 5));
    assert_eq!(observe(pic, id).await.values(), [17, 23, 41, 101, 102]);
    update(pic, id, "remove_first").await;
    assert_eq!(observe(pic, id).await.values(), [23, 41, 101, 102]);
    update(pic, id, "trim").await;
    assert_eq!(observe(pic, id).await.root(), (1, 1));
    stopped_hash(pic, id).await;
    pic.upgrade_eop_canister(id, a, vec![], None).await.unwrap();
    assert_eq!(stopped_hash(pic, id).await, a_hash);
    pic.start_canister(id, None).await.unwrap();
    assert_eq!(observe(pic, id).await.values(), [23]);
    let payload_before_clear = observe(pic, id).await.payload;
    update(pic, id, "clear").await;
    assert_eq!(observe(pic, id).await.root(), (0, 0));
    assert_eq!(observe(pic, id).await.payload, payload_before_clear);
    update(pic, id, "append").await;
    assert_eq!(observe(pic, id).await.values(), [101]);
    assert_eq!(observe(pic, id).await.root(), (1, 1));

    // Even without a source guard, destination admission cannot narrow a corrupt
    // u64 length/capacity into an apparently valid wasm32 descriptor.
    for (offset, value) in [
        (65536, 0),
        (65544, 2),
        (65552, 131080),
        (65560, 8192),
        (65568, 5),
        (65576, 0),
        (65576, 513),
        (65568, (1_u64 << 32) + 3),
        (65576, (1_u64 << 32) + 4),
        (65568, u64::MAX),
        (65576, u64::MAX),
        (65584, 0),
        (65584, 2),
    ] {
        let fault = format!("(i64.store (i32.const {offset}) (i64.const {value}))");
        let corrupt = install(pic, candidate(1, (3, 4), 4096, false, &fault, false)).await;
        update(pic, corrupt, "corrupt").await;
        rejected_update(pic, corrupt, "append").await;
        rejected_update(pic, corrupt, "remove_first").await;
        rejected_upgrade(pic, corrupt, &b).await;
    }

    for root in [(0, 0), (0, 8), (1, 1), (512, 512)] {
        let edge = install(pic, candidate(1, root, 4096, false, "nop", true)).await;
        assert_eq!(observe(pic, edge).await.root(), root);
        if root.0 == 0 {
            let empty = observe(pic, edge).await;
            update(pic, edge, "remove_first").await;
            assert_eq!(observe(pic, edge).await, empty);
        }
        if root.0 == 512 {
            rejected_update(pic, edge, "append").await;
            update(pic, edge, "remove_first").await;
            assert_eq!(observe(pic, edge).await.root(), (511, 512));
            update(pic, edge, "append").await;
            let full = observe(pic, edge).await;
            assert_eq!(full.root(), (512, 512));
            assert_eq!(full.values()[511], 101);
        } else {
            update(pic, edge, "append").await;
            assert_eq!(
                observe(pic, edge).await.root(),
                (root.0 + 1, root.1.max(root.0 + 1))
            );
        }
    }

    let abandoned = install(pic, candidate(1, (3, 4), 4096, false, "nop", false)).await;
    update(pic, abandoned, "abandon_claim").await;
    let lost = observe(pic, abandoned).await;
    assert_eq!(lost.phase, 2);
    assert_eq!(lost.header[6], 0);
    rejected_update(pic, abandoned, "append").await;
    rejected_upgrade(pic, abandoned, &b).await;
    instance.drop().await;
}

//! Execute only the reviewed u64-vector foreign allocation and ownership contract.

use ic_storage_retention_probe::artifact::{sha256, sha256_hex};
use std::{fs, time::Duration};

use ic_testkit::{
    pic::PocketIcStartupConfig,
    pocket_ic::{CanisterId, ErrorCode, PocketIcBuilder, nonblocking::PocketIc},
};

use super::super::installed_tests::{stopped_hash, update};
use super::{Profile, admit_final, prepare};

mod scope_tests;

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
        assert!(self.header[4] <= 512);
        self.payload[..self.header[4] as usize * 8]
            .as_chunks::<8>()
            .0
            .iter()
            .map(|word| u64::from_le_bytes(*word))
            .collect()
    }

    fn settled(&self) {
        assert_eq!(self.phase, 1);
        assert_eq!(self.header[6], 1);
        assert_eq!(&self.header[..4], &[0x4e5650524f563031, 1, 131072, 4096]);
        assert!(self.header[4] <= self.header[5] && self.header[5] <= 512);
    }
}

async fn observe(pic: &PocketIc, id: CanisterId) -> Snapshot {
    let bytes = pic.query_call(id, id, "observe", vec![]).await.unwrap();
    assert_eq!(bytes.len(), 72 + 4096);
    let word = |offset| u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap());
    Snapshot {
        version: word(0),
        phase: word(8),
        header: std::array::from_fn(|i| word(16 + i * 8)),
        payload: bytes[72..].to_vec(),
    }
}

async fn install(pic: &PocketIc, bytes: Vec<u8>) -> CanisterId {
    let id = pic.create_canister().await;
    pic.install_canister(id, bytes, vec![], None).await;
    id
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

async fn upgrade(pic: &PocketIc, id: CanisterId, target: &[u8], version: u64) {
    let before = observe(pic, id).await;
    stopped_hash(pic, id).await;
    pic.upgrade_eop_canister(id, target.to_vec(), vec![], None)
        .await
        .unwrap();
    assert_eq!(stopped_hash(pic, id).await, &sha256(target));
    pic.start_canister(id, None).await.unwrap();
    let after = observe(pic, id).await;
    after.settled();
    assert_eq!(after.version, version);
    assert_eq!(after.header, before.header);
    assert_eq!(after.payload, before.payload);
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "make native-provider-test supplies fresh artifacts and the prepared server"]
async fn native_vector_lifecycle_reclamation_and_rollback() {
    let (directory, _) = super::super::input();
    let raw = fs::read(directory.join("vector/provider_vector.wasm")).unwrap();
    let candidate = |profile| {
        let bytes = prepare(&raw, profile).unwrap();
        admit_final(&bytes, &raw, profile).unwrap();
        let digest = sha256_hex(&bytes);
        fs::write(
            directory.join("vector").join(format!("{digest}.wasm")),
            &bytes,
        )
        .unwrap();
        println!(
            "native vector {profile:?}: sha256={digest} bytes={}",
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
    let a = candidate(Profile::A);
    let b = candidate(Profile::B);
    let id = install(pic, a.clone()).await;
    assert_eq!(observe(pic, id).await.values(), [17, 23, 41]);
    update(pic, id, "vector_mutate").await;
    update(pic, id, "vector_append").await;
    update(pic, id, "vector_append").await;
    let grown = observe(pic, id).await;
    grown.settled();
    assert_eq!(grown.root(), (5, 5));
    assert_eq!(grown.values(), [18, 23, 41, 1, 1]);
    rejected_update(pic, id, "vector_trap").await;
    rejected_upgrade(
        pic,
        id,
        &candidate(Profile {
            extent: 8192,
            ..Profile::B
        }),
    )
    .await;
    rejected_upgrade(
        pic,
        id,
        &candidate(Profile {
            post_trap: true,
            ..Profile::B
        }),
    )
    .await;
    upgrade(pic, id, &b, 2).await;
    update(pic, id, "vector_remove").await;
    assert_eq!(observe(pic, id).await.values(), [23, 41, 1, 1]);
    update(pic, id, "vector_shrink").await;
    assert_eq!(observe(pic, id).await.root(), (4, 4));
    update(pic, id, "vector_append").await;
    // Independent WAT client reads the same descriptor and appends after Rust.
    update(pic, id, "append").await;
    assert_eq!(observe(pic, id).await.values(), [23, 41, 1, 1, 2, 102]);
    upgrade(pic, id, &a, 1).await;
    update(pic, id, "vector_reuse").await;
    assert_eq!(observe(pic, id).await.root(), (1, 1));
    assert_eq!(observe(pic, id).await.values(), [1]);
    let before_clear = observe(pic, id).await.payload;
    update(pic, id, "vector_clear").await;
    assert_eq!(observe(pic, id).await.root(), (0, 0));
    assert_eq!(observe(pic, id).await.payload, before_clear);
    upgrade(pic, id, &b, 2).await;
    update(pic, id, "vector_append").await;
    assert_eq!(observe(pic, id).await.values(), [2]);
    observe(pic, id).await.settled();

    for root in [(0, 0), (0, 8), (1, 1), (512, 512)] {
        let edge = install(pic, candidate(Profile { root, ..Profile::A })).await;
        upgrade(pic, edge, &b, 2).await;
        if root.0 == 0 {
            let empty = observe(pic, edge).await;
            update(pic, edge, "vector_remove").await;
            assert_eq!(observe(pic, edge).await, empty);
        }
        if root.0 == 512 {
            rejected_update(pic, edge, "vector_append").await;
            // The first native call writes and settles, the second fails reserve
            // inside Rust. Rollback must undo both calls, including spare bytes.
            rejected_update(pic, edge, "vector_mutate_append").await;
            update(pic, edge, "vector_remove").await;
            assert_eq!(observe(pic, edge).await.root(), (511, 512));
        }
        update(pic, edge, "vector_append").await;
        assert_eq!(observe(pic, edge).await.values().last(), Some(&2));
        update(pic, edge, "vector_shrink").await;
        let shrunk = observe(pic, edge).await;
        assert_eq!(shrunk.root().0, shrunk.root().1);
        upgrade(pic, edge, &a, 1).await;
        update(pic, edge, "vector_reuse").await;
        let reused = observe(pic, edge).await;
        reused.settled();
        assert_eq!(reused.root(), (1, 1));
        assert_eq!(reused.values(), [1]);
    }

    for fault in [
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
        let corrupt = install(
            pic,
            candidate(Profile {
                fault: Some(fault),
                source_guard: false,
                ..Profile::A
            }),
        )
        .await;
        update(pic, corrupt, "corrupt").await;
        rejected_update(pic, corrupt, "vector_mutate").await;
        rejected_update(pic, corrupt, "vector_append").await;
        // No source pre-upgrade hook: the destination must independently refuse
        // before its first native interpretation, never silently reset metadata.
        rejected_upgrade(pic, corrupt, &b).await;
    }
    let abandoned = install(
        pic,
        candidate(Profile {
            source_guard: false,
            ..Profile::A
        }),
    )
    .await;
    update(pic, abandoned, "abandon_claim").await;
    let lost = observe(pic, abandoned).await;
    assert_eq!((lost.phase, lost.header[6]), (2, 0));
    rejected_update(pic, abandoned, "vector_reuse").await;
    rejected_upgrade(pic, abandoned, &b).await;
    owner.close().await;
}

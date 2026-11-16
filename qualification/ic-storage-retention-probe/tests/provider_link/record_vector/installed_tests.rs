//! Installed execution under the bounded record foreign-allocation contract.

use ic_storage_retention_probe::{
    artifact::{compile_wat, sha256_hex, validate_wasm},
    vector_provider::records,
};
use ic_testkit::{
    pic::PocketIcStartupConfig,
    pocket_ic::{CanisterId, ErrorCode, PocketIcBuilder, nonblocking::PocketIc},
};
use std::{fs, time::Duration};

use super::super::installed_tests::{stopped_hash, update};
use super::link::{self, Profile};

fn inputs() -> (std::path::PathBuf, Vec<u8>) {
    let (directory, _) = super::super::input();
    let raw = fs::read(directory.join("vector/provider_record_vector.wasm")).unwrap();
    (directory, raw)
}

fn candidate(directory: &std::path::Path, raw: &[u8], profile: Profile) -> Vec<u8> {
    let bytes = link::prepare(raw, profile).unwrap();
    link::admit_final(&bytes, raw, profile).unwrap();
    let digest = sha256_hex(&bytes);
    fs::write(
        directory.join(format!("vector/record-{digest}.wasm")),
        &bytes,
    )
    .unwrap();
    println!(
        "installed record vector {profile:?} bytes={} sha256={digest}",
        bytes.len()
    );
    bytes
}

#[derive(Debug, PartialEq, Eq)]
struct Snapshot {
    version: u64,
    phase: u64,
    header: [u64; 18],
    payload: Vec<u8>,
}

impl Snapshot {
    fn values(&self) -> Vec<(u32, u64)> {
        let length = usize::try_from(self.header[4]).unwrap();
        assert!(length <= 16);
        self.payload[..length * 16]
            .as_chunks::<16>()
            .0
            .iter()
            .map(|bytes| {
                (
                    u32::from_le_bytes(bytes[..4].try_into().unwrap()),
                    u64::from_le_bytes(bytes[8..16].try_into().unwrap()),
                )
            })
            .collect()
    }
}

async fn observe(pic: &PocketIc, id: CanisterId) -> Snapshot {
    let bytes = pic.query_call(id, id, "observe", vec![]).await.unwrap();
    assert_eq!(bytes.len(), 16 + 144 + 256);
    let word = |offset| u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap());
    Snapshot {
        version: word(0),
        phase: word(8),
        header: std::array::from_fn(|i| word(16 + i * 8)),
        payload: bytes[160..].to_vec(),
    }
}

async fn rejected_update(pic: &PocketIc, id: CanisterId, method: &str) {
    let before = observe(pic, id).await;
    let error = pic.update_call(id, id, method, vec![]).await.unwrap_err();
    assert_eq!(error.error_code, ErrorCode::CanisterTrapped);
    assert_eq!(observe(pic, id).await, before);
}

async fn rejected_upgrade(pic: &PocketIc, id: CanisterId, target: &[u8]) {
    let before = pic.query_call(id, id, "observe", vec![]).await.unwrap();
    let hash = stopped_hash(pic, id).await;
    let error = pic
        .upgrade_eop_canister(id, target.to_vec(), vec![], None)
        .await
        .unwrap_err();
    assert_eq!(error.error_code, ErrorCode::CanisterTrapped);
    assert_eq!(stopped_hash(pic, id).await, hash);
    pic.start_canister(id, None).await.unwrap();
    assert_eq!(
        pic.query_call(id, id, "observe", vec![]).await.unwrap(),
        before
    );
}

async fn install(pic: &PocketIc, bytes: Vec<u8>) -> CanisterId {
    let id = pic.create_canister().await;
    pic.install_canister(id, bytes, vec![], None).await;
    id
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "make native-provider-test supplies fresh artifacts and the prepared server"]
async fn native_records_mutate_reclaim_and_reopen_across_installed_upgrades() {
    let (directory, raw) = inputs();
    let config = PocketIcStartupConfig::from_env(Duration::from_secs(30)).unwrap();
    let owner = config
        .build_instance(
            PocketIcBuilder::new()
                .with_max_request_time_ms(Some(30_000))
                .with_application_subnet(),
        )
        .await
        .unwrap();
    let pic = owner.instance();
    let b = candidate(&directory, &raw, Profile::B);
    for root in [(0, 0), (0, 8), (3, 4), (16, 16)] {
        let profile = Profile { root, ..Profile::A };
        let a = candidate(&directory, &raw, profile);
        let id = install(pic, a.clone()).await;
        let initial = observe(pic, id).await;
        assert_eq!(&initial.header[7..], records::SCHEMA);
        let mut expected: Vec<_> = (0..root.0).map(|i| (17 + i as u32, 1000)).collect();
        assert_eq!(initial.values(), expected);
        update(pic, id, "vector_mutate").await;
        if let Some(first) = expected.first_mut() {
            first.0 += 1;
        }
        assert_eq!(observe(pic, id).await.values(), expected);
        assert_eq!(
            pic.update_call(id, id, "vector_business_error", vec![])
                .await
                .unwrap(),
            1_u32.to_le_bytes()
        );
        if let Some(first) = expected.first_mut() {
            first.0 += 1;
        }
        if expected.len() < 16 {
            expected.push((1, 1000));
        }
        assert_eq!(observe(pic, id).await.values(), expected);
        update(pic, id, "vector_forget").await;
        expected[0].0 += 1;
        assert_eq!(observe(pic, id).await.values(), expected);
        for method in [
            "vector_trap",
            "vector_error_trap",
            "vector_error_finalize_trap",
            "record_mutate_then_fail",
        ] {
            rejected_update(pic, id, method).await;
        }
        rejected_upgrade(
            pic,
            id,
            &candidate(
                &directory,
                &raw,
                Profile {
                    post_trap: true,
                    ..Profile::B
                },
            ),
        )
        .await;
        let before = observe(pic, id).await;
        let a_hash = stopped_hash(pic, id).await;
        pic.upgrade_eop_canister(id, b.clone(), vec![], None)
            .await
            .unwrap();
        assert_ne!(stopped_hash(pic, id).await, a_hash);
        pic.start_canister(id, None).await.unwrap();
        let after = observe(pic, id).await;
        assert_eq!((after.version, after.phase), (2, 1));
        assert_eq!(after.header, before.header);
        assert_eq!(after.values(), expected);
        // Padding has no logical value; failed transactions compare every byte,
        // while successful native assignments need preserve only logical fields.
        if expected.len() == 16 {
            rejected_update(pic, id, "vector_append").await;
        }
        update(pic, id, "vector_remove").await;
        expected.remove(0);
        update(pic, id, "vector_append").await;
        expected.push((2, 1000));
        assert_eq!(observe(pic, id).await.values(), expected);
        update(pic, id, "vector_shrink").await;
        assert_eq!(observe(pic, id).await.header[5], expected.len() as u64);
        update(pic, id, "vector_clear").await;
        assert_eq!(&observe(pic, id).await.header[4..6], [0, 0]);
        update(pic, id, "vector_append").await;
        update(pic, id, "vector_reuse").await;
        assert_eq!(observe(pic, id).await.values(), [(2, 1000)]);
        stopped_hash(pic, id).await;
        pic.upgrade_eop_canister(id, a, vec![], None).await.unwrap();
        assert_eq!(stopped_hash(pic, id).await, a_hash);
        pic.start_canister(id, None).await.unwrap();
        update(pic, id, "vector_mutate").await;
        assert_eq!(observe(pic, id).await.values(), [(3, 1000)]);
    }
    owner.close().await;
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "make native-provider-test supplies fresh artifacts and the prepared server"]
async fn native_record_admission_refuses_corruption_and_abandoned_custody() {
    let (directory, raw) = inputs();
    let config = PocketIcStartupConfig::from_env(Duration::from_secs(30)).unwrap();
    let owner = config
        .build_instance(
            PocketIcBuilder::new()
                .with_max_request_time_ms(Some(30_000))
                .with_application_subnet(),
        )
        .await
        .unwrap();
    let pic = owner.instance();
    let b = candidate(&directory, &raw, Profile::B);
    let mut faults: Vec<_> = records::SCHEMA
        .iter()
        .enumerate()
        .map(|(i, word)| (65592 + i as u32 * 8, word ^ 1))
        .collect();
    faults.extend([
        (65536, 0),
        (65544, 2),
        (65552, 131080),
        (65560, 4096),
        (65568, 4),
        (65568, 5),
        (65576, 17),
        (65568, (1_u64 << 32) + 3),
        (65576, (1_u64 << 32) + 4),
        (65568, u64::MAX),
        (65576, u64::MAX),
        (65584, 0),
        (65600, (1_u64 << 32) + 3),
        (65648, 8),
        (131080, 0),
        (131112, 0),
    ]);
    for fault in faults {
        let id = install(
            pic,
            candidate(
                &directory,
                &raw,
                Profile {
                    fault: Some(fault),
                    source_guard: false,
                    ..Profile::A
                },
            ),
        )
        .await;
        update(pic, id, "corrupt").await;
        rejected_update(pic, id, "vector_mutate").await;
        rejected_update(pic, id, "vector_append").await;
        rejected_upgrade(pic, id, &b).await;
    }
    let empty = install(
        pic,
        candidate(
            &directory,
            &raw,
            Profile {
                root: (0, 8),
                fault: Some((65600, 2)),
                source_guard: false,
                ..Profile::A
            },
        ),
    )
    .await;
    update(pic, empty, "corrupt").await;
    rejected_update(pic, empty, "vector_append").await;
    rejected_upgrade(pic, empty, &b).await;
    let abandoned = install(
        pic,
        candidate(
            &directory,
            &raw,
            Profile {
                source_guard: false,
                ..Profile::A
            },
        ),
    )
    .await;
    update(pic, abandoned, "abandon_claim").await;
    rejected_update(pic, abandoned, "vector_mutate").await;
    rejected_upgrade(pic, abandoned, &b).await;
    let u64_raw = fs::read(directory.join("vector/provider_vector.wasm")).unwrap();
    let u64_module =
        super::super::vector_link::prepare(&u64_raw, super::super::vector_link::Profile::A)
            .unwrap();
    let old = install(pic, u64_module.clone()).await;
    rejected_upgrade(pic, old, &b).await;
    let records = install(pic, b).await;
    rejected_upgrade(pic, records, &u64_module).await;
    owner.close().await;
}

fn guard_candidate(root: (u64, u64), operation: u32, claim: bool, body: &str) -> Vec<u8> {
    let provider = records::source(1, root, false, "nop", true);
    let source = format!(
        "{}\n{}\n(func (export \"canister_update probe\")\n{}
        (global.set $access_operation (i32.const {operation})) {body}
        {} (call $reply)))",
        provider.trim_end().strip_suffix(')').unwrap(),
        link::guards().unwrap(),
        if claim { "(drop (call $claim))" } else { "" },
        if claim {
            "(call $settle (i64.load (i32.const 65568)) (i64.load (i32.const 65576)))"
        } else {
            ""
        }
    );
    let bytes = compile_wat(&source, 32768, 16384).unwrap();
    validate_wasm(&bytes, 16384).unwrap();
    bytes
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "make native-provider-test supplies fresh artifacts and the prepared server"]
async fn record_guards_bound_stack_fields_spare_capacity_and_copy_windows() {
    let (directory, _) = inputs();
    let config = PocketIcStartupConfig::from_env(Duration::from_secs(30)).unwrap();
    let owner = config
        .build_instance(
            PocketIcBuilder::new()
                .with_max_request_time_ms(Some(30_000))
                .with_application_subnet(),
        )
        .await
        .unwrap();
    let pic = owner.instance();
    let probe = |root, operation, claim, body: &str| {
        let bytes = guard_candidate(root, operation, claim, body);
        fs::write(
            directory.join(format!("vector/record-guard-{}.wasm", sha256_hex(&bytes))),
            &bytes,
        )
        .unwrap();
        bytes
    };
    let stack = install(
        pic,
        probe(
            (3, 4),
            0,
            false,
            "(call $access_store_i32 (i32.const 32704) (i32.const 7) (i64.const 0))
         (call $access_store_i64 (i32.const 32760) (i64.const 9) (i64.const 0))",
        ),
    )
    .await;
    update(pic, stack, "probe").await;
    let fields = install(pic, probe((3, 4), 0, true,
        "(call $access_store_i32 (i32.const 131104) (i32.const 77) (i64.const 0))
         (if (i64.ne (call $access_load_i64 (i32.const 131104) (i64.const 8)) (i64.const 1000)) (then unreachable))")).await;
    update(pic, fields, "probe").await;
    assert_eq!(
        observe(pic, fields).await.values(),
        [(17, 1000), (18, 1000), (77, 1000)]
    );
    let overlap = install(
        pic,
        probe(
            (3, 4),
            2,
            true,
            "(call $access_copy (i32.const 131072) (i32.const 131088) (i32.const 32))",
        ),
    )
    .await;
    update(pic, overlap, "probe").await;
    assert_eq!(
        observe(pic, overlap).await.values(),
        [(18, 1000), (19, 1000), (19, 1000)]
    );
    for (root, op, claim, body) in [
        (
            (3, 4),
            0,
            false,
            "(drop (call $access_load_i32 (i32.const 32700) (i64.const 0)))",
        ),
        (
            (3, 4),
            0,
            false,
            "(drop (call $access_load_i64 (i32.const 32764) (i64.const 0)))",
        ),
        (
            (3, 4),
            0,
            false,
            "(drop (call $access_load_i32 (i32.const 131072) (i64.const 0)))",
        ),
        (
            (3, 4),
            0,
            true,
            "(drop (call $access_load_i32 (i32.const 65568) (i64.const 0)))",
        ),
        (
            (3, 4),
            0,
            true,
            "(drop (call $access_load_i32 (i32.const 131120) (i64.const 0)))",
        ),
        (
            (3, 4),
            0,
            true,
            "(drop (call $access_load_i64 (i32.const 131116) (i64.const 0)))",
        ),
        (
            (3, 4),
            0,
            true,
            "(drop (call $access_load_i32 (i32.const 131072) (i64.const 4294967296)))",
        ),
        (
            (3, 4),
            0,
            true,
            "(drop (call $access_load_i32 (i32.const 131072) (i64.const -1)))",
        ),
        (
            (3, 4),
            1,
            true,
            "(drop (call $access_load_i32 (i32.const 131120) (i64.const 0)))",
        ),
        (
            (3, 4),
            1,
            true,
            "(call $access_store_i32 (i32.const 131136) (i32.const 0) (i64.const 0))",
        ),
        (
            (16, 16),
            1,
            true,
            "(call $access_store_i32 (i32.const 131328) (i32.const 0) (i64.const 0))",
        ),
        (
            (3, 4),
            4,
            true,
            "(drop (call $access_load_i32 (i32.const 131072) (i64.const 0)))",
        ),
        (
            (3, 4),
            4,
            true,
            "(call $access_store_i32 (i32.const 131072) (i32.const 0) (i64.const 0))",
        ),
        (
            (0, 8),
            0,
            true,
            "(drop (call $access_load_i32 (i32.const 131072) (i64.const 0)))",
        ),
        (
            (3, 4),
            8,
            true,
            "(drop (call $access_load_i32 (i32.const 131072) (i64.const 0)))",
        ),
        (
            (3, 4),
            0,
            true,
            "(call $access_copy (i32.const 131072) (i32.const 131088) (i32.const 16))",
        ),
        (
            (3, 4),
            2,
            true,
            "(call $access_copy (i32.const 131072) (i32.const 32704) (i32.const 16))",
        ),
        (
            (3, 4),
            2,
            true,
            "(call $access_copy (i32.const 131072) (i32.const 131088) (i32.const 48))",
        ),
    ] {
        let id = install(pic, probe(root, op, claim, body)).await;
        rejected_update(pic, id, "probe").await;
    }
    owner.close().await;
}

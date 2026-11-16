//! Installed record-element metadata protocol, reusing vector custody transitions.
//! No Rust record, allocator or vector executes inside these WAT canisters.

use ic_storage_retention_probe::artifact::{compile_wat, validate_wasm};
use ic_testkit::pocket_ic::{CanisterId, ErrorCode, PocketIcBuilder, nonblocking::PocketIc};

use super::{admit, install, record, stopped_hash, update};

use ic_storage_retention_probe::vector_provider::records::{
    HEADER_WORDS, PAYLOAD_BYTES, SCHEMA, source,
};

fn candidate(version: u64, root: (u64, u64), trap: bool, fault: &str, guard: bool) -> Vec<u8> {
    let wat = source(version, root, trap, fault, guard);
    let bytes = compile_wat(&wat, 16384, 8192).expect("bounded record metadata fixture");
    validate_wasm(&bytes, 8192).unwrap();
    admit(&bytes).expect("fixed-domain record metadata fixture");
    record("record vector metadata", &bytes);
    bytes
}

#[derive(Debug, PartialEq, Eq)]
struct Snapshot {
    version: u64,
    phase: u64,
    header: [u64; HEADER_WORDS],
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
    let payload_start = 16 + HEADER_WORDS * 8;
    assert_eq!(bytes.len(), payload_start + PAYLOAD_BYTES);
    let word = |offset| u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap());
    Snapshot {
        version: word(0),
        phase: word(8),
        header: std::array::from_fn(|i| word(16 + i * 8)),
        payload: bytes[payload_start..].to_vec(),
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

#[tokio::test(flavor = "current_thread")]
async fn record_elements_preserve_schema_values_and_custody_across_upgrades() {
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
    let b = candidate(2, (3, 4), false, "nop", true);
    for root in [(0, 0), (0, 8), (3, 4), (16, 16)] {
        let a = candidate(1, root, false, "nop", true);
        let id = install(pic, a.clone()).await;
        let initial = observe(pic, id).await;
        assert_eq!(&initial.header[7..], SCHEMA);
        assert_eq!(
            initial.values(),
            (0..root.0)
                .map(|i| (17 + i as u32, 1000))
                .collect::<Vec<_>>()
        );
        for bytes in initial.payload.as_chunks::<16>().0 {
            assert_eq!(&bytes[4..8], &[165; 4]);
        }
        for method in [
            "duplicate_claim",
            "settle_without_claim",
            "invalid_settlement",
            "invalid_element_settlement",
            "trap_after_settlement",
        ] {
            rejected_update(pic, id, method).await;
        }
        rejected_upgrade(pic, id, &candidate(2, root, true, "nop", true)).await;
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
        if root.0 == 16 {
            rejected_update(pic, id, "append").await;
        }
        update(pic, id, "remove_first").await;
        update(pic, id, "append").await;
        let expected: Vec<_> = after
            .values()
            .into_iter()
            .skip(1)
            .chain([(102, 1000)])
            .collect();
        assert_eq!(observe(pic, id).await.values(), expected);
        update(pic, id, "trim").await;
        let trimmed = observe(pic, id).await;
        stopped_hash(pic, id).await;
        pic.upgrade_eop_canister(id, a, vec![], None).await.unwrap();
        assert_eq!(stopped_hash(pic, id).await, a_hash);
        pic.start_canister(id, None).await.unwrap();
        assert_eq!(observe(pic, id).await.values(), trimmed.values());
        update(pic, id, "clear").await;
        assert!(observe(pic, id).await.values().is_empty());
        update(pic, id, "append").await;
        assert_eq!(observe(pic, id).await.values(), [(101, 1000)]);
    }
    instance.drop().await;
}

#[tokio::test(flavor = "current_thread")]
async fn record_element_corruption_refuses_at_destination_without_source_guard() {
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
    let b = candidate(2, (3, 4), false, "nop", true);
    let mut faults: Vec<_> = SCHEMA
        .iter()
        .enumerate()
        .map(|(i, value)| (65592 + i * 8, value ^ 1))
        .collect();
    faults.extend([
        (65536, 0),
        (65544, 2),
        (65552, 131080),
        (65560, 4096),
        (65568, 5),
        (65576, 17),
        (65568, (1_u64 << 32) + 3),
        (65576, (1_u64 << 32) + 4),
        (65568, u64::MAX),
        (65576, u64::MAX),
        (65584, 0),
        (65600, (1_u64 << 32) + 3),
        (65648, 8),  // V2's counter width cannot pass V3's schema.
        (131080, 0), // First initialized record violates counter <= ceiling.
        (131112, 0), // Last initialized record must also be checked.
    ]);
    for (offset, value) in faults {
        let fault = format!("(i64.store (i32.const {offset}) (i64.const {value}))");
        let id = install(pic, candidate(1, (3, 4), false, &fault, false)).await;
        update(pic, id, "corrupt").await;
        rejected_update(pic, id, "append").await;
        rejected_upgrade(pic, id, &b).await;
    }
    // Empty roots still need complete element metadata, even though no element
    // fields are scanned. Spare slots deliberately fail the record invariant.
    let empty = install(
        pic,
        candidate(
            1,
            (0, 8),
            false,
            "(i64.store (i32.const 65600) (i64.const 2))",
            false,
        ),
    )
    .await;
    update(pic, empty, "corrupt").await;
    rejected_upgrade(pic, empty, &b).await;
    let lost = install(pic, candidate(1, (3, 4), false, "nop", false)).await;
    update(pic, lost, "abandon_claim").await;
    rejected_update(pic, lost, "append").await;
    rejected_upgrade(pic, lost, &b).await;
    // A scalar-vector image with valid old metadata cannot masquerade as records.
    let scalar_module = super::candidate(1, (3, 4), 4096, false, "nop", true);
    let scalar = install(pic, scalar_module.clone()).await;
    super::rejected_upgrade(pic, scalar, &b).await;
    let records = install(pic, b).await;
    rejected_upgrade(pic, records, &scalar_module).await;
    instance.drop().await;
}

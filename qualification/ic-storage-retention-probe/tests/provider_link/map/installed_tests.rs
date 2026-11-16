//! Installed map lifecycle and transaction rollback under the closed provider.
use super::super::installed_tests::{stopped_hash, update};
use super::{
    inputs,
    link::{self, Profile},
};
use ic_testkit::pocket_ic::{CanisterId, ErrorCode, PocketIcBuilder, nonblocking::PocketIc};
use std::{collections::BTreeMap, fs, path::Path};

fn candidate(directory: &Path, raw: &[u8], profile: Profile) -> Vec<u8> {
    let bytes = link::prepare(raw, profile).unwrap();
    link::admit_final(&bytes, raw, profile).unwrap();
    let hash = ic_storage_retention_probe::artifact::sha256_hex(&bytes);
    fs::write(directory.join(format!("map-{hash}.wasm")), &bytes).unwrap();
    println!("map {profile:?} sha256={hash} bytes={}", bytes.len());
    bytes
}

#[derive(Debug, PartialEq, Eq)]
struct Snapshot {
    version: u64,
    phase: u64,
    root: [u64; 11],
    payload: Vec<u8>,
}

impl Snapshot {
    fn records(&self) -> BTreeMap<u64, u64> {
        let mut records = BTreeMap::new();
        let base = self.root[10] as usize * 12288;
        for slot in self.payload[base..base + self.root[5] as usize * 24]
            .as_chunks::<24>()
            .0
        {
            let occupied = u64::from_le_bytes(slot[..8].try_into().unwrap());
            let key = u64::from_le_bytes(slot[8..16].try_into().unwrap());
            let value = u64::from_le_bytes(slot[16..].try_into().unwrap());
            if occupied == 1 {
                assert!(records.insert(key, value).is_none());
            } else {
                assert_eq!((occupied, key, value), (0, 0, 0));
            }
        }
        assert_eq!(records.len(), self.root[4] as usize);
        records
    }
}

async fn observe(pic: &PocketIc, id: CanisterId) -> Snapshot {
    let bytes = pic.query_call(id, id, "observe", vec![]).await.unwrap();
    assert_eq!(bytes.len(), 104 + 24576);
    let word = |offset| u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap());
    Snapshot {
        version: word(0),
        phase: word(8),
        root: std::array::from_fn(|i| word(16 + i * 8)),
        payload: bytes[104..].to_vec(),
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
#[ignore = "map collector and prepared PocketIC server"]
async fn maps_retain_records_and_custody_across_upgrades_and_failed_publication() {
    let (directory, raw) = inputs();
    let server = std::env::var("IC_TESTKIT_POCKET_IC_URL").expect("prepared server URL");
    let instance = PocketIcBuilder::new()
        .with_max_request_time_ms(Some(30_000))
        .with_application_subnet()
        .with_server_url(server.parse().unwrap())
        .build_async()
        .await;
    let pic = &instance;
    for buckets in [8, 512] {
        let a = candidate(
            &directory,
            &raw,
            Profile {
                buckets,
                ..Profile::A
            },
        );
        let b = candidate(
            &directory,
            &raw,
            Profile {
                buckets,
                ..Profile::B
            },
        );
        let id = pic.create_canister().await;
        pic.install_canister(id, a.clone(), vec![], None).await;
        assert!(observe(pic, id).await.records().is_empty());
        for method in [
            "insert",
            "insert_second",
            "replace",
            "lookup",
            "delete",
            "absent",
            "business_error",
            "forget",
        ] {
            update(pic, id, method).await;
        }
        let before = observe(pic, id).await;
        assert_eq!(
            before.records(),
            BTreeMap::from([(7, 71), (99, 990), (100, 1000)])
        );
        assert_eq!((before.phase, before.root[6]), (1, 1));
        for method in [
            "trap",
            "finalize_trap",
            "duplicate_claim",
            "settle_without_claim",
        ] {
            rejected_update(pic, id, method).await;
        }
        let failing = candidate(
            &directory,
            &raw,
            Profile {
                buckets,
                post_trap: true,
                ..Profile::B
            },
        );
        rejected_upgrade(pic, id, &failing).await;
        pic.upgrade_eop_canister(id, b.clone(), vec![], None)
            .await
            .unwrap();
        let after = observe(pic, id).await;
        assert_eq!(after.version, 2);
        assert_eq!(after.root, before.root);
        assert_eq!(after.payload, before.payload);
        update(pic, id, "lookup").await;
        // Every successful rebuild alternates banks. Test failures while the
        // candidate is being initialized, after rehash, after native release,
        // and after root publication, then reuse the exact restored source.
        while observe(pic, id).await.root[5] < 512 {
            update(pic, id, "fill").await;
            let before_growth = observe(pic, id).await;
            let old_buckets = before_growth.root[5];
            let upgrade_failure = candidate(
                &directory,
                &raw,
                Profile {
                    version: if before_growth.version == 1 { 2 } else { 1 },
                    buckets,
                    post_trap: true,
                    ..Profile::A
                },
            );
            rejected_upgrade(pic, id, &upgrade_failure).await;
            for method in [
                "grow_write_trap",
                "grow_candidate_trap",
                "grow_finalize_trap",
                "grow_published_trap",
            ] {
                rejected_update(pic, id, method).await;
            }
            let method = match old_buckets {
                8 => "grow_business_error",
                16 => "grow_forget",
                _ => "grow",
            };
            update(pic, id, method).await;
            let grown = observe(pic, id).await;
            let mut expected = before_growth.records();
            expected.insert(999, 999);
            assert_eq!(grown.records(), expected);
            assert_eq!(grown.root[5], old_buckets * 2);
            assert_eq!(grown.root[10], 1 - before_growth.root[10]);
            let old_base = before_growth.root[10] as usize * 12288;
            assert_eq!(
                grown.payload[old_base..old_base + 12288],
                before_growth.payload[old_base..old_base + 12288]
            );
            // Updating an existing key must not allocate or switch the root.
            update(pic, id, "grow").await;
            assert_eq!(observe(pic, id).await, grown);
            let target = if grown.version == 1 {
                b.clone()
            } else {
                a.clone()
            };
            pic.upgrade_eop_canister(id, target, vec![], None)
                .await
                .unwrap();
            let reopened = observe(pic, id).await;
            assert_eq!(reopened.root, grown.root);
            assert_eq!(reopened.payload, grown.payload);
            assert_eq!(reopened.records(), expected);
        }
        update(pic, id, "fill").await;
        let full = observe(pic, id).await;
        let mut expected: BTreeMap<_, _> = (0..384).map(|key| (key, key + 1000)).collect();
        assert_eq!(full.records(), expected);
        update(pic, id, "refuse_growth").await;
        assert_eq!(observe(pic, id).await, full);
        update(pic, id, "delete_full").await;
        update(pic, id, "reuse").await;
        expected.remove(&2);
        expected.insert(999, 999);
        assert_eq!(observe(pic, id).await.records(), expected);
        pic.upgrade_eop_canister(id, a, vec![], None).await.unwrap();
        assert_eq!(observe(pic, id).await.records(), expected);
        update(pic, id, "clear").await;
        update(pic, id, "insert").await;
        assert_eq!(observe(pic, id).await.records(), BTreeMap::from([(7, 70)]));
    }
    // Persisted metadata and malformed native slots must refuse at both source
    // and target boundaries. Skipping the source hook exposes target admission.
    for (offset, value) in [
        (65536, 0),
        (65544, 1),
        (65568, 7),
        (65576, 9),
        (65584, 0),
        (65592, 16),
        (65600, 4),
        (65608, 2),
        (65616, 2),
        (65616, 1 << 32),
        (131072, 2),
        (131080, 99),
    ] {
        for source_guard in [false, true] {
            let a = candidate(
                &directory,
                &raw,
                Profile {
                    buckets: 8,
                    source_guard,
                    fault: Some((offset, value)),
                    ..Profile::A
                },
            );
            let target = candidate(
                &directory,
                &raw,
                Profile {
                    buckets: 8,
                    ..Profile::B
                },
            );
            let id = pic.create_canister().await;
            pic.install_canister(id, a, vec![], None).await;
            update(pic, id, "corrupt").await;
            rejected_update(pic, id, "insert").await;
            rejected_upgrade(pic, id, &target).await;
        }
    }
    guard_boundaries(pic, &directory, &raw).await;
    instance.drop().await;
    fs::write(
        directory.join("INSTALLED"),
        "compatible upgrades, refusal and rollback passed\n",
    )
    .unwrap();
}

async fn guard_boundaries(pic: &PocketIc, directory: &Path, raw: &[u8]) {
    use ic_storage_retention_probe::artifact::compile_wat;
    let mut source = link::source(Profile {
        buckets: 8,
        ..Profile::A
    })
    .unwrap();
    let mut methods = String::new();
    let claim = "(drop (call $claim)) (global.set $operation (i32.const 1))";
    let growth = "(i64.store (i32.const 65568) (i64.const 6)) (drop (call $claim)) (global.set $operation (i32.const 1))";
    let reverse_growth = "(i64.store (i32.const 65616) (i64.const 1)) (i64.store (i32.const 65568) (i64.const 6)) (drop (call $claim)) (global.set $operation (i32.const 1))";
    let settle = "(call $settle (i64.const 0) (i64.const 8) (i64.const 0)) (global.set $operation (i32.const -1))";
    for (name, prefix, operation, suffix) in [
        (
            "stack_ok",
            "",
            "(call $access_store_i32 (i32.const 32592) (i32.const 0) (i64.const 0))",
            "",
        ),
        (
            "payload_ok",
            claim,
            "(drop (call $access_load_i64 (i32.const 131256) (i64.const 0))) (call $access_fill (i32.const 131072) (i32.const 0) (i32.const 192))",
            settle,
        ),
        (
            "unclaimed",
            "",
            "(drop (call $access_load_i64 (i32.const 131072) (i64.const 0)))",
            "",
        ),
        (
            "metadata",
            claim,
            "(drop (call $access_load_i64 (i32.const 65536) (i64.const 0)))",
            "",
        ),
        (
            "spare",
            claim,
            "(drop (call $access_load_i64 (i32.const 131264) (i64.const 0)))",
            "",
        ),
        (
            "inactive_without_growth",
            claim,
            "(drop (call $access_load_i64 (i32.const 143360) (i64.const 0)))",
            "",
        ),
        (
            "publish_unbuilt",
            claim,
            "(call $settle (i64.const 1) (i64.const 16) (i64.const 1))",
            "",
        ),
        (
            "candidate_end",
            growth,
            "(drop (call $access_load_u8 (i32.const 143744) (i64.const 0)))",
            "",
        ),
        (
            "candidate_read_scope",
            growth,
            "(global.set $operation (i32.const 0)) (drop (call $access_load_i64 (i32.const 143360) (i64.const 0)))",
            "",
        ),
        (
            "reverse_candidate_end",
            reverse_growth,
            "(drop (call $access_load_u8 (i32.const 131456) (i64.const 0)))",
            "",
        ),
        (
            "stack_below",
            "",
            "(drop (call $access_load_u8 (i32.const 32591) (i64.const 0)))",
            "",
        ),
        (
            "stack_above",
            "",
            "(drop (call $access_load_u8 (i32.const 32768) (i64.const 0)))",
            "",
        ),
        (
            "offset_wrap",
            claim,
            "(drop (call $access_load_i64 (i32.const 131072) (i64.const -1)))",
            "",
        ),
        (
            "fill_outside",
            claim,
            "(call $access_fill (i32.const 131072) (i32.const 0) (i32.const 193))",
            "",
        ),
        (
            "fill_nonzero",
            claim,
            "(call $access_fill (i32.const 131072) (i32.const 1) (i32.const 24))",
            "",
        ),
        (
            "copy",
            claim,
            "(call $access_copy (i32.const 131072) (i32.const 131096) (i32.const 24))",
            "",
        ),
        (
            "bad_operation",
            claim,
            "(global.set $operation (i32.const 11)) (drop (call $access_load_i64 (i32.const 131072) (i64.const 0)))",
            "",
        ),
        (
            "after_settle",
            claim,
            "(call $settle (i64.const 0) (i64.const 8) (i64.const 0)) (drop (call $access_load_i64 (i32.const 131072) (i64.const 0)))",
            "",
        ),
    ] {
        methods.push_str(&format!("(func (export \"canister_update guard_{name}\") {prefix} {operation} {suffix} (call $reply))\n"));
    }
    source.insert_str(source.rfind(')').unwrap(), &methods);
    let provider = compile_wat(
        &source,
        super::super::artifact::LIMIT,
        super::super::artifact::LIMIT,
    )
    .unwrap();
    let bytes = super::super::vector_link::compose_map(raw, &provider).unwrap();
    // These added endpoints are deliberate negative-test instrumentation, never
    // an admitted product/profile candidate. The original guards are unchanged.
    assert!(
        link::admit_final(
            &bytes,
            raw,
            Profile {
                buckets: 8,
                ..Profile::A
            }
        )
        .is_err()
    );
    fs::write(directory.join("guard-counterexamples.wasm"), &bytes).unwrap();
    let id = pic.create_canister().await;
    pic.install_canister(id, bytes, vec![], None).await;
    let before = observe(pic, id).await;
    for name in ["stack_ok", "payload_ok"] {
        update(pic, id, &format!("guard_{name}")).await;
    }
    assert_eq!(observe(pic, id).await, before);
    for name in [
        "unclaimed",
        "metadata",
        "spare",
        "inactive_without_growth",
        "publish_unbuilt",
        "candidate_end",
        "candidate_read_scope",
        "reverse_candidate_end",
        "stack_below",
        "stack_above",
        "offset_wrap",
        "fill_outside",
        "fill_nonzero",
        "copy",
        "bad_operation",
        "after_settle",
    ] {
        rejected_update(pic, id, &format!("guard_{name}")).await;
    }
    update(pic, id, "insert").await;
    assert_eq!(observe(pic, id).await.records(), BTreeMap::from([(7, 70)]));
}

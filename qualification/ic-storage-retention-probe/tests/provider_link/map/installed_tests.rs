//! Installed pooled maps: independent growth, joint publication and IC rollback.
use super::super::installed_tests::{stopped_hash, update};
use super::{
    inputs,
    link::{self, Profile},
};
use ic_testkit::pocket_ic::{CanisterId, ErrorCode, PocketIcBuilder, nonblocking::PocketIc};
use std::{collections::BTreeMap, fmt, fs, path::Path};

mod growth_tests;

fn candidate(directory: &Path, raw: &[u8], profile: Profile) -> Vec<u8> {
    let bytes = link::prepare(raw, profile).unwrap();
    link::admit_final(&bytes, raw, profile).unwrap();
    let hash = ic_storage_retention_probe::artifact::sha256_hex(&bytes);
    fs::write(directory.join(format!("map-{hash}.wasm")), &bytes).unwrap();
    println!("map {profile:?} sha256={hash} bytes={}", bytes.len());
    bytes
}
#[derive(PartialEq, Eq)]
struct Snapshot {
    version: u64,
    phase: u64,
    pages: u64,
    root: [u64; 10],
    payload: Vec<u8>,
}
impl fmt::Debug for Snapshot {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Snapshot")
            .field("version", &self.version)
            .field("phase", &self.phase)
            .field("pages", &self.pages)
            .field("root", &self.root)
            .field("payload_bytes", &self.payload.len())
            .field(
                "payload_sha256",
                &ic_storage_retention_probe::artifact::sha256_hex(&self.payload),
            )
            .finish()
    }
}
impl Snapshot {
    fn buckets(&self, id: usize) -> usize {
        1 << ((self.root[4 + id] >> 12) & 15)
    }
    fn span(&self, id: usize) -> std::ops::Range<usize> {
        let start = (self.root[4 + id] >> 16) as usize * 24;
        start..start + self.buckets(id) * 24
    }
    fn records(&self, id: usize) -> BTreeMap<u64, u64> {
        let mut records = BTreeMap::new();
        for slot in self.payload[self.span(id)].as_chunks::<24>().0 {
            let occupied = u64::from_le_bytes(slot[..8].try_into().unwrap());
            let key = u64::from_le_bytes(slot[8..16].try_into().unwrap());
            let value = u64::from_le_bytes(slot[16..].try_into().unwrap());
            if occupied == 1 {
                assert!(records.insert(key, value).is_none());
            } else {
                assert_eq!((occupied, key, value), (0, 0, 0));
            }
        }
        assert_eq!(records.len(), (self.root[4 + id] & 4095) as usize);
        records
    }
    fn unchanged_map(&self, before: &Self, id: usize) {
        assert_eq!(self.root[4 + id], before.root[4 + id]);
        assert_eq!(self.payload[self.span(id)], before.payload[before.span(id)]);
        assert_eq!(self.records(id), before.records(id));
    }
}
async fn observe(pic: &PocketIc, id: CanisterId) -> Snapshot {
    let bytes = pic.query_call(id, id, "observe", vec![]).await.unwrap();
    let word = |offset| u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap());
    assert_eq!(bytes.len(), 104 + word(16) as usize * 65536 - 131072);
    Snapshot {
        version: word(0),
        phase: word(8),
        pages: word(16),
        root: std::array::from_fn(|i| word(24 + i * 8)),
        payload: bytes[104..].to_vec(),
    }
}
async fn rejected_update(pic: &PocketIc, id: CanisterId, method: &str) {
    let before = observe(pic, id).await;
    let error = pic.update_call(id, id, method, vec![]).await.unwrap_err();
    assert_eq!(error.error_code, ErrorCode::CanisterTrapped, "{method}");
    assert_eq!(observe(pic, id).await, before, "{method}");
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
async fn retained_upgrade(pic: &PocketIc, id: CanisterId, target: &[u8], version: u64) {
    let before = observe(pic, id).await;
    pic.upgrade_eop_canister(id, target.to_vec(), vec![], None)
        .await
        .unwrap();
    let after = observe(pic, id).await;
    assert_eq!(after.version, version);
    assert_eq!(after.phase, 1);
    assert_eq!(after.pages, before.pages);
    assert_eq!(after.root, before.root);
    assert_eq!(after.payload, before.payload);
    for map in 0..2 {
        after.unchanged_map(&before, map);
    }
}
async fn publication_failures(pic: &PocketIc, id: CanisterId, suffix: &str) {
    for method in [
        "trap",
        "native_release",
        "partial_release",
        "finalize_failure",
        "first_root_failure",
        "second_root_failure",
        "settled_failure",
        "reset_finalize_failure",
        "reset_first_root_failure",
    ] {
        rejected_update(pic, id, &format!("{method}_{suffix}")).await;
    }
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
    let a = candidate(&directory, &raw, Profile::A);
    let b = candidate(&directory, &raw, Profile::B);
    let failing = candidate(
        &directory,
        &raw,
        Profile {
            post_trap: true,
            ..Profile::B
        },
    );
    let id = pic.create_canister().await;
    pic.install_canister(id, a.clone(), vec![], None).await;
    for (index, suffix) in [(0, "a"), (1, "b")] {
        assert!(observe(pic, id).await.records(index).is_empty());
        for method in ["insert", "replace", "lookup", "business_error", "forget"] {
            update(pic, id, &format!("{method}_{suffix}")).await;
        }
        assert_eq!(
            observe(pic, id).await.records(index),
            BTreeMap::from([(7, 71), (99, 990), (100, 1000)])
        );
        publication_failures(pic, id, suffix).await;
    }
    for method in ["duplicate_claim", "settle_without_claim"] {
        rejected_update(pic, id, method).await;
    }
    rejected_upgrade(pic, id, &failing).await;
    retained_upgrade(pic, id, &b, 2).await;
    retained_upgrade(pic, id, &a, 1).await;
    let before = observe(pic, id).await;
    update(pic, id, "delete_a").await;
    observe(pic, id).await.unchanged_map(&before, 1);
    update(pic, id, "both_a").await;
    assert_eq!(observe(pic, id).await.records(0).get(&65535), Some(&999));
    assert_eq!(observe(pic, id).await.records(1).get(&65535), Some(&1000));
    for (index, suffix) in [(0, "a"), (1, "b")] {
        let before = observe(pic, id).await;
        update(pic, id, &format!("reset_{suffix}")).await;
        let after = observe(pic, id).await;
        assert!(after.records(index).is_empty());
        after.unchanged_map(&before, 1 - index);
    }
    update(pic, id, "fill_a").await;
    update(pic, id, "fill_b").await;
    // First root grows to 512 while the other remains live. Then the second
    // grows to 256; its next candidate cannot coexist with both source roots.
    for (index, suffix, maximum) in [(0, "a", 512), (1, "b", 256)] {
        while observe(pic, id).await.buckets(index) < maximum {
            let before = observe(pic, id).await;
            let old_buckets = before.buckets(index);
            rejected_update(pic, id, &format!("write_failure_{suffix}")).await;
            publication_failures(pic, id, suffix).await;
            rejected_upgrade(pic, id, &failing).await;
            let method = match old_buckets {
                8 => "grow_business_error",
                16 => "grow_forget",
                _ => "grow",
            };
            update(pic, id, &format!("{method}_{suffix}")).await;
            let after = observe(pic, id).await;
            let mut expected = before.records(index);
            expected.insert(65535, 999);
            assert_eq!(after.records(index), expected);
            assert_eq!(after.buckets(index), old_buckets * 2);
            assert_ne!(after.span(index), before.span(index));
            after.unchanged_map(&before, 1 - index);
            assert_eq!(
                after.payload[before.span(index)],
                before.payload[before.span(index)]
            );
            update(pic, id, &format!("grow_{suffix}")).await;
            assert_eq!(observe(pic, id).await, after);
            update(pic, id, &format!("remove_growth_{suffix}")).await;
            update(pic, id, &format!("fill_{suffix}")).await;
            let version = if after.version == 1 { 2 } else { 1 };
            retained_upgrade(pic, id, if version == 1 { &a } else { &b }, version).await;
            update(pic, id, "lookup_full_a").await;
            update(pic, id, "lookup_full_b").await;
        }
    }
    let before = observe(pic, id).await;
    update(pic, id, "refuse_growth_b").await;
    assert_eq!(observe(pic, id).await, before);
    rejected_update(pic, id, "grow_b").await;
    update(pic, id, "delete_full_b").await;
    update(pic, id, "grow_b").await;
    assert_eq!(observe(pic, id).await.buckets(1), 256);
    update(pic, id, "remove_growth_b").await;
    update(pic, id, "fill_b").await;
    let before = observe(pic, id).await;
    update(pic, id, "reset_a").await;
    observe(pic, id).await.unchanged_map(&before, 1);
    update(pic, id, "grow_b").await;
    assert_eq!(observe(pic, id).await.buckets(1), 512);
    retained_upgrade(pic, id, &a, 1).await;

    // Both 512-slot roots may be installed together: 768 records total, with
    // clean bounded refusal and reuse after explicitly retiring one root.
    let full_profile = Profile {
        buckets: 512,
        ..Profile::A
    };
    let full_id = pic.create_canister().await;
    pic.install_canister(
        full_id,
        candidate(&directory, &raw, full_profile),
        vec![],
        None,
    )
    .await;
    update(pic, full_id, "fill_a").await;
    update(pic, full_id, "fill_b").await;
    let full = observe(pic, full_id).await;
    for index in 0..2 {
        assert_eq!(
            full.records(index),
            (0..384).map(|key| (key, key + 1000)).collect()
        );
    }
    for suffix in ["a", "b"] {
        update(pic, full_id, &format!("refuse_growth_{suffix}")).await;
        assert_eq!(observe(pic, full_id).await, full);
        for method in ["reset_finalize_failure", "reset_first_root_failure"] {
            rejected_update(pic, full_id, &format!("{method}_{suffix}")).await;
        }
    }
    retained_upgrade(pic, full_id, &b, 2).await;
    update(pic, full_id, "reset_a").await;
    observe(pic, full_id).await.unchanged_map(&full, 1);
    assert!(observe(pic, full_id).await.records(0).is_empty());
    // Reuse both freed regions and jointly grow both small roots in one message.
    update(pic, full_id, "reset_b").await;
    update(pic, full_id, "fill_a").await;
    update(pic, full_id, "fill_b").await;
    publication_failures(pic, full_id, "a").await;
    update(pic, full_id, "both_a").await;
    let both = observe(pic, full_id).await;
    assert_eq!((both.buckets(0), both.buckets(1)), (16, 16));
    assert_eq!(
        (both.records(0).get(&65535), both.records(1).get(&65535)),
        (Some(&999), Some(&1000))
    );
    retained_upgrade(pic, full_id, &a, 1).await;

    // Independent source/target admission: full-width descriptors, bounds,
    // overlap, representation and logical count all fail without resetting.
    for (offset, value) in [
        (65536, 0),
        (65544, 3),
        (65552, 131080),
        (65560, 24568),
        (65568, 1 << 32),
        (65568, 13 << 12),
        (65568, (3 << 12) | 7),
        (65568, (1000 << 16) | (9 << 12)),
        (65576, 3 << 12),
        (65568, (3 << 12) | 1),
        (65560, 98304),
        (65584, 0),
        (65592, 16),
        (65600, 4),
        (65608, 2),
        (131072, 2),
        (131080, 99),
        (131264, 2),
    ] {
        for source_guard in [false, true] {
            let source = candidate(
                &directory,
                &raw,
                Profile {
                    source_guard,
                    fault: Some((offset, value)),
                    ..Profile::A
                },
            );
            let fault_id = pic.create_canister().await;
            pic.install_canister(fault_id, source, vec![], None).await;
            update(pic, fault_id, "corrupt").await;
            rejected_update(pic, fault_id, "insert_a").await;
            rejected_update(pic, fault_id, "pool_grow").await;
            rejected_upgrade(pic, fault_id, &b).await;
        }
    }
    guard_boundaries(pic, &directory, &raw).await;
    growth_tests::qualify(pic, &directory, &raw).await;
    instance.drop().await;
    fs::write(
        directory.join("INSTALLED"),
        "two-root pool upgrades, provider page growth, bounded refusal, guards and whole-pool/page-count rollback passed\n",
    )
    .unwrap();
}

async fn guard_boundaries(pic: &PocketIc, directory: &Path, raw: &[u8]) {
    use ic_storage_retention_probe::artifact::compile_wat;
    let mut source = link::source(Profile::A).unwrap();
    let claim = "(global.set $operation (i32.const 0)) (drop (call $claim))";
    let settle = "(call $settle (i64.or (i64.load (i32.const 65568)) (i64.shl (i64.load (i32.const 65576)) (i64.const 32)))) (global.set $operation (i32.const -1))";
    let cases = [
        (
            "dynamic_tail",
            claim,
            "(drop (call $access_load_i64 (i32.add (i32.const 131064) (i32.wrap_i64 (i64.load (i32.const 65560)))) (i64.const 0)))",
            settle,
            true,
        ),
        (
            "dynamic_end",
            claim,
            "(drop (call $access_load_u8 (i32.add (i32.const 131072) (i32.wrap_i64 (i64.load (i32.const 65560)))) (i64.const 0)))",
            "",
            false,
        ),
        (
            "dynamic_copy_end",
            claim,
            "(call $access_copy (i32.const 131072) (i32.add (i32.const 131064) (i32.wrap_i64 (i64.load (i32.const 65560)))) (i32.const 16))",
            "",
            false,
        ),
        (
            "growth_while_claimed",
            claim,
            "(drop (call $expand_pool (i32.const 0)))",
            "",
            false,
        ),
        (
            "stack_ok",
            "",
            "(call $access_store_i32 (i32.const 30080) (i32.const 0) (i64.const 0)) (call $access_store_i64 (i32.const 32760) (i64.const 0) (i64.const 0))",
            "",
            true,
        ),
        (
            "payload_ok",
            claim,
            "(drop (call $access_load_i64 (i32.const 131072) (i64.const 0))) (drop (call $access_load_i64 (i32.const 155640) (i64.const 0))) (call $access_copy (i32.const 131072) (i32.const 131096) (i32.const 24))",
            settle,
            true,
        ),
        (
            "unclaimed",
            "",
            "(drop (call $access_load_i64 (i32.const 131072) (i64.const 0)))",
            "",
            false,
        ),
        (
            "metadata",
            claim,
            "(drop (call $access_load_i64 (i32.const 65536) (i64.const 0)))",
            "",
            false,
        ),
        (
            "below",
            claim,
            "(drop (call $access_load_u8 (i32.const 131071) (i64.const 0)))",
            "",
            false,
        ),
        (
            "above",
            claim,
            "(drop (call $access_load_u8 (i32.const 155648) (i64.const 0)))",
            "",
            false,
        ),
        (
            "cross_end",
            claim,
            "(drop (call $access_load_i64 (i32.const 155644) (i64.const 0)))",
            "",
            false,
        ),
        (
            "stack_below",
            "",
            "(drop (call $access_load_u8 (i32.const 30079) (i64.const 0)))",
            "",
            false,
        ),
        (
            "stack_above",
            "",
            "(drop (call $access_load_u8 (i32.const 32768) (i64.const 0)))",
            "",
            false,
        ),
        (
            "offset_wrap",
            claim,
            "(drop (call $access_load_i64 (i32.const 131072) (i64.const -1)))",
            "",
            false,
        ),
        (
            "copy_destination",
            claim,
            "(call $access_copy (i32.const 155640) (i32.const 131072) (i32.const 16))",
            "",
            false,
        ),
        (
            "copy_source",
            claim,
            "(call $access_copy (i32.const 131072) (i32.const 155640) (i32.const 16))",
            "",
            false,
        ),
        (
            "copy_gap",
            claim,
            "(call $access_copy (i32.const 131072) (i32.const 32760) (i32.const 16))",
            "",
            false,
        ),
        (
            "fill_unavailable",
            claim,
            "(call $access_fill (i32.const 131072) (i32.const 0) (i32.const 24))",
            "",
            false,
        ),
        (
            "bad_operation",
            claim,
            "(global.set $operation (i32.const 12)) (drop (call $access_load_i64 (i32.const 131072) (i64.const 0)))",
            "",
            false,
        ),
        (
            "bad_selector",
            claim,
            "(global.set $operation (i32.const 512)) (drop (call $access_load_i64 (i32.const 131072) (i64.const 0)))",
            "",
            false,
        ),
        (
            "after_settle",
            claim,
            settle,
            "(drop (call $access_load_i64 (i32.const 131072) (i64.const 0)))",
            false,
        ),
        (
            "overlap",
            claim,
            "(call $settle (i64.const 52776558145536))",
            "",
            false,
        ),
    ];
    let mut methods = String::new();
    for (name, prefix, operation, suffix, _) in cases {
        methods.push_str(&format!("(func (export \"canister_update guard_{name}\") {prefix} {operation} {suffix} (call $reply))\n"));
    }
    // Provider-only instrumentation dirties unloaned space already present in
    // the third page. Successful expansion must initialize it; trapped expansion
    // must restore it even though it is outside the old pool descriptor.
    methods.push_str("(func (export \"canister_update tail_canaries\") (i64.store (i32.const 155648) (i64.const 165)) (i64.store (i32.const 196600) (i64.const 165)) (call $reply))\n");
    source.insert_str(source.rfind(')').unwrap(), &methods);
    let provider = compile_wat(
        &source,
        super::super::artifact::LIMIT,
        super::super::artifact::LIMIT,
    )
    .unwrap();
    let bytes = super::super::vector_link::compose_map(raw, &provider).unwrap();
    // Extra test endpoints never pass exact production-profile admission.
    assert!(link::admit_final(&bytes, raw, Profile::A).is_err());
    fs::write(directory.join("guard-counterexamples.wasm"), &bytes).unwrap();
    let id = pic.create_canister().await;
    pic.install_canister(id, bytes, vec![], None).await;
    for (name, _, _, _, valid) in cases {
        let method = format!("guard_{name}");
        if valid {
            let before = observe(pic, id).await;
            update(pic, id, &method).await;
            assert_eq!(observe(pic, id).await, before);
        } else {
            rejected_update(pic, id, &method).await;
        }
    }
    update(pic, id, "insert_a").await;
    assert_eq!(observe(pic, id).await.records(0), BTreeMap::from([(7, 70)]));
    assert!(observe(pic, id).await.records(1).is_empty());
    update(pic, id, "tail_canaries").await;
    for _ in 0..3 {
        let before = observe(pic, id).await;
        for fault in 1..=4 {
            rejected_update(pic, id, &format!("pool_failure_{fault}")).await;
        }
        update(pic, id, "pool_grow").await;
        let after = observe(pic, id).await;
        assert_eq!(
            after.payload[..before.root[3] as usize],
            before.payload[..before.root[3] as usize]
        );
        assert!(
            after.payload[before.root[3] as usize..after.root[3] as usize]
                .iter()
                .all(|byte| *byte == 0)
        );
        let new_end = after.root[3] as usize;
        if new_end < before.payload.len() {
            assert_eq!(
                after.payload[new_end..before.payload.len()],
                before.payload[new_end..]
            );
        }
        update(pic, id, "guard_dynamic_tail").await;
        for method in [
            "guard_dynamic_end",
            "guard_dynamic_copy_end",
            "guard_growth_while_claimed",
        ] {
            rejected_update(pic, id, method).await;
        }
        assert_eq!(observe(pic, id).await.records(0), BTreeMap::from([(7, 70)]));
    }
}

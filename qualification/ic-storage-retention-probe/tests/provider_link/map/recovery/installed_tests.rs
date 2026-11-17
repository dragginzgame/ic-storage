use super::*;
use crate::{
    installed_tests::{stopped_hash, update},
    migration::tests::installed::{replace_heap, replace_heap_with_options},
};
use ic_storage_retention_probe::artifact::{sha256, sha256_hex};
use ic_testkit::pocket_ic::{CanisterId, ErrorCode, PocketIcBuilder, nonblocking::PocketIc};
use std::{collections::BTreeMap, fs};

fn args(epoch: u64, fault: u64) -> Vec<u8> {
    [epoch, fault]
        .into_iter()
        .flat_map(u64::to_le_bytes)
        .collect()
}
pub(super) fn word(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap())
}
pub(super) async fn query(pic: &PocketIc, id: CanisterId, method: &str) -> Vec<u8> {
    pic.query_call(id, id, method, vec![]).await.unwrap()
}
pub(super) fn records(bytes: &[u8], id: usize) -> BTreeMap<u64, u64> {
    let capacity = word(bytes, 48) as usize;
    assert!(matches!(capacity, 24576 | 49152 | 98304 | 196608));
    assert_eq!(bytes.len(), 104 + capacity);
    assert!(matches!(word(bytes, 32), 5 | 8 | 10));
    if word(bytes, 32) != 10 {
        assert_eq!(capacity, 24576);
    }
    let root = word(bytes, 56 + id * 8);
    let start = 104 + (root >> 16) as usize * 24;
    let buckets = 1usize << ((root >> 12) & 15);
    let mut records = BTreeMap::new();
    for slot in bytes[start..start + buckets * 24].as_chunks::<24>().0 {
        match word(slot, 0) {
            0 => assert_eq!((word(slot, 8), word(slot, 16)), (0, 0)),
            1 => assert!(records.insert(word(slot, 8), word(slot, 16)).is_none()),
            _ => panic!("tag"),
        }
    }
    assert_eq!(records.len(), (root & 4095) as usize);
    records
}
pub(super) fn checkpoint(bytes: &[u8], epoch: u64) -> [BTreeMap<u64, u64>; 2] {
    assert_eq!(&bytes[..8], b"NKPAIR01");
    assert_eq!(word(bytes, 8), 1);
    assert_eq!(word(bytes, 16), epoch);
    assert_eq!((word(bytes, 24), word(bytes, 32)), (1, 2));
    assert_eq!(&bytes[56..64], b"SEALED01");
    assert_eq!(
        &bytes[bytes.len() - 32..],
        &sha256(&bytes[..bytes.len() - 32])
    );
    let a = word(bytes, 40) as usize;
    let b = word(bytes, 48) as usize;
    assert_eq!(bytes.len(), 96 + a + b);
    let part = |part: &[u8]| {
        assert_eq!(&part[..8], b"NKEYMAP1");
        assert_eq!(word(part, 8), epoch);
        assert_eq!(&part[part.len() - 32..], &sha256(&part[..part.len() - 32]));
        let mut records = BTreeMap::new();
        for row in part[24..part.len() - 32].as_chunks::<16>().0 {
            assert!(records.insert(word(row, 0), word(row, 8)).is_none());
        }
        assert_eq!(records.len(), word(part, 16) as usize);
        records
    };
    [part(&bytes[64..64 + a]), part(&bytes[64 + a..64 + a + b])]
}
async fn refused_update(pic: &PocketIc, id: CanisterId, method: &str, arguments: Vec<u8>) {
    let before = query(pic, id, "observe").await;
    let stable = pic.get_stable_memory(id).await;
    let error = pic
        .update_call(id, id, method, arguments)
        .await
        .unwrap_err();
    assert!(
        matches!(
            error.error_code,
            ErrorCode::CanisterTrapped | ErrorCode::CanisterCalledTrap
        ),
        "{error:?}"
    );
    assert_eq!(query(pic, id, "observe").await, before);
    assert_eq!(pic.get_stable_memory(id).await, stable);
}
async fn refused_recovery(
    pic: &PocketIc,
    id: CanisterId,
    target: &[u8],
    epoch: u64,
    fault: u64,
    keep: bool,
) {
    refused_recovery_mode(pic, id, target, epoch, fault, keep, false).await;
}
async fn refused_recovery_mode(
    pic: &PocketIc,
    id: CanisterId,
    target: &[u8],
    epoch: u64,
    fault: u64,
    keep: bool,
    skip_source: bool,
) {
    let before = query(pic, id, "observe").await;
    let stable = pic.get_stable_memory(id).await;
    let hash = stopped_hash(pic, id).await;
    let result = if keep {
        pic.upgrade_eop_canister(id, target.to_vec(), args(epoch, fault), None)
            .await
    } else {
        replace_heap_with_options(pic, id, target, args(epoch, fault), skip_source, None).await
    };
    let error = result.unwrap_err();
    assert!(
        matches!(
            error.error_code,
            ErrorCode::CanisterTrapped | ErrorCode::CanisterCalledTrap
        ),
        "{error:?}"
    );
    assert_eq!(stopped_hash(pic, id).await, hash);
    pic.start_canister(id, None).await.unwrap();
    assert_eq!(query(pic, id, "observe").await, before);
    assert_eq!(pic.get_stable_memory(id).await, stable);
}
#[tokio::test(flavor = "current_thread")]
#[ignore = "map checkpoint collector and prepared PocketIC server"]
async fn maps_recover_from_governed_checkpoint_after_heap_loss() {
    let (directory, raw) = inputs();
    let bridge = fs::read(directory.join("checkpoint-map.wasm")).unwrap();
    let source = prepare(&raw, &bridge, false, false);
    let target = prepare(&raw, &bridge, true, true);
    let reopened = prepare(&raw, &bridge, true, false);
    let reverse = prepare(&raw, &bridge, false, true);
    assert!(admit_final(&target, &raw, &bridge, true, true));
    assert!(!admit_final(&target, &raw, &bridge, true, false));
    assert!(!admit_final(
        &target[..target.len() - 1],
        &raw,
        &bridge,
        true,
        true
    ));
    for payload in wasmparser::Parser::new(0).parse_all(&target) {
        match payload.unwrap() {
            wasmparser::Payload::ImportSection(reader) => {
                for import in reader.into_imports() {
                    assert_eq!(import.unwrap().module, "ic0");
                }
            }
            wasmparser::Payload::ExportSection(reader) => {
                for export in reader {
                    let export = export.unwrap();
                    assert!(export.name == "memory" || export.name.starts_with("canister_"));
                }
            }
            _ => {}
        }
    }
    for (name, bytes) in [
        ("source", &source),
        ("recovery", &target),
        ("reopen", &reopened),
        ("reverse", &reverse),
    ] {
        fs::write(directory.join(format!("{name}.wasm")), bytes).unwrap();
        println!(
            "map checkpoint {name} sha256={} bytes={}",
            sha256_hex(bytes),
            bytes.len()
        );
    }
    assert_ne!(source, target);
    let instance = PocketIcBuilder::new()
        .with_max_request_time_ms(Some(30_000))
        .with_application_subnet()
        .with_server_url(
            std::env::var("IC_TESTKIT_POCKET_IC_URL")
                .unwrap()
                .parse()
                .unwrap(),
        )
        .build_async()
        .await;
    let pic = &instance;
    let empty = pic.create_canister().await;
    pic.install_canister(empty, source.clone(), vec![], None)
        .await;
    pic.update_call(empty, empty, "export_checkpoint", args(0, 0))
        .await
        .unwrap();
    read_tests::check(pic, empty).await;
    refused_update(pic, empty, "scan_past_end", vec![]).await;
    replace_heap(pic, empty, &target, args(0, 0)).await.unwrap();
    assert!(records(&query(pic, empty, "observe").await, 0).is_empty());
    update(pic, empty, "full_width").await;

    let id = pic.create_canister().await;
    pic.install_canister(id, source.clone(), vec![], None).await;
    update(pic, id, "fill_checkpoint").await;
    update(pic, id, "full_width").await;
    let observed = query(pic, id, "observe").await;
    let expected = [records(&observed, 0), records(&observed, 1)];
    assert_eq!((expected[0].len(), expected[1].len()), (64, 64));
    assert_eq!(expected[0].get(&u64::MAX), Some(&(u64::MAX - 1)));
    let error = pic
        .query_call(id, id, "scan_records", vec![])
        .await
        .unwrap_err();
    assert_eq!(error.error_code, ErrorCode::CanisterTrapped);
    update(pic, id, "grow_a").await;
    refused_update(pic, id, "export_checkpoint", args(17, 0)).await;
    update(pic, id, "remove_growth_a").await;
    refused_update(pic, id, "export_checkpoint", args(17, 1)).await;
    pic.update_call(id, id, "export_checkpoint", args(17, 0))
        .await
        .unwrap();
    let saved = query(pic, id, "checkpoint_bytes").await;
    assert_eq!(checkpoint(&saved, 17), expected);
    read_tests::check(pic, id).await;
    assert!(query(pic, id, "scan_restart").await.is_empty());
    for method in [
        "scan_unstarted",
        "scan_wrong_root",
        "scan_invalid_root",
        "scan_value_first",
        "scan_invalid_field",
        "scan_skip",
        "scan_repeat_key",
        "scan_repeat_record",
        "scan_past_end",
    ] {
        refused_update(pic, id, method, vec![]).await;
    }
    read_tests::check(pic, id).await;
    refused_update(pic, id, "insert_a", vec![]).await;
    refused_recovery(pic, id, &target, 17, 0, true).await;
    refused_recovery(pic, id, &target, 18, 0, false).await;
    for fault in 1..=4 {
        refused_recovery(pic, id, &target, 17, fault, false).await;
    }
    let frozen = query(pic, id, "observe").await;
    pic.upgrade_eop_canister(id, source.clone(), vec![], None)
        .await
        .unwrap();
    assert_eq!(query(pic, id, "observe").await, frozen);
    assert_eq!(query(pic, id, "checkpoint_bytes").await, saved);
    read_tests::check(pic, id).await;
    replace_heap(pic, id, &target, args(17, 0)).await.unwrap();
    let recovered = query(pic, id, "observe").await;
    assert_eq!(word(&recovered, 40), 163840);
    for (index, expected) in expected.iter().enumerate() {
        assert_eq!(&records(&recovered, index), expected);
    }
    assert!(
        query(pic, id, "discarded_payload")
            .await
            .iter()
            .all(|byte| *byte == 0)
    );
    assert_eq!(query(pic, id, "checkpoint_bytes").await, saved);
    pic.upgrade_eop_canister(id, reopened, vec![], None)
        .await
        .unwrap();
    assert_eq!(query(pic, id, "observe").await, recovered);
    update(pic, id, "lookup_full_a").await;
    update(pic, id, "lookup_full_b").await;
    update(pic, id, "delete_full_a").await;
    update(pic, id, "grow_a").await;
    let changed = query(pic, id, "observe").await;
    assert_eq!(records(&changed, 1), expected[1]);
    pic.update_call(id, id, "export_checkpoint", args(18, 0))
        .await
        .unwrap();
    replace_heap(pic, id, &reverse, args(18, 0)).await.unwrap();
    for index in 0..2 {
        assert_eq!(
            records(&query(pic, id, "observe").await, index),
            records(&changed, index)
        );
    }
    assert!(
        query(pic, id, "discarded_payload")
            .await
            .iter()
            .all(|byte| *byte == 0)
    );

    for mode in 0..3 {
        let damaged = pic.create_canister().await;
        pic.install_canister(damaged, source.clone(), vec![], None)
            .await;
        update(pic, damaged, "full_width").await;
        pic.update_call(damaged, damaged, "export_checkpoint", args(17, 0))
            .await
            .unwrap();
        pic.update_call(damaged, damaged, "damage_checkpoint", args(mode, 0))
            .await
            .unwrap();
        refused_recovery(pic, damaged, &target, 17, 0, false).await;
    }
    let missing = pic.create_canister().await;
    pic.install_canister(
        missing,
        link::prepare(&raw, link::Profile::A).unwrap(),
        vec![],
        None,
    )
    .await;
    refused_recovery(pic, missing, &target, 17, 0, false).await;
    let sparse = pic.create_canister().await;
    pic.install_canister(sparse, source.clone(), vec![], None)
        .await;
    update(pic, sparse, "fill_checkpoint").await;
    update(pic, sparse, "full_width").await;
    update(pic, sparse, "sparsify_checkpoint").await;
    let native = query(pic, sparse, "observe").await;
    let expected = [records(&native, 0), records(&native, 1)];
    assert_eq!((expected[0].len(), expected[1].len()), (2, 2));
    pic.update_call(sparse, sparse, "export_checkpoint", args(20, 0))
        .await
        .unwrap();
    read_tests::check(pic, sparse).await;
    assert_eq!(
        checkpoint(&query(pic, sparse, "checkpoint_bytes").await, 20),
        expected
    );
    replace_heap(pic, sparse, &target, args(20, 0))
        .await
        .unwrap();
    let recovered = query(pic, sparse, "observe").await;
    assert_eq!([records(&recovered, 0), records(&recovered, 1)], expected);

    let lost = pic.create_canister().await;
    pic.install_canister(lost, source, vec![], None).await;
    update(pic, lost, "full_width").await;
    let before_loss = query(pic, lost, "observe").await;
    pic.update_call(lost, lost, "export_checkpoint", args(19, 0))
        .await
        .unwrap();
    let independent = query(pic, lost, "checkpoint_bytes").await;
    update(pic, lost, "damage_native").await;
    refused_recovery(pic, lost, &target, 19, 0, false).await;
    refused_recovery_mode(pic, lost, &target, 20, 0, false, true).await;
    replace_heap_with_options(pic, lost, &target, args(19, 0), true, None)
        .await
        .unwrap();
    let rebuilt = query(pic, lost, "observe").await;
    for index in 0..2 {
        assert_eq!(records(&rebuilt, index), records(&before_loss, index));
    }
    assert_eq!(query(pic, lost, "checkpoint_bytes").await, independent);
    instance.drop().await;
    fs::write(
        directory.join("RECOVERED"),
        "two-map governed export, full-width recovery, replacement, refusal and rollback passed\n",
    )
    .unwrap();
}

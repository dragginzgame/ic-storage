//! Installed evidence for the record payload of the shared migration protocol.
use super::super::{Post, prepare_job};
use super::{query, values};
use crate::installed_tests::{stopped_hash, update};
use crate::migration::tests::installed::{replace_heap, replace_heap_with_options};
use ic_storage_retention_probe::artifact::{sha256, sha256_hex};
use ic_testkit::pocket_ic::{CanisterId, ErrorCode, PocketIcBuilder, nonblocking::PocketIc};
use std::fs;

mod history;

const A: u64 = 131072;
const B: u64 = 147456;
fn args(words: &[u64]) -> Vec<u8> {
    words.iter().flat_map(|x| x.to_le_bytes()).collect()
}
fn candidate(relocated: bool, root: (u64, u64), post: Post) -> Vec<u8> {
    let (dir, _) = crate::input();
    let raw = fs::read(dir.join("vector/provider_record_vector.wasm")).unwrap();
    let bridge = fs::read(dir.join("checkpoint-record-vector-job.wasm")).unwrap();
    let bytes = prepare_job(&raw, &bridge, relocated, root, post);
    assert_eq!(bytes, prepare_job(&raw, &bridge, relocated, root, post));
    let hash = sha256_hex(&bytes);
    fs::write(dir.join(format!("record-job-{hash}.wasm")), &bytes).unwrap();
    println!(
        "record job relocated={relocated} root={root:?} post={post:?} bytes={} sha256={hash}",
        bytes.len()
    );
    bytes
}
// Independent envelope and nested logical-field oracle, not the canister codec.
fn envelope(
    bytes: &[u8],
    phase: u16,
    epoch: u64,
    source: u64,
    target: u64,
    progress: Option<[u32; 3]>,
) -> &[u8] {
    assert!((104..=360).contains(&bytes.len()));
    assert_eq!(&bytes[..8], b"ICJRV001");
    assert_eq!(
        &bytes[8..10],
        &(if progress.is_some() { 2u16 } else { 1u16 }).to_le_bytes()
    );
    assert_eq!(&bytes[10..12], &phase.to_le_bytes());
    assert_eq!(&bytes[12..16], &64u32.to_le_bytes());
    assert_eq!(&bytes[16..40], args(&[epoch, source, target]));
    let len = u32::from_le_bytes(bytes[40..44].try_into().unwrap()) as usize;
    assert_eq!(len + 104, bytes.len());
    let fields: Vec<_> = progress
        .unwrap_or([0; 3])
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect();
    assert_eq!(&bytes[44..56], fields);
    assert_eq!(&bytes[56..64], [0; 8]);
    assert_eq!(&bytes[64 + len..72 + len], b"JOBDONE1");
    assert_eq!(&bytes[72 + len..], sha256(&bytes[..72 + len]));
    &bytes[64..64 + len]
}
async fn publish(
    pic: &PocketIc,
    id: CanisterId,
    epoch: u64,
    source: u64,
    target: u64,
) -> Vec<(u32, u64)> {
    let records = values(&query(pic, id, "observe").await);
    call(pic, id, "export_checkpoint", &[epoch, target, 0]).await;
    let bytes = query(pic, id, "checkpoint_bytes").await;
    let nested = envelope(&bytes, 1, epoch, source, target, None);
    assert_eq!(&nested[..16], b"RECVEC01RECU64V1");
    assert_eq!(&nested[16..20], &3u32.to_le_bytes());
    assert_eq!(&nested[20..24], &(records.len() as u32).to_le_bytes());
    assert_eq!(&nested[24..32], &epoch.to_le_bytes());
    let payload: Vec<_> = records
        .iter()
        .flat_map(|(c, l)| c.to_le_bytes().into_iter().chain(l.to_le_bytes()))
        .collect();
    assert_eq!(&nested[32..nested.len() - 32], payload);
    assert_eq!(
        &nested[nested.len() - 32..],
        sha256(&nested[..nested.len() - 32])
    );
    records
}
async fn call(pic: &PocketIc, id: CanisterId, method: &str, words: &[u64]) {
    pic.update_call(id, id, method, args(words)).await.unwrap();
}
async fn refuse(pic: &PocketIc, id: CanisterId, sender: CanisterId, method: &str, words: &[u64]) {
    let before = query(pic, id, "observe").await;
    let stable = pic.get_stable_memory(id).await;
    let error = pic
        .update_call(id, sender, method, args(words))
        .await
        .unwrap_err();
    assert!(
        matches!(
            error.error_code,
            ErrorCode::CanisterCalledTrap | ErrorCode::CanisterTrapped
        ),
        "{error:?}"
    );
    assert_eq!(query(pic, id, "observe").await, before);
    assert_eq!(pic.get_stable_memory(id).await, stable);
}
async fn refuse_upgrade(pic: &PocketIc, id: CanisterId, wasm: &[u8], words: &[u64], keep: bool) {
    let before = query(pic, id, "observe").await;
    let stable = pic.get_stable_memory(id).await;
    let hash = stopped_hash(pic, id).await;
    let result = if keep {
        pic.upgrade_eop_canister(id, wasm.to_vec(), args(words), None)
            .await
    } else {
        replace_heap(pic, id, wasm, args(words)).await
    };
    let error = result.unwrap_err();
    assert!(
        matches!(
            error.error_code,
            ErrorCode::CanisterCalledTrap | ErrorCode::CanisterTrapped
        ),
        "{error:?}"
    );
    assert_eq!(stopped_hash(pic, id).await, hash);
    pic.start_canister(id, None).await.unwrap();
    assert_eq!(query(pic, id, "observe").await, before);
    assert_eq!(pic.get_stable_memory(id).await, stable);
}
async fn install(pic: &PocketIc, root: (u64, u64)) -> CanisterId {
    let id = pic.create_canister().await;
    pic.set_controllers(id, None, vec![CanisterId::anonymous(), id])
        .await
        .unwrap();
    pic.install_canister(id, candidate(false, root, Post::Reopen), vec![], None)
        .await;
    id
}
async fn upgrade(pic: &PocketIc, id: CanisterId, wasm: &[u8], words: &[u64], keep: bool) {
    stopped_hash(pic, id).await;
    if keep {
        pic.upgrade_eop_canister(id, wasm.to_vec(), args(words), None)
            .await
            .unwrap();
    } else {
        replace_heap(pic, id, wasm, args(words)).await.unwrap();
    }
    assert_eq!(stopped_hash(pic, id).await, sha256(wasm));
    pic.start_canister(id, None).await.unwrap();
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "native-record-job-test supplies fresh artifacts and prepared server"]
async fn authority_epochs_consumption_and_atomic_record_recovery() {
    let server = std::env::var("IC_TESTKIT_POCKET_IC_URL").expect(
        "run with the prepared server via make retention-setup and the selected test target",
    );
    let instance = PocketIcBuilder::new()
        .with_application_subnet()
        .with_max_request_time_ms(Some(30_000))
        .with_server_url(server.parse().expect("valid prepared server URL"))
        .build_async()
        .await;
    let pic = &instance;
    let recover = candidate(true, (0, 0), Post::Recover);
    let back = candidate(false, (0, 0), Post::Recover);
    let reopen = candidate(true, (0, 0), Post::Reopen);
    for root in [(0, 0), (0, 8), (3, 4), (16, 16)] {
        let id = install(pic, root).await;
        assert!(envelope(&query(pic, id, "checkpoint_bytes").await, 0, 0, A, A, None).is_empty());
        refuse(
            pic,
            id,
            CanisterId::from_slice(&[42]),
            "export_checkpoint",
            &[1, B, 0],
        )
        .await;
        for a in [[2, B, 0], [1, A, 0], [1, B, 1]] {
            refuse(pic, id, id, "export_checkpoint", &a).await;
        }
        refuse_upgrade(pic, id, &recover, &[1, A, 0], false).await;
        let records = publish(pic, id, 1, A, B).await;
        let stable = pic.get_stable_memory(id).await;
        assert_eq!(publish(pic, id, 1, A, B).await, records);
        assert_eq!(pic.get_stable_memory(id).await, stable);
        refuse(pic, id, id, "vector_mutate", &[]).await;
        refuse_upgrade(pic, id, &recover, &[1, A, 0], true).await;
        for a in [[0, A, 0], [2, A, 0], [1, B, 0], [1, A, 1], [1, A, 3]] {
            refuse_upgrade(pic, id, &recover, &a, false).await;
        }
        if root.0 > 0 {
            refuse_upgrade(pic, id, &recover, &[1, A, 2], false).await;
        }
        upgrade(pic, id, &recover, &[1, A, 0], false).await;
        assert_eq!(values(&query(pic, id, "observe").await), records);
        envelope(&query(pic, id, "checkpoint_bytes").await, 2, 1, A, B, None);
        assert_eq!(query(pic, id, "discarded_payload").await, [0; 256]);
        upgrade(pic, id, &reopen, &[], true).await;
        refuse_upgrade(pic, id, &recover, &[1, A, 0], false).await;
        refuse(pic, id, id, "cancel_checkpoint", &[1, B, 0]).await;
        update(pic, id, "vector_reuse").await;
        pic.set_controllers(id, None, vec![CanisterId::anonymous()])
            .await
            .unwrap();
        refuse(pic, id, id, "export_checkpoint", &[2, A, 0]).await;
        pic.set_controllers(id, None, vec![CanisterId::anonymous(), id])
            .await
            .unwrap();
        for epoch in [1, 3] {
            refuse(pic, id, id, "export_checkpoint", &[epoch, A, 0]).await;
        }
        let next = publish(pic, id, 2, B, A).await;
        upgrade(pic, id, &back, &[2, B, 0], false).await;
        assert_eq!(values(&query(pic, id, "observe").await), next);
    }
    instance.drop().await;
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "native-record-job-test supplies fresh artifacts and prepared server"]
async fn frozen_resume_cancellation_and_record_mismatches() {
    let server = std::env::var("IC_TESTKIT_POCKET_IC_URL").expect(
        "run with the prepared server via make retention-setup and the selected test target",
    );
    let instance = PocketIcBuilder::new()
        .with_application_subnet()
        .with_max_request_time_ms(Some(30_000))
        .with_server_url(server.parse().expect("valid prepared server URL"))
        .build_async()
        .await;
    let pic = &instance;
    let frozen = candidate(false, (0, 0), Post::Frozen);
    let reopen = candidate(false, (0, 0), Post::Reopen);
    let recover = candidate(true, (0, 0), Post::Recover);
    let id = install(pic, (3, 4)).await;
    let expected = publish(pic, id, 1, A, B).await;
    refuse_upgrade(pic, id, &reopen, &[], true).await;
    refuse_upgrade(pic, id, &frozen, &[1, B, 1], true).await;
    upgrade(pic, id, &frozen, &[1, B, 0], true).await;
    assert_eq!(publish(pic, id, 1, A, B).await, expected);
    for fault in [1, 2] {
        refuse(pic, id, id, "cancel_checkpoint", &[1, B, fault]).await;
    }
    refuse(
        pic,
        id,
        CanisterId::from_slice(&[42]),
        "cancel_checkpoint",
        &[1, B, 0],
    )
    .await;
    call(pic, id, "cancel_checkpoint", &[1, B, 0]).await;
    envelope(&query(pic, id, "checkpoint_bytes").await, 3, 1, A, B, None);
    update(pic, id, "vector_mutate").await;
    let newer = query(pic, id, "observe").await;
    call(pic, id, "cancel_checkpoint", &[1, B, 0]).await;
    assert_eq!(query(pic, id, "observe").await, newer);
    refuse_upgrade(pic, id, &recover, &[1, A, 0], false).await;
    refuse(pic, id, id, "export_checkpoint", &[1, B, 0]).await;
    let next = publish(pic, id, 2, A, B).await;
    upgrade(pic, id, &recover, &[2, A, 0], false).await;
    assert_eq!(values(&query(pic, id, "observe").await), next);
    for mode in 0..=3 {
        let id = install(pic, (3, 4)).await;
        publish(pic, id, 1, A, B).await;
        call(
            pic,
            id,
            "damage_checkpoint",
            &if mode == 0 { [0, 0, 0] } else { [mode, 1, B] },
        )
        .await;
        refuse_upgrade(pic, id, &frozen, &[1, B, 0], true).await;
        if mode == 0 {
            refuse_upgrade(pic, id, &recover, &[1, A, 0], false).await;
        }
    }
    instance.drop().await;
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "native-record-job-test supplies fresh artifacts and prepared server"]
async fn staged_records_resume_retry_restart_and_publish_once() {
    let server = std::env::var("IC_TESTKIT_POCKET_IC_URL").expect(
        "run with the prepared server via make retention-setup and the selected test target",
    );
    let instance = PocketIcBuilder::new()
        .with_application_subnet()
        .with_max_request_time_ms(Some(30_000))
        .with_server_url(server.parse().expect("valid prepared server URL"))
        .build_async()
        .await;
    let pic = &instance;
    let start = candidate(true, (0, 0), Post::Stage);
    let resume = candidate(true, (0, 0), Post::Importing);
    let restart = candidate(true, (0, 0), Post::Restart);
    for root in [(0, 0), (3, 4), (16, 16)] {
        let id = install(pic, root).await;
        let expected = publish(pic, id, 1, A, B).await;
        refuse_upgrade(pic, id, &restart, &[1, A, 0], false).await;
        refuse_upgrade(pic, id, &start, &[1, A, 1], false).await;
        upgrade(pic, id, &start, &[1, A, 0], false).await;
        envelope(
            &query(pic, id, "checkpoint_bytes").await,
            4,
            1,
            A,
            B,
            Some([0, 0, 0]),
        );
        refuse(pic, id, id, "vector_mutate", &[]).await;
        refuse(
            pic,
            id,
            CanisterId::from_slice(&[42]),
            "import_batch",
            &[1, A, 0, 2, 0],
        )
        .await;
        pic.set_controllers(id, None, vec![CanisterId::anonymous()])
            .await
            .unwrap();
        refuse(pic, id, id, "import_batch", &[1, A, 0, 2, 0]).await;
        pic.set_controllers(id, None, vec![CanisterId::anonymous(), id])
            .await
            .unwrap();
        let mut cursor = 0;
        loop {
            for fault in [1, 3] {
                refuse(pic, id, id, "import_batch", &[1, A, cursor, 2, fault]).await;
            }
            if cursor < (expected.len() as u64) {
                refuse(pic, id, id, "import_batch", &[1, A, cursor, 2, 2]).await;
            }
            call(pic, id, "import_batch", &[1, A, cursor, 2, 0]).await;
            let end = (cursor + 2).min(expected.len() as u64);
            let complete = end == expected.len() as u64;
            envelope(
                &query(pic, id, "checkpoint_bytes").await,
                if complete { 2 } else { 4 },
                1,
                A,
                B,
                Some([end as u32, cursor as u32, 2]),
            );
            let stable = pic.get_stable_memory(id).await;
            call(pic, id, "import_batch", &[1, A, cursor, 2, 0]).await;
            assert_eq!(pic.get_stable_memory(id).await, stable);
            refuse(pic, id, id, "import_batch", &[1, A, cursor, 1, 0]).await;
            if complete {
                break;
            }
            refuse_upgrade(pic, id, &resume, &[1, A, 1], true).await;
            upgrade(pic, id, &resume, &[1, A, 0], true).await;
            cursor = end;
        }
        assert_eq!(values(&query(pic, id, "observe").await), expected);
        update(pic, id, "vector_reuse").await;
        let newer = query(pic, id, "observe").await;
        call(pic, id, "import_batch", &[1, A, cursor, 2, 0]).await;
        assert_eq!(query(pic, id, "observe").await, newer);
        refuse_upgrade(pic, id, &restart, &[1, A, 0], false).await;
    }
    // Incomplete prefixes are disposable only through explicit fresh-heap restart.
    for mode in 0..=4 {
        let id = install(pic, (3, 4)).await;
        let expected = publish(pic, id, 1, A, B).await;
        upgrade(pic, id, &start, &[1, A, 0], false).await;
        call(pic, id, "import_batch", &[1, A, 0, 2, 0]).await;
        call(pic, id, "damage_import", &[mode, 0, 0]).await;
        refuse(pic, id, id, "import_batch", &[1, A, 2, 2, 0]).await;
        refuse_upgrade(pic, id, &resume, &[1, A, 0], true).await;
        if mode == 4 {
            refuse_upgrade(pic, id, &restart, &[1, A, 0], false).await;
            continue;
        }
        stopped_hash(pic, id).await;
        replace_heap_with_options(pic, id, &restart, args(&[1, A, 0]), true, None)
            .await
            .unwrap();
        pic.start_canister(id, None).await.unwrap();
        envelope(
            &query(pic, id, "checkpoint_bytes").await,
            4,
            1,
            A,
            B,
            Some([0, 0, 0]),
        );
        call(pic, id, "import_batch", &[1, A, 0, 16, 0]).await;
        assert_eq!(values(&query(pic, id, "observe").await), expected);
    }
    instance.drop().await;
}

#[test]
#[ignore = "native-record-job-test supplies fresh compiler artifacts"]
fn job_artifact_keeps_callbacks_private_and_rejects_v2_native_code() {
    use wasmparser::{Parser, Payload};
    let bytes = candidate(true, (0, 0), Post::Importing);
    for payload in Parser::new(0).parse_all(&bytes) {
        match payload.unwrap() {
            Payload::ImportSection(imports) => {
                for import in imports.into_imports() {
                    assert_eq!(import.unwrap().module, "ic0");
                }
            }
            Payload::ExportSection(exports) => {
                for export in exports {
                    let name = export.unwrap().name;
                    assert!(name == "memory" || name.starts_with("canister_"));
                }
            }
            _ => {}
        }
    }
    let (dir, _) = crate::input();
    let v2 = fs::read(dir.join("vector/v2/provider_record_vector.wasm")).unwrap();
    let bridge = fs::read(dir.join("checkpoint-record-vector-job.wasm")).unwrap();
    assert!(
        std::panic::catch_unwind(|| prepare_job(&v2, &bridge, false, (0, 0), Post::Reopen))
            .is_err()
    );
}

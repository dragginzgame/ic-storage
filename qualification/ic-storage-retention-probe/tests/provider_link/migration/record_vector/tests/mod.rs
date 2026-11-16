use super::*;
use crate::installed_tests::{stopped_hash, update};
use crate::migration::tests::installed::{replace_heap, replace_heap_with_options};
use ic_storage_retention_probe::artifact::{sha256, sha256_hex};
use ic_testkit::{
    pic::PocketIcStartupConfig,
    pocket_ic::{CanisterId, ErrorCode, PocketIcBuilder, nonblocking::PocketIc},
};
use std::{fs, path::PathBuf, time::Duration};
use wasmparser::{Operator, Parser, Payload};

mod job;
mod narrow;
mod retained;

fn inputs() -> (PathBuf, Vec<u8>, Vec<u8>) {
    let (directory, _) = crate::input();
    let raw = fs::read(directory.join("vector/provider_record_vector.wasm")).unwrap();
    let bridge = fs::read(directory.join("checkpoint-record-vector.wasm")).unwrap();
    (directory, raw, bridge)
}

fn candidate(
    directory: &std::path::Path,
    raw: &[u8],
    bridge: &[u8],
    relocated: bool,
    root: (u64, u64),
    post: Post,
) -> Vec<u8> {
    let bytes = prepare(raw, bridge, relocated, root, post);
    assert_eq!(bytes, prepare(raw, bridge, relocated, root, post));
    let digest = sha256_hex(&bytes);
    fs::write(
        directory.join(format!("record-recovery-{digest}.wasm")),
        &bytes,
    )
    .unwrap();
    println!(
        "record vector recovery relocated={relocated} root={root:?} post={post:?} bytes={} sha256={digest}",
        bytes.len()
    );
    bytes
}

fn args(epoch: u64, fault: u64) -> Vec<u8> {
    [epoch, fault]
        .into_iter()
        .flat_map(u64::to_le_bytes)
        .collect()
}

async fn query(pic: &PocketIc, id: CanisterId, name: &str) -> Vec<u8> {
    pic.query_call(id, id, name, vec![]).await.unwrap()
}

fn values(observation: &[u8]) -> Vec<(u32, u64)> {
    assert_eq!(observation.len(), 416);
    let count = u64::from_le_bytes(observation[48..56].try_into().unwrap()) as usize;
    assert!(count <= 16);
    observation[160..160 + count * 16]
        .as_chunks::<16>()
        .0
        .iter()
        .map(|record| {
            (
                u32::from_le_bytes(record[..4].try_into().unwrap()),
                u64::from_le_bytes(record[8..].try_into().unwrap()),
            )
        })
        .collect()
}

async fn refused_update(pic: &PocketIc, id: CanisterId, method: &str, arguments: Vec<u8>) {
    let explicit_fault = method == "export_checkpoint" && arguments == args(7, 1);
    let before = query(pic, id, "observe").await;
    let stable = pic.get_stable_memory(id).await;
    let error = pic
        .update_call(id, id, method, arguments)
        .await
        .unwrap_err();
    if explicit_fault {
        assert_eq!(error.error_code, ErrorCode::CanisterCalledTrap);
        assert!(
            error
                .reject_message
                .contains("record vector export after stable write")
        );
    } else {
        assert_eq!(error.error_code, ErrorCode::CanisterTrapped);
    }
    assert_eq!(query(pic, id, "observe").await, before);
    assert_eq!(pic.get_stable_memory(id).await, stable);
}

async fn refused_upgrade(
    pic: &PocketIc,
    id: CanisterId,
    target: &[u8],
    arguments: Vec<u8>,
    keep: bool,
) {
    let fault = arguments
        .get(8..16)
        .map(|bytes| u64::from_le_bytes(bytes.try_into().unwrap()))
        .unwrap_or(0);
    let before = query(pic, id, "observe").await;
    let stable = pic.get_stable_memory(id).await;
    let hash = stopped_hash(pic, id).await;
    let result = if keep {
        pic.upgrade_eop_canister(id, target.to_vec(), arguments, None)
            .await
    } else {
        replace_heap(pic, id, target, arguments).await
    };
    let error = result.unwrap_err();
    if !keep && fault != 0 {
        assert_eq!(error.error_code, ErrorCode::CanisterCalledTrap);
        let message = match fault {
            1 => "record vector restore after stable write",
            2 => "record vector restore during construction",
            3 => "record vector restore after publication",
            _ => panic!("unknown test failure"),
        };
        assert!(error.reject_message.contains(message));
    } else {
        assert_eq!(error.error_code, ErrorCode::CanisterTrapped);
    }
    assert_eq!(stopped_hash(pic, id).await, hash);
    pic.start_canister(id, None).await.unwrap();
    assert_eq!(query(pic, id, "observe").await, before);
    assert_eq!(pic.get_stable_memory(id).await, stable);
}

async fn export(pic: &PocketIc, id: CanisterId, epoch: u64) -> Vec<(u32, u64)> {
    let expected = values(&query(pic, id, "observe").await);
    pic.update_call(id, id, "export_checkpoint", args(epoch, 0))
        .await
        .unwrap();
    let bytes = query(pic, id, "checkpoint_bytes").await;
    assert_eq!(bytes.len(), 64 + expected.len() * 12);
    assert_eq!(&bytes[..16], b"RECVEC01RECU64V1");
    assert_eq!(&bytes[16..20], &3u32.to_le_bytes());
    assert_eq!(&bytes[20..24], &(expected.len() as u32).to_le_bytes());
    assert_eq!(&bytes[24..32], &epoch.to_le_bytes());
    let payload: Vec<_> = expected
        .iter()
        .flat_map(|(counter, ceiling)| {
            counter
                .to_le_bytes()
                .into_iter()
                .chain(ceiling.to_le_bytes())
        })
        .collect();
    assert_eq!(&bytes[32..bytes.len() - 32], payload);
    assert_eq!(
        &bytes[bytes.len() - 32..],
        &sha256(&bytes[..bytes.len() - 32])
    );
    expected
}

#[test]
#[ignore = "native-record-vector-test supplies fresh compiler artifacts"]
fn record_recovery_artifact_resolves_callbacks_and_orders_freshness_before_rust() {
    let (directory, raw, bridge) = inputs();
    let bytes = candidate(&directory, &raw, &bridge, true, (0, 0), Post::Recover);
    assert_ne!(bytes, prepare(&raw, &bridge, false, (0, 0), Post::Recover));
    let mut exports = std::collections::BTreeMap::new();
    let mut bodies = Vec::new();
    let mut imports = 0;
    for payload in Parser::new(0).parse_all(&bytes) {
        match payload.unwrap() {
            Payload::ImportSection(r) => {
                for item in r.into_imports() {
                    assert_eq!(item.unwrap().module, "ic0");
                    imports += 1;
                }
            }
            Payload::ExportSection(r) => {
                for item in r {
                    let item = item.unwrap();
                    assert!(item.name == "memory" || item.name.starts_with("canister_"));
                    exports.insert(item.name, item.index);
                }
            }
            Payload::CodeSectionEntry(body) => bodies.push(body),
            _ => {}
        }
    }
    let calls = |index: u32| {
        bodies[(index - imports) as usize]
            .get_operators_reader()
            .unwrap()
            .into_iter()
            .filter_map(|op| {
                if let Operator::Call { function_index } = op.unwrap() {
                    Some(function_index)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
    };
    let init = calls(exports["canister_init"]);
    let post = calls(exports["canister_post_upgrade"]);
    assert_eq!((init.len(), post.len()), (2, 3));
    assert!(post[0] > post[1] && post[2] > post[1]);
    assert_ne!(init[1], post[1]);
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "native-record-vector-test supplies fresh artifacts and prepared server"]
async fn installed_record_vectors_recover_in_a_new_domain_and_continue_allocating() {
    let (directory, raw, bridge) = inputs();
    let owner = PocketIcStartupConfig::from_env(Duration::from_secs(30))
        .unwrap()
        .build_instance(
            PocketIcBuilder::new()
                .with_application_subnet()
                .with_max_request_time_ms(Some(30_000)),
        )
        .await
        .unwrap();
    let pic = owner.instance();
    let reopen_b = candidate(&directory, &raw, &bridge, true, (0, 0), Post::Reopen);
    let recover_b = candidate(&directory, &raw, &bridge, true, (0, 0), Post::Recover);
    let recover_a = candidate(&directory, &raw, &bridge, false, (0, 0), Post::Recover);
    for root in [(0, 0), (0, 8), (3, 4), (16, 16)] {
        let source = candidate(&directory, &raw, &bridge, false, root, Post::Reopen);
        let id = pic.create_canister().await;
        pic.install_canister(id, source.clone(), vec![], None).await;
        // Normal compatible reopening rebuilds the transient checkpoint runtime.
        pic.upgrade_eop_canister(id, source, vec![], None)
            .await
            .unwrap();
        pic.start_canister(id, None).await.unwrap();
        update(pic, id, "vector_mutate").await;
        refused_upgrade(pic, id, &reopen_b, vec![], true).await;
        refused_upgrade(pic, id, &recover_b, args(7, 0), false).await; // no complete checkpoint
        refused_update(pic, id, "export_checkpoint", args(7, 1)).await;
        let expected = export(pic, id, 7).await;
        let stable = pic.get_stable_memory(id).await;
        assert_eq!(export(pic, id, 7).await, expected);
        assert_eq!(pic.get_stable_memory(id).await, stable);
        for method in ["vector_mutate", "vector_append", "vector_clear"] {
            refused_update(pic, id, method, vec![]).await;
        }
        refused_upgrade(pic, id, &recover_b, args(7, 0), true).await;
        for (epoch, fault) in [(8, 0), (7, 1), (7, 3)] {
            refused_upgrade(pic, id, &recover_b, args(epoch, fault), false).await;
        }
        if root.0 != 0 {
            refused_upgrade(pic, id, &recover_b, args(7, 2), false).await;
        }
        stopped_hash(pic, id).await;
        replace_heap(pic, id, &recover_b, args(7, 0)).await.unwrap();
        assert_eq!(stopped_hash(pic, id).await, sha256(&recover_b));
        pic.start_canister(id, None).await.unwrap();
        assert_eq!(pic.get_stable_memory(id).await, stable);
        let recovered = query(pic, id, "observe").await;
        assert_eq!(values(&recovered), expected);
        assert_eq!(&recovered[32..40], &147456u64.to_le_bytes());
        assert_eq!(query(pic, id, "discarded_payload").await, [0; 256]);
        pic.upgrade_eop_canister(id, reopen_b.clone(), vec![], None)
            .await
            .unwrap();
        pic.start_canister(id, None).await.unwrap();
        update(pic, id, "vector_reuse").await;
        update(pic, id, "vector_append").await;
        update(pic, id, "vector_mutate").await;
        update(pic, id, "vector_shrink").await;
        assert_eq!(
            values(&query(pic, id, "observe").await),
            [(2, 1000), (1, 1000)]
        );
        let next = export(pic, id, 8).await;
        if root.0 == 3 {
            // Explicit skip of the unusable old hook is permitted only after
            // this checkpoint exists; the new reader never consults old bytes.
            update(pic, id, "damage_frozen").await;
            refused_update(pic, id, "export_checkpoint", args(8, 0)).await;
            refused_upgrade(pic, id, &recover_a, args(8, 0), false).await;
            stopped_hash(pic, id).await;
            replace_heap_with_options(pic, id, &recover_a, args(8, 0), true, None)
                .await
                .unwrap();
        } else {
            stopped_hash(pic, id).await;
            replace_heap(pic, id, &recover_a, args(8, 0)).await.unwrap();
        }
        pic.start_canister(id, None).await.unwrap();
        assert_eq!(values(&query(pic, id, "observe").await), next);
        assert_eq!(query(pic, id, "discarded_payload").await, [0; 256]);
        update(pic, id, "vector_clear").await;
        update(pic, id, "vector_append").await;
        assert_eq!(values(&query(pic, id, "observe").await), [(1, 1000)]);
    }
    owner.close().await;
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "native-record-vector-test supplies fresh artifacts and prepared server"]
async fn installed_corrupt_record_checkpoints_refuse_without_reset_or_publication() {
    let (directory, raw, bridge) = inputs();
    let source = candidate(&directory, &raw, &bridge, false, (3, 4), Post::Reopen);
    let recovery = candidate(&directory, &raw, &bridge, true, (0, 0), Post::Recover);
    let owner = PocketIcStartupConfig::from_env(Duration::from_secs(30))
        .unwrap()
        .build_instance(
            PocketIcBuilder::new()
                .with_application_subnet()
                .with_max_request_time_ms(Some(30_000)),
        )
        .await
        .unwrap();
    let pic = owner.instance();
    for damage in 0..3 {
        let id = pic.create_canister().await;
        pic.install_canister(id, source.clone(), vec![], None).await;
        export(pic, id, 7).await;
        pic.update_call(id, id, "damage_checkpoint", args(damage, 0))
            .await
            .unwrap();
        refused_upgrade(pic, id, &recovery, args(7, 0), false).await;
        // The compatible frozen source can repair its logical checkpoint. The
        // failed destination did not clear the locator or accept a partial root.
        let expected = export(pic, id, 7).await;
        stopped_hash(pic, id).await;
        replace_heap(pic, id, &recovery, args(7, 0)).await.unwrap();
        pic.start_canister(id, None).await.unwrap();
        assert_eq!(values(&query(pic, id, "observe").await), expected);
    }
    owner.close().await;
}

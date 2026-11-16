//! Explicit logical V3-to-V2 recovery, without interpreting the discarded heap.
use super::retained::{save, v2_inputs, v2_values};
use super::*;

fn post_calls(bytes: &[u8]) -> Vec<u32> {
    let mut imports = 0;
    let mut post = None;
    let mut bodies = Vec::new();
    for payload in Parser::new(0).parse_all(bytes) {
        match payload.unwrap() {
            Payload::ImportSection(r) => imports = r.into_imports().count(),
            Payload::ExportSection(r) => {
                for item in r {
                    let item = item.unwrap();
                    assert!(item.name == "memory" || item.name.starts_with("canister_"));
                    if item.name == "canister_post_upgrade" {
                        post = Some(item.index);
                    }
                }
            }
            Payload::CodeSectionEntry(body) => bodies.push(body),
            _ => {}
        }
    }
    bodies[post.unwrap() as usize - imports]
        .get_operators_reader()
        .unwrap()
        .into_iter()
        .filter_map(|op| match op.unwrap() {
            Operator::Call { function_index } => Some(function_index),
            _ => None,
        })
        .collect()
}

#[test]
#[ignore = "native-record-vector-test supplies V2 and V3 compiler artifacts"]
fn widening_artifact_requires_v2_and_keeps_freshness_before_conversion() {
    let (directory, raw, bridge) = inputs();
    let (v2, v2_bridge, _) = v2_inputs(&directory);
    let exact = prepare_v2(&v2, &v2_bridge, true, (0, 0), Post::Recover);
    let widen = save(
        &directory,
        prepare_v2(&v2, &v2_bridge, true, (0, 0), Post::WidenVector),
    );
    let a = post_calls(&exact);
    let b = post_calls(&widen);
    assert_eq!((a.len(), b.len()), (3, 3));
    assert_eq!((a[0], a[2]), (b[0], b[2]));
    assert!(b[0] > b[1] && b[2] > b[1]);
    assert_ne!(a[1], b[1]);
    assert!(
        std::panic::catch_unwind(|| prepare(&raw, &bridge, true, (0, 0), Post::WidenVector))
            .is_err()
    );
    assert!(
        std::panic::catch_unwind(|| prepare_v2(&raw, &v2_bridge, true, (0, 0), Post::WidenVector))
            .is_err()
    );
    assert!(
        std::panic::catch_unwind(|| prepare_v2(&v2, &v2_bridge, true, (1, 1), Post::WidenVector))
            .is_err()
    );
}

async fn export_v2(pic: &PocketIc, id: CanisterId, epoch: u64) -> Vec<(u64, u64)> {
    let expected = v2_values(&query(pic, id, "observe").await);
    pic.update_call(id, id, "export_checkpoint", args(epoch, 0))
        .await
        .unwrap();
    let bytes = query(pic, id, "checkpoint_bytes").await;
    assert_eq!(bytes.len(), 64 + expected.len() * 16);
    assert_eq!(&bytes[..16], b"RECVEC01RECU64V1");
    assert_eq!(&bytes[16..20], &2u32.to_le_bytes());
    assert_eq!(&bytes[20..24], &(expected.len() as u32).to_le_bytes());
    assert_eq!(&bytes[24..32], &epoch.to_le_bytes());
    let payload: Vec<_> = expected
        .iter()
        .flat_map(|(a, b)| a.to_le_bytes().into_iter().chain(b.to_le_bytes()))
        .collect();
    assert_eq!(&bytes[32..bytes.len() - 32], payload);
    assert_eq!(
        &bytes[bytes.len() - 32..],
        &sha256(&bytes[..bytes.len() - 32])
    );
    expected
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "native-record-vector-test supplies artifacts and prepared server"]
async fn installed_portable_widening_preserves_records_then_accepts_full_width_updates() {
    let (directory, raw, bridge) = inputs();
    let (v2, v2_bridge, _) = v2_inputs(&directory);
    let server = std::env::var("IC_TESTKIT_POCKET_IC_URL").expect("prepared server");
    let instance = PocketIcBuilder::new()
        .with_application_subnet()
        .with_max_request_time_ms(Some(30_000))
        .with_server_url(server.parse().unwrap())
        .build_async()
        .await;
    let pic = &instance;
    for relocated in [false, true] {
        let widen = save(
            &directory,
            prepare_v2(&v2, &v2_bridge, relocated, (0, 0), Post::WidenVector),
        );
        let exact = prepare_v2(&v2, &v2_bridge, !relocated, (0, 0), Post::Recover);
        let reopen = prepare_v2(&v2, &v2_bridge, relocated, (0, 0), Post::Reopen);
        let v3_recovery = prepare(&raw, &bridge, !relocated, (0, 0), Post::Recover);
        for root in [(0, 0), (0, 8), (3, 4), (16, 16)] {
            let source = save(
                &directory,
                prepare_schema(
                    &raw,
                    &bridge,
                    false,
                    link::Profile {
                        root,
                        ..link::Profile::A
                    },
                    Post::Reopen,
                    Mode::V3,
                    (root.0 == 16).then_some((0, u64::from(u32::MAX))),
                ),
            );
            let id = pic.create_canister().await;
            pic.install_canister(id, source, vec![], None).await;
            refused_upgrade(pic, id, &widen, args(7, 0), false).await;
            let mut expected: Vec<_> = export(pic, id, 7)
                .await
                .into_iter()
                .map(|(a, b)| (u64::from(a), b))
                .collect();
            let checkpoint = query(pic, id, "checkpoint_bytes").await;
            let stable = pic.get_stable_memory(id).await;
            refused_upgrade(pic, id, &exact, args(7, 0), false).await;
            refused_upgrade(pic, id, &widen, args(7, 0), true).await;
            for (epoch, fault) in [(8, 0), (7, 1), (7, 3)] {
                refused_upgrade(pic, id, &widen, args(epoch, fault), false).await;
            }
            if root.0 != 0 {
                refused_upgrade(pic, id, &widen, args(7, 2), false).await;
            }
            stopped_hash(pic, id).await;
            replace_heap(pic, id, &widen, args(7, 0)).await.unwrap();
            assert_eq!(stopped_hash(pic, id).await, sha256(&widen));
            pic.start_canister(id, None).await.unwrap();
            assert_eq!(v2_values(&query(pic, id, "observe").await), expected);
            assert_eq!(query(pic, id, "checkpoint_bytes").await, checkpoint);
            assert_eq!(pic.get_stable_memory(id).await, stable);
            assert_eq!(query(pic, id, "discarded_payload").await, [0; 256]);
            pic.upgrade_eop_canister(id, reopen.clone(), vec![], None)
                .await
                .unwrap();
            pic.start_canister(id, None).await.unwrap();
            update(pic, id, "vector_mutate").await;
            if let Some(first) = expected.first_mut() {
                first.0 += 1;
            }
            assert_eq!(v2_values(&query(pic, id, "observe").await), expected);
            if root.0 == 16 {
                assert_eq!(expected[0], (u64::from(u32::MAX) + 1, u64::MAX));
            }
            refused_update(pic, id, "record_mutate_then_fail", vec![]).await;
            assert_eq!(export_v2(pic, id, 8).await, expected);
            refused_upgrade(pic, id, &widen, args(8, 0), false).await;
            refused_upgrade(pic, id, &v3_recovery, args(8, 0), false).await;
            stopped_hash(pic, id).await;
            replace_heap(pic, id, &exact, args(8, 0)).await.unwrap();
            pic.start_canister(id, None).await.unwrap();
            assert_eq!(v2_values(&query(pic, id, "observe").await), expected);
            update(pic, id, "vector_reuse").await;
            update(pic, id, "vector_append").await;
            update(pic, id, "vector_shrink").await;
            assert_eq!(
                v2_values(&query(pic, id, "observe").await),
                [(1, 1000), (1, 1000)]
            );
        }
    }
    instance.drop().await;
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "native-record-vector-test supplies artifacts and prepared server"]
async fn widening_rejects_corrupt_checkpoints_and_can_recover_after_source_heap_damage() {
    let (directory, raw, bridge) = inputs();
    let (v2, v2_bridge, _) = v2_inputs(&directory);
    let source = prepare(&raw, &bridge, false, (3, 4), Post::Reopen);
    let widen = save(
        &directory,
        prepare_v2(&v2, &v2_bridge, true, (0, 0), Post::WidenVector),
    );
    let server = std::env::var("IC_TESTKIT_POCKET_IC_URL").expect("prepared server");
    let instance = PocketIcBuilder::new()
        .with_application_subnet()
        .with_max_request_time_ms(Some(30_000))
        .with_server_url(server.parse().unwrap())
        .build_async()
        .await;
    let pic = &instance;
    for damage in 0..3 {
        let id = pic.create_canister().await;
        pic.install_canister(id, source.clone(), vec![], None).await;
        export(pic, id, 7).await;
        pic.update_call(id, id, "damage_checkpoint", args(damage, 0))
            .await
            .unwrap();
        refused_upgrade(pic, id, &widen, args(7, 0), false).await;
        let expected: Vec<_> = export(pic, id, 7)
            .await
            .into_iter()
            .map(|(a, b)| (u64::from(a), b))
            .collect();
        let stable = pic.get_stable_memory(id).await;
        update(pic, id, "damage_frozen").await;
        refused_update(pic, id, "export_checkpoint", args(7, 0)).await;
        refused_upgrade(pic, id, &widen, args(7, 0), false).await;
        stopped_hash(pic, id).await;
        replace_heap_with_options(pic, id, &widen, args(7, 0), true, None)
            .await
            .unwrap();
        pic.start_canister(id, None).await.unwrap();
        assert_eq!(v2_values(&query(pic, id, "observe").await), expected);
        assert_eq!(pic.get_stable_memory(id).await, stable);
        update(pic, id, "vector_mutate").await;
    }
    instance.drop().await;
}

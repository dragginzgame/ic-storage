//! Actual retained V2 owners, explicit keep conversion, and independent recovery.
use super::*;

fn v2_values(bytes: &[u8]) -> Vec<(u64, u64)> {
    assert_eq!(bytes.len(), 416);
    let count = u64::from_le_bytes(bytes[48..56].try_into().unwrap()) as usize;
    bytes[160..160 + count * 16]
        .as_chunks::<16>()
        .0
        .iter()
        .map(|v| {
            (
                u64::from_le_bytes(v[..8].try_into().unwrap()),
                u64::from_le_bytes(v[8..].try_into().unwrap()),
            )
        })
        .collect()
}

fn save(directory: &std::path::Path, bytes: Vec<u8>) -> Vec<u8> {
    let digest = sha256_hex(&bytes);
    fs::write(
        directory.join(format!("retained-vector-{digest}.wasm")),
        &bytes,
    )
    .unwrap();
    println!("retained vector bytes={} sha256={digest}", bytes.len());
    bytes
}

fn v2_inputs(directory: &std::path::Path) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    (
        fs::read(directory.join("vector/v2/provider_record_vector.wasm")).unwrap(),
        fs::read(directory.join("checkpoint-record-vector-v2.wasm")).unwrap(),
        fs::read(directory.join("checkpoint-record-vector-convert.wasm")).unwrap(),
    )
}

#[test]
#[ignore = "native-record-vector-test supplies concrete V2 and conversion artifacts"]
fn retained_conversion_admits_source_before_reset_and_enters_v3_last() {
    let (directory, raw, _) = inputs();
    let (v2, bridge, convert) = v2_inputs(&directory);
    let source = save(
        &directory,
        prepare_v2(&v2, &bridge, false, (3, 4), Post::Reopen),
    );
    let target = save(
        &directory,
        prepare(&raw, &convert, false, (0, 0), Post::ConvertVector),
    );
    assert!(
        std::panic::catch_unwind(|| prepare(&v2, &convert, false, (0, 0), Post::ConvertVector))
            .is_err()
    );
    assert!(
        std::panic::catch_unwind(|| prepare_v2(&raw, &bridge, false, (0, 0), Post::Recover))
            .is_err()
    );
    assert_ne!(source, target);
    let mut post = 0;
    let mut imports = 0;
    let mut bodies = Vec::new();
    for payload in Parser::new(0).parse_all(&target) {
        match payload.unwrap() {
            Payload::ImportSection(r) => imports = r.into_imports().count(),
            Payload::ExportSection(r) => {
                for e in r {
                    let e = e.unwrap();
                    assert!(e.name == "memory" || e.name.starts_with("canister_"));
                    if e.name == "canister_post_upgrade" {
                        post = e.index;
                    }
                }
            }
            Payload::CodeSectionEntry(body) => bodies.push(body),
            _ => {}
        }
    }
    let calls: Vec<_> = bodies[post as usize - imports]
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
        .collect();
    assert_eq!(calls.len(), 4);
    // The reset is appended after both modules; conversion Rust precedes WAT
    // callbacks in the composition, and native entry is invoked by WAT last.
    assert!(calls[0] > calls[2] && calls[1] > calls[0] && calls[3] > calls[2]);
}

async fn failed_conversion(
    pic: &PocketIc,
    id: CanisterId,
    target: &[u8],
    fault: u64,
    message: &str,
) {
    let before = query(pic, id, "observe").await;
    let stable = pic.get_stable_memory(id).await;
    let hash = stopped_hash(pic, id).await;
    let error = pic
        .upgrade_eop_canister(id, target.to_vec(), args(0, fault), None)
        .await
        .unwrap_err();
    assert_eq!(error.error_code, ErrorCode::CanisterCalledTrap);
    assert!(error.reject_message.contains(message));
    assert_eq!(stopped_hash(pic, id).await, hash);
    pic.start_canister(id, None).await.unwrap();
    assert_eq!(query(pic, id, "observe").await, before);
    assert_eq!(pic.get_stable_memory(id).await, stable);
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "native-record-vector-test supplies artifacts and prepared local server"]
async fn installed_retained_record_conversion_preserves_allocation_and_recovers() {
    let (directory, raw, bridge) = inputs();
    let (v2, v2_bridge, convert) = v2_inputs(&directory);
    let target = save(
        &directory,
        prepare(&raw, &convert, false, (0, 0), Post::ConvertVector),
    );
    let reopen = prepare(&raw, &bridge, false, (0, 0), Post::Reopen);
    let recover = prepare(&raw, &bridge, true, (0, 0), Post::Recover);
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
    for root in [(0, 0), (0, 4), (1, 4), (3, 4), (16, 16)] {
        let source = save(
            &directory,
            prepare_v2(&v2, &v2_bridge, false, root, Post::Reopen),
        );
        let id = pic.create_canister().await;
        pic.install_canister(id, source.clone(), vec![], None).await;
        // An actual V2 Vec is issued on reopen and mutation, before conversion.
        pic.upgrade_eop_canister(id, source.clone(), vec![], None)
            .await
            .unwrap();
        pic.start_canister(id, None).await.unwrap();
        update(pic, id, "vector_mutate").await;
        let before = query(pic, id, "observe").await;
        let stable_before = pic.get_stable_memory(id).await;
        let expected = v2_values(&before);
        refused_upgrade(pic, id, &reopen, vec![], true).await;
        refused_upgrade(pic, id, &target, args(0, 0), false).await;
        for (fault, message) in [(1, "before replacement"), (3, "after publication")] {
            failed_conversion(pic, id, &target, fault, message).await;
        }
        if root.0 != 0 {
            failed_conversion(pic, id, &target, 2, "during replacement").await;
        }
        pic.upgrade_eop_canister(id, target.clone(), args(0, 0), None)
            .await
            .unwrap();
        pic.start_canister(id, None).await.unwrap();
        let after = query(pic, id, "observe").await;
        assert_eq!(pic.get_stable_memory(id).await, stable_before);
        assert_eq!(
            values(&after)
                .into_iter()
                .map(|(a, b)| (u64::from(a), b))
                .collect::<Vec<_>>(),
            expected
        );
        assert_eq!(&before[32..64], &after[32..64]); // base, extent, length, capacity
        assert_eq!(
            &before[160 + root.0 as usize * 16..],
            &after[160 + root.0 as usize * 16..]
        );
        refused_upgrade(pic, id, &target, args(0, 0), true).await; // no repeated conversion
        refused_upgrade(pic, id, &source, vec![], true).await;
        pic.upgrade_eop_canister(id, reopen.clone(), vec![], None)
            .await
            .unwrap();
        pic.start_canister(id, None).await.unwrap();
        // Reclaim and reissue the converted allocation itself, before any
        // portable recovery can replace its heap with a different allocation.
        update(pic, id, "vector_remove").await;
        update(pic, id, "vector_clear").await;
        update(pic, id, "vector_reuse").await;
        update(pic, id, "vector_append").await;
        update(pic, id, "vector_shrink").await;
        let expected = export(pic, id, 9).await;
        stopped_hash(pic, id).await;
        replace_heap(pic, id, &recover, args(9, 0)).await.unwrap();
        pic.start_canister(id, None).await.unwrap();
        assert_eq!(values(&query(pic, id, "observe").await), expected);
        assert_eq!(query(pic, id, "discarded_payload").await, [0; 256]);
        update(pic, id, "vector_clear").await;
        update(pic, id, "vector_reuse").await;
        update(pic, id, "vector_append").await;
        update(pic, id, "vector_shrink").await;
    }
    instance.drop().await;
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "native-record-vector-test supplies artifacts and prepared local server"]
async fn retained_range_failures_keep_full_width_source_recoverable() {
    let (directory, raw, bridge) = inputs();
    let (v2, v2_bridge, convert) = v2_inputs(&directory);
    let target = prepare(&raw, &convert, false, (0, 0), Post::ConvertVector);
    let wrong_domain = prepare(&raw, &convert, true, (0, 0), Post::ConvertVector);
    let recover_v2 = save(
        &directory,
        prepare_v2(&v2, &v2_bridge, true, (0, 0), Post::Recover),
    );
    let recover_v3 = prepare(&raw, &bridge, true, (0, 0), Post::Recover);
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
    for (index, counter) in [
        (0, u64::from(u32::MAX)),
        (0, u64::from(u32::MAX) + 1),
        (8, u64::MAX),
        (15, u64::from(u32::MAX) + 1),
    ] {
        let source = save(
            &directory,
            prepare_v2_boundary(&v2, &v2_bridge, index, counter),
        );
        let id = pic.create_canister().await;
        pic.install_canister(id, source.clone(), vec![], None).await;
        pic.upgrade_eop_canister(id, source, vec![], None)
            .await
            .unwrap();
        pic.start_canister(id, None).await.unwrap();
        update(pic, id, "vector_shrink").await; // real V2 access with full-width fields
        let expected = v2_values(&query(pic, id, "observe").await);
        assert_eq!(expected[index], (counter, u64::MAX));
        refused_upgrade(pic, id, &wrong_domain, args(0, 0), true).await;
        if counter == u64::from(u32::MAX) {
            pic.upgrade_eop_canister(id, target.clone(), args(0, 0), None)
                .await
                .unwrap();
            pic.start_canister(id, None).await.unwrap();
            assert_eq!(
                values(&query(pic, id, "observe").await)[index],
                (u32::MAX, u64::MAX)
            );
            export(pic, id, 11).await;
            stopped_hash(pic, id).await;
            replace_heap(pic, id, &recover_v3, args(11, 0))
                .await
                .unwrap();
            pic.start_canister(id, None).await.unwrap();
            assert_eq!(
                values(&query(pic, id, "observe").await)[index],
                (u32::MAX, u64::MAX)
            );
        } else {
            failed_conversion(
                pic,
                id,
                &target,
                0,
                "retained vector narrowing out of range",
            )
            .await;
            // A refused conversion neither poisons source access nor requires an
            // old Rust allocation to restore the full-width checkpoint.
            update(pic, id, "vector_shrink").await;
            pic.update_call(id, id, "export_checkpoint", args(11, 0))
                .await
                .unwrap();
            let checkpoint = query(pic, id, "checkpoint_bytes").await;
            assert_eq!(checkpoint.len(), 320);
            assert_eq!(&checkpoint[16..20], &2u32.to_le_bytes());
            assert_eq!(
                &checkpoint[32 + index * 16..40 + index * 16],
                &counter.to_le_bytes()
            );
            refused_upgrade(pic, id, &target, args(0, 0), true).await; // frozen source cannot convert
            refused_upgrade(pic, id, &recover_v3, args(11, 0), false).await;
            refused_upgrade(pic, id, &recover_v2, args(12, 0), false).await;
            for fault in [1, 2, 3] {
                refused_upgrade(pic, id, &recover_v2, args(11, fault), false).await;
            }
            stopped_hash(pic, id).await;
            replace_heap(pic, id, &recover_v2, args(11, 0))
                .await
                .unwrap();
            pic.start_canister(id, None).await.unwrap();
            assert_eq!(v2_values(&query(pic, id, "observe").await), expected);
            assert_eq!(query(pic, id, "discarded_payload").await, [0; 256]);
            assert_eq!(query(pic, id, "checkpoint_bytes").await, checkpoint);
            update(pic, id, "vector_clear").await;
            update(pic, id, "vector_reuse").await;
            // Explicit application repair creates a now-representable source;
            // retry converts directly in its current (relocated) domain.
            pic.upgrade_eop_canister(id, wrong_domain.clone(), args(0, 0), None)
                .await
                .unwrap();
            pic.start_canister(id, None).await.unwrap();
            assert_eq!(values(&query(pic, id, "observe").await), vec![(1, 1000)]);
        }
    }
    instance.drop().await;
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "native-record-vector-test supplies artifacts and prepared local server"]
async fn retained_conversion_rejects_corrupt_predecessors_without_the_old_hook() {
    let (directory, raw, _) = inputs();
    let (v2, bridge, convert) = v2_inputs(&directory);
    let target = prepare(&raw, &convert, false, (0, 0), Post::ConvertVector);
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
    // No source pre-upgrade hook can mask a missing destination check. Cover
    // unknown schema, wrong field kind, fabricated shape, lost custody and a
    // later invalid element before the destination enters checkpoint Rust.
    for fault in [
        (65600, 99),
        (65648, 4),
        (65568, 17),
        (65576, u64::MAX),
        (65584, 0),
        (131112, 0),
    ] {
        let source = save(&directory, prepare_v2_corrupt(&v2, &bridge, fault));
        let id = pic.create_canister().await;
        pic.install_canister(id, source, vec![], None).await;
        update(pic, id, "corrupt").await;
        refused_upgrade(pic, id, &target, args(0, 0), true).await;
    }
    instance.drop().await;
}

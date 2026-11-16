//! Explicit portable V2→V3 collection conversion; source V2 state is ordinary Rust.
use super::*;
use wasmparser::DataKind;

fn source(directory: &std::path::Path) -> Vec<u8> {
    let bytes = fs::read(directory.join("checkpoint-record-vector-source.wasm")).unwrap();
    validate_wasm(&bytes, LIMIT).unwrap();
    println!(
        "ordinary V2 vector source bytes={} sha256={}",
        bytes.len(),
        sha256_hex(&bytes)
    );
    bytes
}

fn install_args(count: u64, index: u64, value: u64) -> Vec<u8> {
    [count, index, value]
        .into_iter()
        .flat_map(u64::to_le_bytes)
        .collect()
}

fn source_values(bytes: &[u8]) -> Vec<(u64, u64)> {
    let count = u64::from_le_bytes(bytes[8..16].try_into().unwrap()) as usize;
    assert!(count <= 16);
    assert_eq!(bytes.len(), 16 + 16 * count);
    bytes[16..]
        .as_chunks::<16>()
        .0
        .iter()
        .map(|record| {
            (
                u64::from_le_bytes(record[..8].try_into().unwrap()),
                u64::from_le_bytes(record[8..].try_into().unwrap()),
            )
        })
        .collect()
}

async fn export_source(pic: &PocketIc, id: CanisterId, epoch: u64) -> Vec<(u64, u64)> {
    let expected = source_values(&query(pic, id, "observe").await);
    pic.update_call(id, id, "export_checkpoint", args(epoch, 0))
        .await
        .unwrap();
    let checkpoint = query(pic, id, "checkpoint_bytes").await;
    assert_eq!(checkpoint.len(), 64 + expected.len() * 16);
    assert_eq!(&checkpoint[..16], b"RECVEC01RECU64V1");
    assert_eq!(&checkpoint[16..20], &2u32.to_le_bytes());
    assert_eq!(&checkpoint[20..24], &(expected.len() as u32).to_le_bytes());
    assert_eq!(&checkpoint[24..32], &epoch.to_le_bytes());
    let payload: Vec<_> = expected
        .iter()
        .flat_map(|(a, b)| a.to_le_bytes().into_iter().chain(b.to_le_bytes()))
        .collect();
    assert_eq!(&checkpoint[32..checkpoint.len() - 32], payload);
    assert_eq!(
        &checkpoint[checkpoint.len() - 32..],
        &sha256(&checkpoint[..checkpoint.len() - 32])
    );
    expected
}

async fn range_refusal(pic: &PocketIc, id: CanisterId, target: &[u8]) {
    let before = query(pic, id, "observe").await;
    let stable = pic.get_stable_memory(id).await;
    let hash = stopped_hash(pic, id).await;
    let error = replace_heap(pic, id, target, args(7, 0)).await.unwrap_err();
    assert_eq!(error.error_code, ErrorCode::CanisterCalledTrap);
    assert!(
        error
            .reject_message
            .contains("record vector narrowing out of range")
    );
    assert_eq!(stopped_hash(pic, id).await, hash);
    pic.start_canister(id, None).await.unwrap();
    assert_eq!(query(pic, id, "observe").await, before);
    assert_eq!(pic.get_stable_memory(id).await, stable);
}

#[test]
#[ignore = "native-record-vector-test supplies the explicit source and destination artifacts"]
fn narrowing_has_a_distinct_entry_and_the_ordinary_source_refuses_retained_admission() {
    let (directory, raw, bridge) = inputs();
    let source = source(&directory);
    let mut witness = false;
    for payload in Parser::new(0).parse_all(&source) {
        match payload.unwrap() {
            Payload::ImportSection(r) => {
                for import in r.into_imports() {
                    assert_eq!(import.unwrap().module, "ic0");
                }
            }
            Payload::CustomSection(s) => {
                assert_ne!(s.name(), "icp:private enhanced-orthogonal-persistence")
            }
            Payload::DataSection(r) => {
                for data in r {
                    let data = data.unwrap();
                    let DataKind::Active {
                        memory_index: 0,
                        offset_expr,
                    } = data.kind
                    else {
                        panic!("ordinary source data profile")
                    };
                    let Operator::I32Const { value } =
                        offset_expr.get_operators_reader().read().unwrap()
                    else {
                        panic!("constant source data")
                    };
                    assert!(value >= 65536);
                    // Retaining this source's data cannot satisfy the importer's
                    // all-zero native header gate. This is a concrete artifact fact,
                    // not an assertion about arbitrary ordinary Rust canisters.
                    witness |= data.data.iter().enumerate().any(|(i, byte)| {
                        (65536..65680).contains(&(value as usize + i)) && *byte != 0
                    });
                }
            }
            _ => {}
        }
    }
    assert!(witness);
    let recovery = prepare(&raw, &bridge, true, (0, 0), Post::Recover);
    let narrowing = candidate(&directory, &raw, &bridge, true, (0, 0), Post::NarrowVector);
    assert_ne!(recovery, narrowing);
    let post_calls = |bytes: &[u8]| {
        let mut imports = 0;
        let mut post = None;
        let mut bodies = Vec::new();
        for payload in Parser::new(0).parse_all(bytes) {
            match payload.unwrap() {
                Payload::ImportSection(r) => imports = r.into_imports().count(),
                Payload::ExportSection(r) => {
                    for export in r {
                        let export = export.unwrap();
                        if export.name == "canister_post_upgrade" {
                            post = Some(export.index);
                        }
                        assert!(
                            !export.name.starts_with("checkpoint_")
                                && !export.name.starts_with("ncp_")
                        );
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
            .filter_map(|op| {
                if let Operator::Call { function_index } = op.unwrap() {
                    Some(function_index)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
    };
    let restore = post_calls(&recovery);
    let convert = post_calls(&narrowing);
    assert_eq!((restore.len(), convert.len()), (3, 3));
    assert_eq!((restore[0], restore[2]), (convert[0], convert[2]));
    assert_ne!(restore[1], convert[1]);
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "native-record-vector-test supplies fresh compiler artifacts and the prepared server"]
async fn installed_portable_narrowing_preserves_order_and_ceiling_then_uses_native_v3() {
    let (directory, raw, bridge) = inputs();
    let source = source(&directory);
    let narrow = candidate(&directory, &raw, &bridge, true, (0, 0), Post::NarrowVector);
    let recover = candidate(&directory, &raw, &bridge, false, (0, 0), Post::Recover);
    let reopen = candidate(&directory, &raw, &bridge, true, (0, 0), Post::Reopen);
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
    for count in [0, 1, 16] {
        let id = pic.create_canister().await;
        pic.install_canister(
            id,
            source.clone(),
            install_args(
                count,
                if count == 0 { u64::MAX } else { count - 1 },
                u64::from(u32::MAX),
            ),
            None,
        )
        .await;
        let expected = export_source(pic, id, 7).await;
        let stable = pic.get_stable_memory(id).await;
        refused_upgrade(pic, id, &recover, args(7, 0), false).await; // V3 decoder must reject V2
        refused_upgrade(pic, id, &narrow, args(7, 0), true).await; // require actual heap replacement
        refused_upgrade(pic, id, &narrow, args(8, 0), false).await;
        for fault in [1, 3] {
            refused_upgrade(pic, id, &narrow, args(7, fault), false).await;
        }
        if count > 0 {
            refused_upgrade(pic, id, &narrow, args(7, 2), false).await;
        }
        stopped_hash(pic, id).await;
        replace_heap(pic, id, &narrow, args(7, 0)).await.unwrap();
        assert_eq!(stopped_hash(pic, id).await, sha256(&narrow));
        pic.start_canister(id, None).await.unwrap();
        assert_eq!(pic.get_stable_memory(id).await, stable);
        let converted: Vec<_> = values(&query(pic, id, "observe").await)
            .into_iter()
            .map(|(a, b)| (u64::from(a), b))
            .collect();
        assert_eq!(converted, expected);
        assert_eq!(query(pic, id, "discarded_payload").await, [0; 256]);
        pic.upgrade_eop_canister(id, reopen.clone(), vec![], None)
            .await
            .unwrap();
        pic.start_canister(id, None).await.unwrap();
        update(pic, id, "vector_reuse").await;
        update(pic, id, "vector_append").await;
        update(pic, id, "vector_mutate").await;
        let next = export(pic, id, 8).await;
        refused_upgrade(pic, id, &narrow, args(8, 0), false).await; // conversion never silently accepts V3
        stopped_hash(pic, id).await;
        replace_heap(pic, id, &recover, args(8, 0)).await.unwrap();
        pic.start_canister(id, None).await.unwrap();
        assert_eq!(values(&query(pic, id, "observe").await), next);
        update(pic, id, "vector_clear").await;
    }
    instance.drop().await;
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "native-record-vector-test supplies fresh compiler artifacts and the prepared server"]
async fn installed_failed_narrowing_preserves_source_for_explicit_repair_and_retry() {
    let (directory, raw, bridge) = inputs();
    let source = source(&directory);
    let narrow = candidate(&directory, &raw, &bridge, true, (0, 0), Post::NarrowVector);
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
        (0, u64::from(u32::MAX) + 1),
        (8, u64::MAX),
        (15, u64::from(u32::MAX) + 1),
    ] {
        let id = pic.create_canister().await;
        pic.install_canister(id, source.clone(), install_args(16, index, counter), None)
            .await;
        // Export failure rolls back the cell and the source freeze together.
        let before = query(pic, id, "observe").await;
        let stable = pic.get_stable_memory(id).await;
        let error = pic
            .update_call(id, id, "export_checkpoint", args(7, 1))
            .await
            .unwrap_err();
        assert_eq!(error.error_code, ErrorCode::CanisterCalledTrap);
        assert!(
            error
                .reject_message
                .contains("V2 vector export after stable write")
        );
        assert_eq!(query(pic, id, "observe").await, before);
        assert_eq!(pic.get_stable_memory(id).await, stable);
        export_source(pic, id, 7).await;
        refused_update(pic, id, "set_counter", args(index, 1)).await;
        range_refusal(pic, id, &narrow).await;
        update(pic, id, "cancel_export").await;
        assert!(query(pic, id, "checkpoint_bytes").await.is_empty());
        pic.update_call(id, id, "set_counter", args(index, 123))
            .await
            .unwrap();
        let expected = export_source(pic, id, 8).await;
        refused_upgrade(pic, id, &narrow, args(7, 0), false).await;
        stopped_hash(pic, id).await;
        replace_heap(pic, id, &narrow, args(8, 0)).await.unwrap();
        pic.start_canister(id, None).await.unwrap();
        let actual: Vec<_> = values(&query(pic, id, "observe").await)
            .into_iter()
            .map(|(a, b)| (u64::from(a), b))
            .collect();
        assert_eq!(actual, expected);
    }
    instance.drop().await;
}

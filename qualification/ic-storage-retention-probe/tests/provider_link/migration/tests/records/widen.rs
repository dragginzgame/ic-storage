//! Explicit logical widening, never automatic admission of an older schema.
use super::updates::add;
use super::*;

// Negative source fixture: remove only the old hook's export so refusal must
// come from destination admission, not the source's pre-upgrade guard.
fn without_pre_upgrade(bytes: &[u8]) -> Vec<u8> {
    let mut exports = wasm_encoder::ExportSection::new();
    for payload in Parser::new(0).parse_all(bytes) {
        if let Payload::ExportSection(items) = payload.unwrap() {
            for item in items {
                let item = item.unwrap();
                if item.name == "canister_pre_upgrade" {
                    continue;
                }
                let kind = match item.kind {
                    wasmparser::ExternalKind::Func => wasm_encoder::ExportKind::Func,
                    wasmparser::ExternalKind::Memory => wasm_encoder::ExportKind::Memory,
                    _ => panic!("unexpected composed export"),
                };
                exports.export(item.name, kind, item.index);
            }
        }
    }
    let result = crate::rewrite(bytes, |module: &mut Module, id, data| {
        if id == 7 {
            module.section(&exports);
        } else {
            module.section(&RawSection { id, data });
        }
    });
    validate_wasm(&result, LIMIT).unwrap();
    result
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "native-record-test supplies fresh inputs and the prepared server"]
async fn installed_widening_preserves_storage_and_recovers_full_width_updates() {
    let (directory, raw, bridge) = inputs();
    let candidate = |relocated, root, schema, post| {
        let bytes = prepare(&raw, &bridge, relocated, root, schema, post);
        assert_eq!(bytes, prepare(&raw, &bridge, relocated, root, schema, post));
        let digest = sha256_hex(&bytes);
        fs::write(directory.join(format!("widen-{digest}.wasm")), &bytes).unwrap();
        println!(
            "widen schema={schema} relocated={relocated} root={root:?} post={post:?} sha256={digest}"
        );
        bytes
    };
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
    let widen = candidate(false, (0, 0), 2, Post::Widen);
    let narrow = candidate(false, (0, 0), 3, Post::Narrow);
    let reopen = candidate(false, (0, 0), 2, Post::Reopen);
    let recover = candidate(true, (0, 0), 2, Post::Recover);
    let back = candidate(false, (0, 0), 2, Post::Recover);
    for root in [(0, 0), (0, 8), (3, 4), (512, 512)] {
        let id = pic.create_canister().await;
        pic.install_canister(id, candidate(false, root, 3, Post::Reopen), vec![], None)
            .await;
        let limit = u64::from(u32::MAX);
        add(pic, id, limit - 41, 0, limit).await;
        // Equal outer sizes never authorize an implicit schema change.
        refused(pic, id, &reopen, vec![], true).await;
        refused(pic, id, &widen, vec![], true).await;
        refused(pic, id, &widen, args(1, 0), true).await;
        refused(pic, id, &widen, args(0, 0), false).await;
        for (fault, message) in [
            (1, "record widening before publication"),
            (2, "record widening after publication"),
        ] {
            refused_with_message(pic, id, &widen, args(0, fault), true, Some(message)).await;
        }
        let native = query(pic, id, "observe").await;
        let stable = pic.get_stable_memory(id).await;
        pic.upgrade_eop_canister(id, widen.clone(), args(0, 0), None)
            .await
            .unwrap();
        assert_eq!(stopped_hash(pic, id).await, sha256(&widen));
        pic.start_canister(id, None).await.unwrap();
        assert_eq!(query(pic, id, "observe").await, native);
        assert_eq!(
            words(&query(pic, id, "observe_record").await),
            [2, limit, u64::MAX]
        );
        assert_eq!(pic.get_stable_memory(id).await, stable);
        refused(pic, id, &widen, args(0, 0), true).await;
        // Exact widening and checked narrowing can cycle while range permits.
        pic.upgrade_eop_canister(id, narrow.clone(), args(0, 0), None)
            .await
            .unwrap();
        pic.upgrade_eop_canister(id, widen.clone(), args(0, 0), None)
            .await
            .unwrap();
        assert_eq!(query(pic, id, "observe").await, native);
        pic.upgrade_eop_canister(id, reopen.clone(), vec![], None)
            .await
            .unwrap();
        add(pic, id, 1, 0, limit + 1).await;
        refused_with_message(
            pic,
            id,
            &narrow,
            args(0, 0),
            true,
            Some("record narrowing out of range"),
        )
        .await;
        // The enlarged counter is usable up to u64::MAX and rejects overflow.
        add(pic, id, u64::MAX - (limit + 1), 0, u64::MAX).await;
        add(pic, id, 1, 2, u64::MAX).await;
        pic.update_call(id, id, "export_checkpoint", args(7, 0))
            .await
            .unwrap();
        let checkpoint = query(pic, id, "checkpoint_bytes").await;
        assert_eq!(&checkpoint[..8], b"ICNCP001");
        assert_eq!(&checkpoint[8..10], &3u16.to_le_bytes());
        assert_eq!(&checkpoint[24..28], &2u32.to_le_bytes());
        assert_eq!(&checkpoint[32..40], &7u64.to_le_bytes());
        assert_eq!(&checkpoint[56..64], &u64::MAX.to_le_bytes());
        assert_eq!(&checkpoint[72..80], &u64::MAX.to_le_bytes());
        let values = logical(&native);
        assert_eq!(checkpoint.len(), 120 + values.len() * 8);
        assert_eq!(
            &checkpoint[80..80 + values.len() * 8],
            values
                .iter()
                .copied()
                .flat_map(u64::to_le_bytes)
                .collect::<Vec<_>>()
        );
        let end = checkpoint.len() - 32;
        assert_eq!(&checkpoint[end..], sha256(&checkpoint[..end]));
        for fault in [1, 2] {
            if fault == 1 || root.0 > 0 {
                refused(pic, id, &recover, args(7, fault), false).await;
            }
        }
        let stable = pic.get_stable_memory(id).await;
        stopped_hash(pic, id).await;
        replace_heap(pic, id, &recover, args(7, 0)).await.unwrap();
        pic.start_canister(id, None).await.unwrap();
        assert_eq!(
            words(&query(pic, id, "observe_record").await),
            [2, u64::MAX, u64::MAX]
        );
        assert_eq!(logical(&query(pic, id, "observe").await), values);
        assert_eq!(words(&query(pic, id, "observe").await)[4], 147456);
        assert_eq!(pic.get_stable_memory(id).await, stable);
        update(pic, id, "vector_reuse").await;
        update(pic, id, "vector_append").await;
        pic.update_call(id, id, "export_checkpoint", args(8, 0))
            .await
            .unwrap();
        stopped_hash(pic, id).await;
        replace_heap(pic, id, &back, args(8, 0)).await.unwrap();
        pic.start_canister(id, None).await.unwrap();
        assert_eq!(logical(&query(pic, id, "observe").await), [1, 1]);
        assert_eq!(
            words(&query(pic, id, "observe_record").await),
            [2, u64::MAX, u64::MAX]
        );
        update(pic, id, "vector_clear").await;
        update(pic, id, "vector_append").await;
        assert_eq!(logical(&query(pic, id, "observe").await), [1]);
    }
    // Destination alone rejects V1, V2 and corrupt V3 sources.
    for schema in [1, 2, 3] {
        let source = without_pre_upgrade(&candidate(false, (3, 4), schema, Post::Reopen));
        let digest = sha256_hex(&source);
        fs::write(
            directory.join(format!("widen-source-no-pre-{digest}.wasm")),
            &source,
        )
        .unwrap();
        println!("widen source without old hook schema={schema} sha256={digest}");
        let id = pic.create_canister().await;
        pic.install_canister(id, source, vec![], None).await;
        if schema == 3 {
            update(pic, id, "damage_record_padding").await;
        }
        refused(pic, id, &widen, args(0, 0), true).await;
    }
    // The application ceiling is unchanged, including a tighter non-default value.
    let id = pic.create_canister().await;
    pic.install_canister(id, candidate(false, (3, 4), 1, Post::Reopen), vec![], None)
        .await;
    pic.upgrade_eop_canister(
        id,
        candidate(false, (0, 0), 2, Post::Convert),
        args(43, 0),
        None,
    )
    .await
    .unwrap();
    pic.upgrade_eop_canister(id, narrow, args(0, 0), None)
        .await
        .unwrap();
    pic.upgrade_eop_canister(id, widen, args(0, 0), None)
        .await
        .unwrap();
    add(pic, id, 2, 0, 43).await;
    add(pic, id, 1, 1, 43).await;
    instance.drop().await;
}

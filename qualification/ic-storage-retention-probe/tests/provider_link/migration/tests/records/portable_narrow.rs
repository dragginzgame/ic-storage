//! Range-checked recovery into a new schema after discarding source memory.
use super::updates::add;
use super::*;

#[tokio::test(flavor = "current_thread")]
#[ignore = "native-record-test supplies fresh artifacts and prepared server"]
async fn portable_narrowing_checks_range_before_reconstruction_and_keeps_v2_recoverable() {
    let (directory, raw, bridge) = inputs();
    let candidate = |relocated, root, schema, post| {
        let bytes = prepare(&raw, &bridge, relocated, root, schema, post);
        assert_eq!(bytes, prepare(&raw, &bridge, relocated, root, schema, post));
        let hash = sha256_hex(&bytes);
        fs::write(
            directory.join(format!("portable-narrow-{hash}.wasm")),
            &bytes,
        )
        .unwrap();
        println!(
            "portable narrow schema={schema} relocated={relocated} root={root:?} post={post:?} sha256={hash}"
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
    let narrow = candidate(true, (0, 0), 3, Post::RecoverNarrow);
    let direct = candidate(true, (0, 0), 3, Post::Recover);
    let back = candidate(false, (0, 0), 3, Post::Recover);
    let fallback = candidate(true, (0, 0), 2, Post::Recover);
    let reopen = candidate(true, (0, 0), 3, Post::Reopen);
    for root in [(0, 0), (0, 8), (3, 4), (512, 512)] {
        let id = pic.create_canister().await;
        pic.install_canister(id, candidate(false, root, 2, Post::Reopen), vec![], None)
            .await;
        let counter = u64::from(u32::MAX);
        add(pic, id, counter - 41, 0, counter).await;
        refused(pic, id, &narrow, args(7, 0), false).await;
        let expected = logical(&query(pic, id, "observe").await);
        pic.update_call(id, id, "export_checkpoint", args(7, 0))
            .await
            .unwrap();
        let checkpoint = query(pic, id, "checkpoint_bytes").await;
        assert_eq!(&checkpoint[8..10], &3u16.to_le_bytes());
        assert_eq!(&checkpoint[24..28], &2u32.to_le_bytes());
        assert_eq!(&checkpoint[56..64], &counter.to_le_bytes());
        let end = checkpoint.len() - 32;
        assert_eq!(&checkpoint[end..], sha256(&checkpoint[..end]));
        refused(pic, id, &direct, args(7, 0), false).await;
        refused(pic, id, &narrow, args(7, 0), true).await;
        refused(pic, id, &narrow, args(8, 0), false).await;
        for fault in [1, 2] {
            if fault == 1 || root.0 > 0 {
                refused(pic, id, &narrow, args(7, fault), false).await;
            }
        }
        refused_with_message(
            pic,
            id,
            &narrow,
            args(7, 3),
            false,
            Some("portable record conversion after publication"),
        )
        .await;
        let stable = pic.get_stable_memory(id).await;
        stopped_hash(pic, id).await;
        replace_heap(pic, id, &narrow, args(7, 0)).await.unwrap();
        assert_eq!(stopped_hash(pic, id).await, sha256(&narrow));
        pic.start_canister(id, None).await.unwrap();
        assert_eq!(pic.get_stable_memory(id).await, stable);
        assert_eq!(query(pic, id, "checkpoint_bytes").await, checkpoint);
        assert_eq!(
            words(&query(pic, id, "observe_record").await),
            [3, counter, u64::MAX]
        );
        let native = query(pic, id, "observe").await;
        assert_eq!(logical(&native), expected);
        assert_eq!(words(&native)[4], 147456);
        assert_eq!(words(&native)[7], root.0);
        pic.upgrade_eop_canister(id, reopen.clone(), vec![], None)
            .await
            .unwrap();
        add(pic, id, 1, 2, counter).await;
        update(pic, id, "vector_reuse").await;
        update(pic, id, "vector_append").await;
        pic.update_call(id, id, "export_checkpoint", args(8, 0))
            .await
            .unwrap();
        let next = query(pic, id, "checkpoint_bytes").await;
        assert_eq!(&next[8..10], &4u16.to_le_bytes());
        assert_eq!(&next[56..60], &u32::MAX.to_le_bytes());
        assert_eq!(&next[60..64], [0; 4]);
        refused_with_message(
            pic,
            id,
            &narrow,
            args(8, 0),
            false,
            Some("portable narrowing requires schema two"),
        )
        .await;
        stopped_hash(pic, id).await;
        replace_heap(pic, id, &back, args(8, 0)).await.unwrap();
        pic.start_canister(id, None).await.unwrap();
        assert_eq!(logical(&query(pic, id, "observe").await), [1, 1]);
        assert_eq!(
            words(&query(pic, id, "observe_record").await),
            [3, counter, u64::MAX]
        );
    }
    // Refusing narrowing must leave an independent full-width recovery path.
    for counter in [u64::from(u32::MAX) + 1, u64::MAX] {
        let id = pic.create_canister().await;
        pic.install_canister(id, candidate(false, (3, 4), 2, Post::Reopen), vec![], None)
            .await;
        add(pic, id, counter - 41, 0, counter).await;
        pic.update_call(id, id, "export_checkpoint", args(9, 0))
            .await
            .unwrap();
        refused_with_message(
            pic,
            id,
            &narrow,
            args(9, 0),
            false,
            Some("portable narrowing out of range"),
        )
        .await;
        let stable = pic.get_stable_memory(id).await;
        stopped_hash(pic, id).await;
        replace_heap(pic, id, &fallback, args(9, 0)).await.unwrap();
        pic.start_canister(id, None).await.unwrap();
        assert_eq!(pic.get_stable_memory(id).await, stable);
        assert_eq!(
            words(&query(pic, id, "observe_record").await),
            [2, counter, u64::MAX]
        );
        assert_eq!(logical(&query(pic, id, "observe").await), [17, 23, 41]);
        update(pic, id, "vector_reuse").await;
        add(pic, id, 0, 0, counter).await;
    }
    // A bounded application ceiling is preserved, not replaced by a type limit.
    let id = pic.create_canister().await;
    pic.install_canister(id, candidate(false, (3, 4), 1, Post::Reopen), vec![], None)
        .await;
    pic.update_call(id, id, "export_checkpoint", args(10, 0))
        .await
        .unwrap();
    refused_with_message(
        pic,
        id,
        &narrow,
        args(10, 0),
        false,
        Some("portable narrowing requires schema two"),
    )
    .await;
    // Restore a ready V1 before selecting the explicit field-addition converter.
    stopped_hash(pic, id).await;
    replace_heap(
        pic,
        id,
        &candidate(false, (0, 0), 1, Post::Recover),
        args(10, 0),
    )
    .await
    .unwrap();
    pic.start_canister(id, None).await.unwrap();
    pic.upgrade_eop_canister(
        id,
        candidate(false, (0, 0), 2, Post::Convert),
        args(43, 0),
        None,
    )
    .await
    .unwrap();
    pic.update_call(id, id, "export_checkpoint", args(11, 0))
        .await
        .unwrap();
    stopped_hash(pic, id).await;
    replace_heap(pic, id, &narrow, args(11, 0)).await.unwrap();
    pic.start_canister(id, None).await.unwrap();
    add(pic, id, 2, 0, 43).await;
    add(pic, id, 1, 1, 43).await;
    instance.drop().await;
}

//! Explicit checkpoint conversion after the source heap has been discarded.
use super::super::installed::replace_heap_with_options;
use super::updates::add;
use super::*;

#[tokio::test(flavor = "current_thread")]
#[ignore = "native-record-test supplies fresh inputs and the prepared server"]
async fn portable_widening_requires_v3_and_survives_source_heap_loss() {
    let (directory, raw, bridge) = inputs();
    let candidate = |relocated, root, schema, post| {
        let bytes = prepare(&raw, &bridge, relocated, root, schema, post);
        assert_eq!(bytes, prepare(&raw, &bridge, relocated, root, schema, post));
        let digest = sha256_hex(&bytes);
        fs::write(
            directory.join(format!("portable-widen-{digest}.wasm")),
            &bytes,
        )
        .unwrap();
        println!(
            "portable widen schema={schema} relocated={relocated} root={root:?} post={post:?} sha256={digest}"
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
    let widen = candidate(true, (0, 0), 2, Post::RecoverWiden);
    let direct = candidate(true, (0, 0), 2, Post::Recover);
    let back = candidate(false, (0, 0), 2, Post::Recover);
    let reopen = candidate(true, (0, 0), 2, Post::Reopen);
    for root in [(0, 0), (0, 8), (3, 4), (512, 512)] {
        let id = pic.create_canister().await;
        pic.install_canister(id, candidate(false, root, 3, Post::Reopen), vec![], None)
            .await;
        let counter = u64::from(u32::MAX);
        add(pic, id, counter - 41, 0, counter).await;
        refused(pic, id, &widen, args(7, 0), false).await; // no checkpoint
        let expected = logical(&query(pic, id, "observe").await);
        pic.update_call(id, id, "export_checkpoint", args(7, 0))
            .await
            .unwrap();
        let checkpoint = query(pic, id, "checkpoint_bytes").await;
        assert_eq!(&checkpoint[8..10], &4u16.to_le_bytes());
        assert_eq!(&checkpoint[24..28], &3u32.to_le_bytes());
        assert_eq!(&checkpoint[32..40], &7u64.to_le_bytes());
        assert_eq!(&checkpoint[56..60], &u32::MAX.to_le_bytes());
        assert_eq!(&checkpoint[60..64], [0; 4]);
        assert_eq!(&checkpoint[72..80], &u64::MAX.to_le_bytes());
        let end = checkpoint.len() - 32;
        assert_eq!(&checkpoint[end..], sha256(&checkpoint[..end]));
        refused(pic, id, &direct, args(7, 0), false).await; // never implicit
        refused(pic, id, &widen, args(7, 0), true).await; // requires fresh heap
        refused(pic, id, &widen, args(8, 0), false).await;
        refused(pic, id, &widen, args(7, 1), false).await;
        if root.0 > 0 {
            refused(pic, id, &widen, args(7, 2), false).await;
        }
        refused_with_message(
            pic,
            id,
            &widen,
            args(7, 3),
            false,
            Some("portable record conversion after publication"),
        )
        .await;
        let stable = pic.get_stable_memory(id).await;
        if root.0 == 3 {
            update(pic, id, "damage_record_padding").await;
            refused(pic, id, &widen, args(7, 0), false).await;
            stopped_hash(pic, id).await;
            replace_heap_with_options(pic, id, &widen, args(7, 0), true, None)
                .await
                .unwrap();
        } else {
            stopped_hash(pic, id).await;
            replace_heap(pic, id, &widen, args(7, 0)).await.unwrap();
        }
        assert_eq!(stopped_hash(pic, id).await, sha256(&widen));
        pic.start_canister(id, None).await.unwrap();
        assert_eq!(pic.get_stable_memory(id).await, stable);
        assert_eq!(query(pic, id, "checkpoint_bytes").await, checkpoint);
        assert_eq!(
            words(&query(pic, id, "observe_record").await),
            [2, counter, u64::MAX]
        );
        let native = query(pic, id, "observe").await;
        assert_eq!(logical(&native), expected);
        assert_eq!(words(&native)[4], 147456);
        assert_eq!(words(&native)[7], root.0); // logical recovery discards spare capacity
        pic.upgrade_eop_canister(id, reopen.clone(), vec![], None)
            .await
            .unwrap();
        add(pic, id, 1, 0, counter + 1).await;
        update(pic, id, "vector_reuse").await;
        update(pic, id, "vector_append").await;
        pic.update_call(id, id, "export_checkpoint", args(8, 0))
            .await
            .unwrap();
        let next = query(pic, id, "checkpoint_bytes").await;
        assert_eq!(&next[8..10], &3u16.to_le_bytes());
        assert_eq!(&next[56..64], &(counter + 1).to_le_bytes());
        refused_with_message(
            pic,
            id,
            &widen,
            args(8, 0),
            false,
            Some("portable widening requires schema three"),
        )
        .await;
        stopped_hash(pic, id).await;
        replace_heap(pic, id, &back, args(8, 0)).await.unwrap();
        pic.start_canister(id, None).await.unwrap();
        assert_eq!(
            words(&query(pic, id, "observe_record").await),
            [2, counter + 1, u64::MAX]
        );
        assert_eq!(logical(&query(pic, id, "observe").await), [1, 1]);
    }
    // A valid older checkpoint and malformed V3 envelopes cannot be reinterpreted.
    for schema in [1, 2, 3] {
        let id = pic.create_canister().await;
        pic.install_canister(
            id,
            candidate(false, (3, 4), schema, Post::Reopen),
            vec![],
            None,
        )
        .await;
        for damage in 0..3 {
            pic.update_call(id, id, "export_checkpoint", args(9, 0))
                .await
                .unwrap();
            if schema == 3 {
                pic.update_call(id, id, "damage_checkpoint", args(damage, 0))
                    .await
                    .unwrap();
            }
            refused(pic, id, &widen, args(9, 0), false).await;
        }
    }
    instance.drop().await;
}

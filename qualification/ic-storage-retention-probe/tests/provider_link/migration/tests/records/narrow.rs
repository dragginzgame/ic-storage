use super::super::installed::{refused_update, replace_heap_with_options};
use super::updates::{add, trapped_add};
use super::*;

#[tokio::test(flavor = "current_thread")]
#[ignore = "native-record-test supplies fresh inputs and the prepared server"]
async fn installed_narrowing_preserves_vector_and_recovers_schema_three() {
    let (directory, raw, bridge) = inputs();
    let candidate = |relocated, root, schema, post| {
        let bytes = prepare(&raw, &bridge, relocated, root, schema, post);
        let digest = Sha256Digest::compute(&bytes);
        fs::write(directory.join(format!("narrow-{digest}.wasm")), &bytes).unwrap();
        println!(
            "narrow schema={schema} relocated={relocated} root={root:?} post={post:?} sha256={digest}"
        );
        bytes
    };
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
    let narrow = candidate(false, (0, 0), 3, Post::Narrow);
    let reopen = candidate(false, (0, 0), 3, Post::Reopen);
    let reopen_b = candidate(true, (0, 0), 3, Post::Reopen);
    let recover = candidate(true, (0, 0), 3, Post::Recover);
    let return_a = candidate(false, (0, 0), 3, Post::Recover);
    let old = candidate(false, (0, 0), 2, Post::Reopen);
    let old_recover = candidate(true, (0, 0), 2, Post::Recover);
    for root in [(0, 0), (3, 4), (512, 512)] {
        let id = pic.create_canister().await;
        pic.install_canister(id, candidate(false, root, 2, Post::Reopen), vec![], None)
            .await;
        add(pic, id, 1, 0, 42).await;
        refused(pic, id, &reopen, vec![], true).await;
        for (fault, message) in [
            (1, "record narrowing before publication"),
            (2, "record narrowing after publication"),
        ] {
            refused_with_message(pic, id, &narrow, args(0, fault), true, Some(message)).await;
        }
        refused(pic, id, &narrow, vec![], true).await;
        refused(pic, id, &narrow, args(1, 0), true).await;
        let native = query(pic, id, "observe").await;
        let stable = pic.get_stable_memory(id).await;
        pic.upgrade_eop_canister(id, narrow.clone(), args(0, 0), None)
            .await
            .unwrap();
        assert_eq!(
            stopped_hash(pic, id).await,
            Sha256Digest::compute(&narrow).as_bytes()
        );
        pic.start_canister(id, None).await.unwrap();
        assert_eq!(query(pic, id, "observe").await, native);
        assert_eq!(
            words(&query(pic, id, "observe_record").await),
            [3, 42, u64::MAX]
        );
        assert_eq!(pic.get_stable_memory(id).await, stable);
        refused(pic, id, &narrow, args(0, 0), true).await;
        refused(pic, id, &old, vec![], true).await;
        pic.upgrade_eop_canister(id, reopen.clone(), vec![], None)
            .await
            .unwrap();
        add(pic, id, 1, 0, 43).await;
        add(pic, id, u64::from(u32::MAX), 2, 43).await;
        add(pic, id, u64::from(u32::MAX) + 1, 3, 43).await;
        assert!(
            trapped_add(pic, id, args(1, 1))
                .await
                .contains("record update after publication")
        );
        pic.update_call(id, id, "export_checkpoint", args(7, 0))
            .await
            .unwrap();
        let checkpoint = query(pic, id, "checkpoint_bytes").await;
        assert_eq!(&checkpoint[8..10], &4u16.to_le_bytes());
        assert_eq!(&checkpoint[24..28], &3u32.to_le_bytes());
        assert_eq!(&checkpoint[56..60], &43u32.to_le_bytes());
        assert_eq!(&checkpoint[60..64], &[0; 4]);
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
        assert_eq!(
            &checkpoint[end..],
            Sha256Digest::compute(&checkpoint[..end]).as_bytes()
        );
        trapped_add(pic, id, args(1, 0)).await;
        refused(pic, id, &old_recover, args(7, 0), false).await;
        refused(pic, id, &recover, args(7, 0), true).await;
        refused(pic, id, &recover, args(8, 0), false).await;
        refused(pic, id, &recover, args(7, 1), false).await;
        if root.0 > 0 {
            refused(pic, id, &recover, args(7, 2), false).await;
        }
        let stable = pic.get_stable_memory(id).await;
        stopped_hash(pic, id).await;
        replace_heap(pic, id, &recover, args(7, 0)).await.unwrap();
        assert_eq!(
            stopped_hash(pic, id).await,
            Sha256Digest::compute(&recover).as_bytes()
        );
        pic.start_canister(id, None).await.unwrap();
        assert_eq!(pic.get_stable_memory(id).await, stable);
        let restored = query(pic, id, "observe").await;
        assert_eq!(logical(&restored), values);
        assert_eq!(words(&restored)[4], 147456);
        assert_eq!(
            words(&query(pic, id, "observe_record").await),
            [3, 43, u64::MAX]
        );
        pic.upgrade_eop_canister(id, reopen_b.clone(), vec![], None)
            .await
            .unwrap();
        add(pic, id, 2, 0, 45).await;
        update(pic, id, "vector_reuse").await;
        pic.update_call(id, id, "export_checkpoint", args(8, 0))
            .await
            .unwrap();
        stopped_hash(pic, id).await;
        replace_heap(pic, id, &return_a, args(8, 0)).await.unwrap();
        pic.start_canister(id, None).await.unwrap();
        assert_eq!(logical(&query(pic, id, "observe").await), [1]);
        add(pic, id, 1, 0, 46).await;
        if root.0 == 3 {
            // A checkpoint remains independently readable when the retained
            // source is corrupt. Explicit replacement skips the unusable old hook.
            pic.update_call(id, id, "export_checkpoint", args(9, 0))
                .await
                .unwrap();
            update(pic, id, "damage_record_padding").await;
            let damaged = query(pic, id, "observe_record").await;
            assert_eq!(&damaged[12..16], &1u32.to_le_bytes());
            refused_update(pic, id, id, "export_checkpoint", args(9, 0)).await;
            assert_eq!(query(pic, id, "observe_record").await, damaged);
            refused(pic, id, &reopen, vec![], true).await;
            stopped_hash(pic, id).await;
            replace_heap_with_options(pic, id, &recover, args(9, 0), true, None)
                .await
                .unwrap();
            pic.start_canister(id, None).await.unwrap();
            add(pic, id, 1, 0, 47).await;
        }
    }
    for counter in [u64::from(u32::MAX), u64::from(u32::MAX) + 1, u64::MAX] {
        let id = pic.create_canister().await;
        pic.install_canister(id, old.clone(), vec![], None).await;
        add(pic, id, counter - 41, 0, counter).await;
        if counter > u64::from(u32::MAX) {
            refused_with_message(
                pic,
                id,
                &narrow,
                args(0, 0),
                true,
                Some("record narrowing out of range"),
            )
            .await;
        } else {
            pic.upgrade_eop_canister(id, narrow.clone(), args(0, 0), None)
                .await
                .unwrap();
            add(pic, id, 0, 0, counter).await;
            add(pic, id, 1, 2, counter).await;
        }
    }
    // Narrowing requires schema 2 and preserves its explicit application ceiling.
    let id = pic.create_canister().await;
    pic.install_canister(id, candidate(false, (0, 0), 1, Post::Reopen), vec![], None)
        .await;
    refused(pic, id, &narrow, args(0, 0), true).await;
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
    add(pic, id, 2, 0, 43).await;
    add(pic, id, 1, 1, 43).await;
    owner.close().await;
}

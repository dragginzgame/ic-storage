use super::super::installed::refused_update;
use super::*;

pub(super) async fn add(pic: &PocketIc, id: CanisterId, delta: u64, status: u64, counter: u64) {
    let native = query(pic, id, "observe").await;
    let record = words(&query(pic, id, "observe_record").await);
    assert!(matches!(record[0], 2 | 3));
    let stable = pic.get_stable_memory(id).await;
    let reply = pic
        .update_call(id, id, "record_add", args(delta, 0))
        .await
        .unwrap();
    assert_eq!(words(&reply), [status]);
    assert_eq!(
        words(&query(pic, id, "observe_record").await),
        [record[0], counter, record[2]]
    );
    assert_eq!(query(pic, id, "observe").await, native);
    assert_eq!(pic.get_stable_memory(id).await, stable);
}

pub(super) async fn trapped_add(pic: &PocketIc, id: CanisterId, arguments: Vec<u8>) -> String {
    let before = query(pic, id, "observe_record").await;
    let error = refused_update(pic, id, id, "record_add", arguments).await;
    assert_eq!(query(pic, id, "observe_record").await, before);
    error.reject_message
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "native-record-test supplies fresh inputs and the prepared server"]
async fn checked_updates_survive_reopen_and_migration_with_trap_rollback() {
    let (directory, raw, bridge) = inputs();
    let candidate = |relocated, root, schema, post| {
        let bytes = prepare(&raw, &bridge, relocated, root, schema, post);
        let digest = Sha256Digest::compute(&bytes);
        fs::write(
            directory.join(format!("record-update-{digest}.wasm")),
            &bytes,
        )
        .unwrap();
        println!(
            "record update schema={schema} relocated={relocated} root={root:?} post={post:?} sha256={digest}"
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
    let convert = candidate(false, (0, 0), 2, Post::Convert);
    let reopen = candidate(false, (0, 0), 2, Post::Reopen);
    let recover = candidate(true, (0, 0), 2, Post::Recover);
    let return_a = candidate(false, (0, 0), 2, Post::Recover);
    for root in [(0, 0), (3, 4), (512, 512)] {
        let id = pic.create_canister().await;
        pic.install_canister(id, candidate(false, root, 1, Post::Reopen), vec![], None)
            .await;
        // This endpoint is V2-only; an unconverted record is never reinterpreted.
        trapped_add(pic, id, args(1, 0)).await;
        pic.upgrade_eop_canister(id, convert.clone(), args(100, 0), None)
            .await
            .unwrap();
        add(pic, id, 1, 0, 42).await;
        add(pic, id, 59, 1, 42).await;
        assert!(
            trapped_add(pic, id, args(1, 1))
                .await
                .contains("record update after publication")
        );
        trapped_add(pic, id, vec![]).await;
        trapped_add(pic, id, args(1, 2)).await;
        pic.upgrade_eop_canister(id, reopen.clone(), vec![], None)
            .await
            .unwrap();
        add(pic, id, 1, 0, 43).await;
        let native = query(pic, id, "observe").await;
        pic.update_call(id, id, "export_checkpoint", args(7, 0))
            .await
            .unwrap();
        let checkpoint = query(pic, id, "checkpoint_bytes").await;
        assert_eq!(&checkpoint[56..64], &43u64.to_le_bytes());
        assert_eq!(&checkpoint[72..80], &100u64.to_le_bytes());
        trapped_add(pic, id, args(1, 0)).await;
        assert_eq!(query(pic, id, "checkpoint_bytes").await, checkpoint);
        stopped_hash(pic, id).await;
        replace_heap(pic, id, &recover, args(7, 0)).await.unwrap();
        pic.start_canister(id, None).await.unwrap();
        assert_eq!(logical(&query(pic, id, "observe").await), logical(&native));
        assert_eq!(words(&query(pic, id, "observe_record").await), [2, 43, 100]);
        add(pic, id, 2, 0, 45).await;
        pic.update_call(id, id, "export_checkpoint", args(8, 0))
            .await
            .unwrap();
        stopped_hash(pic, id).await;
        replace_heap(pic, id, &return_a, args(8, 0)).await.unwrap();
        pic.start_canister(id, None).await.unwrap();
        add(pic, id, 1, 0, 46).await;
        update(pic, id, "vector_reuse").await;
        assert_eq!(logical(&query(pic, id, "observe").await), [1]);
        assert_eq!(words(&query(pic, id, "observe_record").await), [2, 46, 100]);
    }
    // The maximum scalar is valid data, not a sentinel. Overflow is an ordinary
    // refusal and must preserve it through reopening and portable reconstruction.
    let id = pic.create_canister().await;
    pic.install_canister(id, reopen.clone(), vec![], None).await;
    add(pic, id, u64::MAX - 41, 0, u64::MAX).await;
    add(pic, id, 1, 2, u64::MAX).await;
    pic.upgrade_eop_canister(id, reopen, vec![], None)
        .await
        .unwrap();
    add(pic, id, 0, 0, u64::MAX).await;
    pic.update_call(id, id, "export_checkpoint", args(9, 0))
        .await
        .unwrap();
    stopped_hash(pic, id).await;
    replace_heap(pic, id, &recover, args(9, 0)).await.unwrap();
    pic.start_canister(id, None).await.unwrap();
    add(pic, id, 1, 2, u64::MAX).await;
    owner.close().await;
}

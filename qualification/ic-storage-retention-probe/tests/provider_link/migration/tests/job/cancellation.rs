use super::*;

async fn cancel(pic: &PocketIc, id: CanisterId, epoch: u64, target: u64) {
    pic.update_call(id, id, "cancel_checkpoint", args(epoch, target, 0))
        .await
        .unwrap();
}

async fn refused_cancel(
    pic: &PocketIc,
    id: CanisterId,
    sender: CanisterId,
    arguments: Vec<u8>,
    message: Option<&str>,
) {
    let native = query(pic, id, "observe").await;
    let stable = pic.get_stable_memory(id).await;
    let error = pic
        .update_call(id, sender, "cancel_checkpoint", arguments)
        .await
        .unwrap_err();
    assert!(matches!(
        error.error_code,
        ErrorCode::CanisterCalledTrap | ErrorCode::CanisterTrapped
    ));
    if let Some(message) = message {
        assert!(
            error.reject_message.contains(message),
            "expected {message}: {error:?}"
        );
    }
    assert_eq!(query(pic, id, "observe").await, native);
    assert_eq!(pic.get_stable_memory(id).await, stable);
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "native-job-test supplies the job bridge and prepared server"]
async fn cancelled_jobs_resume_in_place_without_replaying_retired_epochs() {
    let (directory, _) = super::super::super::super::input();
    let raw = fs::read(directory.join("vector/provider_vector.wasm")).unwrap();
    let bridge = fs::read(directory.join("checkpoint-job.wasm")).unwrap();
    let candidate = |relocated, root, recovery| {
        let bytes = prepare_job(&raw, &bridge, relocated, root, recovery);
        let digest = Sha256Digest::compute(&bytes);
        fs::write(
            directory.join(format!("cancellation-{digest}.wasm")),
            &bytes,
        )
        .unwrap();
        println!(
            "cancellation relocated={relocated} root={root:?} recovery={recovery} bytes={} sha256={digest}",
            bytes.len()
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
    let a = candidate(false, (0, 0), true);
    let b = candidate(true, (0, 0), true);
    let reopen = candidate(false, (0, 0), false);
    for root in [(3, 4), (0, 0), (512, 512)] {
        let id = pic.create_canister().await;
        pic.set_controllers(id, None, vec![CanisterId::anonymous(), id])
            .await
            .unwrap();
        pic.install_canister(id, candidate(false, root, false), vec![], None)
            .await;
        refused_cancel(pic, id, id, args(0, 147456, 0), None).await;
        let before = query(pic, id, "observe").await;
        let sealed = publish(pic, id, 1, 131072, 147456).await;
        refused_cancel(
            pic,
            id,
            CanisterId::from_slice(&[42]),
            args(1, 147456, 0),
            Some("migration controller required"),
        )
        .await;
        for (epoch, target) in [(0, 147456), (2, 147456), (1, 131072)] {
            refused_cancel(pic, id, id, args(epoch, target, 0), None).await;
        }
        refused_cancel(pic, id, id, vec![0; 16], None).await;
        refused_cancel(
            pic,
            id,
            id,
            args(1, 147456, 1),
            Some("job cancellation after stable write"),
        )
        .await;
        refused_cancel(
            pic,
            id,
            id,
            args(1, 147456, 2),
            Some("job cancellation after native thaw"),
        )
        .await;
        pic.set_controllers(id, None, vec![CanisterId::anonymous()])
            .await
            .unwrap();
        refused_cancel(
            pic,
            id,
            id,
            args(1, 147456, 0),
            Some("migration controller required"),
        )
        .await;
        pic.set_controllers(id, None, vec![CanisterId::anonymous(), id])
            .await
            .unwrap();
        // A failed destructive attempt still leaves the original source cancellable.
        refused_import(pic, id, &b, args(1, 131072, 3)).await;
        cancel(pic, id, 1, 147456).await;
        assert_eq!(query(pic, id, "observe").await, before);
        let cancelled = query(pic, id, "checkpoint_bytes").await;
        assert_eq!(
            envelope(&cancelled, 3, 1, 131072, 147456),
            envelope(&sealed, 1, 1, 131072, 147456)
        );
        refused_import(pic, id, &b, args(1, 131072, 0)).await;
        update(pic, id, "vector_reuse").await;
        update(pic, id, "vector_append").await;
        let resumed = query(pic, id, "observe").await;
        assert_eq!(logical(&resumed), [1, 1]);
        let stable = pic.get_stable_memory(id).await;
        cancel(pic, id, 1, 147456).await;
        assert_eq!(query(pic, id, "observe").await, resumed);
        assert_eq!(pic.get_stable_memory(id).await, stable);
        pic.upgrade_eop_canister(id, reopen.clone(), vec![], None)
            .await
            .unwrap();
        cancel(pic, id, 1, 147456).await;
        assert_eq!(query(pic, id, "observe").await, resumed);
        assert_eq!(pic.get_stable_memory(id).await, stable);
        refused_import(pic, id, &b, args(1, 131072, 0)).await;
        refused_export(pic, id, id, args(1, 147456, 0)).await;
        refused_export(pic, id, id, args(3, 147456, 0)).await;
        publish(pic, id, 2, 131072, 147456).await;
        refused_cancel(pic, id, id, args(1, 147456, 0), None).await;
        import(pic, id, &b, 2, 131072, 147456).await;
        refused_cancel(pic, id, id, args(2, 147456, 0), None).await;
        publish(pic, id, 3, 147456, 131072).await;
        cancel(pic, id, 3, 131072).await;
        publish(pic, id, 4, 147456, 131072).await;
        import(pic, id, &a, 4, 147456, 131072).await;
        assert_eq!(logical(&query(pic, id, "observe").await), [1, 1]);
    }
    for corrupt_job in [true, false] {
        let id = pic.create_canister().await;
        pic.set_controllers(id, None, vec![CanisterId::anonymous(), id])
            .await
            .unwrap();
        pic.install_canister(id, candidate(false, (3, 4), false), vec![], None)
            .await;
        publish(pic, id, 1, 131072, 147456).await;
        if corrupt_job {
            pic.update_call(id, id, "damage_checkpoint", args(0, 0, 0))
                .await
                .unwrap();
        } else {
            cancel(pic, id, 1, 147456).await;
            update(pic, id, "abandon_claim").await;
        }
        // Neither corrupt stable data nor abandoned native custody may be reset.
        refused_cancel(pic, id, id, args(1, 147456, 0), None).await;
    }
    owner.close().await;
}

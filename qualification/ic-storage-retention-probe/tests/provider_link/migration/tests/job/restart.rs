//! Explicit post-cutover recovery from the independent checkpoint, not old heap data.
use super::super::installed::replace_heap_with_options;
use super::staged::{batch_args, progress, resume};
use super::*;
use ic_storage_retention_probe::artifact::{sha256, sha256_hex};

async fn refused_restart(
    pic: &PocketIc,
    id: CanisterId,
    wasm: &[u8],
    arguments: Vec<u8>,
    sender: Option<CanisterId>,
) {
    let native = query(pic, id, "observe").await;
    let stable = pic.get_stable_memory(id).await;
    let hash = stopped_hash(pic, id).await;
    let error = replace_heap_with_options(pic, id, wasm, arguments, true, sender)
        .await
        .unwrap_err();
    if sender.is_none() {
        assert!(matches!(
            error.error_code,
            ErrorCode::CanisterCalledTrap | ErrorCode::CanisterTrapped
        ));
    }
    assert_eq!(stopped_hash(pic, id).await, hash);
    assert_eq!(pic.get_stable_memory(id).await, stable);
    pic.start_canister(id, None).await.unwrap();
    assert_eq!(query(pic, id, "observe").await, native);
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "native-job-test supplies fresh compiled inputs and the prepared server"]
async fn explicit_restart_recovers_partial_heaps_but_never_completed_or_corrupt_jobs() {
    let (directory, raw, bridge) = job_inputs();
    let save = |bytes: Vec<u8>| {
        let digest = sha256_hex(&bytes);
        fs::write(directory.join(format!("restart-{digest}.wasm")), &bytes).unwrap();
        println!("restart artifact bytes={} sha256={digest}", bytes.len());
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
    for relocated in [false, true] {
        let source = if relocated { 131072 } else { 147456 };
        let target = if relocated { 147456 } else { 131072 };
        let restart = save(prepare_restart_job(&raw, &bridge, relocated));
        assert_eq!(restart, prepare_restart_job(&raw, &bridge, relocated));
        let wrong = save(prepare_restart_job(&raw, &bridge, !relocated));
        let stage = save(prepare_staged_job(&raw, &bridge, relocated, false));
        let importing = save(prepare_staged_job(&raw, &bridge, relocated, true));
        let ready = save(prepare_job(&raw, &bridge, relocated, (0, 0), false));
        for (count, damage) in [
            (0, None),
            (65, None),
            (65, Some(0)),
            (65, Some(1)),
            (65, Some(2)),
            (65, Some(3)),
            (65, Some(4)),
            (512, Some(3)),
        ] {
            let id = pic.create_canister().await;
            pic.set_controllers(id, None, vec![CanisterId::anonymous(), id])
                .await
                .unwrap();
            pic.install_canister(
                id,
                save(prepare_job(
                    &raw,
                    &bridge,
                    !relocated,
                    (count, count),
                    false,
                )),
                vec![],
                None,
            )
            .await;
            let values = logical(&query(pic, id, "observe").await);
            let sealed = publish(pic, id, 1, source, target).await;
            let checkpoint = envelope(&sealed, 1, 1, source, target);
            // This explicit mode cannot bypass the first staged-start decision.
            refused_restart(pic, id, &restart, args(1, source, 0), None).await;
            replace_heap(pic, id, &stage, args(1, source, 0))
                .await
                .unwrap();
            if count != 0 {
                pic.update_call(id, id, "import_batch", batch_args(1, source, 0, 64, 0))
                    .await
                    .unwrap();
            }
            if let Some(mode) = damage {
                pic.update_call(id, id, "damage_import", args(mode, 0, 0))
                    .await
                    .unwrap();
            }
            if damage == Some(4) {
                refused_restart(pic, id, &restart, args(1, source, 0), None).await;
                continue;
            }
            // Retention is never a substitute for an explicit fresh destination.
            frozen::refused_resume(pic, id, &restart, args(1, source, 0), None).await;
            refused_restart(pic, id, &wrong, args(1, source, 0), None).await;
            refused_restart(pic, id, &restart, args(2, source, 0), None).await;
            refused_restart(pic, id, &restart, args(1, target, 0), None).await;
            refused_restart(pic, id, &restart, args(1, source, 1), None).await;
            refused_restart(
                pic,
                id,
                &restart,
                args(1, source, 0),
                Some(CanisterId::from_slice(&[42])),
            )
            .await;
            if damage == Some(3) {
                // Even Replace cannot run the old pre-upgrade validator here.
                refused_import(pic, id, &restart, args(1, source, 0)).await;
            }
            replace_heap_with_options(pic, id, &restart, args(1, source, 0), true, None)
                .await
                .unwrap();
            assert_eq!(stopped_hash(pic, id).await, &sha256(&restart));
            pic.start_canister(id, None).await.unwrap();
            let bytes = query(pic, id, "checkpoint_bytes").await;
            assert_eq!(progress(&bytes, 4, source, target, 0, 0, 0), checkpoint);
            let observation = query(pic, id, "observe").await;
            let header = words(&observation);
            assert_eq!(
                (header[1], header[4], header[6], header[7], header[8]),
                (5, target, 0, 0, 3)
            );
            assert!(logical(&observation).is_empty());
            refused_update(pic, id, id, "vector_reuse", vec![]).await;
            // Old progress is no longer an acknowledgement, and zero is required.
            refused_update(
                pic,
                id,
                id,
                "import_batch",
                batch_args(1, source, 64, 64, 0),
            )
            .await;
            resume(pic, id, &importing, source).await;
            let mut offset = 0;
            loop {
                pic.update_call(id, id, "import_batch", batch_args(1, source, offset, 64, 0))
                    .await
                    .unwrap();
                let end = (offset + 64).min(count);
                if end == count {
                    break;
                }
                offset = end;
            }
            let observation = query(pic, id, "observe").await;
            assert_eq!(logical(&observation), values);
            assert_eq!(words(&observation)[1], 1);
            let bytes = query(pic, id, "checkpoint_bytes").await;
            assert_eq!(
                progress(&bytes, 2, source, target, count as u32, offset as u32, 64),
                checkpoint
            );
            update(pic, id, "vector_reuse").await;
            pic.upgrade_eop_canister(id, ready.clone(), vec![], None)
                .await
                .unwrap();
            // Successful publication and later native writes cannot be rolled back
            // through the recovery mode, even with the old hook skipped.
            refused_restart(pic, id, &restart, args(1, source, 0), None).await;
            assert_eq!(logical(&query(pic, id, "observe").await), [1]);
        }
    }
    owner.close().await;
}

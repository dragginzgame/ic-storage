use super::*;

pub(super) fn batch_args(epoch: u64, source: u64, offset: u64, limit: u64, fault: u64) -> Vec<u8> {
    [epoch, source, offset, limit, fault]
        .into_iter()
        .flat_map(u64::to_le_bytes)
        .collect()
}

pub(super) fn progress(
    bytes: &[u8],
    phase: u16,
    source: u64,
    target: u64,
    cursor: u32,
    start: u32,
    limit: u32,
) -> &[u8] {
    assert!((224..=4320).contains(&bytes.len()));
    assert_eq!(&bytes[..8], b"ICJOB001");
    assert_eq!(&bytes[8..10], &2u16.to_le_bytes());
    assert_eq!(&bytes[10..12], &phase.to_le_bytes());
    assert_eq!(&bytes[12..16], &64u32.to_le_bytes());
    assert_eq!(words(&bytes[16..40]), [1, source, target]);
    let len = u32::from_le_bytes(bytes[40..44].try_into().unwrap()) as usize;
    assert_eq!(bytes.len(), len + 104);
    let fields: Vec<_> = bytes[44..56]
        .as_chunks::<4>()
        .0
        .iter()
        .map(|v| u32::from_le_bytes(*v))
        .collect();
    assert_eq!(fields, [cursor, start, limit]);
    assert_eq!(&bytes[56..64], &[0; 8]);
    assert_eq!(&bytes[64 + len..72 + len], b"JOBDONE1");
    assert_eq!(
        &bytes[72 + len..],
        Sha256Digest::compute(&bytes[..72 + len]).as_bytes()
    );
    &bytes[64..64 + len]
}

pub(super) async fn refused(
    pic: &PocketIc,
    id: CanisterId,
    sender: CanisterId,
    method: &str,
    arguments: Vec<u8>,
) {
    let native = query(pic, id, "observe").await;
    let stable = pic.get_stable_memory(id).await;
    let error = pic
        .update_call(id, sender, method, arguments)
        .await
        .unwrap_err();
    assert!(
        matches!(
            error.error_code,
            ErrorCode::CanisterCalledTrap | ErrorCode::CanisterTrapped
        ),
        "{error:?}"
    );
    assert_eq!(query(pic, id, "observe").await, native);
    assert_eq!(pic.get_stable_memory(id).await, stable);
}

async fn retry(pic: &PocketIc, id: CanisterId, arguments: Vec<u8>) {
    let native = query(pic, id, "observe").await;
    let stable = pic.get_stable_memory(id).await;
    pic.update_call(id, id, "import_batch", arguments)
        .await
        .unwrap();
    assert_eq!(query(pic, id, "observe").await, native);
    assert_eq!(pic.get_stable_memory(id).await, stable);
}

pub(super) async fn resume(pic: &PocketIc, id: CanisterId, wasm: &[u8], source: u64) {
    let native = query(pic, id, "observe").await;
    let stable = pic.get_stable_memory(id).await;
    pic.upgrade_eop_canister(id, wasm.to_vec(), args(1, source, 0), None)
        .await
        .unwrap();
    assert_eq!(pic.get_stable_memory(id).await, stable);
    assert_eq!(query(pic, id, "observe").await, native);
    assert_eq!(
        stopped_hash(pic, id).await,
        Sha256Digest::compute(wasm).as_bytes()
    );
    pic.start_canister(id, None).await.unwrap();
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "native-job-test supplies fresh compiled inputs and the prepared server"]
async fn bounded_imports_resume_retry_and_publish_only_complete_vectors() {
    let (directory, _) = super::super::super::super::input();
    let raw = fs::read(directory.join("vector/provider_vector.wasm")).unwrap();
    let bridge = fs::read(directory.join("checkpoint-job.wasm")).unwrap();
    let save = |bytes: Vec<u8>| {
        let digest = Sha256Digest::compute(&bytes);
        fs::write(directory.join(format!("staged-{digest}.wasm")), &bytes).unwrap();
        println!("staged artifact bytes={} sha256={digest}", bytes.len());
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
        let stage = save(prepare_staged_job(&raw, &bridge, relocated, false));
        let importing = save(prepare_staged_job(&raw, &bridge, relocated, true));
        let wrong = save(prepare_staged_job(&raw, &bridge, !relocated, true));
        let ready = save(prepare_job(&raw, &bridge, relocated, (0, 0), false));
        let frozen = save(prepare_frozen_job(&raw, &bridge, relocated));
        let recover = save(prepare_job(&raw, &bridge, relocated, (0, 0), true));
        for count in [0, 3, 65, 512] {
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
            refused_import(pic, id, &stage, args(2, source, 0)).await;
            refused_import(pic, id, &stage, args(1, target, 0)).await;
            refused_import(pic, id, &stage, args(1, source, 1)).await;
            frozen::refused_resume(pic, id, &stage, args(1, source, 0), None).await;
            replace_heap(pic, id, &stage, args(1, source, 0))
                .await
                .unwrap();
            assert_eq!(
                stopped_hash(pic, id).await,
                Sha256Digest::compute(&stage).as_bytes()
            );
            pic.start_canister(id, None).await.unwrap();
            let bytes = query(pic, id, "checkpoint_bytes").await;
            assert_eq!(progress(&bytes, 4, source, target, 0, 0, 0), checkpoint);
            resume(pic, id, &importing, source).await;
            for candidate in [&ready, &frozen, &wrong, &stage] {
                frozen::refused_resume(pic, id, candidate, args(1, source, 0), None).await;
            }
            frozen::refused_resume(pic, id, &importing, args(2, source, 0), None).await;
            frozen::refused_resume(
                pic,
                id,
                &importing,
                args(1, source, 1),
                Some("import resume after validation"),
            )
            .await;
            refused_import(pic, id, &stage, args(1, source, 0)).await;
            refused_import(pic, id, &recover, args(1, source, 0)).await;
            for method in [
                "vector_mutate",
                "vector_append",
                "vector_remove",
                "vector_shrink",
                "vector_clear",
                "vector_reuse",
                "vector_business_error",
                "vector_forget",
                "vector_error_trap",
            ] {
                refused(pic, id, id, method, vec![]).await;
            }
            refused(pic, id, id, "cancel_checkpoint", args(1, source, 0)).await;
            refused(pic, id, id, "export_checkpoint", args(2, source, 0)).await;
            refused(
                pic,
                id,
                CanisterId::from_slice(&[42]),
                "import_batch",
                batch_args(1, source, 0, 64, 0),
            )
            .await;
            for arguments in [
                vec![],
                batch_args(2, source, 0, 64, 0),
                batch_args(1, target, 0, 64, 0),
                batch_args(1, source, 1, 64, 0),
                batch_args(1, source, 0, 0, 0),
                batch_args(1, source, 0, 65, 0),
                batch_args(1, source, u64::MAX, 64, 0),
            ] {
                refused(pic, id, id, "import_batch", arguments).await;
            }
            let mut offset = 0u64;
            loop {
                for fault in [1, 3] {
                    refused(
                        pic,
                        id,
                        id,
                        "import_batch",
                        batch_args(1, source, offset, 64, fault),
                    )
                    .await;
                }
                if count != 0 {
                    refused(
                        pic,
                        id,
                        id,
                        "import_batch",
                        batch_args(1, source, offset, 64, 2),
                    )
                    .await;
                }
                let request = batch_args(1, source, offset, 64, 0);
                pic.update_call(id, id, "import_batch", request.clone())
                    .await
                    .unwrap();
                let end = (offset + 64).min(count);
                let complete = end == count;
                let bytes = query(pic, id, "checkpoint_bytes").await;
                assert_eq!(
                    progress(
                        &bytes,
                        if complete { 2 } else { 4 },
                        source,
                        target,
                        end as u32,
                        offset as u32,
                        64
                    ),
                    checkpoint
                );
                let observation = query(pic, id, "observe").await;
                assert_eq!(logical(&observation), values[..end as usize]);
                let header = words(&observation);
                assert_eq!(
                    (header[1], header[4], header[6], header[7], header[8]),
                    (
                        if complete { 1 } else { 5 },
                        target,
                        end,
                        end,
                        if complete { 1 } else { 3 }
                    )
                );
                retry(pic, id, request.clone()).await;
                refused(
                    pic,
                    id,
                    id,
                    "import_batch",
                    batch_args(1, source, offset, 63, 0),
                )
                .await;
                if complete {
                    frozen::refused_resume(pic, id, &importing, args(1, source, 0), None).await;
                    update(pic, id, "vector_reuse").await;
                    retry(pic, id, request.clone()).await;
                    pic.upgrade_eop_canister(id, ready.clone(), vec![], None)
                        .await
                        .unwrap();
                    retry(pic, id, request).await;
                    publish(pic, id, 2, target, source).await;
                    refused(
                        pic,
                        id,
                        id,
                        "import_batch",
                        batch_args(1, source, offset, 64, 0),
                    )
                    .await;
                    break;
                }
                refused(pic, id, id, "vector_reuse", vec![]).await;
                resume(pic, id, &importing, source).await;
                retry(pic, id, request).await;
                offset = end;
            }
        }
        for mode in 0..=2 {
            let id = pic.create_canister().await;
            pic.set_controllers(id, None, vec![CanisterId::anonymous(), id])
                .await
                .unwrap();
            pic.install_canister(
                id,
                save(prepare_job(&raw, &bridge, !relocated, (65, 65), false)),
                vec![],
                None,
            )
            .await;
            publish(pic, id, 1, source, target).await;
            replace_heap(pic, id, &stage, args(1, source, 0))
                .await
                .unwrap();
            pic.update_call(id, id, "import_batch", batch_args(1, source, 0, 64, 0))
                .await
                .unwrap();
            let bytes = query(pic, id, "checkpoint_bytes").await;
            refused(
                pic,
                id,
                CanisterId::from_slice(&[42]),
                "damage_import",
                args(mode, 0, 0),
            )
            .await;
            pic.update_call(id, id, "damage_import", args(mode, 0, 0))
                .await
                .unwrap();
            assert_eq!(query(pic, id, "checkpoint_bytes").await, bytes);
            let message = if mode == 2 {
                "import prefix values mismatch"
            } else {
                "import prefix metadata mismatch"
            };
            frozen::refused_resume(pic, id, &importing, args(1, source, 0), Some(message)).await;
            refused(
                pic,
                id,
                id,
                "import_batch",
                batch_args(1, source, 64, 64, 0),
            )
            .await;
            refused(pic, id, id, "import_batch", batch_args(1, source, 0, 64, 0)).await;
            refused(pic, id, id, "vector_reuse", vec![]).await;
        }
    }
    owner.close().await;
}

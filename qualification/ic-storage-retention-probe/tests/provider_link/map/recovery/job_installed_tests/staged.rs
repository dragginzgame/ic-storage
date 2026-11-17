//! Installed bounded progress, interruption and independent-checkpoint restart.
use super::*;
use std::collections::BTreeMap;

struct Artifacts {
    source: Vec<u8>,
    stage: Vec<u8>,
    importing: Vec<u8>,
    restart: Vec<u8>,
    reopen: Vec<u8>,
    reverse: Vec<u8>,
}
fn artifacts() -> (std::path::PathBuf, Artifacts) {
    let (directory, raw) = inputs();
    let bridge = fs::read(directory.join("checkpoint-map-job.wasm")).unwrap();
    let make = |relocated, post| prepare_mode(&raw, &bridge, relocated, post, true);
    let artifacts = Artifacts {
        source: make(false, Post::Reopen),
        stage: make(true, Post::Stage),
        importing: make(true, Post::Importing),
        restart: make(true, Post::Restart),
        reopen: make(true, Post::Reopen),
        reverse: make(false, Post::Stage),
    };
    for (name, bytes) in [
        ("stage", &artifacts.stage),
        ("importing", &artifacts.importing),
        ("restart", &artifacts.restart),
        ("stage-reverse", &artifacts.reverse),
    ] {
        fs::write(directory.join(format!("{name}.wasm")), bytes).unwrap();
        println!(
            "map {name} sha256={} bytes={}",
            sha256_hex(bytes),
            bytes.len()
        );
    }
    (directory, artifacts)
}
async fn server() -> PocketIc {
    PocketIcBuilder::new()
        .with_max_request_time_ms(Some(30_000))
        .with_application_subnet()
        .with_server_url(
            std::env::var("IC_TESTKIT_POCKET_IC_URL")
                .unwrap()
                .parse()
                .unwrap(),
        )
        .build_async()
        .await
}
fn batch(epoch: u64, source: u64, start: u64, limit: u64, fault: u64) -> Vec<u8> {
    [epoch, source, start, limit, fault]
        .into_iter()
        .flat_map(u64::to_le_bytes)
        .collect()
}
fn progress(
    bytes: &[u8],
    complete: bool,
    cursor: [u32; 3],
    binding: [u64; 3],
) -> [BTreeMap<u64, u64>; 2] {
    assert!((104..=2360).contains(&bytes.len()));
    assert_eq!(&bytes[..8], b"ICJMP001");
    assert_eq!(&bytes[8..10], &2u16.to_le_bytes());
    assert_eq!(
        &bytes[10..12],
        &(if complete { 2u16 } else { 4 }).to_le_bytes()
    );
    assert_eq!(&bytes[12..16], &64u32.to_le_bytes());
    assert_eq!([word(bytes, 16), word(bytes, 24), word(bytes, 32)], binding);
    let word32 = |offset| u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap());
    assert_eq!([word32(44), word32(48), word32(52)], cursor);
    assert_eq!(&bytes[56..64], &[0; 8]);
    let len = word32(40) as usize;
    assert_eq!(len + 104, bytes.len());
    assert_eq!(&bytes[64 + len..72 + len], b"JOBDONE1");
    assert_eq!(&bytes[72 + len..], &sha256(&bytes[..72 + len]));
    checkpoint(&bytes[64..64 + len], binding[0])
}
async fn prefix(
    pic: &PocketIc,
    id: CanisterId,
    expected: &[BTreeMap<u64, u64>; 2],
    next: usize,
    complete: bool,
) {
    let bytes = query(pic, id, "observe").await;
    assert_eq!(word(&bytes, 32), 8);
    assert_eq!(word(&bytes, 8), if complete { 1 } else { 6 });
    assert_eq!(word(&bytes, 72), if complete { 1 } else { 3 });
    let counts = [
        next.min(expected[0].len()),
        next.saturating_sub(expected[0].len()),
    ];
    for id in 0..2 {
        let wanted: BTreeMap<_, _> = expected[id]
            .iter()
            .take(counts[id])
            .map(|(&key, &value)| (key, value))
            .collect();
        assert_eq!(records(&bytes, id), wanted);
    }
    if !complete {
        super::super::read_tests::check(pic, id).await;
    }
}
async fn run_batch(pic: &PocketIc, id: CanisterId, arguments: Vec<u8>) {
    pic.update_call(id, id, "import_batch", arguments)
        .await
        .unwrap();
}
async fn blocked(pic: &PocketIc, id: CanisterId) {
    for method in ["insert_a", "reset_b", "grow_a", "full_width"] {
        refused_update(pic, id, method, vec![]).await;
    }
    refused_update(pic, id, "export_checkpoint", args(2, A, 0)).await;
    refused_update(pic, id, "cancel_checkpoint", args(1, B, 0)).await;
}
#[tokio::test(flavor = "current_thread")]
#[ignore = "prepared map job artifacts and PocketIC server"]
async fn batches_validate_both_prefixes_and_retry_without_replaying_new_writes() {
    let (directory, artifacts) = artifacts();
    let pic = server().await;
    for shape in 0..4 {
        let id = install(&pic, &artifacts.source).await;
        authorize(&pic, id).await;
        match shape {
            0 => {}
            1 => update(&pic, id, "insert_b").await,
            2 => update(&pic, id, "insert_a").await,
            3 => {
                update(&pic, id, "fill_checkpoint").await;
                update(&pic, id, "full_width").await;
            }
            _ => unreachable!(),
        }
        let saved = publish(&pic, id, 1, A, B).await;
        let expected = checkpoint(envelope(&saved, 1, 1, A, B), 1);
        let total = expected[0].len() + expected[1].len();
        refused_upgrade(&pic, id, &artifacts.restart, args(1, A, 0), false).await;
        if shape == 3 {
            refused_upgrade(&pic, id, &artifacts.stage, args(1, A, 0), true).await;
            for arguments in [
                args(2, A, 0),
                args(1, B, 0),
                args(1, A, 1),
                args(1, A, 2),
                args(1, A, 3),
                args(1, A, 5),
            ] {
                refused_upgrade(&pic, id, &artifacts.stage, arguments, false).await;
            }
        }
        replace_heap(&pic, id, &artifacts.stage, args(1, A, 0))
            .await
            .unwrap();
        assert_eq!(
            progress(
                &query(&pic, id, "checkpoint_bytes").await,
                false,
                [0, 0, 0],
                [1, A, B]
            ),
            expected
        );
        prefix(&pic, id, &expected, 0, false).await;
        blocked(&pic, id).await;
        assert!(
            query(&pic, id, "discarded_payload")
                .await
                .iter()
                .all(|byte| *byte == 0)
        );
        let mut next = 0;
        let mut step = 0;
        let mut last = Vec::new();
        loop {
            // Cross root 1 -> root 2 at 63+2 and finish with a short batch.
            let limit = if shape == 3 { [63, 2, 64][step] } else { 1 };
            let request = batch(1, A, next as u64, limit, 0);
            if shape == 3 {
                for arguments in [
                    batch(1, A, next as u64, 0, 0),
                    batch(1, A, next as u64, 65, 0),
                    batch(1, B, next as u64, limit, 0),
                    batch(2, A, next as u64, limit, 0),
                    batch(1, A, next as u64 + 1, limit, 0),
                    vec![],
                ] {
                    refused_update(&pic, id, "import_batch", arguments).await;
                }
                for fault in 1..=7 {
                    refused_update(
                        &pic,
                        id,
                        "import_batch",
                        batch(1, A, next as u64, limit, fault),
                    )
                    .await;
                }
                if step == 1 {
                    pic.set_controllers(id, None, vec![CanisterId::anonymous()])
                        .await
                        .unwrap();
                    refused_update(&pic, id, "import_batch", request.clone()).await;
                    refused_update(&pic, id, "damage_import", args(1, 0, 0)).await;
                    authorize(&pic, id).await;
                }
            }
            run_batch(&pic, id, request.clone()).await;
            let end = (next + limit as usize).min(total);
            let complete = end == total;
            let bytes = query(&pic, id, "checkpoint_bytes").await;
            assert_eq!(
                progress(
                    &bytes,
                    complete,
                    [end as u32, next as u32, limit as u32],
                    [1, A, B]
                ),
                expected
            );
            prefix(&pic, id, &expected, end, complete).await;
            let before = query(&pic, id, "observe").await;
            run_batch(&pic, id, request.clone()).await;
            assert_eq!(query(&pic, id, "observe").await, before);
            assert_eq!(query(&pic, id, "checkpoint_bytes").await, bytes);
            if !last.is_empty() {
                refused_update(&pic, id, "import_batch", last).await;
            }
            last = request;
            if complete {
                break;
            }
            blocked(&pic, id).await;
            refused_upgrade(&pic, id, &artifacts.reopen, vec![], true).await;
            for arguments in [args(2, A, 0), args(1, B, 0), args(1, A, 1)] {
                refused_upgrade(&pic, id, &artifacts.importing, arguments, true).await;
            }
            pic.upgrade_eop_canister(id, artifacts.importing.clone(), args(1, A, 0), None)
                .await
                .unwrap();
            assert_eq!(query(&pic, id, "observe").await, before);
            assert_eq!(query(&pic, id, "checkpoint_bytes").await, bytes);
            next = end;
            step += 1;
        }
        // Completion is irreversible and its exact retry must preserve new writes.
        let consumed = query(&pic, id, "checkpoint_bytes").await;
        update(&pic, id, "reset_a").await;
        update(&pic, id, "insert_a").await;
        let newer = query(&pic, id, "observe").await;
        pic.upgrade_eop_canister(id, artifacts.reopen.clone(), vec![], None)
            .await
            .unwrap();
        run_batch(&pic, id, last.clone()).await;
        assert_eq!(query(&pic, id, "observe").await, newer);
        assert_eq!(query(&pic, id, "checkpoint_bytes").await, consumed);
        refused_upgrade(&pic, id, &artifacts.restart, args(1, A, 0), false).await;
        refused_upgrade(&pic, id, &artifacts.importing, args(1, A, 0), true).await;
        refused_update(&pic, id, "cancel_checkpoint", args(1, A, 0)).await;
        if shape == 3 {
            let reverse = publish(&pic, id, 2, B, A).await;
            let expected = checkpoint(envelope(&reverse, 1, 2, B, A), 2);
            replace_heap(&pic, id, &artifacts.reverse, args(2, B, 0))
                .await
                .unwrap();
            run_batch(&pic, id, batch(2, B, 0, 64, 0)).await;
            run_batch(&pic, id, batch(2, B, 64, 64, 0)).await;
            prefix(&pic, id, &expected, 65, true).await;
            refused_update(&pic, id, "import_batch", last).await;
        }
    }
    pic.drop().await;
    fs::write(directory.join("STAGED_MAP_RECOVERED"), "map prefixes, bounded batches, authority, retries, interruption and publication rollback passed\n").unwrap();
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "prepared map job artifacts and PocketIC server"]
async fn restart_rebuilds_only_unfinished_imports_from_the_independent_checkpoint() {
    let (directory, artifacts) = artifacts();
    let pic = server().await;
    for mode in 0..=5 {
        let id = install(&pic, &artifacts.source).await;
        authorize(&pic, id).await;
        update(&pic, id, "fill_checkpoint").await;
        update(&pic, id, "full_width").await;
        let bytes = publish(&pic, id, 1, A, B).await;
        let expected = checkpoint(envelope(&bytes, 1, 1, A, B), 1);
        replace_heap(&pic, id, &artifacts.stage, args(1, A, 0))
            .await
            .unwrap();
        run_batch(&pic, id, batch(1, A, 0, 64, 0)).await;
        run_batch(&pic, id, batch(1, A, 64, 1, 0)).await;
        if mode < 5 {
            pic.update_call(id, id, "damage_import", args(mode, 0, 0))
                .await
                .unwrap();
            refused_update(&pic, id, "import_batch", batch(1, A, 65, 64, 0)).await;
            refused_upgrade(&pic, id, &artifacts.importing, args(1, A, 0), true).await;
        }
        // Healthy/native-value-corrupt prefixes can run their source hook.
        let skip = mode == 0 || mode == 2;
        if skip {
            refused_upgrade(&pic, id, &artifacts.restart, args(1, A, 0), false).await;
        }
        for arguments in [
            args(2, A, 0),
            args(1, B, 0),
            args(1, A, 1),
            args(1, A, 2),
            args(1, A, 3),
            args(1, A, 5),
        ] {
            let native = query(&pic, id, "observe").await;
            let stable = pic.get_stable_memory(id).await;
            let hash = stopped_hash(&pic, id).await;
            trapped(
                &replace_heap_with_options(&pic, id, &artifacts.restart, arguments, skip, None)
                    .await
                    .unwrap_err(),
            );
            assert_eq!(stopped_hash(&pic, id).await, hash);
            pic.start_canister(id, None).await.unwrap();
            assert_eq!(query(&pic, id, "observe").await, native);
            assert_eq!(pic.get_stable_memory(id).await, stable);
        }
        if mode == 3 || mode == 4 {
            refused_upgrade(&pic, id, &artifacts.restart, args(1, A, 0), false).await;
            continue;
        }
        replace_heap_with_options(&pic, id, &artifacts.restart, args(1, A, 0), skip, None)
            .await
            .unwrap();
        prefix(&pic, id, &expected, 0, false).await;
        assert_eq!(
            progress(
                &query(&pic, id, "checkpoint_bytes").await,
                false,
                [0, 0, 0],
                [1, A, B]
            ),
            expected
        );
        refused_update(&pic, id, "import_batch", batch(1, A, 64, 1, 0)).await;
        blocked(&pic, id).await;
        run_batch(&pic, id, batch(1, A, 0, 64, 0)).await;
        run_batch(&pic, id, batch(1, A, 64, 64, 0)).await;
        prefix(&pic, id, &expected, 128, true).await;
        refused_upgrade(&pic, id, &artifacts.restart, args(1, A, 0), false).await;
    }
    pic.drop().await;
    fs::write(
        directory.join("STAGED_MAP_RESTARTED"),
        "map restart, corruption refusal, explicit source-hook bypass and rollback passed\n",
    )
    .unwrap();
}

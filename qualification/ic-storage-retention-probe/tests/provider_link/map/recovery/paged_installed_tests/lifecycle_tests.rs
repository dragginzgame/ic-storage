//! Cancellation and repeated recovery against one installed governed namespace.
use super::*;

fn batch_epoch(epoch: u64, source: u64, start: u64) -> Vec<u8> {
    [epoch, source, start, 1, 0]
        .into_iter()
        .flat_map(u64::to_le_bytes)
        .collect()
}

async fn complete(
    pic: &PocketIc,
    id: CanisterId,
    expected: &[BTreeMap<u64, u64>; 2],
    identity: [u64; 3],
) -> Vec<u8> {
    let [epoch, source, target] = identity;
    let pages: usize = expected.iter().map(|root| root.len().div_ceil(64)).sum();
    for start in 0..pages.max(1) {
        let arguments = batch_epoch(epoch, source, start as u64);
        pic.update_call(id, id, "import_batch", arguments.clone())
            .await
            .unwrap();
        let end = (start + 1).min(pages);
        prefix(pic, id, expected, end, end == pages).await;
        let stable = pic.get_stable_memory(id).await;
        let native = query(pic, id, "observe").await;
        snapshot_epoch(
            &query(pic, id, "checkpoint_bytes").await,
            if end == pages { 2 } else { 4 },
            [end as u32, start as u32, 1],
            [epoch, source, target],
        );
        pic.update_call(id, id, "import_batch", arguments)
            .await
            .unwrap();
        assert_eq!(pic.get_stable_memory(id).await, stable);
        assert_eq!(query(pic, id, "observe").await, native);
    }
    batch_epoch(epoch, source, pages.saturating_sub(1) as u64)
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "prepared paged-native artifacts and PocketIC server"]
async fn cancel_and_repeated_epochs_preserve_authority_newer_writes_and_old_checkpoints_on_traps() {
    let (dir, a) = artifacts();
    let raw = fs::read(dir.join("provider_map.wasm")).unwrap();
    let bridge = fs::read(dir.join("checkpoint-map-paged.wasm")).unwrap();
    let reverse_stage = prepare_mode(&raw, &bridge, false, Post::Stage, Profile::Paged);
    let reverse_frozen = prepare_mode(&raw, &bridge, true, Post::Frozen, Profile::Paged);
    for (name, bytes) in [
        ("reverse-stage", &reverse_stage),
        ("reverse-frozen", &reverse_frozen),
    ] {
        fs::write(dir.join(format!("paged-{name}.wasm")), bytes).unwrap();
        println!(
            "paged native {name}: {} bytes sha256={}",
            bytes.len(),
            sha256_hex(bytes)
        );
    }
    let pic = server().await;
    let id = install(&pic, &a.source).await;
    authorize(&pic, id).await;
    refused_update(&pic, id, "cancel_checkpoint", args(1, B, 0)).await;
    update(&pic, id, "pages_large").await;
    publish(&pic, id).await;
    pic.set_controllers(id, None, vec![CanisterId::anonymous()])
        .await
        .unwrap();
    refused_update(&pic, id, "cancel_checkpoint", args(1, B, 0)).await;
    authorize(&pic, id).await;
    for arguments in [args(2, B, 0), args(1, A, 0), args(1, B, 1), args(1, B, 2)] {
        refused_update(&pic, id, "cancel_checkpoint", arguments).await;
    }
    pic.upgrade_eop_canister(id, a.frozen.clone(), args(1, B, 0), None)
        .await
        .unwrap();
    pic.update_call(id, id, "cancel_checkpoint", args(1, B, 0))
        .await
        .unwrap();
    snapshot_epoch(
        &query(&pic, id, "checkpoint_bytes").await,
        3,
        [0, 0, 0],
        [1, A, B],
    );
    refused_upgrade(&pic, id, &a.stage, args(1, A, 0), false).await;
    pic.upgrade_eop_canister(id, a.source.clone(), vec![], None)
        .await
        .unwrap();
    for method in ["reset_a", "reset_b", "pages_partial"] {
        update(&pic, id, method).await;
    }
    let native = query(&pic, id, "observe").await;
    let stable = pic.get_stable_memory(id).await;
    pic.update_call(id, id, "cancel_checkpoint", args(1, B, 0))
        .await
        .unwrap();
    assert_eq!(query(&pic, id, "observe").await, native);
    assert_eq!(pic.get_stable_memory(id).await, stable);

    // Replace 72 pages with five, preserving the old checkpoint on every trap.
    for fault in 1..=4 {
        refused_update(&pic, id, "export_checkpoint", args(2, B, fault)).await;
    }
    for epoch in [1, 3] {
        refused_update(&pic, id, "export_checkpoint", args(epoch, B, 0)).await;
    }
    let expected = publish_epoch(&pic, id, 2, A, B).await;
    refused_update(&pic, id, "cancel_checkpoint", args(1, B, 0)).await;
    refused_upgrade(&pic, id, &a.stage, args(1, A, 0), false).await;
    replace_heap(&pic, id, &a.stage, args(2, A, 0))
        .await
        .unwrap();
    refused_update(&pic, id, "cancel_checkpoint", args(2, A, 0)).await;
    refused_update(&pic, id, "export_checkpoint", args(3, A, 0)).await;
    refused_update(&pic, id, "import_batch", batch_epoch(1, A, 0)).await;
    let completed_two = complete(&pic, id, &expected, [2, A, B]).await;
    refused_update(&pic, id, "cancel_checkpoint", args(2, A, 0)).await;
    pic.upgrade_eop_canister(id, a.reopen.clone(), vec![], None)
        .await
        .unwrap();
    update(&pic, id, "replace_a").await;
    let newer = query(&pic, id, "observe").await;
    pic.update_call(id, id, "import_batch", completed_two.clone())
        .await
        .unwrap();
    assert_eq!(query(&pic, id, "observe").await, newer);

    // Reverse direction, including removal of every previous page. Fault 1 now
    // occurs after a pop; all faults must restore the old vector length and data.
    update(&pic, id, "reset_a").await;
    update(&pic, id, "reset_b").await;
    for fault in 1..=4 {
        refused_update(&pic, id, "export_checkpoint", args(3, A, fault)).await;
    }
    let empty = publish_epoch(&pic, id, 3, B, A).await;
    assert_eq!(empty.each_ref().map(BTreeMap::len), [0, 0]);
    pic.upgrade_eop_canister(id, reverse_frozen, args(3, A, 0), None)
        .await
        .unwrap();
    replace_heap(&pic, id, &reverse_stage, args(3, B, 0))
        .await
        .unwrap();
    let completed_three = complete(&pic, id, &empty, [3, B, A]).await;
    refused_update(&pic, id, "import_batch", completed_two.clone()).await;
    pic.upgrade_eop_canister(id, a.source.clone(), vec![], None)
        .await
        .unwrap();
    update(&pic, id, "pages_full").await;
    let newer = query(&pic, id, "observe").await;
    pic.update_call(id, id, "import_batch", completed_three.clone())
        .await
        .unwrap();
    assert_eq!(query(&pic, id, "observe").await, newer);
    // Grow the same page vector again. No stale completed retry can republish
    // roots from an earlier epoch, even after a subsequent recovery completes.
    for fault in 1..=4 {
        refused_update(&pic, id, "export_checkpoint", args(4, B, fault)).await;
    }
    let expected = publish_epoch(&pic, id, 4, A, B).await;
    replace_heap(&pic, id, &a.stage, args(4, A, 0))
        .await
        .unwrap();
    complete(&pic, id, &expected, [4, A, B]).await;
    for stale in [completed_two, completed_three] {
        refused_update(&pic, id, "import_batch", stale).await;
    }
    fs::write(
        dir.join("PAGED_NATIVE_LIFECYCLE"),
        b"cancellation and four alternating epochs passed\n",
    )
    .unwrap();
    pic.drop().await;
}

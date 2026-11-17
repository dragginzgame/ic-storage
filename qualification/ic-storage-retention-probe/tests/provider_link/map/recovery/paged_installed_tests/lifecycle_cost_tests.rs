//! Extend the initial cost cohort through later epochs in the same namespace.
use super::*;
use cost_tests::record;

fn batch(epoch: u64, start: usize) -> Vec<u8> {
    [epoch, B, start as u64, 1, 0]
        .into_iter()
        .flat_map(u64::to_le_bytes)
        .collect()
}

pub(super) async fn followup(
    pic: &PocketIc,
    id: CanisterId,
    dir: &std::path::Path,
    shape: &str,
    reverse_stage: &[u8],
) {
    let path = dir.join(format!("paged-lifecycle-costs-{shape}.tsv"));
    let mut report = costs::HEADER.to_owned();
    // The caller completed epoch 1 into B. Change authoritative values before
    // retiring its consumed continuation, keeping each measured shape's count.
    if shape != "0+0" {
        update(pic, id, "replace_a").await;
    }
    let expected = publish_epoch(pic, id, 2, B, A).await;
    let pages: usize = expected.iter().map(|root| root.len().div_ceil(64)).sum();
    let values = record(
        pic,
        id,
        &path,
        &mut report,
        shape,
        "export_after_consumed",
        2,
    )
    .await;
    assert_eq!(values[6], 944 + pages as u64 * 1104); // Existing slots, no length writes.
    assert_eq!(values[7], 0);
    pic.update_call(id, id, "cancel_checkpoint", args(2, A, 0))
        .await
        .unwrap();
    let values = record(pic, id, &path, &mut report, shape, "cancel", 2).await;
    assert_eq!(&values[5..9], &[1, 104, 0, 0]);
    if shape != "0+0" {
        update(pic, id, "replace_b").await;
    }
    let native = query(pic, id, "observe").await;
    let stable = pic.get_stable_memory(id).await;
    pic.update_call(id, id, "cancel_checkpoint", args(2, A, 0))
        .await
        .unwrap();
    let retry = record(pic, id, &path, &mut report, shape, "cancel_retry", 2).await;
    assert_eq!(&retry[5..9], &[0; 4]);
    assert_eq!(query(pic, id, "observe").await, native);
    assert_eq!(pic.get_stable_memory(id).await, stable);

    let expected = publish_epoch(pic, id, 3, B, A).await;
    let values = record(
        pic,
        id,
        &path,
        &mut report,
        shape,
        "export_after_cancelled",
        3,
    )
    .await;
    assert_eq!(values[6], 944 + pages as u64 * 1104);
    assert_eq!(values[7], 0);
    replace_heap(pic, id, reverse_stage, args(3, B, 0))
        .await
        .unwrap();
    record(pic, id, &path, &mut report, shape, "reverse_start", 3).await;
    for page in 0..pages.max(1) {
        pic.update_call(id, id, "import_batch", batch(3, page))
            .await
            .unwrap();
        let values = record(pic, id, &path, &mut report, shape, "reverse_batch", page).await;
        assert_eq!(values[6], 616);
        // Two exact layout/record reopens, plus one page for a nonempty batch.
        assert_eq!(
            values[4],
            2 * (128 + 48 + 24 + 328 + 104 + 512) + if pages == 0 { 0 } else { 1104 }
        );
        assert_eq!(values[7], 0);
        prefix(pic, id, &expected, (page + 1).min(pages), page + 1 >= pages).await;
        if [0, 6, 12, 24, 47, 48, 71].contains(&page) {
            let native = query(pic, id, "observe").await;
            let stable = pic.get_stable_memory(id).await;
            pic.update_call(id, id, "import_batch", batch(3, page))
                .await
                .unwrap();
            let retry = record(pic, id, &path, &mut report, shape, "reverse_retry", page).await;
            assert_eq!(&retry[5..9], &[0; 4]);
            assert_eq!(query(pic, id, "observe").await, native);
            assert_eq!(pic.get_stable_memory(id).await, stable);
        }
    }
    // A consumed retry acknowledges publication without overwriting newer roots.
    // Earlier replacement set key 7 to 71; insert sets it back to 70 (or adds
    // it to the empty shape). Prove a changed logical value, not just a call.
    update(pic, id, "insert_a").await;
    let native = query(pic, id, "observe").await;
    assert_ne!(records(&native, 0), expected[0]);
    let stable = pic.get_stable_memory(id).await;
    pic.update_call(id, id, "import_batch", batch(3, pages.saturating_sub(1)))
        .await
        .unwrap();
    let values = record(
        pic,
        id,
        &path,
        &mut report,
        shape,
        "completed_retry_after_write",
        3,
    )
    .await;
    assert_eq!(&values[5..9], &[0; 4]);
    assert_eq!(query(pic, id, "observe").await, native);
    assert_eq!(pic.get_stable_memory(id).await, stable);

    // Exercise truncation and then reuse of the same physical page allocation.
    update(pic, id, "reset_a").await;
    update(pic, id, "reset_b").await;
    let empty = publish_epoch(pic, id, 4, A, B).await;
    assert!(empty.iter().all(BTreeMap::is_empty));
    let values = record(
        pic,
        id,
        &path,
        &mut report,
        shape,
        "export_shrink_to_empty",
        4,
    )
    .await;
    assert_eq!(values[6], 944 + pages as u64 * 8); // Each pop commits only the length.
    assert_eq!(values[7], 0);
    pic.update_call(id, id, "cancel_checkpoint", args(4, B, 0))
        .await
        .unwrap();
    let values = record(pic, id, &path, &mut report, shape, "cancel_empty", 4).await;
    assert_eq!(&values[5..9], &[1, 104, 0, 0]);
    let fill = match shape {
        "0+0" => "insert_a",
        "192+192" => "pages_full",
        "3072+1536" => "pages_large",
        _ => unreachable!(),
    };
    update(pic, id, fill).await;
    let expected = publish_epoch(pic, id, 5, A, B).await;
    let new_pages: usize = expected.iter().map(|root| root.len().div_ceil(64)).sum();
    assert_eq!(new_pages, pages.max(1));
    let values = record(
        pic,
        id,
        &path,
        &mut report,
        shape,
        "export_grow_from_empty",
        5,
    )
    .await;
    assert_eq!(values[6], 944 + new_pages as u64 * 1112); // Page plus appended length.
    assert_eq!(values[7], 0);
}

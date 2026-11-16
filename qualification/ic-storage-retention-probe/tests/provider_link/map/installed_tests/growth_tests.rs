//! Provider page growth happens only between loans; failures preserve pages too.
use super::*;

async fn expand(pic: &PocketIc, id: CanisterId, failing_upgrade: &[u8]) {
    let before = observe(pic, id).await;
    for fault in 1..=4 {
        rejected_update(pic, id, &format!("pool_failure_{fault}")).await;
    }
    rejected_upgrade(pic, id, failing_upgrade).await;
    update(pic, id, "pool_grow").await;
    let after = observe(pic, id).await;
    assert_eq!(after.root[3], before.root[3] * 2);
    assert_eq!(after.pages, (131072 + after.root[3]).div_ceil(65536));
    let mut expected = before.root;
    expected[3] *= 2;
    assert_eq!(after.root, expected);
    for index in 0..2 {
        after.unchanged_map(&before, index);
    }
    assert_eq!(
        after.payload[..before.root[3] as usize],
        before.payload[..before.root[3] as usize]
    );
    assert!(
        after.payload[before.root[3] as usize..after.root[3] as usize]
            .iter()
            .all(|byte| *byte == 0)
    );
}

async fn grow_map(pic: &PocketIc, id: CanisterId, index: usize, suffix: &str) {
    let before = observe(pic, id).await;
    rejected_update(pic, id, &format!("write_failure_{suffix}")).await;
    update(pic, id, &format!("grow_{suffix}")).await;
    let after = observe(pic, id).await;
    assert_eq!(after.buckets(index), before.buckets(index) * 2);
    let mut expected = before.records(index);
    expected.insert(65535, 999);
    assert_eq!(after.records(index), expected);
    after.unchanged_map(&before, 1 - index);
    assert_eq!(
        after.payload[before.span(index)],
        before.payload[before.span(index)]
    );
    assert_eq!(after.pages, before.pages);
    update(pic, id, &format!("remove_growth_{suffix}")).await;
    update(pic, id, &format!("fill_{suffix}")).await;
    let full = observe(pic, id).await;
    let count = full.buckets(index) as u64 * 3 / 4;
    assert_eq!(
        full.records(index),
        (0..count).map(|key| (key, key + 1000)).collect()
    );
}

async fn refuse_map(pic: &PocketIc, id: CanisterId, suffix: &str) {
    let before = observe(pic, id).await;
    update(pic, id, &format!("refuse_growth_{suffix}")).await;
    assert_eq!(observe(pic, id).await, before);
    rejected_update(pic, id, &format!("grow_{suffix}")).await;
}

pub(super) async fn qualify(pic: &PocketIc, directory: &Path, raw: &[u8]) {
    let a = candidate(
        directory,
        raw,
        Profile {
            buckets: 512,
            ..Profile::A
        },
    );
    let b = candidate(directory, raw, Profile::B);
    let failing = candidate(
        directory,
        raw,
        Profile {
            post_pool_trap: true,
            ..Profile::B
        },
    );
    let id = pic.create_canister().await;
    pic.install_canister(id, a.clone(), vec![], None).await;
    update(pic, id, "fill_a").await;
    update(pic, id, "fill_b").await;
    expand(pic, id, &failing).await; // 1024 -> 2048 Slots; fits existing third page.
    grow_map(pic, id, 0, "a").await;
    refuse_map(pic, id, "b").await;
    retained_upgrade(pic, id, &b, 2).await;

    expand(pic, id, &failing).await; // 2048 -> 4096 Slots; grows a fourth page.
    grow_map(pic, id, 1, "b").await;
    // Enough total free space but no contiguous 2048-slot candidate.
    refuse_map(pic, id, "a").await;
    let before = observe(pic, id).await;
    update(pic, id, "reset_b").await;
    observe(pic, id).await.unchanged_map(&before, 0);
    grow_map(pic, id, 0, "a").await;
    retained_upgrade(pic, id, &a, 1).await;

    expand(pic, id, &failing).await; // 4096 -> 8192 Slots; grows a fifth page.
    grow_map(pic, id, 0, "a").await;
    update(pic, id, "fill_b").await;
    while observe(pic, id).await.buckets(1) < 2048 {
        grow_map(pic, id, 1, "b").await;
    }
    let full = observe(pic, id).await;
    assert_eq!((full.pages, full.root[3]), (5, 196608));
    assert_eq!((full.records(0).len(), full.records(1).len()), (3072, 1536));
    refuse_map(pic, id, "a").await;
    refuse_map(pic, id, "b").await;
    update(pic, id, "pool_refuse").await;
    assert_eq!(observe(pic, id).await, full);
    rejected_update(pic, id, "pool_grow").await;
    retained_upgrade(pic, id, &b, 2).await;
    retained_upgrade(pic, id, &a, 1).await;
    for (index, suffix) in [(0, "a"), (1, "b")] {
        let before = observe(pic, id).await;
        update(pic, id, &format!("lookup_full_{suffix}")).await;
        update(pic, id, &format!("delete_full_{suffix}")).await;
        update(pic, id, &format!("grow_{suffix}")).await;
        update(pic, id, &format!("remove_growth_{suffix}")).await;
        update(pic, id, &format!("fill_{suffix}")).await;
        observe(pic, id).await.unchanged_map(&before, 1 - index);
    }
    // Backshift deletion and reinsertion may change valid probe placement.
    // Compare logical contents, capacity and page count after successful writes.
    let reused = observe(pic, id).await;
    assert_eq!(reused.root, full.root);
    assert_eq!(reused.pages, full.pages);
    for index in 0..2 {
        assert_eq!(reused.records(index), full.records(index));
    }
    update(pic, id, "reset_a").await;
    update(pic, id, "reset_b").await;
    let empty = observe(pic, id).await;
    assert!(empty.records(0).is_empty() && empty.records(1).is_empty());
    assert_eq!((empty.pages, empty.root[3]), (5, 196608)); // Reuse does not shrink memory.
    update(pic, id, "insert_a").await;
    update(pic, id, "replace_a").await;
    update(pic, id, "lookup_a").await;

    // Actual memory.grow returns -1 under the module maximum. Refusal must be
    // ordinary, leave the complete image intact, and keep both roots usable.
    let bounded = pic.create_canister().await;
    let bounded_code = candidate(
        directory,
        raw,
        Profile {
            max_pages: 3,
            ..Profile::A
        },
    );
    pic.install_canister(bounded, bounded_code, vec![], None)
        .await;
    update(pic, bounded, "insert_a").await;
    update(pic, bounded, "replace_a").await;
    update(pic, bounded, "pool_grow").await;
    let before = observe(pic, bounded).await;
    assert_eq!((before.pages, before.root[3]), (3, 49152));
    update(pic, bounded, "pool_refuse").await;
    assert_eq!(observe(pic, bounded).await, before);
    rejected_update(pic, bounded, "pool_grow").await;
    update(pic, bounded, "lookup_a").await;
    update(pic, bounded, "insert_b").await;
}

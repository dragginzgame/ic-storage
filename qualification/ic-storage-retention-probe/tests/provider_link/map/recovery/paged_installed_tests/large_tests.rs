//! Larger native prefixes expand the private loan within the batch transaction.
use super::*;

#[tokio::test(flavor = "current_thread")]
#[ignore = "prepared paged-native artifacts and PocketIC server"]
async fn large_private_import_grows_rolls_back_restarts_and_preserves_later_writes() {
    let (dir, a) = artifacts();
    let pic = server().await;
    for (method, counts) in [
        ("pages_large", [3072, 1536]),
        ("pages_large_empty_a", [0, 1536]),
        ("pages_large_empty_b", [3072, 0]),
    ] {
        let id = install(&pic, &a.source).await;
        authorize(&pic, id).await;
        update(&pic, id, method).await;
        for (root, suffix) in [(0, "a"), (1, "b")] {
            if counts[root] != 0 {
                for operation in ["replace", "lookup", "delete", "insert"] {
                    update(&pic, id, &format!("{operation}_{suffix}")).await;
                }
            }
        }
        let expected = publish(&pic, id).await;
        assert_eq!(expected.each_ref().map(BTreeMap::len), counts);
        pic.upgrade_eop_canister(id, a.frozen.clone(), args(1, B, 0), None)
            .await
            .unwrap();
        replace_heap(&pic, id, &a.stage, args(1, A, 0))
            .await
            .unwrap();
        let pages = counts
            .into_iter()
            .map(|count| count.div_ceil(64))
            .sum::<usize>();
        let mut expansions = Vec::new();
        let mut restarted = false;
        let mut start = 0;
        while start < pages {
            let before = query(&pic, id, "observe").await;
            let prior_bytes = word(&before, 48);
            if method == "pages_large" && [6, 12, 24].contains(&start) && !restarted {
                let window = query(&pic, id, "pool_window").await;
                for fault in [1, 4, 9, 10, 11, 12, 13, 14] {
                    refused_update(&pic, id, "import_batch", batch(start as u64, fault)).await;
                    assert_eq!(query(&pic, id, "pool_window").await, window);
                }
            }
            if method == "pages_large" && [47, 48, 71].contains(&start) {
                for fault in [3, 6, 8, 9, 10] {
                    refused_update(&pic, id, "import_batch", batch(start as u64, fault)).await;
                }
            }
            refused_update(&pic, id, "pool_grow", vec![]).await;
            run(&pic, id, start as u64, 0).await;
            let native = query(&pic, id, "observe").await;
            let capacity = word(&native, 48);
            if capacity != prior_bytes {
                assert_eq!(capacity, prior_bytes * 2);
                expansions.push((start, capacity));
            }
            let end = start + 1;
            snapshot(
                &query(&pic, id, "checkpoint_bytes").await,
                if end == pages { 2 } else { 4 },
                [end as u32, start as u32, 1],
            );
            prefix(&pic, id, &expected, end, end == pages).await;
            if method == "pages_large" && [6, 12, 24, 47, 48, 71].contains(&start) {
                let stable = pic.get_stable_memory(id).await;
                run(&pic, id, start as u64, 14).await;
                assert_eq!(query(&pic, id, "observe").await, native);
                assert_eq!(pic.get_stable_memory(id).await, stable);
                if end != pages {
                    pic.upgrade_eop_canister(id, a.importing.clone(), args(1, A, 0), None)
                        .await
                        .unwrap();
                    assert_eq!(query(&pic, id, "observe").await, native);
                }
            }
            if method == "pages_large" && start == 24 && !restarted {
                assert_eq!(expansions, [(6, 49152), (12, 98304), (24, 196608)]);
                pic.update_call(id, id, "damage_import", args(1, 0, 0))
                    .await
                    .unwrap();
                refused_update(&pic, id, "import_batch", batch(25, 0)).await;
                replace_heap(&pic, id, &a.restart, args(1, A, 0))
                    .await
                    .unwrap();
                prefix(&pic, id, &expected, 0, false).await;
                assert_eq!(word(&query(&pic, id, "observe").await, 48), 24576);
                restarted = true;
                expansions.clear();
                start = 0;
            } else {
                start = end;
            }
        }
        if method == "pages_large" {
            assert_eq!(expansions, [(6, 49152), (12, 98304), (24, 196608)]);
        }
        pic.upgrade_eop_canister(id, a.reopen.clone(), vec![], None)
            .await
            .unwrap();
        prefix(&pic, id, &expected, pages, true).await;
        let checkpoint = query(&pic, id, "checkpoint_bytes").await;
        for (root, suffix) in [(0, "a"), (1, "b")] {
            if counts[root] != 0 {
                update(&pic, id, &format!("replace_{suffix}")).await;
                update(&pic, id, &format!("lookup_{suffix}")).await;
                update(&pic, id, &format!("delete_{suffix}")).await;
            }
        }
        let newer = query(&pic, id, "observe").await;
        for (root, values) in expected.iter().enumerate() {
            let mut values = values.clone();
            values.remove(&7);
            assert_eq!(records(&newer, root), values);
        }
        run(&pic, id, pages as u64 - 1, 0).await;
        assert_eq!(query(&pic, id, "observe").await, newer);
        assert_eq!(query(&pic, id, "checkpoint_bytes").await, checkpoint);
    }
    fs::write(
        dir.join("PAGED_NATIVE_LARGE"),
        b"4608-record private growth and recovery passed\n",
    )
    .unwrap();
    pic.drop().await;
}

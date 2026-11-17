//! Combined retained-root, continuation and progress transactions on the IC.
mod growth_tests;
use super::{
    Post, Profile, inputs,
    installed_tests::{query, records, word},
    job_installed_tests::{args, authorize, install, refused_update, refused_upgrade},
    prepare_mode, read_tests,
};
use crate::{
    installed_tests::update,
    migration::tests::installed::{replace_heap, replace_heap_with_options},
};
use ic_storage_retention_probe::artifact::{sha256, sha256_hex};
use ic_testkit::pocket_ic::{
    CanisterId, PocketIcBuilder, common::rest::BlobCompression, nonblocking::PocketIc,
};
use std::{collections::BTreeMap, fs, path::PathBuf};
use wasmparser::{Operator, Parser, Payload};

const A: u64 = 131072;
const B: u64 = 393216;
struct Artifacts {
    source: Vec<u8>,
    frozen: Vec<u8>,
    stage: Vec<u8>,
    importing: Vec<u8>,
    restart: Vec<u8>,
    reopen: Vec<u8>,
}
fn artifacts() -> (PathBuf, Artifacts) {
    let (dir, raw) = inputs();
    let bridge = fs::read(dir.join("checkpoint-map-paged.wasm")).unwrap();
    let make = |relocated, post| prepare_mode(&raw, &bridge, relocated, post, Profile::Paged);
    let a = Artifacts {
        source: make(false, Post::Reopen),
        frozen: make(false, Post::Frozen),
        stage: make(true, Post::Stage),
        importing: make(true, Post::Importing),
        restart: make(true, Post::Restart),
        reopen: make(true, Post::Reopen),
    };
    for (name, bytes) in [
        ("source", &a.source),
        ("frozen", &a.frozen),
        ("stage", &a.stage),
        ("importing", &a.importing),
        ("restart", &a.restart),
        ("reopen", &a.reopen),
    ] {
        fs::write(dir.join(format!("paged-{name}.wasm")), bytes).unwrap();
        println!(
            "paged native {name}: {} bytes sha256={}",
            bytes.len(),
            sha256_hex(bytes)
        );
        let mut exports = BTreeMap::new();
        let mut bodies = Vec::new();
        let mut imports = 0;
        for payload in Parser::new(0).parse_all(bytes) {
            match payload.unwrap() {
                Payload::ImportSection(reader) => imports = reader.into_imports().count(),
                Payload::ExportSection(reader) => {
                    for export in reader {
                        let export = export.unwrap();
                        assert!(export.name == "memory" || export.name.starts_with("canister_"));
                        exports.insert(export.name, export.index);
                    }
                }
                Payload::CodeSectionEntry(body) => bodies.push(body),
                _ => {}
            }
        }
        assert!(exports.contains_key("canister_query checkpoint_page"));
        assert!(!exports.contains_key("canister_update cancel_checkpoint"));
        let calls = |name: &str| {
            bodies[exports[name] as usize - imports]
                .get_operators_reader()
                .unwrap()
                .into_iter()
                .filter_map(|op| match op.unwrap() {
                    Operator::Call { function_index } => Some(function_index),
                    _ => None,
                })
                .collect::<Vec<_>>()
        };
        let init = calls("canister_init");
        let batch = calls("canister_update import_batch");
        assert_eq!(batch.len(), 6);
        assert!(batch[2] >= init[0] && batch[3] < init[0] && batch[4] >= init[0]);
        if name == "stage" || name == "restart" {
            let upgrade = calls("canister_post_upgrade");
            assert_eq!(upgrade.len(), 5);
            assert_eq!(&upgrade[2..], &batch[2..5]);
        }
    }
    (dir, a)
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
fn batch(start: u64, fault: u64) -> Vec<u8> {
    [1, A, start, 1, fault]
        .into_iter()
        .flat_map(u64::to_le_bytes)
        .collect()
}
async fn run(pic: &PocketIc, id: CanisterId, start: u64, fault: u64) {
    pic.update_call(id, id, "import_batch", batch(start, fault))
        .await
        .unwrap();
}
fn snapshot(bytes: &[u8], phase: u16, cursor: [u32; 3]) {
    assert_eq!(bytes.len(), 952);
    assert_eq!(&bytes[..8], b"ICJPG001");
    assert_eq!(
        &bytes[8..10],
        &(if phase == 1 { 1u16 } else { 2 }).to_le_bytes()
    );
    assert_eq!(&bytes[10..12], &phase.to_le_bytes());
    assert_eq!(
        [word(bytes, 16), word(bytes, 24), word(bytes, 32)],
        [1, A, B]
    );
    let u32_at = |offset| u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap());
    assert_eq!([u32_at(44), u32_at(48), u32_at(52)], cursor);
    assert_eq!(u32_at(40), 328);
    let mut sealed = bytes[..64].to_vec();
    sealed.extend_from_slice(&bytes[104..432]);
    sealed.extend_from_slice(&bytes[64..72]);
    assert_eq!(&bytes[72..104], &sha256(&sealed));
    let manifest = &bytes[104..432];
    assert_eq!(&manifest[..8], b"NKPPAIR1");
    assert_eq!(&manifest[296..], &sha256(&manifest[..296]));
    if phase == 1 {
        assert!(bytes[432..944].iter().all(|b| *b == 0));
    } else {
        let continuation = &bytes[432..944];
        assert_eq!(&continuation[..8], b"NKPCNT01");
        assert_eq!(&continuation[16..120], &bytes[..104]);
        assert_eq!(&continuation[480..], &sha256(&continuation[..480]));
    }
}
async fn publish(pic: &PocketIc, id: CanisterId) -> [BTreeMap<u64, u64>; 2] {
    let native = query(pic, id, "observe").await;
    let expected = [records(&native, 0), records(&native, 1)];
    pic.update_call(id, id, "export_checkpoint", args(1, B, 0))
        .await
        .unwrap();
    let bytes = query(pic, id, "checkpoint_bytes").await;
    snapshot(&bytes, 1, [0, 0, 0]);
    let mut pages = 0;
    for (root, values) in expected.iter().enumerate() {
        let manifest = &bytes[136 + root * 128..264 + root * 128];
        assert_eq!(&manifest[..8], b"NKPAGED1");
        assert_eq!(&manifest[96..], &sha256(&manifest[..96]));
        assert_eq!(
            [word(manifest, 16), word(manifest, 24), word(manifest, 32)],
            [1, root as u64 + 1, values.len() as u64]
        );
        let count = values.len().div_ceil(64) as u64;
        pages += count;
        assert_eq!(word(manifest, 40), count);
        let mut expected_hash = manifest[56..88].to_vec();
        for index in 0..count {
            let page = pic
                .query_call(id, id, "checkpoint_page", args(1, root as u64, index))
                .await
                .unwrap();
            assert_eq!(page.len(), 1104);
            let mut hashed = Vec::from(*b"NKPAGEH1");
            hashed.extend_from_slice(&page);
            assert_eq!(expected_hash.as_slice(), &sha256(&hashed));
            expected_hash = page[48..80].to_vec();
            assert_eq!(&page[..8], b"NKPAGE01");
            assert_eq!(
                [
                    word(&page, 8),
                    word(&page, 16),
                    word(&page, 24),
                    word(&page, 32),
                    word(&page, 40)
                ],
                [
                    1,
                    root as u64 + 1,
                    index,
                    (values.len() as u64 - index * 64).min(64),
                    values.len() as u64
                ]
            );
            let selected: Vec<_> = values.iter().skip(index as usize * 64).take(64).collect();
            for (index, (&key, &value)) in selected.iter().copied().enumerate() {
                assert_eq!(
                    [word(&page, 80 + index * 16), word(&page, 88 + index * 16)],
                    [key, value]
                );
            }
            assert!(
                page[80 + selected.len() * 16..]
                    .iter()
                    .all(|byte| *byte == 0)
            );
        }
        assert!(expected_hash.iter().all(|byte| *byte == 0));
    }
    assert_eq!(word(&bytes, 944), pages);
    read_tests::check_prefix(pic, id).await;
    expected
}
async fn prefix(
    pic: &PocketIc,
    id: CanisterId,
    expected: &[BTreeMap<u64, u64>; 2],
    next: usize,
    complete: bool,
) {
    let native = query(pic, id, "observe").await;
    assert_eq!(word(&native, 32), 10);
    assert_eq!(word(&native, 72), if complete { 1 } else { 3 });
    let mut remaining = next;
    for (root, values) in expected.iter().enumerate() {
        let pages = remaining.min(values.len().div_ceil(64));
        remaining -= pages;
        let wanted: BTreeMap<_, _> = values
            .iter()
            .take(pages * 64)
            .map(|(&key, &value)| (key, value))
            .collect();
        assert_eq!(records(&native, root), wanted);
    }
    if !complete {
        read_tests::check_prefix(pic, id).await;
        let observed = query(pic, id, "native_prefix").await;
        let stored = query(pic, id, "checkpoint_bytes").await;
        for root in 0..2 {
            assert_eq!(
                &stored[840 + root * 32..872 + root * 32],
                &observed[8 + root * 40..40 + root * 40]
            );
        }
    }
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "prepared paged-native artifacts and PocketIC server"]
async fn paged_native_writes_progress_and_publication_roll_back_together() {
    let (dir, a) = artifacts();
    let pic = server().await;
    for shape in 0..8 {
        let id = install(&pic, &a.source).await;
        refused_update(&pic, id, "export_checkpoint", args(1, B, 0)).await;
        authorize(&pic, id).await;
        match shape {
            0 => {}
            1 => update(&pic, id, "insert_b").await,
            2 => update(&pic, id, "insert_a").await,
            3 => {
                update(&pic, id, "fill_checkpoint").await;
                update(&pic, id, "full_width").await;
            }
            4 => update(&pic, id, "pages_empty_a").await,
            5 => update(&pic, id, "pages_empty_b").await,
            6 => update(&pic, id, "pages_partial").await,
            7 => update(&pic, id, "pages_full").await,
            _ => unreachable!(),
        }
        refused_upgrade(&pic, id, &a.stage, args(1, A, 0), false).await;
        if shape == 3 || shape == 7 {
            for fault in [1, 2, 4] {
                refused_update(&pic, id, "export_checkpoint", args(1, B, fault)).await;
            }
        }
        let expected = publish(&pic, id).await;
        refused_update(&pic, id, "export_checkpoint", args(1, B, 0)).await;
        pic.upgrade_eop_canister(id, a.frozen.clone(), args(1, B, 0), None)
            .await
            .unwrap();
        if shape == 3 || shape == 7 {
            refused_upgrade(&pic, id, &a.stage, args(1, A, 0), true).await;
            for fault in [1, 2, 3, 5, 9, 10] {
                refused_upgrade(&pic, id, &a.stage, args(1, A, fault), false).await;
            }
        }
        replace_heap(&pic, id, &a.stage, args(1, A, 0))
            .await
            .unwrap();
        snapshot(&query(&pic, id, "checkpoint_bytes").await, 4, [0, 0, 0]);
        prefix(&pic, id, &expected, 0, false).await;
        if shape == 3 || shape == 7 {
            pic.set_controllers(id, None, vec![CanisterId::anonymous()])
                .await
                .unwrap();
            refused_update(&pic, id, "import_batch", batch(0, 0)).await;
            authorize(&pic, id).await;
        }
        assert!(
            query(&pic, id, "discarded_payload")
                .await
                .iter()
                .all(|byte| *byte == 0)
        );
        for method in ["insert_a", "reset_b", "full_width"] {
            refused_update(&pic, id, method, vec![]).await;
        }
        let pages: usize = expected.iter().map(|root| root.len().div_ceil(64)).sum();
        for start in 0..pages.max(1) {
            refused_upgrade(&pic, id, &a.importing, args(1, A, 1), true).await;
            pic.upgrade_eop_canister(id, a.importing.clone(), args(1, A, 0), None)
                .await
                .unwrap();
            if shape == 3 || shape == 7 {
                for fault in 1..=10 {
                    let error =
                        refused_update(&pic, id, "import_batch", batch(start as u64, fault)).await;
                    if fault == 6 || fault == 8 {
                        assert!(
                            error
                                .reject_message
                                .contains("paged native records differ after apply"),
                            "native corruption must reach post-apply verification: {error:?}"
                        );
                    }
                }
                for bad in [
                    [1, A, start as u64, 2, 0],
                    [2, A, start as u64, 1, 0],
                    [1, B, start as u64, 1, 0],
                    [1, A, start as u64 + 1, 1, 0],
                ] {
                    refused_update(
                        &pic,
                        id,
                        "import_batch",
                        bad.into_iter().flat_map(u64::to_le_bytes).collect(),
                    )
                    .await;
                }
            }
            run(&pic, id, start as u64, 0).await;
            let end = (start + 1).min(pages);
            let complete = end == pages;
            let state = query(&pic, id, "checkpoint_bytes").await;
            snapshot(
                &state,
                if complete { 2 } else { 4 },
                [end as u32, start as u32, 1],
            );
            prefix(&pic, id, &expected, end, complete).await;
            let stable = pic.get_stable_memory(id).await;
            let native = query(&pic, id, "observe").await;
            run(&pic, id, start as u64, 10).await;
            assert_eq!(pic.get_stable_memory(id).await, stable);
            assert_eq!(query(&pic, id, "observe").await, native);
        }
        let consumed = query(&pic, id, "checkpoint_bytes").await;
        pic.upgrade_eop_canister(id, a.reopen.clone(), vec![], None)
            .await
            .unwrap();
        update(&pic, id, "insert_a").await;
        let newer = query(&pic, id, "observe").await;
        run(&pic, id, pages.saturating_sub(1) as u64, 0).await;
        assert_eq!(query(&pic, id, "observe").await, newer);
        assert_eq!(query(&pic, id, "checkpoint_bytes").await, consumed);
        refused_upgrade(&pic, id, &a.restart, args(1, A, 0), false).await;
    }
    fs::write(
        dir.join("PAGED_NATIVE_RECOVERED"),
        b"combined native and governed rollback passed\n",
    )
    .unwrap();
    pic.drop().await;
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "prepared paged-native artifacts and PocketIC server"]
async fn paged_native_corruption_refuses_and_explicit_restart_uses_checkpoint() {
    let (dir, a) = artifacts();
    let pic = server().await;
    for damage in 0..7 {
        let id = install(&pic, &a.source).await;
        authorize(&pic, id).await;
        update(&pic, id, "fill_checkpoint").await;
        update(&pic, id, "full_width").await;
        let expected = publish(&pic, id).await;
        replace_heap(&pic, id, &a.stage, args(1, A, 0))
            .await
            .unwrap();
        if damage != 3 {
            run(&pic, id, 0, 0).await;
        }
        if damage < 4 {
            pic.update_call(id, id, "damage_checkpoint", args(damage, 0, 0))
                .await
                .unwrap();
        } else {
            pic.update_call(id, id, "damage_import", args(damage - 4, 0, 0))
                .await
                .unwrap();
        }
        refused_update(&pic, id, "import_batch", batch(u64::from(damage != 3), 0)).await;
        if damage == 0 || damage == 1 || damage == 3 {
            if damage != 3 {
                refused_upgrade(&pic, id, &a.restart, args(1, A, 0), false).await;
            } else {
                // Restart may reset an unfinished target, but cannot repair a corrupt page.
                replace_heap(&pic, id, &a.restart, args(1, A, 0))
                    .await
                    .unwrap();
                refused_update(&pic, id, "import_batch", batch(0, 0)).await;
            }
        } else {
            if damage == 2 || damage == 5 {
                refused_upgrade(&pic, id, &a.importing, args(1, A, 0), true).await;
            }
            replace_heap_with_options(&pic, id, &a.restart, args(1, A, 0), true, None)
                .await
                .unwrap();
            prefix(&pic, id, &expected, 0, false).await;
            run(&pic, id, 0, 0).await;
            run(&pic, id, 1, 0).await;
            prefix(&pic, id, &expected, 2, true).await;
        }
    }
    // A consumed page is not reread while continuing from an admitted cursor.
    let id = install(&pic, &a.source).await;
    authorize(&pic, id).await;
    update(&pic, id, "fill_checkpoint").await;
    let expected = publish(&pic, id).await;
    replace_heap(&pic, id, &a.stage, args(1, A, 0))
        .await
        .unwrap();
    run(&pic, id, 0, 0).await;
    pic.update_call(id, id, "damage_checkpoint", args(3, 0, 0))
        .await
        .unwrap();
    run(&pic, id, 1, 0).await;
    prefix(&pic, id, &expected, 2, true).await;
    let id = install(&pic, &a.source).await;
    authorize(&pic, id).await;
    publish(&pic, id).await;
    pic.set_stable_memory(id, vec![], BlobCompression::NoCompression)
        .await;
    refused_upgrade(&pic, id, &a.stage, args(1, A, 0), false).await;
    assert!(pic.get_stable_memory(id).await.is_empty());
    fs::write(
        dir.join("PAGED_NATIVE_REFUSED"),
        b"corruption refusal and explicit restart passed\n",
    )
    .unwrap();
    pic.drop().await;
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "prepared paged-native artifacts and PocketIC server"]
async fn later_pages_rehash_resume_and_refuse_corruption_without_replaying_consumed_pages() {
    let (dir, a) = artifacts();
    let pic = server().await;
    for case in 0..4 {
        let id = install(&pic, &a.source).await;
        authorize(&pic, id).await;
        update(&pic, id, "pages_full").await;
        let expected = publish(&pic, id).await;
        assert_eq!(expected.each_ref().map(BTreeMap::len), [192, 192]);
        replace_heap(&pic, id, &a.stage, args(1, A, 0))
            .await
            .unwrap();
        run(&pic, id, 0, 0).await;
        let first = query(&pic, id, "observe").await;
        assert_eq!((word(&first, 56) >> 12) & 15, 7); // 128 buckets for 64 records.
        if case < 3 {
            // Exporter stores each root last page first: physical 0 is future
            // logical page 2, physical 2 is the already consumed logical page 0.
            let physical = if case == 0 { 0 } else { 2 };
            refused_update(&pic, id, "damage_checkpoint", args(4, 6, 0)).await;
            pic.update_call(id, id, "damage_checkpoint", args(4, physical, 0))
                .await
                .unwrap();
            pic.upgrade_eop_canister(id, a.importing.clone(), args(1, A, 0), None)
                .await
                .unwrap();
            if case == 2 {
                replace_heap(&pic, id, &a.restart, args(1, A, 0))
                    .await
                    .unwrap();
                prefix(&pic, id, &expected, 0, false).await;
                refused_update(&pic, id, "import_batch", batch(0, 0)).await;
                continue;
            }
        }
        run(&pic, id, 1, 0).await;
        let second = query(&pic, id, "observe").await;
        assert_eq!((word(&second, 56) >> 12) & 15, 8); // Rehash to 256 buckets.
        assert_ne!(word(&first, 56) >> 16, word(&second, 56) >> 16);
        prefix(&pic, id, &expected, 2, false).await;
        if case == 0 {
            refused_update(&pic, id, "import_batch", batch(2, 0)).await;
            snapshot(&query(&pic, id, "checkpoint_bytes").await, 4, [2, 1, 1]);
            prefix(&pic, id, &expected, 2, false).await;
            continue;
        }
        run(&pic, id, 2, 0).await;
        run(&pic, id, 3, 0).await;
        if case == 3 {
            // Root 1 is complete but private while root 2 is incomplete.
            prefix(&pic, id, &expected, 4, false).await;
            for fault in [1, 9, 10] {
                refused_upgrade(&pic, id, &a.restart, args(1, A, fault), false).await;
            }
            pic.update_call(id, id, "damage_import", args(1, 0, 0))
                .await
                .unwrap();
            refused_update(&pic, id, "import_batch", batch(4, 0)).await;
            replace_heap(&pic, id, &a.restart, args(1, A, 0))
                .await
                .unwrap();
            prefix(&pic, id, &expected, 0, false).await;
            for start in 0..4 {
                run(&pic, id, start, 0).await;
            }
        }
        run(&pic, id, 4, 0).await;
        run(&pic, id, 5, 0).await;
        prefix(&pic, id, &expected, 6, true).await;
    }
    let id = install(&pic, &a.source).await;
    authorize(&pic, id).await;
    update(&pic, id, "pages_excess").await;
    refused_update(&pic, id, "export_checkpoint", args(1, B, 0)).await;
    fs::write(
        dir.join("PAGED_NATIVE_MULTIPAGE"),
        b"same-root pages, rehash, late corruption and restart passed\n",
    )
    .unwrap();
    pic.drop().await;
}

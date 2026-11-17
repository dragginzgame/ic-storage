//! Combined retained-root, continuation and progress transactions on the IC.
use super::{
    Post, inputs,
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
const B: u64 = 163840;
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
    let make = |relocated, post| prepare_mode(&raw, &bridge, relocated, post, true);
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
        let count = u64::from(!values.is_empty());
        pages += count;
        assert_eq!(word(manifest, 40), count);
        if count == 0 {
            continue;
        }
        let page = pic
            .query_call(id, id, "checkpoint_page", args(1, root as u64, 0))
            .await
            .unwrap();
        assert_eq!(page.len(), 1104);
        let mut hashed = Vec::from(*b"NKPAGEH1");
        hashed.extend_from_slice(&page);
        assert_eq!(&manifest[56..88], &sha256(&hashed));
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
                0,
                values.len() as u64,
                values.len() as u64
            ]
        );
        assert!(page[48..80].iter().all(|byte| *byte == 0));
        for (index, (&key, &value)) in values.iter().enumerate() {
            assert_eq!(
                [word(&page, 80 + index * 16), word(&page, 88 + index * 16)],
                [key, value]
            );
        }
        assert!(page[80 + values.len() * 16..].iter().all(|byte| *byte == 0));
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
    assert_eq!(word(&native, 72), if complete { 1 } else { 3 });
    let mut remaining = next;
    for (root, values) in expected.iter().enumerate() {
        let wanted = if remaining != 0 && !values.is_empty() {
            remaining -= 1;
            values.clone()
        } else {
            BTreeMap::new()
        };
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
    for shape in 0..4 {
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
            _ => unreachable!(),
        }
        refused_upgrade(&pic, id, &a.stage, args(1, A, 0), false).await;
        if shape == 3 {
            for fault in [1, 2, 4] {
                refused_update(&pic, id, "export_checkpoint", args(1, B, fault)).await;
            }
        }
        let expected = publish(&pic, id).await;
        refused_update(&pic, id, "export_checkpoint", args(1, B, 0)).await;
        pic.upgrade_eop_canister(id, a.frozen.clone(), args(1, B, 0), None)
            .await
            .unwrap();
        if shape == 3 {
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
        if shape == 3 {
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
        let pages = expected.iter().filter(|root| !root.is_empty()).count();
        for start in 0..pages.max(1) {
            refused_upgrade(&pic, id, &a.importing, args(1, A, 1), true).await;
            pic.upgrade_eop_canister(id, a.importing.clone(), args(1, A, 0), None)
                .await
                .unwrap();
            if shape == 3 {
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

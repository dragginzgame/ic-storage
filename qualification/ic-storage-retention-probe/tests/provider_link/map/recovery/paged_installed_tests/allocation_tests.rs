//! Requested live bytes in an opt-in checkpoint System-allocator build.
use super::*;
use std::fmt::Write;

async fn reset(pic: &PocketIc, id: CanisterId) {
    pic.update_call(id, id, "allocation_reset", vec![])
        .await
        .unwrap();
}
async fn metrics(pic: &PocketIc, id: CanisterId) -> [u64; 8] {
    let bytes = query(pic, id, "allocation_metrics").await;
    assert_eq!(bytes.len(), 64);
    std::array::from_fn(|index| word(&bytes, index * 8))
}
async fn sample(
    pic: &PocketIc,
    id: CanisterId,
    path: &std::path::Path,
    report: &mut String,
    shape: &str,
    operation: &str,
    page: usize,
) {
    let values = metrics(pic, id).await;
    assert_eq!(
        metrics(pic, id).await,
        values,
        "query must not alter observations"
    );
    let native = query(pic, id, "observe").await;
    let slots = [word(&native, 56), word(&native, 64)]
        .into_iter()
        .map(|root| (1u64 << ((root >> 12) & 15)) * 24)
        .sum::<u64>();
    let pool = word(&native, 48);
    let wasm = word(&native, 16) * 65536;
    write!(report, "{shape}\t{operation}\t{page}").unwrap();
    for value in values.into_iter().chain([slots, pool, wasm]) {
        write!(report, "\t{value}").unwrap();
    }
    report.push('\n');
    fs::write(path, &report).unwrap();
    assert_eq!(
        values[0], values[1],
        "request must return to its live-byte baseline"
    );
    assert!(values[0] <= values[2] && values[2] <= values[3]);
    assert!(
        values[3] <= 256 * 1024,
        "checkpoint requested-byte upper budget: {values:?}"
    );
    assert_eq!(
        values[7], 0,
        "successful workload allocation requests must succeed"
    );
    assert!(slots <= pool && pool <= 196608 && wasm <= 2097152);
}

async fn calibrate(pic: &PocketIc, source: &[u8]) {
    let id = install(pic, source).await;
    let initial = metrics(pic, id).await;
    refused_update(pic, id, "allocation_reset", vec![]).await;
    refused_update(pic, id, "allocation_calibrate", args(0, 0, 0)).await;
    assert_eq!(metrics(pic, id).await, initial);
    authorize(pic, id).await;
    reset(pic, id).await;
    let baseline = metrics(pic, id).await;
    assert_eq!(
        baseline,
        [
            baseline[0],
            baseline[0],
            baseline[0],
            baseline[0],
            0,
            0,
            0,
            0
        ]
    );
    let native = query(pic, id, "observe").await;
    let stable = pic.get_stable_memory(id).await;
    refused_update(pic, id, "allocation_calibrate", args(1, 0, 0)).await;
    assert_eq!(
        metrics(pic, id).await,
        baseline,
        "trap rolls counters back too"
    );
    pic.update_call(id, id, "allocation_calibrate", args(0, 0, 0))
        .await
        .unwrap();
    let live = baseline[0];
    assert_eq!(
        metrics(pic, id).await,
        [live, live, live + 160, live + 192, 208, 4, 4, 2]
    );
    assert_eq!(query(pic, id, "observe").await, native);
    assert_eq!(pic.get_stable_memory(id).await, stable);
    reset(pic, id).await;
    assert_eq!(metrics(pic, id).await, baseline);
    pic.update_call(id, id, "allocation_calibrate", args(2, 0, 0))
        .await
        .unwrap();
    assert_eq!(
        metrics(pic, id).await,
        [live + 96, live, live + 160, live + 192, 112, 2, 4, 2]
    );
    reset(pic, id).await;
    assert_eq!(metrics(pic, id).await, baseline);
}

#[test]
#[ignore = "prepared allocation-meter bridge"]
fn allocation_diagnostics_require_the_complete_paged_export_contract() {
    use wasm_encoder::{ExportKind, ExportSection, RawSection};
    use wasmparser::ExternalKind;
    let (dir, raw) = inputs();
    let bridge = fs::read(dir.join("checkpoint-map-paged-allocations.wasm")).unwrap();
    for missing in [
        "checkpoint_alloc_reset",
        "checkpoint_alloc_read",
        "checkpoint_alloc_calibrate",
        "checkpoint_page",
    ] {
        let changed = crate::rewrite(&bridge, |module, id, bytes| {
            if id != 7 {
                module.section(&RawSection { id, data: bytes });
                return;
            }
            let mut exports = ExportSection::new();
            for payload in Parser::new(0).parse_all(&bridge) {
                if let Payload::ExportSection(reader) = payload.unwrap() {
                    for export in reader {
                        let export = export.unwrap();
                        if export.name != missing {
                            let kind = match export.kind {
                                ExternalKind::Func => ExportKind::Func,
                                ExternalKind::Global => ExportKind::Global,
                                ExternalKind::Memory => ExportKind::Memory,
                                _ => unreachable!(),
                            };
                            exports.export(export.name, kind, export.index);
                        }
                    }
                }
            }
            module.section(&exports);
        });
        ic_storage_retention_probe::artifact::validate_wasm(&changed, super::super::LIMIT).unwrap();
        assert!(
            std::panic::catch_unwind(|| prepare_mode(
                &raw,
                &changed,
                false,
                Post::Reopen,
                Profile::Paged
            ))
            .is_err()
        );
    }
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "prepared allocation-meter bridge and PocketIC server"]
async fn checkpoint_requested_live_bytes_are_calibrated_across_recovery_and_later_epochs() {
    let (dir, raw) = inputs();
    let bridge = fs::read(dir.join("checkpoint-map-paged-allocations.wasm")).unwrap();
    let make = |relocated, post| prepare_mode(&raw, &bridge, relocated, post, Profile::Paged);
    let (source, frozen, stage, importing, reopen) = (
        make(false, Post::Reopen),
        make(false, Post::Frozen),
        make(true, Post::Stage),
        make(true, Post::Importing),
        make(true, Post::Reopen),
    );
    for (name, bytes) in [
        ("source", &source),
        ("frozen", &frozen),
        ("stage", &stage),
        ("importing", &importing),
        ("reopen", &reopen),
    ] {
        fs::write(dir.join(format!("allocations-paged-{name}.wasm")), bytes).unwrap();
        println!(
            "allocation-meter paged {name}: sha256={}",
            sha256_hex(bytes)
        );
    }
    let pic = server().await;
    calibrate(&pic, &source).await;
    let path = dir.join("paged-allocations.tsv");
    let mut report = "shape\toperation\tpage\tstart_live\tend_live\tpeak_live\trealloc_upper\tsuccessful_requested\tallocations\tdeallocations\tfailed_allocations\tnative_slots_end\tnative_pool_bytes\twasm_bytes\n".to_owned();
    for (shape, fill) in [
        ("0+0", None),
        ("192+192", Some("pages_full")),
        ("3072+1536", Some("pages_large")),
    ] {
        let id = install(&pic, &source).await;
        authorize(&pic, id).await;
        if let Some(fill) = fill {
            update(&pic, id, fill).await;
        }
        reset(&pic, id).await;
        let expected = publish(&pic, id).await;
        sample(&pic, id, &path, &mut report, shape, "export", 0).await;
        pic.upgrade_eop_canister(id, frozen.clone(), args(1, B, 0), None)
            .await
            .unwrap();
        sample(
            &pic,
            id,
            &path,
            &mut report,
            shape,
            "frozen_post_upgrade",
            0,
        )
        .await;
        replace_heap(&pic, id, &stage, args(1, A, 0)).await.unwrap();
        sample(&pic, id, &path, &mut report, shape, "replace_start", 0).await;
        let pages: usize = expected.iter().map(|root| root.len().div_ceil(64)).sum();
        for page in 0..pages.max(1) {
            reset(&pic, id).await;
            if [0, 6, 12, 24, 71].contains(&page) {
                let before = metrics(&pic, id).await;
                refused_update(&pic, id, "import_batch", batch(page as u64, 9)).await;
                assert_eq!(metrics(&pic, id).await, before);
            }
            run(&pic, id, page as u64, 0).await;
            sample(&pic, id, &path, &mut report, shape, "batch", page).await;
            prefix(
                &pic,
                id,
                &expected,
                (page + 1).min(pages),
                page + 1 >= pages,
            )
            .await;
            if [0, 6, 12, 24, 47, 48, 71].contains(&page) {
                reset(&pic, id).await;
                run(&pic, id, page as u64, 0).await;
                sample(&pic, id, &path, &mut report, shape, "retry", page).await;
                if page + 1 < pages {
                    pic.upgrade_eop_canister(id, importing.clone(), args(1, A, 0), None)
                        .await
                        .unwrap();
                    sample(
                        &pic,
                        id,
                        &path,
                        &mut report,
                        shape,
                        "importing_post_upgrade",
                        page,
                    )
                    .await;
                }
            }
        }
        pic.upgrade_eop_canister(id, reopen.clone(), vec![], None)
            .await
            .unwrap();
        sample(
            &pic,
            id,
            &path,
            &mut report,
            shape,
            "active_post_upgrade",
            pages,
        )
        .await;
        if pages != 0 {
            update(&pic, id, "replace_a").await;
        }
        reset(&pic, id).await;
        publish_epoch(&pic, id, 2, B, A).await;
        sample(
            &pic,
            id,
            &path,
            &mut report,
            shape,
            "export_after_consumed",
            2,
        )
        .await;
        reset(&pic, id).await;
        pic.update_call(id, id, "cancel_checkpoint", args(2, A, 0))
            .await
            .unwrap();
        sample(&pic, id, &path, &mut report, shape, "cancel", 2).await;
        reset(&pic, id).await;
        pic.update_call(id, id, "cancel_checkpoint", args(2, A, 0))
            .await
            .unwrap();
        sample(&pic, id, &path, &mut report, shape, "cancel_retry", 2).await;
        update(&pic, id, "reset_a").await;
        update(&pic, id, "reset_b").await;
        reset(&pic, id).await;
        publish_epoch(&pic, id, 3, B, A).await;
        sample(
            &pic,
            id,
            &path,
            &mut report,
            shape,
            "export_shrink_to_empty",
            3,
        )
        .await;
    }
    fs::write(
        dir.join("PAGED_NATIVE_ALLOCATIONS"),
        b"calibrated checkpoint requested-byte peaks and rollback passed\n",
    )
    .unwrap();
    pic.drop().await;
}

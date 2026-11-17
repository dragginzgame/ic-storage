//! Whole successful Wasm entrypoints, including native admission and publication.
use super::*;
use std::fmt::Write;

async fn measure(pic: &PocketIc, id: CanisterId) -> [u64; costs::WORDS] {
    let bytes = query(pic, id, "measure_costs").await;
    assert_eq!(bytes.len(), costs::WORDS * 8);
    std::array::from_fn(|index| word(&bytes, index * 8))
}

async fn record(
    pic: &PocketIc,
    id: CanisterId,
    path: &std::path::Path,
    report: &mut String,
    shape: &str,
    operation: &str,
    page: usize,
) -> [u64; costs::WORDS] {
    let values = measure(pic, id).await;
    write!(report, "{shape}\t{operation}\t{page}").unwrap();
    for value in values {
        write!(report, "\t{value}").unwrap();
    }
    report.push('\n');
    fs::write(path, report).unwrap(); // Preserve measurements even if a later gate fails.
    assert!(
        values[0] > 0 && values[0] <= 150_000_000,
        "{shape}/{operation}/{page}: {values:?}"
    );
    // Qualification budgets for this admitted shape, not network limits or an SLO.
    assert!(values[1] <= 2 * 1024 * 1024, "initial Wasm pages suffice");
    assert!(values[2] <= 50 * 65536, "governed stable allocation budget");
    assert!(values[9] <= 196608, "bounded retained native loan");
    values
}

async fn calibrate(pic: &PocketIc) {
    let raw = wat::parse_str(include_str!("costs/calibration.wat")).unwrap();
    let id = install(pic, &costs::instrument(&raw)).await;
    update(pic, id, "work").await;
    let values = measure(pic, id).await;
    assert!(values[0] > 0);
    assert_eq!(&values[1..], &[3 * 65536, 65536, 1, 8, 1, 8, 1, 1, 24576]);
    let stable = pic.get_stable_memory(id).await;
    let error = pic.update_call(id, id, "fail", vec![]).await.unwrap_err();
    assert_eq!(
        error.error_code,
        ic_testkit::pocket_ic::ErrorCode::CanisterTrapped
    );
    assert_eq!(measure(pic, id).await, values);
    assert_eq!(pic.get_stable_memory(id).await, stable);
    update(pic, id, "empty").await;
    let empty = measure(pic, id).await;
    assert!(empty[0] > 0 && empty[0] < values[0]);
    assert_eq!(&empty[3..9], &[0; 6]);
    assert_eq!(empty[1], values[1]); // A trapped grow did not inflate memory.
    assert_eq!(measure(pic, id).await, empty); // The query does not reset counters.
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "prepared paged-native artifacts and PocketIC server"]
async fn complete_entrypoint_costs_include_native_work_and_backing_io() {
    let (dir, a) = artifacts();
    let mut measured = Vec::new();
    for (name, raw) in [
        ("source", &a.source),
        ("frozen", &a.frozen),
        ("stage", &a.stage),
        ("importing", &a.importing),
        ("reopen", &a.reopen),
    ] {
        let bytes = costs::instrument(raw);
        println!("metered paged {name}: sha256={}", sha256_hex(&bytes));
        fs::write(dir.join(format!("metered-paged-{name}.wasm")), &bytes).unwrap();
        measured.push(bytes);
    }
    let [source, frozen, stage, importing, reopen] = measured.as_slice() else {
        unreachable!()
    };
    let pic = server().await;
    calibrate(&pic).await;
    let path = dir.join("paged-costs.tsv");
    let mut report = costs::HEADER.to_owned();
    for (shape, fill) in [
        ("0+0", None),
        ("192+192", Some("pages_full")),
        ("3072+1536", Some("pages_large")),
    ] {
        let id = install(&pic, source).await;
        authorize(&pic, id).await;
        if let Some(fill) = fill {
            update(&pic, id, fill).await;
        }
        update(&pic, id, "measure_admission").await;
        record(&pic, id, &path, &mut report, shape, "active_admission", 0).await;
        let expected = publish(&pic, id).await;
        record(&pic, id, &path, &mut report, shape, "export", 0).await;
        pic.upgrade_eop_canister(id, frozen.clone(), args(1, B, 0), None)
            .await
            .unwrap();
        record(
            &pic,
            id,
            &path,
            &mut report,
            shape,
            "frozen_post_upgrade",
            0,
        )
        .await;
        replace_heap(&pic, id, stage, args(1, A, 0)).await.unwrap();
        record(&pic, id, &path, &mut report, shape, "replace_start", 0).await;
        let pages = expected
            .iter()
            .map(|map| map.len().div_ceil(64))
            .sum::<usize>()
            .max(1);
        for page in 0..pages {
            run(&pic, id, page as u64, 0).await;
            let values = record(&pic, id, &path, &mut report, shape, "batch", page).await;
            assert_eq!(values[6], 616, "continuation plus progress backing writes");
            assert_eq!(values[4], if shape == "0+0" { 5552 } else { 6656 });
            assert_eq!(values[7], 0, "import reuses existing stable allocations");
            prefix(&pic, id, &expected, page + 1, page + 1 == pages).await;
            if [0, 6, 12, 24, 47, 48, 71].contains(&page) {
                let native = query(&pic, id, "observe").await;
                run(&pic, id, page as u64, 0).await;
                let retry = record(&pic, id, &path, &mut report, shape, "retry", page).await;
                assert_eq!(&retry[5..9], &[0; 4]);
                assert_eq!(query(&pic, id, "observe").await, native);
                if page + 1 != pages {
                    pic.upgrade_eop_canister(id, importing.clone(), args(1, A, 0), None)
                        .await
                        .unwrap();
                    record(
                        &pic,
                        id,
                        &path,
                        &mut report,
                        shape,
                        "importing_post_upgrade",
                        page,
                    )
                    .await;
                    assert_eq!(query(&pic, id, "observe").await, native);
                }
            }
        }
        pic.upgrade_eop_canister(id, reopen.clone(), vec![], None)
            .await
            .unwrap();
        record(
            &pic,
            id,
            &path,
            &mut report,
            shape,
            "active_post_upgrade",
            pages,
        )
        .await;
        prefix(&pic, id, &expected, pages, true).await;
    }
    fs::write(
        dir.join("PAGED_NATIVE_COSTS"),
        b"complete entrypoint costs and calibrated backing IO passed\n",
    )
    .unwrap();
    pic.drop().await;
}

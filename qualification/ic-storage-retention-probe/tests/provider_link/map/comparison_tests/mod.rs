//! Installed two-way comparison; no legacy Storage backend or checkpoint bridge.
use super::{inputs, installed_tests::observe};
use ic_storage_retention_probe::artifact::{compile_wat, sha256, sha256_hex, validate_wasm};
use ic_testkit::pocket_ic::{CanisterId, ErrorCode, PocketIcBuilder, nonblocking::PocketIc};
use std::{collections::BTreeMap, fs, io::Write};

mod batch_tests;
mod meter;
mod native;
const LIMIT: usize = 2 * 1024 * 1024;
const OPERATIONS: [&str; 6] = [
    "control", "lookup", "replace", "delete", "reinsert", "ingest",
];

fn args(a: u64, b: u64) -> Vec<u8> {
    a.to_le_bytes().into_iter().chain(b.to_le_bytes()).collect()
}
fn words(bytes: &[u8]) -> Vec<u64> {
    assert_eq!(bytes.len() % 8, 0);
    bytes
        .as_chunks::<8>()
        .0
        .iter()
        .map(|b| u64::from_le_bytes(*b))
        .collect()
}
async fn measurement(pic: &PocketIc, id: CanisterId) -> u64 {
    let result = pic
        .query_call(id, id, "bench_measure", vec![])
        .await
        .unwrap();
    assert_eq!(result.len(), 8);
    u64::from_le_bytes(result.try_into().unwrap())
}

async fn calibrate(pic: &PocketIc) {
    let bytes = compile_wat(
        r#"(module
      (import "ic0" "msg_reply" (func $reply))
      (import "ic0" "msg_reply_data_append" (func (param i32 i32)))
      (memory (export "memory") 1) (global (mut i32) (i32.const 0))
      (func (export "canister_update bench_empty") (call $reply))
      (func (export "canister_update bench_work") (local $i i32)
        (loop $next
          (i32.store (i32.const 32) (local.get $i))
          (local.set $i (i32.add (local.get $i) (i32.const 1)))
          (br_if $next (i32.lt_u (local.get $i) (i32.const 1000))))
        (call $reply))
      (func (export "canister_update bench_trap") unreachable))"#,
        LIMIT,
        LIMIT,
    )
    .unwrap();
    let id = pic.create_canister().await;
    pic.install_canister(id, meter::instrument(&bytes), vec![], None)
        .await;
    pic.update_call(id, id, "bench_empty", vec![])
        .await
        .unwrap();
    let empty = measurement(pic, id).await;
    assert!(empty > 0);
    pic.update_call(id, id, "bench_work", vec![]).await.unwrap();
    let work = measurement(pic, id).await;
    assert!(work > empty);
    assert_eq!(measurement(pic, id).await, work);
    let error = pic
        .update_call(id, id, "bench_trap", vec![])
        .await
        .unwrap_err();
    assert_eq!(error.error_code, ErrorCode::CanisterTrapped);
    assert_eq!(measurement(pic, id).await, work);
    pic.stop_canister(id, None).await.unwrap();
    pic.delete_canister(id, None).await.unwrap();
}

async fn state(pic: &PocketIc, id: CanisterId, native: bool) -> (BTreeMap<u64, u64>, u64) {
    if native {
        let snapshot = observe(pic, id).await;
        assert!(snapshot.records(1).is_empty());
        (snapshot.records(0), 0)
    } else {
        let bytes = pic.query_call(id, id, "bench_state", vec![]).await.unwrap();
        let values = words(&bytes);
        assert!(values.len() >= 3);
        assert_eq!(values[1], 0, "other root stays empty");
        assert_eq!(values.len(), 3 + 2 * values[2] as usize);
        let mut records = BTreeMap::new();
        for pair in values[3..].as_chunks::<2>().0 {
            assert!(records.insert(pair[0], pair[1]).is_none());
        }
        (records, values[0] * 65536)
    }
}

// Host oracle applies the logical operation to an independent ordered map.
fn expected(records: &mut BTreeMap<u64, u64>, rows: u64, seed: u64, batch: u64, operation: usize) {
    for key in (seed * 13..)
        .step_by(17)
        .take(batch as usize)
        .map(|key| key % rows)
    {
        match operation {
            0 | 5 => {}
            1 => assert_eq!(records.get(&key), Some(&(key + 1000))),
            2 => {
                assert_eq!(records.insert(key, key + 2000), Some(key + 1000));
            }
            3 => {
                assert_eq!(records.remove(&key), Some(key + 2000));
            }
            4 => {
                assert_eq!(records.insert(key, key + 1000), None);
            }
            _ => unreachable!(),
        }
    }
    if operation == 5 {
        records.extend((rows..rows + batch).map(|key| (key, key + 1000)));
    }
}

#[test]
fn comparison_meter_refuses_double_instrumentation() {
    let raw = compile_wat(
        r#"(module
      (import "ic0" "msg_reply" (func $reply))
      (import "ic0" "msg_reply_data_append" (func (param i32 i32)))
      (memory (export "memory") 1) (global (mut i32) (i32.const 0))
      (func (export "canister_update bench_empty") (call $reply)))"#,
        LIMIT,
        LIMIT,
    )
    .unwrap();
    let measured = meter::instrument(&raw);
    assert!(std::panic::catch_unwind(|| meter::instrument(&measured)).is_err());
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "comparison collector and prepared PocketIC server"]
async fn installed_tests_native_map_vs_stable_structures() {
    let (dir, raw) = inputs();
    let stable_raw = fs::read(dir.join("stable-map.wasm")).unwrap();
    validate_wasm(&stable_raw, LIMIT).unwrap();
    let stable = meter::instrument(&stable_raw);
    fs::write(dir.join("stable-map-metered.wasm"), &stable).unwrap();
    fs::write(dir.join("comparison-configuration.txt"), format!(
        "backends=native-map,ic-stable-structures-0.7.2\nrows=64,192,1536\nbatch=1,8,64\nseeds=1,2,3\nmetric=IC counter zero at end of complete update, before meter epilogue\ncompiler=Rust 1.100.0; opt=3; LTO; codegen-units=1; panic=abort\nnative_ingest=one scope per batch\nnative_other_operations=one scope per batch\nstable_raw_sha256={}\nstable_metered_sha256={}\nnative_raw_sha256={}\n",
        sha256_hex(&stable_raw), sha256_hex(&stable), sha256_hex(&raw)
    )).unwrap();
    let server = std::env::var("IC_TESTKIT_POCKET_IC_URL").expect("prepared server URL");
    let pic = PocketIcBuilder::new()
        .with_application_subnet()
        .with_max_request_time_ms(Some(120_000))
        .with_server_url(server.parse().unwrap())
        .build_async()
        .await;
    calibrate(&pic).await;
    batch_tests::verify(&pic, &raw, &dir).await;
    let mut csv = fs::File::create(dir.join("map-comparison.csv")).unwrap();
    let mut admission = fs::File::create(dir.join("native-admission.csv")).unwrap();
    writeln!(admission, "rows,seed,batch,phase,instructions").unwrap();
    let mut admissions = 0;
    writeln!(
        csv,
        "backend,rows,seed,batch,operation,instructions,instructions_per_operation,stable_bytes"
    )
    .unwrap();
    let mut samples = 0;
    for rows in [64, 192, 1536] {
        for batch in [1, 8, 64] {
            for seed in 1..=3 {
                let native_raw = native::prepare(&raw, rows, batch, seed);
                let native = meter::instrument(&native_raw);
                fs::write(
                    dir.join(format!("native-{rows}-{batch}-{seed}.wasm")),
                    &native_raw,
                )
                .unwrap();
                fs::write(
                    dir.join(format!("native-{rows}-{batch}-{seed}-metered.wasm")),
                    &native,
                )
                .unwrap();
                let order = if seed % 2 == 1 {
                    [true, false]
                } else {
                    [false, true]
                };
                for is_native in order {
                    let module = if is_native { &native } else { &stable };
                    let backend = if is_native {
                        "native"
                    } else {
                        "stable-structures"
                    };
                    let id = pic.create_canister().await;
                    pic.install_canister(id, module.clone(), vec![], None).await;
                    assert_eq!(
                        pic.canister_status(id, None)
                            .await
                            .unwrap()
                            .module_hash
                            .unwrap(),
                        sha256(module)
                    );
                    pic.update_call(id, id, "bench_setup", args(rows, seed))
                        .await
                        .unwrap();
                    let mut records: BTreeMap<_, _> =
                        (0..rows).map(|key| (key, key + 1000)).collect();
                    assert_eq!(state(&pic, id, is_native).await.0, records);
                    if is_native {
                        pic.update_call(id, id, "bench_admit", vec![])
                            .await
                            .unwrap();
                        let instructions = measurement(&pic, id).await;
                        writeln!(admission, "{rows},{seed},{batch},before,{instructions}").unwrap();
                        assert_eq!(state(&pic, id, true).await.0, records);
                        admissions += 1;
                    }
                    for (operation, name) in OPERATIONS.iter().enumerate() {
                        let method = if is_native {
                            format!("bench_{operation}")
                        } else {
                            "bench_run".to_owned()
                        };
                        pic.update_call(id, id, &method, args(operation as u64, batch))
                            .await
                            .unwrap();
                        let instructions = measurement(&pic, id).await;
                        assert!(instructions > 0);
                        expected(&mut records, rows, seed, batch, operation);
                        let (actual, stable_bytes) = state(&pic, id, is_native).await;
                        assert_eq!(actual, records, "{backend}/{rows}/{batch}/{seed}/{name}");
                        assert_eq!(
                            measurement(&pic, id).await,
                            instructions,
                            "queries preserve sample"
                        );
                        writeln!(csv, "{backend},{rows},{seed},{batch},{name},{instructions},{:.3},{stable_bytes}", instructions as f64 / batch as f64).unwrap();
                        csv.flush().unwrap();
                        samples += 1;
                    }
                    if is_native {
                        pic.update_call(id, id, "bench_admit", vec![])
                            .await
                            .unwrap();
                        let instructions = measurement(&pic, id).await;
                        writeln!(admission, "{rows},{seed},{batch},after,{instructions}").unwrap();
                        admission.flush().unwrap();
                        assert_eq!(state(&pic, id, true).await.0, records);
                        admissions += 1;
                    }
                    pic.stop_canister(id, None).await.unwrap();
                    pic.delete_canister(id, None).await.unwrap();
                }
                println!("comparison verified rows={rows} batch={batch} seed={seed}");
            }
        }
    }
    pic.drop().await;
    assert_eq!(samples, 324);
    assert_eq!(admissions, 54);
    fs::write(
        dir.join("MAP_COMPARISON"),
        format!("{samples} validated two-way samples\n"),
    )
    .unwrap();
}

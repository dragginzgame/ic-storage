//! On-demand matched matrix for generation-owned native maps and stable trees.
use super::{LIMIT, args, calibrate, measurement, meter, words};
use ic_storage_retention_probe::{
    artifact::{sha256, sha256_hex, validate_wasm},
    startup,
};
use ic_testkit::pocket_ic::{CanisterId, PocketIcBuilder, nonblocking::PocketIc};
use std::{collections::BTreeMap, fs, io::Write, path::PathBuf};

mod report;
#[cfg(test)]
mod tests;

const OPERATIONS: [&str; 9] = [
    "control",
    "lookup_hit",
    "lookup_miss",
    "replace",
    "delete",
    "reinsert",
    "ingest",
    "churn",
    "scan",
];

struct Sample {
    native: bool,
    rows: u64,
    other: u64,
    seed: u64,
    batch: u64,
    operation: usize,
    instructions: u64,
    units: u64,
    memory: [u64; 3],
}

async fn state(pic: &PocketIc, id: CanisterId) -> ([BTreeMap<u64, u64>; 2], [u64; 3]) {
    let data = words(&pic.query_call(id, id, "bench_state", vec![]).await.unwrap());
    let memory = [data[0] * 65536, data[1] * 65536, data[2]];
    let mut cursor = 3;
    let maps = core::array::from_fn(|_| {
        let count = data[cursor] as usize;
        cursor += 1;
        let mut map = BTreeMap::new();
        for _ in 0..count {
            assert!(map.insert(data[cursor], data[cursor + 1]).is_none());
            cursor += 2;
        }
        map
    });
    assert_eq!(cursor, data.len());
    (maps, memory)
}

fn expected(
    map: &mut BTreeMap<u64, u64>,
    rows: u64,
    seed: u64,
    batch: u64,
    operation: usize,
) -> u64 {
    if operation == 8 {
        return map
            .values()
            .fold(0u64, |sum, value| sum.wrapping_add(*value));
    }
    let keys: Vec<_> = (0..batch)
        .map(|index| (seed * 13 + index * 17) % rows)
        .collect();
    for (index, key) in keys.into_iter().enumerate() {
        let index = index as u64;
        match operation {
            0 => {}
            1 => assert_eq!(map[&key], key + 1000),
            2 => assert!(!map.contains_key(&(rows + 1024 + index))),
            3 => assert_eq!(map.insert(key, key + 2000), Some(key + 1000)),
            4 => assert_eq!(map.remove(&key), Some(key + 2000)),
            5 => assert_eq!(map.insert(key, key + 1000), None),
            6 => assert_eq!(map.insert(rows + index, rows + index + 1000), None),
            7 => {
                assert_eq!(map.remove(&key), Some(key + 1000));
                assert_eq!(
                    map.insert(rows + 64 + index, rows + 64 + index + 1000),
                    None
                );
            }
            _ => unreachable!(),
        }
    }
    0
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "on-demand matrix collector and prepared server"]
async fn installed_tests_matrix() {
    let dir = PathBuf::from(std::env::var_os("NATIVE_SESSION_INPUTS").expect("collector inputs"));
    assert!(dir.is_absolute());
    let profile = std::env::var("STORAGE_MATRIX_PROFILE").unwrap_or_else(|_| "full".into());
    let (populations, batches, siblings, seeds): (&[u64], &[u64], &[u64], &[u64]) =
        match profile.as_str() {
            "full" => (&[64, 192, 1536], &[1, 8, 64], &[0, 192], &[1, 2, 3]),
            "smoke" => (&[192], &[8], &[192], &[1]),
            _ => panic!("unknown matrix profile"),
        };
    let native_raw = fs::read(dir.join("session-bench.wasm")).unwrap();
    assert!(startup::prepare_session(&native_raw).is_err());
    let admitted = startup::prepare_session_benchmark(&native_raw).unwrap();
    let native = meter::instrument_session(&admitted.bytes);
    let stable_raw = fs::read(dir.join("stable-matrix.wasm")).unwrap();
    validate_wasm(&stable_raw, LIMIT).unwrap();
    let stable = meter::instrument(&stable_raw);
    fs::write(dir.join("matrix-native.wasm"), &native).unwrap();
    fs::write(dir.join("matrix-stable.wasm"), &stable).unwrap();
    fs::write(dir.join("native-layout.txt"), admitted.report).unwrap();
    fs::write(dir.join("matrix-configuration.txt"), format!(
        "profile={profile}\nrows={populations:?}\nbatches={batches:?}\nsibling_rows={siblings:?}\nseeds={seeds:?}\noperations={OPERATIONS:?}\nmetric=whole-update IC counter zero after reply; excludes meter epilogue\nnative_owner_lifetime=module-generation\nnative_access=trusted-source; not per-access WAT guards\ncontrol_subtraction=none\nstable_dependency=ic-stable-structures-0.7.2\nnative_raw_sha256={}\nnative_final_sha256={}\nstable_raw_sha256={}\nstable_final_sha256={}\nnative_wasm_bytes={}\nstable_wasm_bytes={}\n",
        sha256_hex(&native_raw), sha256_hex(&native), sha256_hex(&stable_raw), sha256_hex(&stable), native.len(), stable.len())).unwrap();
    let server = std::env::var("IC_TESTKIT_POCKET_IC_URL").expect("prepared server URL");
    let pic = PocketIcBuilder::new()
        .with_application_subnet()
        .with_max_request_time_ms(Some(120_000))
        .with_server_url(server.parse().unwrap())
        .build_async()
        .await;
    calibrate(&pic).await;
    let mut csv = fs::File::create(dir.join("matrix.csv")).unwrap();
    writeln!(csv, "backend,rows,sibling_rows,seed,batch,operation,instructions,work_units,instructions_per_unit,wasm_memory_bytes,stable_memory_bytes,native_slot_bytes").unwrap();
    let mut samples = Vec::new();
    for &rows in populations {
        for &other in siblings {
            for &batch in batches {
                for &seed in seeds {
                    for is_native in if seed % 2 == 1 {
                        [true, false]
                    } else {
                        [false, true]
                    } {
                        let module = if is_native { &native } else { &stable };
                        let backend = if is_native {
                            "native-session"
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
                        pic.update_call(id, id, "bench_setup", args(rows | (other << 32), seed))
                            .await
                            .unwrap();
                        let mut oracle = [rows, other].map(|count| {
                            (0..count)
                                .map(|key| (key, key + 1000))
                                .collect::<BTreeMap<_, _>>()
                        });
                        assert_eq!(state(&pic, id).await.0, oracle);
                        for (operation, name) in OPERATIONS.iter().enumerate() {
                            let sum = expected(&mut oracle[0], rows, seed, batch, operation);
                            let reply = pic
                                .update_call(id, id, "bench_run", args(operation as u64, batch))
                                .await
                                .unwrap();
                            assert_eq!(words(&reply), [sum]);
                            let instructions = measurement(&pic, id).await;
                            assert!(instructions > 0);
                            let (actual, memory) = state(&pic, id).await;
                            assert_eq!(
                                actual, oracle,
                                "{backend}/{rows}/{other}/{batch}/{seed}/{name}"
                            );
                            assert_eq!(measurement(&pic, id).await, instructions);
                            if is_native {
                                let status = words(
                                    &pic.query_call(id, id, "session_status", vec![])
                                        .await
                                        .unwrap(),
                                );
                                assert_eq!(status[2], 1, "no owner reconstruction between updates");
                            }
                            let units = if operation == 8 {
                                oracle[0].len() as u64
                            } else {
                                batch
                            };
                            writeln!(csv, "{backend},{rows},{other},{seed},{batch},{name},{instructions},{units},{:.3},{},{},{}", instructions as f64 / units as f64, memory[0], memory[1], memory[2]).unwrap();
                            csv.flush().unwrap();
                            samples.push(Sample {
                                native: is_native,
                                rows,
                                other,
                                seed,
                                batch,
                                operation,
                                instructions,
                                units,
                                memory,
                            });
                        }
                        pic.stop_canister(id, None).await.unwrap();
                        pic.delete_canister(id, None).await.unwrap();
                    }
                    println!(
                        "matrix verified rows={rows} sibling={other} batch={batch} seed={seed}"
                    );
                }
            }
        }
    }
    pic.drop().await;
    assert_eq!(
        samples.len(),
        2 * populations.len() * siblings.len() * batches.len() * seeds.len() * OPERATIONS.len()
    );
    fs::write(
        dir.join("summary.md"),
        report::render(&samples, seeds, &profile).unwrap(),
    )
    .unwrap();
    fs::write(
        dir.join("MATRIX"),
        format!(
            "{} correctness-verified samples; all pairs complete\n",
            samples.len()
        ),
    )
    .unwrap();
}

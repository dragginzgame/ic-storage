//! Installed instruction benchmark. Fixed bounded matrix, no timing of host RPCs.
#![forbid(unsafe_code)]

use std::{
    collections::BTreeMap,
    fs,
    io::Write,
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use ic_host_artifacts::{
    artifact::{Sha256Digest, read_reader},
    wasm,
};
use ic_storage_retention_probe::startup::{prepare_benchmark, prepare_graph};
use ic_storage_simple_benchmarks::OPERATIONS;
use ic_testkit::{pic::PocketIcStartupConfig, pocket_ic::PocketIcBuilder};

fn read(path: &Path) -> Vec<u8> {
    read_reader(
        fs::File::open(path).expect("run make benchmark-simple-build"),
        1024 * 1024,
    )
    .expect("bounded artifact")
}
fn words(bytes: &[u8]) -> Vec<u64> {
    assert_eq!(bytes.len() % 8, 0, "malformed result length");
    bytes
        .as_chunks::<8>()
        .0
        .iter()
        .map(|v| u64::from_le_bytes(*v))
        .collect()
}
fn input(a: u64, b: u64) -> Vec<u8> {
    a.to_le_bytes().into_iter().chain(b.to_le_bytes()).collect()
}

// Independently specified host oracle: no call to the measured workload or codecs.
fn expected(state: &mut Vec<u64>, operation: usize, batch: u64) -> u64 {
    match operation {
        0 => batch * 10_000 + batch * (batch - 1) / 2,
        1 => state[0] * batch,
        2 => {
            state[0] = 10_000 + batch - 1;
            batch * 10_000 + batch * (batch - 1) / 2
        }
        3 => state[1..].iter().sum::<u64>() * batch,
        4 => {
            for (i, v) in state[1..].iter_mut().enumerate() {
                *v = 10_000 + batch - 1 + i as u64;
            }
            batch * 10_000 + batch * (batch - 1) / 2
        }
        5 => {
            state.extend((0..batch).map(|i| 10_000 + i));
            batch * 10_000 + batch * (batch - 1) / 2
        }
        _ => unreachable!(),
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let managed_raw=read(&root.join("target/qualification/bench-managed/wasm32-unknown-unknown/release/ic_storage_rust_startup_probe.wasm"));
    let managed = prepare_benchmark(&managed_raw).expect("admit managed benchmark");
    assert!(
        prepare_graph(&managed_raw).is_err(),
        "benchmark imports require their own admission profile"
    );
    let storage=read(&root.join("target/qualification/bench-storage/wasm32-unknown-unknown/release/examples/storage_wasm.wasm"));
    wasm::validate(&storage, 1024 * 1024).expect("Storage artifact");
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();
    let output = root.join(format!("target/evidence/simple-benchmarks/{stamp}"));
    fs::create_dir_all(output.parent().unwrap()).unwrap();
    fs::create_dir(&output).expect("fresh evidence directory");
    let mut sources = BTreeMap::new();
    for path in [
        "Makefile",
        "rust-toolchain.toml",
        "Cargo.toml",
        "Cargo.lock",
        "qualification/Cargo.toml",
        "qualification/Cargo.lock",
        "crates/ic-storage-new/Cargo.toml",
        "crates/ic-storage-new/src/lib.rs",
        "crates/ic-storage-new/src/m0/mod.rs",
        "crates/ic-storage-new/src/m0/layout.rs",
        "crates/ic-storage-new/src/m0/image/mod.rs",
        "qualification/ic-storage-rust-startup-probe/Cargo.toml",
        "qualification/ic-storage-rust-startup-probe/src/lib.rs",
        "qualification/ic-storage-rust-startup-probe/src/graph/mod.rs",
        "qualification/ic-storage-rust-startup-probe/src/graph/kernel.rs",
        "qualification/ic-storage-rust-startup-probe/src/graph/benchmark.rs",
        "qualification/ic-storage-retention-probe/Cargo.toml",
        "qualification/ic-storage-retention-probe/src/lib.rs",
        "qualification/ic-storage-retention-probe/src/startup/mod.rs",
        "qualification/ic-storage-retention-probe/src/startup/growth.rs",
        "qualification/ic-storage-simple-benchmarks/Cargo.toml",
        "qualification/ic-storage-simple-benchmarks/src/lib.rs",
        "qualification/ic-storage-simple-benchmarks/src/wire.rs",
        "qualification/ic-storage-simple-benchmarks/benches/storage/mod.rs",
        "qualification/ic-storage-simple-benchmarks/benches/runner.rs",
    ] {
        let bytes = read(&root.join(path));
        sources.insert(path, Sha256Digest::compute(&bytes).to_string());
        let copy = output.join("sources").join(path);
        fs::create_dir_all(copy.parent().unwrap()).unwrap();
        fs::write(copy, bytes).unwrap();
    }
    fs::write(output.join("managed.wasm"), &managed.bytes).unwrap();
    fs::write(output.join("managed-raw.wasm"), &managed_raw).unwrap();
    fs::write(output.join("storage.wasm"), &storage).unwrap();
    fs::write(output.join("managed-admission.txt"), &managed.report).unwrap();
    fs::write(output.join("configuration.json"),serde_json::to_vec_pretty(&serde_json::json!({
        "schema":1,"completion_marker":"COMPLETE","storage":"0.74.1","runtime":"0.22.5","testkit":"0.106.0","sources_sha256":sources,
        "rust":"1.100.0","target":"wasm32-unknown-unknown","profile":"release; opt=3; lto=true; codegen-units=1; panic=abort; no post-link optimizer",
        "rows":[1,16,64],"batch":[1,8],"seeds":[1,2,3],"operations":OPERATIONS,
        "metric":"performance_counter(0) delta including backend entry/admission; excludes setup, wire and correctness queries",
        "managed_sha256":Sha256Digest::compute(&managed.bytes).to_string(),
        "storage_sha256":Sha256Digest::compute(&storage).to_string(),
        "qualification_lock_sha256":Sha256Digest::compute(&fs::read(root.join("qualification/Cargo.lock")).unwrap()).to_string()
    })).unwrap()).unwrap();
    let mut csv = fs::File::create(output.join("samples.csv")).unwrap();
    writeln!(
        csv,
        "backend,rows,seed,batch,operation,instructions,instructions_per_iteration,result"
    )
    .unwrap();
    csv.flush().unwrap();
    println!("Evidence: {}", output.display());
    let owner = PocketIcStartupConfig::from_env(Duration::from_secs(30))
        .expect("make retention-setup")
        .build_instance(
            PocketIcBuilder::new()
                .with_application_subnet()
                .with_max_request_time_ms(Some(120_000)),
        )
        .await
        .expect("start admitted server");
    let pic = owner.instance();
    let mut samples = 0;
    for rows in [1, 16, 64] {
        for seed in 1..=3 {
            for batch in [1, 8] {
                // Alternate backend order to avoid consistently favoring the first run.
                let backends = if seed % 2 == 1 {
                    [("managed", &managed.bytes), ("storage", &storage)]
                } else {
                    [("storage", &storage), ("managed", &managed.bytes)]
                };
                for (backend, module) in backends {
                    let id = pic.create_canister().await;
                    pic.install_canister(id, module.clone(), input(rows, seed), None)
                        .await;
                    let hash = pic
                        .canister_status(id, None)
                        .await
                        .unwrap()
                        .module_hash
                        .unwrap();
                    assert_eq!(hash, Sha256Digest::compute(module).as_bytes());
                    let mut state = vec![seed];
                    state.extend((0..rows).map(|i| seed * 100 + i));
                    assert_eq!(
                        words(&pic.query_call(id, id, "bench_state", vec![]).await.unwrap()),
                        state,
                        "initial state"
                    );
                    for (operation, name) in OPERATIONS.iter().enumerate() {
                        let result = pic
                            .update_call(id, id, "bench_run", input(operation as u64, batch))
                            .await
                            .expect("benchmark update; no retry");
                        let result = words(&result);
                        assert_eq!(result.len(), 2);
                        assert!(result[0] > 0);
                        assert_eq!(
                            result[1],
                            expected(&mut state, operation, batch),
                            "result {backend}/{rows}/{batch}/{name}"
                        );
                        assert_eq!(
                            words(&pic.query_call(id, id, "bench_state", vec![]).await.unwrap()),
                            state,
                            "state {backend}/{rows}/{batch}/{name}"
                        );
                        writeln!(
                            csv,
                            "{backend},{rows},{seed},{batch},{name},{},{:.3},{}",
                            result[0],
                            result[0] as f64 / batch as f64,
                            result[1]
                        )
                        .unwrap();
                        csv.flush().unwrap();
                        samples += 1;
                    }
                    pic.stop_canister(id, None).await.unwrap();
                    pic.delete_canister(id, None).await.unwrap();
                }
                println!("verified rows={rows} seed={seed} batch={batch}");
            }
        }
    }
    owner.close().await;
    assert_eq!(samples, 216);
    // Never mark a partial or failed attempt complete; raw rows remain diagnostic.
    fs::write(
        output.join("COMPLETE"),
        format!("{samples} validated samples\n"),
    )
    .unwrap();
    println!("Completed {samples} samples: {}", output.display());
}

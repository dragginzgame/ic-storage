//! Run the canister probe against an explicitly supplied, pre-admitted server.
mod fixtures;
use ic_testkit::pic::{
    CandidCallExt, CanisterInstallExt, InstallSpec, PocketIc, PocketIcBuilder,
    PocketIcStartupConfig, update_candid_args,
};
use ic_testkit_candid::DecodeLimits;
use std::{error::Error, path::Path, time::Duration};
// Replies contain only a bool or four integer counters, never stored payloads.
const REPLY_LIMITS: DecodeLimits = DecodeLimits {
    reply_bytes: 256,
    decoding_quota: 10_000,
    skipping_quota: 0,
    type_entries: 0,
    header_bytes: 32,
};
fn main() -> Result<(), Box<dyn Error>> {
    let args = std::env::args_os().skip(1).collect::<Vec<_>>();
    if !(args.len() == 2 || (args.len() == 3 && args[2] == "workloads")) {
        return Err("usage: ic-storage-bench POCKET_IC_BIN BENCH_WASM [workloads]".into());
    }
    let wasm = ic_host_fs::read::read_file(Path::new(&args[1]), 32 * 1024 * 1024)?;
    eprintln!(
        "wasm: {:?}",
        ic_host_fs::read::hash_file(Path::new(&args[1]), 32 * 1024 * 1024)?
    );
    eprintln!(
        "server: {:?}",
        ic_host_fs::read::hash_file(Path::new(&args[0]), 512 * 1024 * 1024)?
    );
    let server =
        PocketIcStartupConfig::spawn(&args[0], Duration::from_secs(30)).start_managed_server()?;
    println!("run,engine,profile,operation,instructions,heap_bytes,stable_bytes,result_bytes");
    if args.len() == 3 {
        return run_workloads(server.url(), &wasm);
    }
    for run in 0..2 {
        for (profile, fixture, dense_keys) in fixtures::FIXTURES
            .iter()
            .enumerate()
            .flat_map(|(id, fixture)| [false, true].map(|dense| (id, fixture, dense)))
        {
            let profile = u8::try_from(profile)?;
            let name = format!("{}{}", fixture.name, if dense_keys { "_dense" } else { "" });
            let batch = (524_288 / (fixture.key_len + fixture.value_len)).clamp(1, 32) as u64;
            assert!(fixture.count <= 2048 * batch && fixture.kind <= 2);
            for engine in 0u8..=1 {
                let (pic, id) = fresh_canister(server.url(), &wasm, engine, profile, dense_keys)?;
                let mut seeded = false;
                for _ in 0..2048 {
                    if pic.update_candid::<bool, _>(
                        id,
                        candid::Principal::anonymous(),
                        "seed",
                        (batch,),
                        REPLY_LIMITS,
                    )? {
                        seeded = true;
                        break;
                    }
                }
                if !seeded {
                    return Err("seeding exceeded the bounded call budget".into());
                }
                for (operation, label) in [
                    (0u8, "get"),
                    (4, "missing_get"),
                    (1, "replace"),
                    (10, "replace_changed"),
                    (2, "scan_keys"),
                    (5, "scan_values"),
                    (6, "shrink"),
                    (7, "regrow"),
                    (3, "remove"),
                    (8, "refill"),
                    (9, "insert_new"),
                ] {
                    let (instructions, heap, stable, result): (u128, u128, u128, u64) =
                        update_candid_args(
                            &pic,
                            id,
                            candid::Principal::anonymous(),
                            "measure",
                            (operation,),
                            REPLY_LIMITS,
                        )?;
                    println!(
                        "{run},{engine},{name},{label},{instructions},{heap},{stable},{result}"
                    );
                }
            }
        }
    }
    Ok(())
}

fn run_workloads(server: &str, wasm: &[u8]) -> Result<(), Box<dyn Error>> {
    for run in 0..2 {
        for (profile, name) in ["mixed_rows", "mixed_index"].iter().enumerate() {
            for engine in 0u8..=1 {
                let profile = u8::try_from(fixtures::FIXTURES.len() + profile)?;
                let (pic, id) = fresh_canister(server, wasm, engine, profile, false)?;
                for batch in 0..32 {
                    let done = pic.update_candid::<bool, _>(
                        id,
                        candid::Principal::anonymous(),
                        "workload_seed",
                        (),
                        REPLY_LIMITS,
                    )?;
                    assert_eq!(done, batch == 31);
                }
                for round in 0..4 {
                    for (operation, label) in [
                        "read_mix",
                        "fold_put",
                        "fold_delete",
                        "refill",
                        "keys_forward",
                        "keys_reverse",
                        "values_forward",
                        "values_reverse",
                    ]
                    .iter()
                    .enumerate()
                    {
                        let (instructions, heap, stable, result): (u128, u128, u128, u64) =
                            update_candid_args(
                                &pic,
                                id,
                                candid::Principal::anonymous(),
                                "workload_measure",
                                (u8::try_from(operation)?,),
                                REPLY_LIMITS,
                            )?;
                        println!(
                            "{run},{engine},{name},{label}_{round},{instructions},{heap},{stable},{result}"
                        );
                    }
                }
            }
        }
    }
    Ok(())
}

// Each comparison owns fresh memory and an independent simulator instance.
fn fresh_canister(
    server: &str,
    wasm: &[u8],
    engine: u8,
    profile: u8,
    dense_keys: bool,
) -> Result<(PocketIc, candid::Principal), Box<dyn Error>> {
    let pic = PocketIcBuilder::new()
        .with_server_url(server.parse()?)
        .with_application_subnet()
        .build();
    let id = pic.try_create_and_install(InstallSpec {
        wasm: wasm.to_vec(),
        init_bytes: candid::encode_args((engine, profile, dense_keys))?,
        cycles: 100_000_000_000_000,
        install_sender: None,
        label: None,
    })?;
    Ok((pic, id))
}

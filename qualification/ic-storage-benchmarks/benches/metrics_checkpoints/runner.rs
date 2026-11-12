//! Installed API timings and separate native backing-IO attribution.
mod workload;
use clap::Parser;
use ic_storage::testing::MeteredMemory;
use ic_testkit::pocket_ic::PocketIcBuilder;
use std::{fs, io::Write, path::PathBuf};
use workload::{OPERATIONS, Workload};

#[derive(Parser)]
struct Args {
    #[arg(long)]
    wasm: Option<PathBuf>,
    #[arg(long)]
    output: PathBuf,
    /// Capture debug records from an explicitly instrumented canister.
    #[arg(long, requires = "wasm")]
    canister_logs: Option<PathBuf>,
}
fn native<const N: usize>(output: &mut fs::File, rows: usize) {
    let memory: MeteredMemory = MeteredMemory::default();
    let mut state = Workload::<_, N>::new(memory.clone(), rows);
    for round in 0..3 {
        for (operation, label) in OPERATIONS.iter().enumerate() {
            memory.reset();
            state.step(operation, || 0);
            let io = memory.counts();
            assert!(!io.overflowed);
            if matches!(operation, 1 | 2 | 3 | 5) {
                assert_eq!(io.writes, 0);
                assert_eq!(io.grows, 0);
            }
            writeln!(
                output,
                "{N},{rows},{round},{label},{},{},{},{}",
                io.reads, io.read_bytes, io.writes, io.write_bytes
            )
            .unwrap();
        }
    }
}
#[tokio::main(flavor = "current_thread")]
async fn main() {
    let args = Args::parse();
    let mut logs = args
        .canister_logs
        .map(|path| fs::File::create_new(path).unwrap());
    let mut output = fs::File::create_new(&args.output).unwrap();
    let Some(wasm) = args.wasm else {
        writeln!(
            output,
            "bounds,rows,round,operation,reads,read_bytes,writes,write_bytes"
        )
        .unwrap();
        for rows in [1, 64] {
            native::<1>(&mut output, rows);
            native::<16>(&mut output, rows);
            native::<256>(&mut output, rows);
        }
        return;
    };
    let wasm = fs::read(wasm).unwrap();
    let url = std::env::var("IC_TESTKIT_POCKET_IC_URL").unwrap();
    let pic = PocketIcBuilder::new()
        .with_max_request_time_ms(Some(300_000))
        .with_server_url(url.parse().unwrap())
        .with_application_subnet()
        .build_async()
        .await;
    writeln!(
        output,
        "bounds,rows,round,operation,first_instructions,second_instructions,pages"
    )
    .unwrap();
    for bounds in [1_u64, 16, 256] {
        for rows in [1_u64, 64] {
            let canister = pic.create_canister().await;
            pic.add_cycles(canister, 100_000_000_000_000).await;
            pic.install_canister(
                canister,
                wasm.clone(),
                [bounds.to_le_bytes(), rows.to_le_bytes()].concat(),
                None,
            )
            .await;
            let mut next_log = 0;
            if logs.is_some() {
                next_log = pic
                    .fetch_canister_logs(canister, ic_principal::Principal::anonymous())
                    .await
                    .unwrap()
                    .last()
                    .map_or(0, |record| record.idx + 1);
            }
            for round in 0..3 {
                for (operation, label) in OPERATIONS.iter().enumerate() {
                    let repetitions = if operation <= 3 { 2 } else { 1 };
                    let bytes = pic
                        .update_call(
                            canister,
                            canister,
                            "run",
                            vec![u8::try_from(operation).unwrap(), repetitions],
                        )
                        .await
                        .unwrap();
                    assert_eq!(bytes.len(), 24);
                    let numbers: Vec<_> = bytes
                        .as_chunks::<8>()
                        .0
                        .iter()
                        .map(|v| u64::from_le_bytes(*v))
                        .collect();
                    writeln!(
                        output,
                        "{bounds},{rows},{round},{label},{},{},{}",
                        numbers[0], numbers[1], numbers[2]
                    )
                    .unwrap();
                    output.flush().unwrap();
                    if let Some(logs) = &mut logs {
                        for record in pic
                            .fetch_canister_logs(canister, ic_principal::Principal::anonymous())
                            .await
                            .unwrap()
                        {
                            if record.idx < next_log {
                                continue;
                            }
                            next_log = record.idx + 1;
                            writeln!(
                                logs,
                                "{}",
                                serde_json::json!({
                                    "bounds": bounds, "rows": rows, "round": round,
                                    "operation": label, "index": record.idx,
                                    "message": String::from_utf8(record.content).unwrap(),
                                })
                            )
                            .unwrap();
                        }
                        logs.flush().unwrap();
                    }
                }
            }
            pic.stop_canister(canister, None).await.unwrap();
            pic.delete_canister(canister, None).await.unwrap();
        }
    }
    pic.drop().await;
}

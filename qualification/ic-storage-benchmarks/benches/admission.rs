//! Installed admission scaling. ICSS does not supply Storage's governance checks.
use clap::Parser;
use ic_testkit::pocket_ic::PocketIcBuilder;
use std::{fs, io::Write, path::PathBuf};
#[derive(Parser)]
struct Args {
    #[arg(long)]
    wasm: PathBuf,
    #[arg(long)]
    output: PathBuf,
    #[arg(long)]
    icss: bool,
}
#[tokio::main(flavor = "current_thread")]
async fn main() {
    let args = Args::parse();
    let wasm = fs::read(&args.wasm).unwrap();
    let url = std::env::var("IC_TESTKIT_POCKET_IC_URL").unwrap();
    let pic = PocketIcBuilder::new(std::time::Duration::from_secs(300))
        .with_server_url(url.parse().unwrap())
        .with_application_subnet()
        .build()
        .await
        .unwrap();
    let mut output = fs::File::create(args.output).unwrap();
    writeln!(
        output,
        "names,buckets,operation,sample,instructions,value,pages"
    )
    .unwrap();
    for names in [1_u64, 16, 64, 255, 256, 1024] {
        if args.icss && names > 255 {
            continue;
        }
        for buckets in [0_u64, 1, 32] {
            let canister = pic.create_canister().await.unwrap();
            pic.add_cycles(canister, 100_000_000_000_000).await.unwrap();
            pic.install_canister(
                canister,
                wasm.clone(),
                [names.to_le_bytes(), buckets.to_le_bytes()].concat(),
                None,
            )
            .await
            .unwrap();
            for (operation, label) in ["one", "all", "reopen", "append"].into_iter().enumerate() {
                if args.icss && operation == 3 && names > 252 {
                    continue;
                }
                for sample in 0..3 {
                    let result = pic
                        .update_call(canister, canister, "run", vec![operation as u8])
                        .await
                        .unwrap();
                    assert_eq!(result.len(), 24);
                    let values: Vec<_> = result
                        .as_chunks::<8>()
                        .0
                        .iter()
                        .map(|v| u64::from_le_bytes(*v))
                        .collect();
                    assert_eq!(values[1], if operation >= 2 { 0 } else { buckets * 8 });
                    assert_eq!(values[2], buckets * 8);
                    writeln!(
                        output,
                        "{names},{buckets},{label},{sample},{},{},{}",
                        values[0], values[1], values[2]
                    )
                    .unwrap();
                    output.flush().unwrap();
                }
            }
            pic.stop_canister(canister, None).await.unwrap();
            pic.delete_canister(canister, None).await.unwrap();
        }
    }
    pic.drop().await.unwrap();
}

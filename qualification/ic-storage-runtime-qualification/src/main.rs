//! Explicit installed qualification of the supplied current-graph fixture.
mod logging;
mod memory;
mod ownership;
mod storage;

use clap::Parser;
use ic_testkit::pocket_ic::{
    CanisterId, ErrorCode, PocketIcBuilder, RejectResponse, nonblocking::PocketIc,
};
use std::env;

///
/// Arguments
///
/// Select measurement-only execution before reading inputs or contacting a server.
///

#[derive(Debug, Parser)]
#[command(about = "Qualify the supplied Wasm against an explicitly prepared PocketIC server")]
struct Arguments {
    /// Measure valid IO only, skipping rejection and upgrade checks.
    #[arg(long)]
    measure_valid_io: bool,
    /// Qualify only the supplied logging fixture, without Memory/Storage suites.
    #[arg(long, conflicts_with = "measure_valid_io")]
    logging_only: bool,
    /// Qualify only collection IO, admission, upgrade and rollback in the Storage fixture.
    #[arg(long, conflicts_with_all = ["measure_valid_io", "logging_only"])]
    storage_only: bool,
    /// Populate using this earlier compatible fixture, then upgrade to the selected fixture.
    #[arg(long, requires = "storage_only")]
    storage_install_wasm: Option<std::path::PathBuf>,
}

fn assert_trap(error: RejectResponse) {
    assert!(
        matches!(
            error.error_code,
            ErrorCode::CanisterTrapped | ErrorCode::CanisterCalledTrap
        ),
        "expected an execution trap, got {error:?}",
    );
}

async fn connect_to_server() -> PocketIc {
    // The caller owns server startup/teardown. Never implicitly spawn or
    // download a binary as part of qualification.
    let url = env::var("IC_TESTKIT_POCKET_IC_URL").expect("caller-selected PocketIC server URL");
    PocketIcBuilder::new()
        .with_max_request_time_ms(Some(300_000))
        .with_server_url(url.parse().unwrap())
        .with_application_subnet()
        .build_async()
        .await
}

async fn install_fixture(pic: &PocketIc, wasm: &[u8], init: Vec<u8>) -> CanisterId {
    let canister = pic.create_canister().await;
    pic.add_cycles(canister, 100_000_000_000_000).await;
    pic.install_canister(canister, wasm.to_vec(), init, None)
        .await;
    canister
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let arguments = Arguments::parse();
    let pic = connect_to_server().await;
    if arguments.storage_only {
        storage::qualify(&pic, arguments.storage_install_wasm.as_deref()).await;
    } else if arguments.logging_only {
        logging::qualify(&pic).await;
    } else {
        memory::qualify(&pic, arguments.measure_valid_io).await;
        if !arguments.measure_valid_io {
            storage::qualify(&pic, None).await;
            ownership::qualify(&pic).await;
            logging::qualify(&pic).await;
        }
    }
    pic.drop().await;
}

#[cfg(test)]
mod tests;

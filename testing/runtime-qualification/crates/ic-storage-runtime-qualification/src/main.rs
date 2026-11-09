//! Explicit installed qualification of the supplied current-graph fixture.
mod logging;
mod memory;
mod ownership;
mod storage;

use clap::Parser;
use ic_testkit::pocket_ic::{CanisterId, ClientError, ErrorCode, PocketIc, PocketIcBuilder};
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
}

fn assert_trap(error: ClientError) {
    let ClientError::Canister(error) = error else {
        panic!("expected a canister trap, got client failure: {error:?}");
    };
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
    PocketIcBuilder::new(std::time::Duration::from_secs(300))
        .with_server_url(url.parse().unwrap())
        .with_application_subnet()
        .build()
        .await
        .expect("connect to prepared PocketIC server")
}

async fn install_fixture(pic: &PocketIc, wasm: &[u8], init: Vec<u8>) -> CanisterId {
    let canister = pic
        .create_canister()
        .await
        .expect("create fixture canister");
    pic.add_cycles(canister, 100_000_000_000_000)
        .await
        .expect("fund fixture canister");
    pic.install_canister(canister, wasm.to_vec(), init, None)
        .await
        .expect("install fixture canister");
    canister
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let arguments = Arguments::parse();
    let pic = connect_to_server().await;
    if arguments.storage_only {
        storage::qualify(&pic).await;
    } else if arguments.logging_only {
        logging::qualify(&pic).await;
    } else {
        memory::qualify(&pic, arguments.measure_valid_io).await;
        if !arguments.measure_valid_io {
            storage::qualify(&pic).await;
            ownership::qualify(&pic).await;
            logging::qualify(&pic).await;
        }
    }
    pic.drop().await.expect("delete qualification instance");
}

#[cfg(test)]
mod tests;

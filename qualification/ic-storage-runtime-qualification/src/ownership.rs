//! Same-canister backing custody and retained component handles.
use super::memory::{frame, protocol, read};
use ic_testkit::pocket_ic::PocketIc;

pub(super) async fn qualify(pic: &PocketIc) {
    let wasm =
        std::fs::read(std::env::var("IC_MEMORY_QUALIFICATION_WASM").expect("Memory fixture path"))
            .unwrap();
    let canister = super::install_fixture(pic, &wasm, frame(protocol::INIT_ID, &())).await;
    let snapshot = async || read(pic, canister).await;
    let initial = snapshot().await;
    for mode in 0..6_u8 {
        super::assert_trap(
            pic.update_call(
                canister,
                canister,
                "metadata_fault",
                frame(protocol::METADATA_FAULT_ID, &mode),
            )
            .await
            .unwrap_err(),
        );
        assert_eq!(
            snapshot().await,
            initial,
            "manager fault and preceding payload write must roll back"
        );
    }
    println!(
        "installed directory checkpoints: six open/growth faults trap, poison old handles and roll back metadata/payload writes"
    );
    let reply = pic
        .update_call(
            canister,
            canister,
            "composition",
            frame(protocol::COMPOSITION_ID, &()),
        )
        .await
        .unwrap();
    ic_wire::native::decode_response::<()>(protocol::COMPOSITION_ID, &reply, protocol::RESPONSE)
        .unwrap();
    assert_eq!(
        snapshot().await,
        protocol::Snapshot {
            generation: initial.generation,
            ..initial
        }
    );
    pic.advance_time(std::time::Duration::from_secs(300))
        .await
        .expect("advance PocketIC time");
    pic.upgrade_canister(canister, wasm, frame(protocol::UPGRADE_ID, &0_u8), None)
        .await
        .unwrap();
    assert_eq!(
        snapshot().await,
        protocol::Snapshot {
            generation: initial.generation,
            ..initial
        }
    );
    let reply = pic
        .update_call(
            canister,
            canister,
            "composition",
            frame(protocol::COMPOSITION_ID, &()),
        )
        .await
        .unwrap();
    ic_wire::native::decode_response::<()>(protocol::COMPOSITION_ID, &reply, protocol::RESPONSE)
        .unwrap();
    assert_eq!(
        snapshot().await,
        protocol::Snapshot {
            generation: initial.generation,
            ..initial
        }
    );
    println!(
        "installed backing custody: composed handles refuse duplicate owners through runtime drop; final release and upgrade reopen preserve payloads/pages"
    );
}

#[cfg(test)]
mod tests;

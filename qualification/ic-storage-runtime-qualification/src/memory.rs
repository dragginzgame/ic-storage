//! Installed Memory scenarios and actual valid-IO measurements.
pub(super) use ic_storage_test_fixtures::io as protocol;
use ic_testkit::pocket_ic::{CanisterId, nonblocking::PocketIc};
use ic_wire::native;
use protocol::Snapshot;
use std::{env, fs, time::Duration};

pub(super) fn frame(id: u64, value: &impl serde::Serialize) -> Vec<u8> {
    native::encode_request(id, value, protocol::REQUEST).unwrap()
}
pub(super) async fn read(pic: &PocketIc, canister: CanisterId) -> Snapshot {
    let bytes = pic
        .query_call(
            canister,
            canister,
            "snapshot",
            frame(protocol::SNAPSHOT_ID, &()),
        )
        .await
        .unwrap();
    native::decode_response(protocol::SNAPSHOT_ID, &bytes, protocol::RESPONSE).unwrap()
}

pub(super) async fn qualify(pic: &PocketIc, measure_valid_io: bool) {
    let wasm = fs::read(env::var("IC_MEMORY_QUALIFICATION_WASM").expect("fixture path")).unwrap();
    let canister = super::install_fixture(pic, &wasm, frame(protocol::INIT_ID, &())).await;
    let snapshot = async || read(pic, canister).await;
    let initial = snapshot().await;
    assert!(initial.generation >= 1);
    assert_eq!(initial.markers, [0x42, 0x6D]);
    assert_eq!(initial.rows_pages, 1);

    if !measure_valid_io {
        let valid = frame(
            protocol::IO_ID,
            &protocol::IoRequest {
                operation: 0,
                count: 1,
                offset: 7,
            },
        );
        let mut trailing = valid.clone();
        trailing.push(0);
        for bytes in [
            Vec::new(),
            valid[..valid.len() - 1].to_vec(),
            trailing,
            frame(protocol::SNAPSHOT_ID, &()),
            vec![0; protocol::REQUEST.bytes + 1],
        ] {
            super::assert_trap(
                pic.update_call(canister, canister, "io", bytes)
                    .await
                    .unwrap_err(),
            );
            assert_eq!(
                snapshot().await,
                initial,
                "refused Wire frame must preserve state"
            );
        }
        println!(
            "installed Memory Wire: five malformed/foreign/oversized frames refuse without retained mutation"
        );
        // Check all three delegates, including empty spans beyond the extent.
        for operation in 0..3 {
            for (offset, count) in [(65_536, 1), (65_537, 0), (u64::MAX, 1), (u64::MAX - 1, 2)] {
                let args = frame(
                    protocol::IO_ID,
                    &protocol::IoRequest {
                        operation,
                        count,
                        offset,
                    },
                );
                super::assert_trap(
                    pic.update_call(canister, canister, "io", args)
                        .await
                        .unwrap_err(),
                );
                assert_eq!(
                    snapshot().await,
                    initial,
                    "invalid IO must roll back the preceding valid write"
                );
            }
        }
        println!("installed invalid IO: 12 traps, neighboring marker and prior mutation preserved");

        let physical_end = initial.physical_pages * 65_536;
        for operation in 3..6 {
            for (offset, count) in [
                (physical_end, 1),
                (physical_end + 1, 0),
                (u64::MAX, 1),
                (u64::MAX - 1, 2),
            ] {
                let args = frame(
                    protocol::IO_ID,
                    &protocol::IoRequest {
                        operation,
                        count,
                        offset,
                    },
                );
                super::assert_trap(
                    pic.update_call(canister, canister, "io", args)
                        .await
                        .unwrap_err(),
                );
                assert_eq!(
                    snapshot().await,
                    initial,
                    "invalid physical IO must roll back prior writes"
                );
            }
        }
        println!("installed physical backing IO: 12 range/overflow traps preserve state");

        for mode in [1_u8, 2] {
            pic.advance_time(Duration::from_secs(300)).await;
            super::assert_trap(
                pic.upgrade_canister(
                    canister,
                    wasm.clone(),
                    frame(protocol::UPGRADE_ID, &mode),
                    None,
                )
                .await
                .unwrap_err(),
            );
            assert_eq!(
                snapshot().await,
                initial,
                "failed upgrade must preserve original state"
            );
        }
        pic.advance_time(Duration::from_secs(300)).await;
        pic.upgrade_canister(
            canister,
            wasm.clone(),
            frame(protocol::UPGRADE_ID, &0_u8),
            None,
        )
        .await
        .unwrap();
        let upgraded = snapshot().await;
        assert_eq!(
            upgraded,
            Snapshot {
                generation: initial.generation,
                ..initial
            }
        );
        println!(
            "installed upgrade: post-bootstrap trap and resource-limit conflict roll back; clean reopen preserves the directory generation"
        );
    }

    for operation in 0..3 {
        for (offset, count) in [(7_u64, 1), (65_536, 0), (65_535, 1)] {
            let args = frame(
                protocol::IO_ID,
                &protocol::IoRequest {
                    operation,
                    count,
                    offset,
                },
            );
            let cycles_before = pic.cycle_balance(canister).await;
            let reply = pic
                .update_call(canister, canister, "io", args)
                .await
                .unwrap();
            let cycles = cycles_before
                .checked_sub(pic.cycle_balance(canister).await)
                .expect("measured update must not increase the cycle balance");
            let reply: protocol::IoReply =
                native::decode_response(protocol::IO_ID, &reply, protocol::RESPONSE).unwrap();
            let instructions = reply.instructions;
            println!("valid_io,{operation},{offset},{count},1000,{instructions}");
            println!("valid_io_cycles,{operation},{offset},{count},1000,{cycles}");
        }
    }
    let final_snapshot = snapshot().await;
    assert_eq!(final_snapshot.markers, [0x99, 0x6D]);
    assert_eq!(final_snapshot.physical_pages, initial.physical_pages);
    assert_eq!(final_snapshot.rows_pages, initial.rows_pages);
    println!("fixture_wasm_bytes,{}", wasm.len());
}

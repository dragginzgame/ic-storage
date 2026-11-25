//! Matched installed measurements. Update calls expose accidental stable writes.
use ic_testkit::pocket_ic::PocketIcBuilder;

#[tokio::test]
#[ignore = "requires prepared Testkit server and read_paths Wasm"]
async fn matched_read_paths() {
    let pic = PocketIcBuilder::new()
        .with_application_subnet()
        .with_server_url(
            std::env::var("IC_TESTKIT_POCKET_IC_URL")
                .unwrap()
                .parse()
                .unwrap(),
        )
        .build_async()
        .await;
    let wasm = std::fs::read(std::env::var_os("STORAGE_READ_WASM").unwrap()).unwrap();
    let args = |words: [u64; 4]| {
        words
            .into_iter()
            .flat_map(u64::to_le_bytes)
            .collect::<Vec<_>>()
    };
    for size in [16, 4096] {
        let id = pic.create_canister().await;
        pic.install_canister(id, wasm.clone(), vec![], None).await;
        for start in (0..512).step_by(32) {
            pic.update_call(id, id, "seed", args([start, size, 0, 0]))
                .await
                .unwrap();
        }
        for phase in ["fresh", "reopened"] {
            if phase == "reopened" {
                pic.upgrade_canister(id, wasm.clone(), vec![], None)
                    .await
                    .unwrap();
            }
            let backing = pic.get_stable_memory(id).await;
            for count in [1, 8, 32] {
                for pattern in 0..3 {
                    let mut point_nodes = 0;
                    for mode in 0..4 {
                        if mode >= 2 && pattern != 0 {
                            continue;
                        }
                        let reply = pic
                            .update_call(id, id, "measure", args([mode, count, pattern, size]))
                            .await
                            .unwrap();
                        let [instructions, heap, nodes, payloads]: [u64; 4] =
                            std::array::from_fn(|i| {
                                u64::from_le_bytes(reply[8 * i..8 * i + 8].try_into().unwrap())
                            });
                        assert!(nodes > 0);
                        assert_eq!(
                            payloads,
                            if size == 16 || mode == 3 || pattern == 2 {
                                0
                            } else {
                                count
                            }
                        );
                        if mode == 0 {
                            point_nodes = nodes;
                        }
                        if mode == 1 {
                            assert!(nodes <= point_nodes);
                        }
                        println!(
                            "read,{phase},{size},{count},{pattern},{mode},{instructions},{heap},{nodes},{payloads}"
                        );
                    }
                }
            }
            assert_eq!(pic.get_stable_memory(id).await, backing);
        }
    }
    pic.drop().await;
}

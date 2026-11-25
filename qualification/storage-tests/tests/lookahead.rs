//! Compare matched pagination probes and their stable read counts.
use ic_testkit::pocket_ic::PocketIcBuilder;

#[tokio::test]
#[ignore = "requires prepared Testkit server and lookahead Wasm"]
async fn positioned_read_costs() {
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
    let wasm = std::fs::read(std::env::var_os("LOOKAHEAD_WASM").unwrap()).unwrap();
    let args = |words: [u64; 4]| {
        words
            .into_iter()
            .flat_map(u64::to_le_bytes)
            .collect::<Vec<_>>()
    };
    for width in [16, 1024] {
        let id = pic.create_canister().await;
        pic.install_canister(id, wasm.clone(), vec![], None).await;
        pic.update_call(id, id, "seed", args([0, width, 0, 0]))
            .await
            .unwrap();
        let backing = pic.get_stable_memory(id).await;
        for reverse in 0..2 {
            for exhausted in 0..2 {
                for mode in 0..3 {
                    let reply = pic
                        .update_call(id, id, "lookahead", args([mode, reverse, exhausted, width]))
                        .await
                        .unwrap();
                    let [key, total, probe, nodes, values, heap]: [u64; 6] =
                        std::array::from_fn(|i| {
                            u64::from_le_bytes(reply[i * 8..(i + 1) * 8].try_into().unwrap())
                        });
                    assert_eq!(
                        key,
                        if exhausted == 1 {
                            u64::MAX
                        } else if reverse == 1 {
                            12
                        } else {
                            24
                        }
                    );
                    assert_eq!(
                        values,
                        u64::from(mode == 0 && width == 1024 && exhausted == 0)
                    );
                    println!(
                        "lookahead width={width} reverse={reverse} exhausted={exhausted} mode={mode} total={total} probe={probe} nodes={nodes} values={values} heap={heap}"
                    );
                }
            }
        }
        assert_eq!(backing, pic.get_stable_memory(id).await);
    }
    pic.drop().await;
}

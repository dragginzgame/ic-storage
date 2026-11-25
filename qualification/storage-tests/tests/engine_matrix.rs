//! Run the identical example Wasm on fresh PocketIC instances for both engines.
use ic_testkit::pocket_ic::PocketIcBuilder;

#[tokio::test]
#[ignore = "requires the comparison Wasm and the local PocketIC server"]
async fn compare_engines() {
    let url = std::env::var("IC_TESTKIT_POCKET_IC_URL").unwrap();
    let pic = PocketIcBuilder::new()
        .with_application_subnet()
        .with_server_url(url.parse().unwrap())
        .build_async()
        .await;
    let wasm = std::fs::read(std::env::var_os("ENGINE_MATRIX_WASM").unwrap()).unwrap();
    for (n, width) in [
        (256_u64, 0_u64),
        (256, 16),
        (256, 31),
        (256, 32),
        (256, 33),
        (256, 236),
        (256, 237),
        (256, 1004),
        (256, 1005),
        (256, 1024),
        (256, 8172),
        (256, 8173),
        (256, 8192),
        (2048, 16),
        (2048, 33),
    ] {
        for engine in [0_u64, 1] {
            let mut first = None;
            for _ in 0..2 {
                let id = pic.create_canister().await;
                pic.install_canister(id, wasm.clone(), vec![], None).await;
                let args = [engine, width, n]
                    .into_iter()
                    .flat_map(u64::to_le_bytes)
                    .collect::<Vec<_>>();
                let reply = pic
                    .update_call(id, id, "matrix", args.clone())
                    .await
                    .unwrap_or_else(|err| panic!("engine={engine} n={n} width={width}: {err}"));
                if n == 256 && width == 16 {
                    pic.upgrade_canister(id, wasm.clone(), vec![], None)
                        .await
                        .unwrap();
                    pic.query_call(id, id, "verify", args.clone())
                        .await
                        .unwrap();
                    let before = pic.get_stable_memory(id).await;
                    let failure = pic
                        .update_call(id, id, "rollback", args.clone())
                        .await
                        .unwrap_err();
                    assert!(
                        failure
                            .reject_message
                            .contains("matrix rollback after writes"),
                        "{failure}"
                    );
                    assert_eq!(
                        before.as_slice(),
                        pic.get_stable_memory(id).await.as_slice()
                    );
                    pic.query_call(id, id, "verify", args.clone())
                        .await
                        .unwrap();
                    let optional = pic.update_call(id, id, "optional_none", args).await;
                    if engine == 0 {
                        optional.unwrap();
                    } else {
                        let error = optional.unwrap_err();
                        assert!(error.reject_message.contains("expected a fixed-size element with length 5 bytes, but found 1 bytes"), "{error}");
                    }
                }
                if let Some(prior) = &first {
                    assert_eq!(prior, &reply, "fresh replicas disagree");
                } else {
                    for line in std::str::from_utf8(&reply).unwrap().lines() {
                        println!("matrix {engine} {n} {width} {line}");
                    }
                    first = Some(reply);
                }
            }
        }
    }
    pic.drop().await;
}

#[tokio::test]
#[ignore = "requires the comparison Wasm and the local PocketIC server"]
async fn compare_stored_keys() {
    let url = std::env::var("IC_TESTKIT_POCKET_IC_URL").unwrap();
    let pic = PocketIcBuilder::new()
        .with_application_subnet()
        .with_server_url(url.parse().unwrap())
        .build_async()
        .await;
    let wasm = std::fs::read(std::env::var_os("ENGINE_MATRIX_WASM").unwrap()).unwrap();
    for width in [16, 512] {
        let args = |arm: u64| {
            [arm, width, 256]
                .into_iter()
                .flat_map(u64::to_le_bytes)
                .collect::<Vec<_>>()
        };
        let id = pic.create_canister().await;
        pic.install_canister(id, wasm.clone(), vec![], None).await;
        pic.update_call(id, id, "seed_comparison_keys", args(0))
            .await
            .unwrap();
        let before = pic.get_stable_memory(id).await;
        for arm in [0, 1] {
            let reply = pic
                .update_call(id, id, "compare_stored_keys", args(arm))
                .await
                .unwrap();
            for line in std::str::from_utf8(&reply).unwrap().lines() {
                println!("comparison {width} {arm} {line}");
            }
            assert_eq!(pic.get_stable_memory(id).await, before);
        }
    }
    pic.drop().await;
}

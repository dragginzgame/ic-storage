//! Installed ordinary-upgrade, query isolation and whole-message rollback witness.
use ic_testkit::pocket_ic::{CanisterId, PocketIcBuilder, nonblocking::PocketIc};

fn args(words: [u64; 4]) -> Vec<u8> {
    words.into_iter().flat_map(u64::to_le_bytes).collect()
}

async fn query(pic: &PocketIc, id: CanisterId, method: &str, words: [u64; 4]) -> Vec<u8> {
    pic.query_call(id, id, method, args(words)).await.unwrap()
}

#[tokio::test]
#[ignore = "requires prepared local Testkit server and built storage-canister artifact"]
async fn stable_collections_survive_upgrades_and_roll_back_failed_messages() {
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
    let wasm = std::fs::read(std::env::var_os("STORAGE_CANISTER_WASM").unwrap()).unwrap();
    let id = pic.create_canister().await;
    pic.install_canister(id, wasm.clone(), vec![], None).await;
    for key in 1..=32 {
        pic.update_call(id, id, "put", args([key, key, 4096, 0]))
            .await
            .unwrap();
    }
    let expected = query(&pic, id, "stats", [0; 4]).await;
    assert_eq!(expected.len(), 32);
    let stats: [u64; 4] = std::array::from_fn(|i| {
        u64::from_le_bytes(expected[i * 8..(i + 1) * 8].try_into().unwrap())
    });
    assert_eq!(&stats[..3], &[32, 32, 32]);
    assert!(stats[3] > 0);
    for (key, size) in [(1, 4096), (100, 1024 * 1024)] {
        assert!(
            pic.update_call(id, id, "put", args([key, 99, size, 1]))
                .await
                .is_err()
        );
        assert_eq!(query(&pic, id, "stats", [0; 4]).await, expected);
    }
    pic.query_call(id, id, "query_put", args([1, 99, 4096, 0]))
        .await
        .unwrap();
    assert_eq!(query(&pic, id, "get", [1, 0, 0, 0]).await, vec![1; 4096]);
    assert_eq!(query(&pic, id, "stats", [0; 4]).await, expected);
    assert!(
        pic.upgrade_canister(id, wasm.clone(), vec![1], None)
            .await
            .is_err()
    );
    assert_eq!(query(&pic, id, "stats", [0; 4]).await, expected);
    pic.upgrade_canister(id, wasm.clone(), vec![], None)
        .await
        .unwrap();
    assert_eq!(query(&pic, id, "stats", [0; 4]).await, expected);
    for key in 1..=32 {
        assert_eq!(
            query(&pic, id, "get", [key, 0, 0, 0]).await,
            vec![key as u8; 4096]
        );
    }
    assert!(
        pic.update_call(id, id, "delete", args([1, 0, 0, 1]))
            .await
            .is_err()
    );
    assert_eq!(query(&pic, id, "get", [1, 0, 0, 0]).await, vec![1; 4096]);
    pic.update_call(id, id, "delete", args([1, 0, 0, 0]))
        .await
        .unwrap();
    pic.upgrade_canister(id, wasm, vec![], None).await.unwrap();
    assert!(query(&pic, id, "get", [1, 0, 0, 0]).await.is_empty());
    pic.update_call(id, id, "put", args([1, 77, 8192, 0]))
        .await
        .unwrap();
    assert_eq!(query(&pic, id, "get", [1, 0, 0, 0]).await, vec![77; 8192]);
    pic.drop().await;
}

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
        let rejection = pic
            .update_call(id, id, "put", args([key, 99, size, 1]))
            .await
            .unwrap_err();
        assert!(
            rejection
                .reject_message
                .contains("rollback rows, index, revision and allocation growth"),
            "{rejection}"
        );
        assert_eq!(query(&pic, id, "stats", [0; 4]).await, expected);
    }
    pic.query_call(id, id, "query_put", args([1, 99, 4096, 0]))
        .await
        .unwrap();
    assert_eq!(query(&pic, id, "get", [1, 0, 0, 0]).await, vec![1; 4096]);
    assert_eq!(query(&pic, id, "stats", [0; 4]).await, expected);
    let rejection = pic
        .upgrade_canister(id, wasm.clone(), vec![1], None)
        .await
        .unwrap_err();
    assert!(
        rejection.reject_message.contains("failed target startup"),
        "{rejection}"
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
    let rejection = pic
        .update_call(id, id, "delete", args([1, 0, 0, 1]))
        .await
        .unwrap_err();
    assert!(
        rejection.reject_message.contains("rollback deletion"),
        "{rejection}"
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

#[tokio::test]
#[ignore = "requires prepared local Testkit server and built storage-canister artifact"]
async fn compare_prepared_bulk_and_point_removal() {
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
    for mutations in [32, 256] {
        // Three indexes per child and one index for the parent in the IC DB fixture.
        let count = (mutations - 1) * 4 + 2;
        for stride in [1, 2] {
            for bulk in [0, 1] {
                let id = pic.create_canister().await;
                pic.install_canister(id, wasm.clone(), vec![], None).await;
                for start in (0..count * 2).step_by(64) {
                    let chunk = (count * 2 - start).min(64);
                    pic.update_call(id, id, "seed_delete_rows", args([start, chunk, 0, 0]))
                        .await
                        .unwrap();
                }
                if bulk == 1 {
                    let before = pic.get_stable_memory(id).await;
                    let err = pic
                        .update_call(
                            id,
                            id,
                            "measure_delete",
                            args([count, stride, bulk, count + 1]),
                        )
                        .await
                        .unwrap_err();
                    assert!(
                        err.reject_message
                            .contains("prepared removal count changed"),
                        "{err}"
                    );
                    assert_eq!(
                        pic.get_stable_memory(id).await.as_slice(),
                        before.as_slice(),
                        "stable backing changed after trap"
                    );
                }
                let reply = pic
                    .update_call(id, id, "measure_delete", args([count, stride, bulk, count]))
                    .await
                    .unwrap();
                let [removed, instructions, pages, heap] = std::array::from_fn(|i| {
                    u64::from_le_bytes(reply[i * 8..i * 8 + 8].try_into().unwrap())
                });
                assert_eq!(removed, count);
                println!(
                    "delete mutations={mutations} keys={count} stride={stride} bulk={bulk} instructions={instructions} wasm_growth_pages={pages} peak_extra_heap_bytes={heap}"
                );
                pic.update_call(id, id, "measure_delete", args([count, stride, bulk, 0]))
                    .await
                    .unwrap();
                pic.upgrade_canister(id, wasm.clone(), vec![], None)
                    .await
                    .unwrap();
                assert_eq!(
                    u64::from_le_bytes(
                        query(&pic, id, "stats", [0; 4]).await[..8]
                            .try_into()
                            .unwrap()
                    ),
                    count
                );
                let survivor = if stride == 1 { count } else { 1 };
                assert_eq!(
                    query(&pic, id, "get", [survivor, 0, 0, 0]).await,
                    vec![survivor as u8; if survivor % 13 == 0 { 8192 } else { 15 }]
                );
                assert!(query(&pic, id, "get", [0; 4]).await.is_empty());
            }
        }
    }
    pic.drop().await;
}

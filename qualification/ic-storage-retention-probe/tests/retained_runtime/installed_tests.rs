use super::*;
use ic_testkit::pocket_ic::{
    CanisterId, CanisterInstallMode, PocketIcBuilder, nonblocking::PocketIc,
};
fn args(words: [u64; 3]) -> Vec<u8> {
    words.into_iter().flat_map(u64::to_le_bytes).collect()
}
async fn query(pic: &PocketIc, id: CanisterId, name: &str) -> Vec<u8> {
    pic.query_call(id, id, name, vec![]).await.unwrap()
}
async fn pages(pic: &PocketIc, id: CanisterId) -> u64 {
    u64::from_le_bytes(query(pic, id, "memory_pages").await.try_into().unwrap())
}
async fn update(pic: &PocketIc, id: CanisterId, name: &str, words: [u64; 3]) {
    pic.update_call(id, id, name, args(words)).await.unwrap();
}
async fn replace(
    pic: &PocketIc,
    id: CanisterId,
    wasm: &[u8],
    words: [u64; 3],
) -> Result<(), ic_testkit::pocket_ic::RejectResponse> {
    let mut mode = CanisterInstallMode::Upgrade(Some(Default::default()));
    let CanisterInstallMode::Upgrade(Some(flags)) = &mut mode else {
        unreachable!()
    };
    flags.wasm_memory_persistence = Some(Default::default());
    assert_eq!(
        format!("{:?}", flags.wasm_memory_persistence),
        "Some(Replace)"
    );
    pic.clear_chunk_store(id, None).await?;
    let hash = pic.upload_chunk(id, None, wasm.to_vec()).await?;
    pic.install_chunked_canister(id, None, mode, id, vec![hash.clone()], hash, args(words))
        .await
}
#[tokio::test]
#[ignore = "requires prepared local PocketIC and fresh public runtime artifacts"]
async fn installed_public_runtime_without_copied_allocator_or_startup() {
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
    let a = artifact("a");
    let b = artifact("b");
    let id = pic.create_canister().await;
    pic.install_canister(id, a.clone(), vec![], None).await;
    for key in 0..17 {
        update(&pic, id, "put", [key, 77, 0]).await;
    }
    let expected = query(&pic, id, "records").await;
    for fault in 1..=5 {
        assert!(
            pic.update_call(id, id, "put", args([0, 99, fault]))
                .await
                .is_err()
        );
        assert_eq!(query(&pic, id, "records").await, expected);
    }
    pic.query_call(id, id, "query_put", args([0, 99, 0]))
        .await
        .unwrap();
    assert_eq!(query(&pic, id, "records").await, expected);
    for _ in 0..4 {
        update(&pic, id, "cycle", [0, 0, 0]).await;
        assert_eq!(query(&pic, id, "records").await, expected);
    }
    // Ordinary heap owners survive messages; compatible startup intentionally discards them.
    update(&pic, id, "heap", [12 * 1024 * 1024, 99, 0]).await;
    assert_eq!(
        u64::from_le_bytes(query(&pic, id, "heap_len").await.try_into().unwrap()),
        12 * 1024 * 1024
    );
    assert_eq!(query(&pic, id, "records").await, expected);
    let grown_pages = pages(&pic, id).await;
    let hash = pic.canister_status(id, None).await.unwrap().module_hash;
    assert!(
        pic.upgrade_eop_canister(id, a.clone(), args([0, 1, 0]), None)
            .await
            .is_err()
    );
    assert_eq!(
        pic.canister_status(id, None).await.unwrap().module_hash,
        hash
    );
    assert_eq!(query(&pic, id, "records").await, expected);
    pic.upgrade_eop_canister(id, a.clone(), args([0, 0, 0]), None)
        .await
        .unwrap();
    assert_eq!(query(&pic, id, "records").await, expected);
    assert_eq!(
        u64::from_le_bytes(query(&pic, id, "heap_len").await.try_into().unwrap()),
        0
    );
    // Rust's ordinary allocator is recreated. Retained Wasm pages cannot shrink;
    // newly grown ordinary heap can increase the high-water mark each generation.
    update(&pic, id, "heap", [12 * 1024 * 1024, 99, 0]).await;
    let next_pages = pages(&pic, id).await;
    assert!(next_pages >= grown_pages);
    println!("ordinary heap pages before/after compatible generation: {grown_pages}/{next_pages}");
    assert!(
        pic.upgrade_eop_canister(id, b.clone(), args([0, 0, 0]), None)
            .await
            .is_err()
    );
    assert_eq!(query(&pic, id, "records").await, expected);
    update(&pic, id, "checkpoint", [0, 0, 0]).await;
    assert!(replace(&pic, id, &b, [1, 1, 0]).await.is_err());
    assert_eq!(query(&pic, id, "records").await, expected);
    replace(&pic, id, &b, [1, 0, 0]).await.unwrap();
    assert_eq!(query(&pic, id, "records").await, expected);
    let recovered_pages = pages(&pic, id).await;
    assert!(recovered_pages < next_pages);
    println!("ordinary heap pages after portable replacement: {recovered_pages}");
    update(&pic, id, "put", [0, 88, 0]).await;
    let next = query(&pic, id, "records").await;
    update(&pic, id, "checkpoint", [0, 0, 0]).await;
    replace(&pic, id, &a, [1, 0, 0]).await.unwrap();
    assert_eq!(query(&pic, id, "records").await, next);
    for variant in ["no-close", "fail-close"] {
        let id = pic.create_canister().await;
        pic.install_canister(id, artifact(variant), vec![], None)
            .await;
        update(&pic, id, "put", [0, 42, 0]).await;
        let original = query(&pic, id, "records").await;
        assert!(
            pic.upgrade_eop_canister(id, a.clone(), args([0, 0, 0]), None)
                .await
                .is_err()
        );
        assert_eq!(query(&pic, id, "records").await, original);
        update(&pic, id, "put", [1, 43, 0]).await;
    }
    fs::write(directory().join("INSTALLED"),"public four-root runtime; no copied kernel; stale grants; ordinary heap lifetime/growth; compatible startup; missing source close; failed target/source rollback; both-domain portable recovery\n").unwrap();
}

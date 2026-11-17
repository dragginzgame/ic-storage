mod family;
mod paged;

use super::{candidate, directory, sha256};
use ic_testkit::pocket_ic::{
    CanisterId, CanisterInstallMode, ErrorCode, PocketIcBuilder, nonblocking::PocketIc,
};
use std::{collections::BTreeMap, fmt::Write, fs};

fn args(words: [u64; 3]) -> Vec<u8> {
    words.into_iter().flat_map(u64::to_le_bytes).collect()
}
fn word(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap())
}
fn same_native_payload(before: &[u8], after: &[u8]) {
    // Closing publishes formerly stale descriptors. All initialized native
    // window bytes and interpretation/freeze policy must remain identical.
    assert_eq!(&before[360..], &after[360..]);
    for index in [0, 1, 8, 9, 13] {
        assert_eq!(word(before, index * 8), word(after, index * 8));
    }
}
async fn server() -> PocketIc {
    PocketIcBuilder::new()
        .with_application_subnet()
        .with_max_request_time_ms(Some(30_000))
        .with_server_url(
            std::env::var("IC_TESTKIT_POCKET_IC_URL")
                .unwrap()
                .parse()
                .unwrap(),
        )
        .build_async()
        .await
}
async fn install(pic: &PocketIc, bytes: &[u8]) -> CanisterId {
    let id = pic.create_canister().await;
    pic.install_canister(id, bytes.to_vec(), vec![], None).await;
    id
}
async fn query(pic: &PocketIc, id: CanisterId, name: &str) -> Vec<u8> {
    pic.query_call(id, id, name, vec![]).await.unwrap()
}
async fn update(pic: &PocketIc, id: CanisterId, name: &str, words: [u64; 3]) {
    pic.update_call(id, id, name, args(words)).await.unwrap();
}

fn logical(bytes: &[u8]) -> BTreeMap<Vec<u8>, Vec<u8>> {
    assert_eq!(&bytes[..8], b"ORDMAP01");
    assert_eq!(word(bytes, 8), 0);
    let end = bytes.len() - 32;
    assert_eq!(sha256(&bytes[..end]), bytes[end..]);
    let mut offset = 24;
    let mut entries = BTreeMap::new();
    let mut prior = None;
    for _ in 0..word(bytes, 16) {
        let k = u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap()) as usize;
        let v = u32::from_le_bytes(bytes[offset + 4..offset + 8].try_into().unwrap()) as usize;
        offset += 8;
        let key = bytes[offset..offset + k].to_vec();
        offset += k;
        let value = bytes[offset..offset + v].to_vec();
        offset += v;
        assert!(prior.as_ref().is_none_or(|prior: &Vec<u8>| prior < &key));
        prior = Some(key.clone());
        assert!(entries.insert(key, value).is_none());
    }
    assert_eq!(offset, end);
    entries
}
async fn verify(pic: &PocketIc, id: CanisterId, expected: &BTreeMap<u64, u64>) {
    let session = query(pic, id, "session_status").await;
    let actual = logical(&query(pic, id, "records").await);
    assert_eq!(actual.len(), expected.len() * 2);
    for (&id, &value) in expected {
        let mut primary = vec![1];
        primary.extend_from_slice(&id.to_be_bytes());
        let row = actual.get(&primary).unwrap();
        assert_eq!(row.len(), 4096);
        assert_eq!(word(row, 0), value);
        assert!(row[8..].iter().all(|byte| *byte == (id ^ value) as u8));
        let mut index = vec![2, (value % 8) as u8];
        index.extend_from_slice(&id.to_be_bytes());
        assert_eq!(actual.get(&index), Some(&primary));
    }
    for reverse in [0, 1] {
        for group in 0..8 {
            let mut wanted: Vec<_> = expected
                .iter()
                .filter(|(_, v)| **v % 8 == group)
                .map(|(&k, &v)| (k, v))
                .collect();
            if reverse == 1 {
                wanted.reverse();
            }
            let mut found = Vec::new();
            let mut cursor = None;
            loop {
                let page = pic
                    .query_call(
                        id,
                        id,
                        "page",
                        args([
                            group,
                            cursor.unwrap_or(0),
                            reverse | if cursor.is_some() { 2 } else { 0 },
                        ]),
                    )
                    .await
                    .unwrap();
                assert_eq!(page.len() % 16, 0);
                assert!(page.len() <= 5 * 16);
                if page.is_empty() {
                    break;
                }
                for entry in page.as_chunks::<16>().0 {
                    found.push((word(entry, 0), word(entry, 8)));
                }
                cursor = Some(found.last().unwrap().0);
                assert!(found.len() <= expected.len());
            }
            assert_eq!(found, wanted);
        }
    }
    assert_eq!(query(pic, id, "session_status").await, session);
}

#[tokio::test]
#[ignore = "requires prepared server and freshly admitted compiler artifacts"]
async fn installed_session_owns_both_allocations_until_explicit_close() {
    let a = candidate("a");
    let b = candidate("b");
    let pic = server().await;
    let id = install(&pic, &a).await;
    let mut expected = BTreeMap::new();
    for key in 0..32 {
        update(&pic, id, "put", [key, key * 9, 0]).await;
        expected.insert(key, key * 9);
    }
    let status = query(&pic, id, "session_status").await;
    let metadata = word(&status, 40);
    assert!((1..=65_536).contains(&metadata));
    println!("public ordered session metadata_bytes={metadata} reservation_bytes=65536");
    assert_eq!(
        (word(&status, 0), word(&status, 8), word(&status, 16)),
        (2, 1, 32)
    );
    verify(&pic, id, &expected).await;
    let before = query(&pic, id, "observe").await;
    pic.query_call(id, id, "query_put", args([7, 999, 0]))
        .await
        .unwrap();
    assert_eq!(query(&pic, id, "observe").await, before);
    assert_eq!(query(&pic, id, "session_status").await, status);
    verify(&pic, id, &expected).await;
    refused(&pic, id, "session_close", [1, 0, 0]).await;
    update(&pic, id, "session_close", [0, 0, 0]).await;
    assert_eq!(word(&query(&pic, id, "session_status").await, 0), 1);
    assert_eq!(word(&query(&pic, id, "session_status").await, 40), 0);
    refused(&pic, id, "session_close", [0, 0, 0]).await;
    refused(&pic, id, "put", [7, 77, 0]).await;
    assert!(pic.query_call(id, id, "records", vec![]).await.is_err());
    refused(&pic, id, "lookup", [7, 63, 0]).await;
    // Real allocation refusal during public owner construction must roll back
    // native loan issuance and metadata selector/ledger state as one IC message.
    refused(&pic, id, "session_open", [1, 0, 0]).await;
    update(&pic, id, "session_open", [0, 0, 0]).await;
    let status = query(&pic, id, "session_status").await;
    assert_eq!(
        (word(&status, 0), word(&status, 8), word(&status, 16)),
        (2, 2, 32)
    );
    refused(&pic, id, "session_open", [0, 0, 0]).await;
    same_native_payload(&before, &query(&pic, id, "observe").await);
    verify(&pic, id, &expected).await;
    for _ in 0..8 {
        assert_eq!(word(&query(&pic, id, "session_status").await, 40), metadata);
        refused(&pic, id, "put", [7, 999, 4]).await;
        update(&pic, id, "session_close", [0, 0, 0]).await;
        assert_eq!(word(&query(&pic, id, "session_status").await, 40), 0);
        update(&pic, id, "session_open", [0, 0, 0]).await;
        // Repeated scratch-heavy query exports cannot overwrite live metadata.
        verify(&pic, id, &expected).await;
    }
    pic.upgrade_eop_canister(id, b.clone(), args([0, 0, 0]), None)
        .await
        .unwrap();
    let status = query(&pic, id, "session_status").await;
    assert_eq!(
        (word(&status, 0), word(&status, 8), word(&status, 16)),
        (2, 1, 1)
    );
    verify(&pic, id, &expected).await;
    // Failed source close and omitted close must preserve the old service,
    // including its allocator ledgers and working owners, not just record bytes.
    for variant in ["no-close", "pre-fail"] {
        let id = install(&pic, &candidate(variant)).await;
        update(&pic, id, "put", [1, 11, 0]).await;
        let native = query(&pic, id, "observe").await;
        let status = query(&pic, id, "session_status").await;
        let hash = pic.canister_status(id, None).await.unwrap().module_hash;
        trapped(
            pic.upgrade_eop_canister(id, b.clone(), args([0, 0, 0]), None)
                .await
                .unwrap_err()
                .error_code,
        );
        assert_eq!(query(&pic, id, "observe").await, native);
        assert_eq!(query(&pic, id, "session_status").await, status);
        assert_eq!(
            pic.canister_status(id, None).await.unwrap().module_hash,
            hash
        );
        update(&pic, id, "put", [2, 22, 0]).await;
        verify(&pic, id, &BTreeMap::from([(1, 11), (2, 22)])).await;
        if variant == "no-close" {
            update(&pic, id, "session_close", [0, 0, 0]).await;
            pic.upgrade_eop_canister(id, b.clone(), args([0, 0, 0]), None)
                .await
                .unwrap();
            verify(&pic, id, &BTreeMap::from([(1, 11), (2, 22)])).await;
        }
    }
    fs::write(directory().join("SESSION"), "public grants; generation metadata survives scratch reuse; allocation refusal; repeated dual close/reopen; late reply rollback; query isolation; startup reset and source rollback\n").unwrap();
}

fn trapped(code: ErrorCode) {
    assert!(matches!(
        code,
        ErrorCode::CanisterTrapped | ErrorCode::CanisterCalledTrap
    ));
}
async fn refused(pic: &PocketIc, id: CanisterId, method: &str, words: [u64; 3]) {
    let native = query(pic, id, "observe").await;
    let session = query(pic, id, "session_status").await;
    let stable = pic.get_stable_memory(id).await;
    trapped(
        pic.update_call(id, id, method, args(words))
            .await
            .unwrap_err()
            .error_code,
    );
    assert_eq!(query(pic, id, "observe").await, native);
    assert_eq!(query(pic, id, "session_status").await, session);
    assert_eq!(pic.get_stable_memory(id).await, stable);
}
async fn replace(
    pic: &PocketIc,
    id: CanisterId,
    wasm: &[u8],
    words: [u64; 3],
    skip: bool,
) -> Result<(), ic_testkit::pocket_ic::RejectResponse> {
    let mut mode = CanisterInstallMode::Upgrade(Some(Default::default()));
    let CanisterInstallMode::Upgrade(Some(flags)) = &mut mode else {
        unreachable!()
    };
    flags.wasm_memory_persistence = Some(Default::default());
    flags.skip_pre_upgrade = Some(skip);
    assert_eq!(
        format!("{:?}", flags.wasm_memory_persistence),
        "Some(Replace)"
    );
    pic.clear_chunk_store(id, None).await?;
    let hash = pic.upload_chunk(id, None, wasm.to_vec()).await?;
    assert_eq!(hash, sha256(wasm));
    pic.install_chunked_canister(id, None, mode, id, vec![hash.clone()], hash, args(words))
        .await
}
async fn refused_replacement(
    pic: &PocketIc,
    id: CanisterId,
    wasm: &[u8],
    words: [u64; 3],
    skip: bool,
) {
    let native = query(pic, id, "observe").await;
    let session = query(pic, id, "session_status").await;
    let stable = pic.get_stable_memory(id).await;
    pic.stop_canister(id, None).await.unwrap();
    let hash = pic.canister_status(id, None).await.unwrap().module_hash;
    trapped(
        replace(pic, id, wasm, words, skip)
            .await
            .unwrap_err()
            .error_code,
    );
    assert_eq!(
        pic.canister_status(id, None).await.unwrap().module_hash,
        hash
    );
    pic.start_canister(id, None).await.unwrap();
    assert_eq!(query(pic, id, "observe").await, native);
    assert_eq!(query(pic, id, "session_status").await, session);
    assert_eq!(pic.get_stable_memory(id).await, stable);
}
async fn cost(pic: &PocketIc, id: CanisterId, operation: &str, report: &mut String) {
    let bytes = query(pic, id, "costs").await;
    assert_eq!(bytes.len(), 64);
    let values: [u64; 8] = std::array::from_fn(|i| word(&bytes, i * 8));
    assert!(
        values[1] > 0 && values[1] < 50_000_000,
        "{operation} {values:?}"
    );
    assert!(values[2] <= 1024 * 1024);
    if operation.starts_with("lookup_") {
        assert_eq!(values[2], 0);
    }
    assert_eq!(values[3], 49);
    assert_eq!(values[4], 8 * 1024 * 1024);
    assert_eq!(values[5], 1_589_248);
    assert!(values[6] <= 280_000);
    assert!(values[7] <= 550_000);
    writeln!(
        report,
        "{operation}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
        values[1],
        values[2],
        values[3] * 65536,
        values[4],
        values[5],
        values[6],
        values[7]
    )
    .unwrap();
    fs::write(directory().join("costs.tsv"), report).unwrap();
}

#[tokio::test]
#[ignore = "requires prepared server and freshly admitted compiler artifacts"]
async fn installed_ordered_lifecycle_and_row_index_atomicity() {
    let a = candidate("a");
    let b = candidate("b");
    let recovery = candidate("recovery");
    let pic = server().await;
    let id = install(&pic, &a).await;
    let mut expected = BTreeMap::new();
    let mut report =
        "operation\tmeasured_body_instructions\ttransient_bytes\tstable_bytes\twasm_bytes\tnative_window_bytes\tinitialized_native_bytes\treserved_native_bytes\n"
            .to_owned();
    for key in 0..64 {
        update(&pic, id, "put", [key, key * 8, 0]).await;
        expected.insert(key, key * 8);
    }
    cost(&pic, id, "put_at_64_rows", &mut report).await;
    for key in [0, 31, 63] {
        let result = pic
            .update_call(id, id, "lookup", args([key, key * 8, 0]))
            .await
            .unwrap();
        assert_eq!(word(&result, 0), key * 8);
        cost(&pic, id, &format!("lookup_{key}_at_64_rows"), &mut report).await;
    }
    assert_eq!(word(&query(&pic, id, "session_status").await, 8), 1);
    verify(&pic, id, &expected).await;
    refused(&pic, id, "put", [64, 512, 0]).await; // 128 physical entries is the admitted live limit.
    for fault in 1..=3 {
        refused(&pic, id, "put", [7, 71, fault]).await;
    }
    refused(&pic, id, "delete", [7, 0, 1]).await;
    let native = query(&pic, id, "observe").await;
    pic.upgrade_eop_canister(id, b.clone(), args([0, 0, 0]), None)
        .await
        .unwrap();
    same_native_payload(&native, &query(&pic, id, "observe").await);
    assert_eq!(word(&query(&pic, id, "session_status").await, 8), 1);
    let native = query(&pic, id, "observe").await;
    verify(&pic, id, &expected).await;
    // Failed post-upgrade descriptor publication must retain code, nodes and data.
    let stable = pic.get_stable_memory(id).await;
    let hash = pic.canister_status(id, None).await.unwrap().module_hash;
    trapped(
        pic.upgrade_eop_canister(id, a.clone(), args([0, 0, 3]), None)
            .await
            .unwrap_err()
            .error_code,
    );
    assert_eq!(query(&pic, id, "observe").await, native);
    assert_eq!(pic.get_stable_memory(id).await, stable);
    update(&pic, id, "delete", [7, 0, 0]).await;
    assert_eq!(
        pic.canister_status(id, None).await.unwrap().module_hash,
        hash
    );
    expected.remove(&7);
    update(&pic, id, "put", [u64::MAX, 800, 0]).await;
    expected.insert(u64::MAX, 800);
    // Resuming after a deleted cursor sees the new key beyond it, never old keys.
    let page = pic
        .query_call(id, id, "page", args([0, 7, 2]))
        .await
        .unwrap();
    assert_eq!(word(&page, 0), 8);
    update(&pic, id, "put", [9, 73, 0]).await;
    expected.insert(9, 73);
    verify(&pic, id, &expected).await;
    for fault in 1..=3 {
        refused(&pic, id, "checkpoint", [1, 0, fault]).await;
    }
    update(&pic, id, "checkpoint", [1, 0, 0]).await;
    cost(&pic, id, "checkpoint_64_rows", &mut report).await;
    refused(&pic, id, "put", [1, 99, 0]).await;
    pic.upgrade_eop_canister(id, a.clone(), args([0, 0, 0]), None)
        .await
        .unwrap();
    // A retained source cannot be silently enrolled as a fresh recovery target.
    let native = query(&pic, id, "observe").await;
    let stable = pic.get_stable_memory(id).await;
    trapped(
        pic.upgrade_eop_canister(id, recovery.clone(), args([1, 1, 0]), None)
            .await
            .unwrap_err()
            .error_code,
    );
    assert_eq!(query(&pic, id, "observe").await, native);
    assert_eq!(pic.get_stable_memory(id).await, stable);
    refused_replacement(&pic, id, &recovery, [0, 0, 0], false).await;
    for fault in 1..=3 {
        refused_replacement(&pic, id, &recovery, [1, 1, fault], false).await;
    }
    replace(&pic, id, &recovery, [1, 1, 0], false)
        .await
        .unwrap();
    cost(&pic, id, "replace_recovery_64_rows", &mut report).await;
    assert_eq!(word(&query(&pic, id, "observe").await, 8), 6_291_456);
    verify(&pic, id, &expected).await;
    update(&pic, id, "put", [9, 81, 0]).await;
    expected.insert(9, 81);
    update(&pic, id, "delete", [10, 0, 0]).await;
    expected.remove(&10);
    update(&pic, id, "checkpoint", [2, 0, 0]).await;
    cost(&pic, id, "checkpoint_epoch_2_63_rows", &mut report).await;
    replace(&pic, id, &b, [1, 2, 0], false).await.unwrap();
    cost(&pic, id, "reverse_recovery_63_rows", &mut report).await;
    assert_eq!(word(&query(&pic, id, "observe").await, 8), 4_194_304);
    verify(&pic, id, &expected).await;
    fs::write(
        directory().join("LIFECYCLE"),
        "compatible upgrades, two-domain recovery, indexes and full native/stable rollback\n",
    )
    .unwrap();
}

#[tokio::test]
#[ignore = "requires prepared server and freshly admitted compiler artifacts"]
async fn installed_unknown_native_and_checkpoint_bytes_never_reset() {
    let a = candidate("a");
    let recovery = candidate("recovery");
    let pic = server().await;
    let id = install(&pic, &a).await;
    refused_replacement(&pic, id, &recovery, [1, 1, 0], false).await;
    update(&pic, id, "put", [1, u64::MAX, 0]).await;
    update(&pic, id, "checkpoint", [1, 0, 0]).await;
    refused_replacement(&pic, id, &recovery, [1, 2, 0], false).await;
    update(&pic, id, "damage", [0, 0, 0]).await;
    refused(&pic, id, "put", [2, 3, 0]).await;
    refused_replacement(&pic, id, &recovery, [1, 1, 0], false).await;
    // Explicit operator recovery bypasses the corrupt source hook only after a seal exists.
    replace(&pic, id, &recovery, [1, 1, 0], true).await.unwrap();
    verify(&pic, id, &BTreeMap::from([(1, u64::MAX)])).await;
    update(&pic, id, "checkpoint", [2, 0, 0]).await;
    update(&pic, id, "damage", [1, 0, 0]).await;
    refused_replacement(&pic, id, &a, [1, 2, 0], false).await;
    let stable = pic.get_stable_memory(id).await;
    assert!(!stable.is_empty());
    for kind in [2, 3, 4, 5, 6, 9] {
        let id = install(&pic, &a).await;
        update(&pic, id, "put", [1, 11, 0]).await;
        update(&pic, id, "checkpoint", [1, 0, 0]).await;
        update(&pic, id, "damage", [kind, 0, 0]).await;
        refused(&pic, id, "put", [2, 22, 0]).await;
        refused_replacement(&pic, id, &recovery, [1, 1, 0], false).await;
        replace(&pic, id, &recovery, [1, 1, 0], true).await.unwrap();
        verify(&pic, id, &BTreeMap::from([(1, 11)])).await;
    }
    fs::write(
        directory().join("REFUSALS"),
        "missing/corrupt native and checkpoint state refused without reset\n",
    )
    .unwrap();
}

use super::*;

type Records = BTreeMap<Vec<u8>, Vec<u8>>;

fn decode(bytes: &[u8]) -> [Records; 4] {
    assert_eq!(&bytes[..8], b"ORDFAM04");
    assert_eq!(word(bytes, 8), 0);
    let end = bytes.len() - 32;
    assert_eq!(sha256(&bytes[..end]), bytes[end..]);
    let mut at = 240;
    let result = std::array::from_fn(|index| {
        let descriptor = &bytes[16 + index * 56..16 + (index + 1) * 56];
        let declaration = [
            index as u64 + 1,
            index as u64 + 1,
            1,
            [128, 32, 32, 17][index],
            [512, 512, 512, 1][index],
            [4096, 4096, 4096, 16384][index],
        ];
        for (field, expected) in declaration.into_iter().enumerate() {
            assert_eq!(word(descriptor, field * 8), expected);
        }
        let length = usize::try_from(word(descriptor, 48)).unwrap();
        let result = logical(&bytes[at..at + length]);
        at += length;
        result
    });
    assert_eq!(at, end);
    result
}

fn key(slot: u8) -> Vec<u8> {
    let mut key = vec![0xa5; 512];
    key[..8].copy_from_slice(&u64::from(slot).to_be_bytes());
    key
}
fn expected(value: u8, active_jobs: bool) -> [Records; 4] {
    std::array::from_fn(|root| {
        (0..17)
            .filter_map(|slot| {
                if root >= 2 && !active_jobs {
                    return None;
                }
                let key = if root == 3 { vec![slot] } else { key(slot) };
                let byte = if root == 1 { slot } else { value };
                Some((key, vec![byte; if root == 3 { 16384 } else { 4096 }]))
            })
            .collect()
    })
}
async fn verify_family(pic: &PocketIc, id: CanisterId, expected: &[Records; 4]) {
    let status = query(pic, id, "session_status").await;
    assert_eq!(&decode(&query(pic, id, "family_records").await), expected);
    assert_eq!(query(pic, id, "session_status").await, status);
}
async fn measure(pic: &PocketIc, id: CanisterId, operation: &str, report: &mut String) {
    let bytes = query(pic, id, "costs").await;
    let values: [u64; 8] = std::array::from_fn(|i| word(&bytes, i * 8));
    assert!(
        values[1] > 0 && values[1] < 100_000_000,
        "{operation}: {values:?}"
    );
    assert!(values[2] < 2 * 1024 * 1024);
    assert_eq!(values[5], 1_589_248);
    assert!(values[6] <= values[7] && values[7] <= values[5]);
    writeln!(
        report,
        "{operation}\t{}\t{}\t{}\t{}",
        values[1], values[2], values[6], values[7]
    )
    .unwrap();
    fs::write(directory().join("family-costs.tsv"), report).unwrap();
}

#[tokio::test]
#[ignore = "requires prepared server and freshly admitted compiler artifacts"]
async fn installed_four_root_grants_recovery_and_job_reuse() {
    let a = candidate("a");
    let b = candidate("b");
    let recovery = candidate("recovery");
    let pic = server().await;
    let id = install(&pic, &a).await;
    let mut report = "operation\tinstructions\ttransient_bytes\tinitialized_native_bytes\treserved_native_bytes\n".to_owned();
    for slot in 0..17 {
        update(&pic, id, "family_write", [slot, 77, 0]).await;
    }
    measure(&pic, id, "write_four_roots_at_17_jobs", &mut report).await;
    let first = expected(77, true);
    verify_family(&pic, id, &first).await;
    let metadata = word(&query(&pic, id, "session_status").await, 40);
    println!("four-root public session metadata_bytes={metadata}");
    for fault in 1..=6 {
        refused(&pic, id, "family_write", [0, 99, fault]).await;
    }
    // The eighteenth job must refuse after earlier roots changed in this message.
    refused(&pic, id, "family_write", [17, 99, 0]).await;
    refused(&pic, id, "family_cleanup", [0, 0, 1]).await;
    let native = query(&pic, id, "observe").await;
    let status = query(&pic, id, "session_status").await;
    pic.query_call(id, id, "query_family_write", args([0, 99, 0]))
        .await
        .unwrap();
    assert_eq!(query(&pic, id, "observe").await, native);
    assert_eq!(query(&pic, id, "session_status").await, status);
    verify_family(&pic, id, &first).await;
    refused(&pic, id, "session_close", [1, 0, 0]).await;
    for _ in 0..3 {
        update(&pic, id, "session_close", [0, 0, 0]).await;
        assert_eq!(word(&query(&pic, id, "session_status").await, 40), 0);
        refused(&pic, id, "session_open", [1, 0, 0]).await;
        update(&pic, id, "session_open", [0, 0, 0]).await;
        assert_eq!(word(&query(&pic, id, "session_status").await, 40), metadata);
        verify_family(&pic, id, &first).await;
    }
    pic.upgrade_eop_canister(id, b.clone(), args([0, 0, 0]), None)
        .await
        .unwrap();
    same_native_payload(&native, &query(&pic, id, "observe").await);
    verify_family(&pic, id, &first).await;
    for fault in 1..=3 {
        refused(&pic, id, "checkpoint", [1, 0, fault]).await;
    }
    update(&pic, id, "checkpoint", [1, 0, 0]).await;
    measure(&pic, id, "checkpoint_four_roots_17_jobs", &mut report).await;
    refused(&pic, id, "family_write", [0, 99, 0]).await;
    refused(&pic, id, "family_cleanup", [0, 0, 0]).await;
    for fault in 1..=3 {
        refused_replacement(&pic, id, &recovery, [1, 1, fault], false).await;
    }
    replace(&pic, id, &recovery, [1, 1, 0], false)
        .await
        .unwrap();
    assert_eq!(word(&query(&pic, id, "observe").await, 8), 6_291_456);
    measure(&pic, id, "recover_four_roots_17_jobs", &mut report).await;
    verify_family(&pic, id, &first).await;
    // Each cleanup is bounded to one target/job pair. Source and main survive.
    for slot in 0..17 {
        update(&pic, id, "family_cleanup", [slot, 0, 0]).await;
    }
    let empty_jobs = expected(77, false);
    verify_family(&pic, id, &empty_jobs).await;
    update(&pic, id, "checkpoint", [2, 0, 0]).await;
    replace(&pic, id, &a, [1, 2, 0], false).await.unwrap();
    assert_eq!(word(&query(&pic, id, "observe").await, 8), 4_194_304);
    verify_family(&pic, id, &empty_jobs).await;
    // A new cycle uses all seventeen slots again without changing source rows.
    for slot in 0..17 {
        update(&pic, id, "family_write", [slot, 88, 0]).await;
    }
    let second = expected(88, true);
    verify_family(&pic, id, &second).await;
    update(&pic, id, "session_close", [0, 0, 0]).await;
    update(&pic, id, "session_open", [0, 0, 0]).await;
    verify_family(&pic, id, &second).await;
    for kind in [7, 8] {
        let id = install(&pic, &a).await;
        update(&pic, id, "family_write", [0, 66, 0]).await;
        let original = query(&pic, id, "family_records").await;
        update(&pic, id, "checkpoint", [1, 0, 0]).await;
        update(&pic, id, "damage", [kind, 0, 0]).await;
        // A valid outer seal cannot authorize changed declarations or a corrupt
        // final map. Refusal restores the source after partial target recovery.
        refused_replacement(&pic, id, &recovery, [1, 1, 0], false).await;
        update(&pic, id, "session_open", [0, 0, 0]).await;
        assert_eq!(query(&pic, id, "family_records").await, original);
    }
    fs::write(directory().join("FAMILY"), "four independently granted roots; 512-byte keys; 4KiB rows; 17x16KiB jobs; composition and late-reply rollback; query isolation; close/reopen; compatible upgrade; complete recovery both domains; bounded cleanup and job reuse\n").unwrap();
}

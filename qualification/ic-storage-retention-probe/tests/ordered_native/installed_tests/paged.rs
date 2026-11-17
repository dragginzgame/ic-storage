use super::*;

fn progress(job: &[u8]) -> u32 {
    u32::from_le_bytes(job[44..48].try_into().unwrap())
}
fn pages(job: &[u8]) -> u32 {
    assert_eq!(&job[..8], b"ICJOF001");
    assert_eq!(job.len(), 808);
    (0..4)
        .map(|root| word(job, 64 + root * 176 + 48 + 40) as u32)
        .sum()
}
async fn unavailable(pic: &PocketIc, id: CanisterId) {
    for query in ["records", "family_records"] {
        assert!(pic.query_call(id, id, query, vec![]).await.is_err());
    }
    assert!(
        pic.query_call(id, id, "page", args([0, 0, 0]))
            .await
            .is_err()
    );
    refused(pic, id, "lookup", [0, 77, 0]).await;
    refused(pic, id, "family_write", [0, 99, 0]).await;
    refused(pic, id, "checkpoint", [9, 0, 0]).await;
}
async fn metrics(
    pic: &PocketIc,
    id: CanisterId,
    label: &str,
    page_reads: u64,
    report: &mut String,
) {
    let bytes = query(pic, id, "job_metrics").await;
    let values: [u64; 5] = std::array::from_fn(|i| word(&bytes, i * 8));
    assert!(
        values[0] > 0 && values[0] < 50_000_000,
        "{label}: {values:?}"
    );
    assert!(values[1] <= 65_536, "{label}: {values:?}");
    assert!(values[2] <= 40_000, "{label}: {values:?}");
    assert!(values[3] <= 2048, "{label}: {values:?}");
    assert_eq!(values[4], page_reads);
    writeln!(
        report,
        "{label}\t{}\t{}\t{}\t{}\t{}",
        values[0], values[1], values[2], values[3], values[4]
    )
    .unwrap();
    fs::write(directory().join("paged-costs.tsv"), report).unwrap();
}

#[tokio::test]
#[ignore = "requires prepared server and freshly admitted compiler artifacts"]
async fn installed_paged_family_recovery_commits_one_page_with_progress() {
    let pic = server().await;
    let a = candidate("a");
    let b = candidate("b");
    let recovery = candidate("recovery");
    let id = install(&pic, &a).await;
    for slot in 0..17 {
        update(&pic, id, "family_write", [slot, 77, 0]).await;
    }
    let expected = query(&pic, id, "family_records").await;
    for fault in 1..=3 {
        refused(&pic, id, "job_seal", [1, 6_291_456, fault]).await;
    }
    update(&pic, id, "job_seal", [1, 6_291_456, 0]).await;
    update(&pic, id, "job_seal", [1, 6_291_456, 0]).await; // identical frozen export
    refused(&pic, id, "family_write", [0, 99, 0]).await;
    for fault in 1..=2 {
        refused(&pic, id, "job_cancel", [1, 6_291_456, fault]).await;
    }
    update(&pic, id, "job_cancel", [1, 6_291_456, 0]).await;
    update(&pic, id, "job_cancel", [1, 6_291_456, 0]).await;
    assert_eq!(query(&pic, id, "family_records").await, expected);
    refused_replacement(&pic, id, &recovery, [2, 1, 0], false).await;
    update(&pic, id, "job_seal", [2, 6_291_456, 0]).await;
    pic.upgrade_eop_canister(id, b.clone(), args([0, 0, 0]), None)
        .await
        .unwrap();
    for fault in 1..=4 {
        refused_replacement(&pic, id, &recovery, [2, 2, fault], false).await;
    }
    refused_replacement(&pic, id, &a, [2, 2, 0], false).await; // wrong target domain
    replace(&pic, id, &recovery, [2, 2, 0], false)
        .await
        .unwrap();
    unavailable(&pic, id).await;
    let total = pages(&query(&pic, id, "job_status").await);
    assert!(total > 4);
    let mut report =
        "call\tinstructions\tscratch_bytes\tstable_read_bytes\tstable_write_bytes\tpage_reads\n"
            .to_owned();
    for fault in 1..=4 {
        refused(&pic, id, "job_step", [2, 0, fault]).await;
    }
    refused(&pic, id, "job_step", [1, 0, 0]).await;
    refused(&pic, id, "job_step", [2, 1, 0]).await;
    let native = query(&pic, id, "observe").await;
    let stable = pic.get_stable_memory(id).await;
    let status = query(&pic, id, "session_status").await;
    pic.query_call(id, id, "query_job_step", args([2, 0, 0]))
        .await
        .unwrap();
    assert_eq!(query(&pic, id, "observe").await, native);
    assert_eq!(pic.get_stable_memory(id).await, stable);
    assert_eq!(query(&pic, id, "session_status").await, status);
    update(&pic, id, "job_step", [2, 0, 0]).await;
    metrics(&pic, id, "initial_page", 1, &mut report).await;
    update(&pic, id, "job_damage", [0, 0, 0]).await;
    // Reopen native custody and the verifier; consumed pages are never reread.
    pic.upgrade_eop_canister(id, recovery.clone(), args([0, 0, 0]), None)
        .await
        .unwrap();
    update(&pic, id, "job_step", [2, 1, 0]).await;
    metrics(&pic, id, "after_compatible_interrupt", 1, &mut report).await;
    for fault in 1..=4 {
        refused_replacement(&pic, id, &recovery, [3, 2, fault], false).await;
    }
    replace(&pic, id, &recovery, [3, 2, 0], false)
        .await
        .unwrap();
    assert_eq!(progress(&query(&pic, id, "job_status").await), 0);
    // Restart discarded the old candidate, so the corrupt first page now refuses.
    refused(&pic, id, "job_step", [2, 0, 0]).await;
    update(&pic, id, "job_damage", [0, 0, 0]).await;
    for index in 0..total {
        if index == total - 1 {
            for fault in 1..=4 {
                refused(&pic, id, "job_step", [2, u64::from(index), fault]).await;
            }
            unavailable(&pic, id).await;
        }
        update(&pic, id, "job_step", [2, u64::from(index), 0]).await;
        metrics(&pic, id, &format!("page_{index}"), 1, &mut report).await;
        assert_eq!(progress(&query(&pic, id, "job_status").await), index + 1);
        let native = query(&pic, id, "observe").await;
        let stable = pic.get_stable_memory(id).await;
        update(&pic, id, "job_step", [2, u64::from(index), 0]).await;
        metrics(&pic, id, &format!("retry_{index}"), 0, &mut report).await;
        assert_eq!(query(&pic, id, "observe").await, native);
        assert_eq!(pic.get_stable_memory(id).await, stable);
    }
    assert_eq!(query(&pic, id, "family_records").await, expected);
    // Completion cannot be restarted; a retry must preserve later native writes.
    update(&pic, id, "family_write", [0, 88, 0]).await;
    let newer = query(&pic, id, "family_records").await;
    update(&pic, id, "job_step", [2, u64::from(total - 1), 0]).await;
    assert_eq!(query(&pic, id, "family_records").await, newer);
    refused_replacement(&pic, id, &recovery, [3, 2, 0], false).await;
    refused(&pic, id, "job_step", [2, 0, 0]).await;
    update(&pic, id, "job_seal", [3, 4_194_304, 0]).await;
    replace(&pic, id, &a, [2, 3, 0], false).await.unwrap();
    for index in 0..pages(&query(&pic, id, "job_status").await) {
        update(&pic, id, "job_step", [3, u64::from(index), 0]).await;
    }
    assert_eq!(query(&pic, id, "family_records").await, newer);
    // Entirely empty families still require an explicit completion transaction.
    let empty = install(&pic, &a).await;
    let expected_empty = query(&pic, empty, "family_records").await;
    update(&pic, empty, "job_seal", [1, 6_291_456, 0]).await;
    replace(&pic, empty, &recovery, [2, 1, 0], false)
        .await
        .unwrap();
    unavailable(&pic, empty).await;
    update(&pic, empty, "job_step", [1, 0, 0]).await;
    metrics(&pic, empty, "empty_complete", 0, &mut report).await;
    update(&pic, empty, "job_step", [1, 0, 0]).await;
    assert_eq!(query(&pic, empty, "family_records").await, expected_empty);
    fs::write(directory().join("PAGED"), "one-page four-root recovery; atomic native/cursor/job progress; exact retries; query isolation; interruption without rereading consumed pages; explicit fresh restart; cancellation and sequential epochs; empty completion; bounded backing I/O\n").unwrap();
}

//! Normal callback errors commit earlier writes; forgetting a view loses no owner.
use super::*;
use ic_storage_retention_probe::artifact::sha256_hex;

#[tokio::test(flavor = "current_thread")]
#[ignore = "make native-provider-test supplies fresh artifacts and the prepared server"]
async fn callback_errors_and_forgotten_views_finalize_custody_across_upgrades() {
    let (directory, _) = super::super::super::input();
    let raw = fs::read(directory.join("vector/provider_vector.wasm")).unwrap();
    let candidate = |profile| {
        let bytes = prepare(&raw, profile).unwrap();
        admit_final(&bytes, &raw, profile).unwrap();
        let digest = sha256_hex(&bytes);
        fs::write(
            directory.join("vector").join(format!("{digest}.wasm")),
            &bytes,
        )
        .unwrap();
        println!(
            "native scope {profile:?}: sha256={digest} bytes={}",
            bytes.len()
        );
        bytes
    };
    let server = std::env::var("IC_TESTKIT_POCKET_IC_URL").expect(
        "run with the prepared server via make retention-setup and the selected test target",
    );
    let instance = PocketIcBuilder::new()
        .with_application_subnet()
        .with_max_request_time_ms(Some(30_000))
        .with_server_url(server.parse().expect("valid prepared server URL"))
        .build_async()
        .await;
    let pic = &instance;
    let a = candidate(Profile::A);
    let b = candidate(Profile::B);
    for root in [(0, 0), (0, 8), (3, 3), (3, 4), (512, 512)] {
        let id = install(pic, candidate(Profile { root, ..Profile::A })).await;
        // Include the original empty/spare roots before successful operations
        // change their length. Native release has completed at this failpoint.
        rejected_update(pic, id, "vector_error_finalize_trap").await;
        // Forgetting an empty view must leave even spare allocation custody intact.
        let before = observe(pic, id).await;
        update(pic, id, "vector_forget").await;
        let mut expected = before.payload.clone();
        if root.0 != 0 {
            let first = before.values()[0].wrapping_add(1);
            expected[..8].copy_from_slice(&first.to_le_bytes());
        }
        let forgotten = observe(pic, id).await;
        forgotten.settled();
        assert_eq!(forgotten.header, before.header);
        assert_eq!(forgotten.payload, expected);

        for (version, target) in [(1, &a), (2, &b), (1, &a)] {
            upgrade(pic, id, target, version).await;
            let before = observe(pic, id).await;
            let mut expected = before.payload.clone();
            if before.root().0 != 0 {
                let first = before.values()[0].wrapping_add(version);
                expected[..8].copy_from_slice(&first.to_le_bytes());
            }
            let length = before.root().0;
            let end = (length + 1).min(512);
            if length < 512 {
                expected[length as usize * 8..end as usize * 8]
                    .copy_from_slice(&version.to_le_bytes());
            }
            let response = pic
                .update_call(id, id, "vector_business_error", vec![])
                .await
                .unwrap();
            assert_eq!(response, 1u32.to_le_bytes());
            let after = observe(pic, id).await;
            after.settled();
            assert_eq!(after.root(), (end, before.root().1.max(end)));
            assert_eq!(after.payload, expected);
            // A subsequent trap undoes this entire call, including a settled
            // ordinary error. Earlier successful calls must remain committed.
            rejected_update(pic, id, "vector_error_trap").await;
            // A business Err cannot turn failed finalization into a successful
            // call. Compare the complete header and payload before reopening.
            rejected_update(pic, id, "vector_error_finalize_trap").await;
            update(pic, id, "vector_forget").await;
            let forgotten = observe(pic, id).await;
            forgotten.settled();
            assert_eq!(forgotten.header, after.header);
            let first = after.values()[0].wrapping_add(version);
            expected[..8].copy_from_slice(&first.to_le_bytes());
            assert_eq!(forgotten.payload, expected);
        }
        update(pic, id, "vector_clear").await;
        update(pic, id, "vector_reuse").await;
        let reused = observe(pic, id).await;
        reused.settled();
        assert_eq!(reused.root(), (1, 1));
        assert_eq!(reused.values(), [1]);
        upgrade(pic, id, &b, 2).await;
    }
    instance.drop().await;
}

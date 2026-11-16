//! Normal callback errors commit earlier writes; forgetting a view loses no owner.
use super::*;

#[tokio::test(flavor = "current_thread")]
#[ignore = "make native-provider-test supplies fresh artifacts and the prepared server"]
async fn callback_errors_and_forgotten_views_finalize_custody_across_upgrades() {
    let (directory, _) = super::super::super::input();
    let raw = fs::read(directory.join("vector/provider_vector.wasm")).unwrap();
    let candidate = |profile| {
        let bytes = prepare(&raw, profile).unwrap();
        admit_final(&bytes, &raw, profile).unwrap();
        let digest = Sha256Digest::compute(&bytes);
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
    let owner = PocketIcStartupConfig::from_env(Duration::from_secs(30))
        .unwrap()
        .build_instance(
            PocketIcBuilder::new()
                .with_application_subnet()
                .with_max_request_time_ms(Some(30_000)),
        )
        .await
        .unwrap();
    let pic = owner.instance();
    let a = candidate(Profile::A);
    let b = candidate(Profile::B);
    for root in [(0, 0), (0, 8), (3, 3), (3, 4), (512, 512)] {
        let id = install(pic, candidate(Profile { root, ..Profile::A })).await;
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
    owner.close().await;
}

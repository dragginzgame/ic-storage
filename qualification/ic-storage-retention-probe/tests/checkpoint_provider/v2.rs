//! Full retained-vector wire capacity through the existing governed provider.

use super::*;
use ic_storage_retention_probe::artifact::sha256;

async fn refused_update(pic: &PocketIc, id: CanisterId, method: &str, words: &[u64]) {
    let before = query(pic, id, "state").await;
    let stable = pic.get_stable_memory(id).await;
    let error = pic
        .update_call(id, id, method, encode(words))
        .await
        .unwrap_err();
    assert_eq!(error.error_code, ErrorCode::CanisterTrapped);
    assert_eq!(query(pic, id, "state").await, before);
    assert_eq!(pic.get_stable_memory(id).await, stable);
}

fn checkpoint(bytes: &[u8], values: &[u64], counter: u64, epoch: u64) {
    assert_eq!(bytes.len(), 120 + values.len() * 8);
    assert_eq!(&bytes[..8], b"ICNCP001");
    assert_eq!(&bytes[8..10], &2_u16.to_le_bytes());
    assert_eq!(&bytes[28..32], &(values.len() as u32).to_le_bytes());
    assert_eq!(&bytes[32..40], &epoch.to_le_bytes());
    assert_eq!(&bytes[56..64], &counter.to_le_bytes());
    let seal = 80 + values.len() * 8;
    assert_eq!(&bytes[80..seal], encode(values));
    assert_eq!(&bytes[seal..seal + 8], b"SEALED01");
    assert_eq!(&bytes[seal + 8..], &sha256(&bytes[..seal + 8]));
}

#[tokio::test(flavor = "current_thread")]
async fn version_two_recovers_full_capacity_and_preserves_failed_imports() {
    let a = artifacts::input("a");
    let b = artifacts::input("b");
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
    for count in [0, 17, 512] {
        let id = install(pic, &a).await;
        update(pic, id, "fill", &[count, 1000]).await;
        let mut values: Vec<_> = (1000..1000 + count).collect();
        let mut expected = vec![101, 999, 0, 0, 42, 8, count];
        expected.extend_from_slice(&values);
        state(pic, id, &expected).await;
        refused_update(pic, id, "fill", &[513, 0]).await;
        refused_update(pic, id, "fill", &[2, u64::MAX]).await;
        refused_update(pic, id, "export_checkpoint", &[3]).await;
        if count > 16 {
            // The old no-argument export command must refuse rather than
            // silently write a new envelope or freeze an unexported source.
            refused_update(pic, id, "export_checkpoint", &[]).await;
        }
        update(pic, id, "export_checkpoint", &[2]).await;
        let bytes = query(pic, id, "checkpoint_bytes").await;
        checkpoint(&bytes, &values, 42, 8);
        refused_update(pic, id, "fill", &[0, 0]).await;
        refused(pic, id, &b, &[9, 0]).await;
        refused(pic, id, &b, &[8, 1]).await;
        upgrade(pic, id, &b, 8).await;
        expected[..7].copy_from_slice(&[202, 202, 1, 0, 42, 8, count]);
        state(pic, id, &expected).await;
        assert_eq!(query(pic, id, "checkpoint_bytes").await, bytes);
        let mut counter = 42;
        let mut epoch = 8;
        if count == 512 {
            refused_update(pic, id, "append", &[9999]).await;
            update(pic, id, "pop", &[]).await;
            values.pop();
            counter += 1;
            epoch += 1;
        }
        update(pic, id, "append", &[9999]).await;
        values.push(9999);
        counter += 1;
        epoch += 1;
        update(pic, id, "export_checkpoint", &[2]).await;
        checkpoint(
            &query(pic, id, "checkpoint_bytes").await,
            &values,
            counter,
            epoch,
        );
        upgrade(pic, id, &a, epoch).await;
        let mut expected = vec![101, 101, 1, 0, counter, epoch, values.len() as u64];
        expected.extend_from_slice(&values);
        state(pic, id, &expected).await;
    }

    for mode in 0..3 {
        let id = install(pic, &a).await;
        update(pic, id, "fill", &[512, 1000]).await;
        update(pic, id, "export_checkpoint", &[2]).await;
        update(pic, id, "damage_checkpoint", &[mode]).await;
        refused(pic, id, &b, &[8, 0]).await;
    }
    instance.drop().await;
}

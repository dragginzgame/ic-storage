use super::installed::{logical, query, replace_heap, words};
use super::*;
use crate::installed_tests::{stopped_hash, update};
use ic_testkit::{
    pic::PocketIcStartupConfig,
    pocket_ic::{CanisterId, ErrorCode, PocketIcBuilder, nonblocking::PocketIc},
};
use std::time::Duration;

fn args(epoch: u64, domain: u64, fault: u64) -> Vec<u8> {
    [epoch, domain, fault]
        .into_iter()
        .flat_map(u64::to_le_bytes)
        .collect()
}

// Independent wire observations, without linking the codec used by the canister.
fn envelope(bytes: &[u8], phase: u16, epoch: u64, source: u64, target: u64) -> &[u8] {
    assert!((104..=4320).contains(&bytes.len()));
    assert_eq!(&bytes[..8], b"ICJOB001");
    assert_eq!(&bytes[8..10], &1u16.to_le_bytes());
    assert_eq!(&bytes[10..12], &phase.to_le_bytes());
    assert_eq!(&bytes[12..16], &64u32.to_le_bytes());
    assert_eq!(words(&bytes[16..40]), [epoch, source, target]);
    let len = u32::from_le_bytes(bytes[40..44].try_into().unwrap()) as usize;
    assert_eq!(len, bytes.len() - 104);
    assert_eq!(&bytes[44..64], &[0; 20]);
    assert_eq!(&bytes[64 + len..72 + len], b"JOBDONE1");
    assert_eq!(
        &bytes[72 + len..],
        Sha256Digest::compute(&bytes[..72 + len]).as_bytes()
    );
    &bytes[64..64 + len]
}

async fn refused_export(pic: &PocketIc, id: CanisterId, sender: CanisterId, arguments: Vec<u8>) {
    let native = query(pic, id, "observe").await;
    let stable = pic.get_stable_memory(id).await;
    let error = pic
        .update_call(id, sender, "export_checkpoint", arguments)
        .await
        .unwrap_err();
    assert!(matches!(
        error.error_code,
        ErrorCode::CanisterCalledTrap | ErrorCode::CanisterTrapped
    ));
    assert_eq!(query(pic, id, "observe").await, native);
    assert_eq!(pic.get_stable_memory(id).await, stable);
}

async fn refused_import(pic: &PocketIc, id: CanisterId, target: &[u8], arguments: Vec<u8>) {
    let native = query(pic, id, "observe").await;
    let stable = pic.get_stable_memory(id).await;
    let hash = stopped_hash(pic, id).await;
    let error = replace_heap(pic, id, target, arguments).await.unwrap_err();
    assert!(
        matches!(
            error.error_code,
            ErrorCode::CanisterCalledTrap | ErrorCode::CanisterTrapped
        ),
        "{error:?}"
    );
    assert_eq!(stopped_hash(pic, id).await, hash);
    assert_eq!(pic.get_stable_memory(id).await, stable);
    pic.start_canister(id, None).await.unwrap();
    assert_eq!(query(pic, id, "observe").await, native);
}

async fn publish(pic: &PocketIc, id: CanisterId, epoch: u64, source: u64, target: u64) -> Vec<u8> {
    let values = logical(&query(pic, id, "observe").await);
    pic.update_call(id, id, "export_checkpoint", args(epoch, target, 0))
        .await
        .unwrap();
    let bytes = query(pic, id, "checkpoint_bytes").await;
    let checkpoint = envelope(&bytes, 1, epoch, source, target);
    assert_eq!(&checkpoint[..8], b"ICNCP001");
    assert_eq!(&checkpoint[8..10], &2u16.to_le_bytes());
    assert_eq!(&checkpoint[32..40], &epoch.to_le_bytes());
    assert_eq!(checkpoint.len(), 120 + values.len() * 8);
    assert_eq!(words(&checkpoint[80..80 + values.len() * 8]), values);
    let end = checkpoint.len() - 32;
    assert_eq!(
        &checkpoint[end..],
        Sha256Digest::compute(&checkpoint[..end]).as_bytes()
    );
    bytes
}

async fn import(
    pic: &PocketIc,
    id: CanisterId,
    target_wasm: &[u8],
    epoch: u64,
    source: u64,
    target: u64,
) {
    let native = logical(&query(pic, id, "observe").await);
    let before = query(pic, id, "checkpoint_bytes").await;
    replace_heap(pic, id, target_wasm, args(epoch, source, 0))
        .await
        .unwrap();
    assert_eq!(
        stopped_hash(pic, id).await,
        Sha256Digest::compute(target_wasm).as_bytes()
    );
    pic.start_canister(id, None).await.unwrap();
    let after = query(pic, id, "checkpoint_bytes").await;
    assert_eq!(
        envelope(&before, 1, epoch, source, target),
        envelope(&after, 2, epoch, source, target)
    );
    let observation = query(pic, id, "observe").await;
    assert_eq!(logical(&observation), native);
    assert_eq!(words(&observation)[4], target);
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "native-job-test supplies the job bridge and prepared server"]
async fn controller_authority_domains_epochs_and_single_use_survive_upgrades() {
    let (directory, _) = super::super::super::input();
    let raw = fs::read(directory.join("vector/provider_vector.wasm")).unwrap();
    let bridge = fs::read(directory.join("checkpoint-job.wasm")).unwrap();
    let candidate = |relocated, root, recovery| {
        let bytes = prepare_job(&raw, &bridge, relocated, root, recovery);
        assert_eq!(bytes, prepare_job(&raw, &bridge, relocated, root, recovery));
        let digest = Sha256Digest::compute(&bytes);
        fs::write(directory.join(format!("job-{digest}.wasm")), &bytes).unwrap();
        println!(
            "job relocated={relocated} root={root:?} recovery={recovery} bytes={} sha256={digest}",
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
    let a = candidate(false, (0, 0), true);
    let b = candidate(true, (0, 0), true);
    let reopen_b = candidate(true, (0, 0), false);
    for root in [(3, 4), (0, 0), (512, 512)] {
        let id = pic.create_canister().await;
        pic.set_controllers(id, None, vec![CanisterId::anonymous(), id])
            .await
            .unwrap();
        pic.install_canister(id, candidate(false, root, false), vec![], None)
            .await;
        let initial = query(pic, id, "checkpoint_bytes").await;
        assert!(envelope(&initial, 0, 0, 131072, 131072).is_empty());
        refused_export(pic, id, CanisterId::from_slice(&[42]), args(1, 147456, 0)).await;
        refused_export(pic, id, id, args(2, 147456, 0)).await;
        refused_export(pic, id, id, args(1, 131072, 0)).await;
        refused_export(pic, id, id, args(1, 147456, 1)).await;
        refused_import(pic, id, &b, args(1, 131072, 0)).await;
        let sealed = publish(pic, id, 1, 131072, 147456).await;
        assert_eq!(publish(pic, id, 1, 131072, 147456).await, sealed);
        refused_export(pic, id, id, args(2, 147456, 0)).await;
        refused_import(pic, id, &a, args(1, 131072, 0)).await;
        refused_import(pic, id, &b, args(1, 147456, 0)).await;
        refused_import(pic, id, &b, args(0, 131072, 0)).await;
        refused_import(pic, id, &b, args(2, 131072, 0)).await;
        for fault in [1, 3] {
            refused_import(pic, id, &b, args(1, 131072, fault)).await;
        }
        if root.0 > 0 {
            refused_import(pic, id, &b, args(1, 131072, 2)).await;
        }
        import(pic, id, &b, 1, 131072, 147456).await;
        refused_import(pic, id, &b, args(1, 131072, 0)).await;
        let stable = pic.get_stable_memory(id).await;
        pic.upgrade_eop_canister(id, reopen_b.clone(), vec![], None)
            .await
            .unwrap();
        assert_eq!(pic.get_stable_memory(id).await, stable);
        refused_import(pic, id, &b, args(1, 131072, 0)).await;
        update(pic, id, "vector_reuse").await;
        refused_export(pic, id, id, args(1, 131072, 0)).await;
        refused_export(pic, id, id, args(3, 131072, 0)).await;
        pic.set_controllers(id, None, vec![CanisterId::anonymous()])
            .await
            .unwrap();
        refused_export(pic, id, id, args(2, 131072, 0)).await;
        pic.set_controllers(id, None, vec![CanisterId::anonymous(), id])
            .await
            .unwrap();
        publish(pic, id, 2, 147456, 131072).await;
        import(pic, id, &a, 2, 147456, 131072).await;
        assert_eq!(logical(&query(pic, id, "observe").await), [1]);
    }
    let id = pic.create_canister().await;
    pic.set_controllers(id, None, vec![CanisterId::anonymous(), id])
        .await
        .unwrap();
    pic.install_canister(id, candidate(false, (3, 4), false), vec![], None)
        .await;
    publish(pic, id, 1, 131072, 147456).await;
    pic.update_call(id, id, "damage_checkpoint", args(0, 0, 0))
        .await
        .unwrap();
    refused_import(pic, id, &b, args(1, 131072, 0)).await;
    refused_export(pic, id, id, args(1, 147456, 0)).await;
    owner.close().await;
}

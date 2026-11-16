use super::*;
use crate::installed_tests::{stopped_hash, update};
use ic_testkit::{
    pic::PocketIcStartupConfig,
    pocket_ic::{
        CanisterId, CanisterInstallMode, ErrorCode, PocketIcBuilder, RejectResponse,
        nonblocking::PocketIc,
    },
};
use std::time::Duration;

pub(super) async fn query(pic: &PocketIc, id: CanisterId, name: &str) -> Vec<u8> {
    pic.query_call(id, id, name, vec![]).await.unwrap()
}
fn args(epoch: u64, fault: u64) -> Vec<u8> {
    [epoch, fault]
        .into_iter()
        .flat_map(u64::to_le_bytes)
        .collect()
}
pub(super) async fn replace_heap(
    pic: &PocketIc,
    id: CanisterId,
    target: &[u8],
    arguments: Vec<u8>,
) -> Result<(), RejectResponse> {
    replace_heap_with_options(pic, id, target, arguments, false, None).await
}

// Recovery tests explicitly skip the old hook only after a checkpoint exists.
// Management authorization still applies, including to the chunk-store calls.
pub(super) async fn replace_heap_with_options(
    pic: &PocketIc,
    id: CanisterId,
    target: &[u8],
    arguments: Vec<u8>,
    skip_pre_upgrade: bool,
    sender: Option<CanisterId>,
) -> Result<(), RejectResponse> {
    // The selected management DTO defaults persistence to Replace. Populate
    // the option explicitly: Upgrade(None) is forbidden for EOP canisters.
    let mut mode = CanisterInstallMode::Upgrade(Some(Default::default()));
    let CanisterInstallMode::Upgrade(Some(flags)) = &mut mode else {
        unreachable!()
    };
    flags.wasm_memory_persistence = Some(Default::default());
    flags.skip_pre_upgrade = Some(skip_pre_upgrade);
    assert_eq!(
        format!("{:?}", flags.wasm_memory_persistence),
        "Some(Replace)"
    );
    pic.clear_chunk_store(id, sender).await?;
    let hash = pic.upload_chunk(id, sender, target.to_vec()).await?;
    assert_eq!(hash, Sha256Digest::compute(target).as_bytes());
    pic.install_chunked_canister(id, sender, mode, id, vec![hash.clone()], hash, arguments)
        .await
}

pub(super) fn words(bytes: &[u8]) -> Vec<u64> {
    bytes
        .as_chunks::<8>()
        .0
        .iter()
        .map(|w| u64::from_le_bytes(*w))
        .collect()
}
pub(super) fn logical(bytes: &[u8]) -> Vec<u64> {
    let w = words(bytes);
    assert_eq!(w[2], 0x4e5650524f563031);
    assert_eq!(w[3], 2);
    assert!(w[6] <= w[7] && w[7] <= 512);
    w[9..9 + w[6] as usize].to_vec()
}

pub(super) async fn export(pic: &PocketIc, id: CanisterId, epoch: u64) -> Vec<u8> {
    let values = logical(&query(pic, id, "observe").await);
    pic.update_call(id, id, "export_checkpoint", args(epoch, 0))
        .await
        .unwrap();
    let bytes = query(pic, id, "checkpoint_bytes").await;
    assert_eq!(bytes.len(), 120 + values.len() * 8);
    assert_eq!(&bytes[..8], b"ICNCP001");
    assert_eq!(&bytes[8..10], &2u16.to_le_bytes());
    assert_eq!(&bytes[32..40], &epoch.to_le_bytes());
    assert_eq!(&bytes[56..64], &41u64.to_le_bytes());
    assert_eq!(
        &bytes[80..80 + values.len() * 8],
        values
            .into_iter()
            .flat_map(u64::to_le_bytes)
            .collect::<Vec<_>>()
    );
    let end = bytes.len() - 32;
    assert_eq!(&bytes[end - 8..end], b"SEALED01");
    assert_eq!(
        &bytes[end..],
        Sha256Digest::compute(&bytes[..end]).as_bytes()
    );
    bytes
}

// Shared update rollback assertion for checkpoint and governed-job suites.
// Upgrade/restart assertions stay separate: they also own module identity and
// management-call lifecycle checks. Return the rejection for diagnostic checks.
pub(super) async fn refused_update(
    pic: &PocketIc,
    id: CanisterId,
    sender: CanisterId,
    name: &str,
    arguments: Vec<u8>,
) -> RejectResponse {
    let before = query(pic, id, "observe").await;
    let stable = pic.get_stable_memory(id).await;
    let error = pic
        .update_call(id, sender, name, arguments)
        .await
        .unwrap_err();
    assert!(
        matches!(
            error.error_code,
            ErrorCode::CanisterTrapped | ErrorCode::CanisterCalledTrap
        ),
        "{name}: {error:?}"
    );
    assert_eq!(query(pic, id, "observe").await, before);
    assert_eq!(pic.get_stable_memory(id).await, stable);
    error
}

pub(super) async fn refused(
    pic: &PocketIc,
    id: CanisterId,
    target: &[u8],
    epoch: u64,
    fault: u64,
    keep: bool,
) {
    let before = query(pic, id, "observe").await;
    let stable = pic.get_stable_memory(id).await;
    let hash = stopped_hash(pic, id).await;
    let result = if keep {
        pic.upgrade_eop_canister(id, target.to_vec(), args(epoch, fault), None)
            .await
    } else {
        replace_heap(pic, id, target, args(epoch, fault)).await
    };
    let error = result.unwrap_err();
    assert!(
        matches!(
            error.error_code,
            ErrorCode::CanisterTrapped | ErrorCode::CanisterCalledTrap
        ),
        "{error:?}"
    );
    if !keep && fault != 0 {
        assert!(error.reject_message.contains(if fault == 1 {
            "native restore fault after stable write"
        } else {
            "native restore fault during construction"
        }));
    }
    assert_eq!(stopped_hash(pic, id).await, hash);
    assert_eq!(pic.get_stable_memory(id).await, stable);
    pic.start_canister(id, None).await.unwrap();
    assert_eq!(query(pic, id, "observe").await, before);
}

pub(super) async fn recover(pic: &PocketIc, id: CanisterId, target: &[u8], epoch: u64, base: u64) {
    let before = logical(&query(pic, id, "observe").await);
    let checkpoint = query(pic, id, "checkpoint_bytes").await;
    stopped_hash(pic, id).await;
    replace_heap(pic, id, target, args(epoch, 0)).await.unwrap();
    assert_eq!(
        stopped_hash(pic, id).await,
        Sha256Digest::compute(target).as_bytes()
    );
    pic.start_canister(id, None).await.unwrap();
    let snapshot = query(pic, id, "observe").await;
    assert_eq!(logical(&snapshot), before);
    let header = words(&snapshot);
    assert_eq!((header[1], header[4], header[8]), (1, base, 1));
    assert_eq!(header[6], header[7]);
    assert_eq!(query(pic, id, "checkpoint_bytes").await, checkpoint);
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "native-checkpoint-test supplies fresh inputs and the prepared server"]
async fn native_export_and_incompatible_domain_reconstruction() {
    let (directory, raw, bridge) = inputs();
    let candidate = |relocated, root, recovery| {
        let bytes = prepare(&raw, &bridge, relocated, root, recovery);
        assert!(admit_final(
            &bytes, &raw, &bridge, relocated, root, recovery
        ));
        let digest = Sha256Digest::compute(&bytes);
        fs::write(directory.join(format!("migration-{digest}.wasm")), &bytes).unwrap();
        println!(
            "migration base={} root={root:?} recovery={recovery} bytes={} sha256={digest}",
            if relocated { 147456 } else { 131072 },
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
    let b = candidate(true, (0, 0), true);
    let return_a = candidate(false, (0, 0), true);
    for root in [(3, 4), (0, 0), (512, 512)] {
        let id = pic.create_canister().await;
        pic.install_canister(id, candidate(false, root, false), vec![], None)
            .await;
        update(pic, id, "vector_mutate").await;
        if root.0 < 512 && root.0 > 0 {
            update(pic, id, "vector_append").await;
        }
        refused(pic, id, &b, 7, 0, true).await;
        refused(pic, id, &b, 7, 0, false).await; // No complete checkpoint.
        refused_update(pic, id, id, "export_checkpoint", args(7, 1)).await;
        export(pic, id, 7).await;
        refused_update(pic, id, id, "vector_append", vec![]).await;
        refused(pic, id, &b, 7, 0, true).await;
        refused(pic, id, &b, 8, 0, false).await;
        refused(pic, id, &b, 7, 1, false).await;
        if root.0 > 0 {
            refused(pic, id, &b, 7, 2, false).await;
        }
        recover(pic, id, &b, 7, 147456).await;
        if root.0 == 512 {
            update(pic, id, "vector_remove").await;
            update(pic, id, "vector_shrink").await;
        }
        update(pic, id, "vector_append").await;
        update(pic, id, "vector_reuse").await;
        assert_eq!(logical(&query(pic, id, "observe").await), [1]);
        export(pic, id, 8).await;
        recover(pic, id, &return_a, 8, 131072).await;
        update(pic, id, "vector_append").await;
        assert_eq!(logical(&query(pic, id, "observe").await), [1, 1]);
    }
    for mode in 0..3 {
        let id = pic.create_canister().await;
        pic.install_canister(id, candidate(false, (3, 4), false), vec![], None)
            .await;
        export(pic, id, 9).await;
        pic.update_call(id, id, "damage_checkpoint", args(mode, 0))
            .await
            .unwrap();
        refused(pic, id, &b, 9, 0, false).await;
    }
    owner.close().await;
}

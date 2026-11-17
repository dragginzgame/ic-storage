//! Reserved native windows cannot overlap checkpoint initialization or reset.
use super::*;
use wasm_encoder::{ConstExpr, DataSection, RawSection};

#[test]
#[ignore = "prepared paged-native artifacts"]
fn checkpoint_initialization_inside_reserved_windows_refuses_composition() {
    let (dir, raw) = inputs();
    let bridge = fs::read(dir.join("checkpoint-map-paged.wasm")).unwrap();
    for offset in [196608, 393216, 655359] {
        let changed = crate::rewrite(&bridge, |module, id, bytes| {
            if id != 11 {
                module.section(&RawSection { id, data: bytes });
                return;
            }
            let mut data = DataSection::new();
            for payload in Parser::new(0).parse_all(&bridge) {
                if let Payload::DataSection(reader) = payload.unwrap() {
                    for segment in reader {
                        data.active(
                            0,
                            &ConstExpr::i32_const(offset),
                            segment.unwrap().data.to_vec(),
                        );
                    }
                }
            }
            module.section(&data);
        });
        ic_storage_retention_probe::artifact::validate_wasm(&changed, super::super::LIMIT).unwrap();
        assert!(
            std::panic::catch_unwind(|| {
                prepare_mode(&raw, &changed, false, Post::Reopen, Profile::Paged)
            })
            .is_err()
        );
    }
}

async fn expand(pic: &PocketIc, id: CanisterId) {
    let before = query(pic, id, "observe").await;
    let window = query(pic, id, "pool_window").await;
    for fault in 1..=4 {
        refused_update(pic, id, &format!("pool_failure_{fault}"), vec![]).await;
        assert_eq!(query(pic, id, "pool_window").await, window);
    }
    update(pic, id, "pool_grow").await;
    let after = query(pic, id, "observe").await;
    let old = word(&before, 48) as usize;
    assert_eq!(word(&after, 48) as usize, old * 2);
    assert_eq!(word(&after, 16), word(&before, 16)); // Reserved Wasm pages already exist.
    assert_eq!(&after[104..104 + old], &before[104..]);
    assert!(after[104 + old..].iter().all(|byte| *byte == 0));
    for root in 0..2 {
        assert_eq!(records(&after, root), records(&before, root));
    }
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "prepared paged-native artifacts and PocketIC server"]
async fn grown_windows_survive_reset_and_recover_into_the_disjoint_target() {
    let (dir, a) = artifacts();
    let pic = server().await;
    let id = install(&pic, &a.source).await;
    authorize(&pic, id).await;
    update(&pic, id, "insert_a").await;
    for _ in 0..3 {
        expand(&pic, id).await;
    }
    update(&pic, id, "reset_a").await;
    update(&pic, id, "pages_tail").await;
    let before = query(&pic, id, "observe").await;
    assert_eq!([word(&before, 40), word(&before, 48)], [A, 196608]);
    assert_eq!(records(&before, 0).len(), 192);
    assert!(before[104 + 65536..].iter().any(|byte| *byte != 0));
    update(&pic, id, "pool_refuse").await;
    refused_update(&pic, id, "pool_grow", vec![]).await;
    // Source reopening actually resets checkpoint globals/allocator memory.
    // A reset from the former 196608 floor would destroy the occupied tail.
    pic.upgrade_eop_canister(id, a.source.clone(), vec![], None)
        .await
        .unwrap();
    assert_eq!(query(&pic, id, "observe").await, before);
    let expected = publish(&pic, id).await;
    refused_update(&pic, id, "pool_grow", vec![]).await;
    pic.upgrade_eop_canister(id, a.frozen.clone(), args(1, B, 0), None)
        .await
        .unwrap();
    assert_eq!(query(&pic, id, "pool_window").await, before[104..]);
    replace_heap(&pic, id, &a.stage, args(1, A, 0))
        .await
        .unwrap();
    assert!(
        query(&pic, id, "discarded_payload")
            .await
            .iter()
            .all(|byte| *byte == 0)
    );
    // Private roots exclude ordinary growth, including after interruption.
    for start in 0..6 {
        refused_update(&pic, id, "pool_grow", vec![]).await;
        run(&pic, id, start, 0).await;
    }
    prefix(&pic, id, &expected, 6, true).await;
    let sealed = query(&pic, id, "checkpoint_bytes").await;
    for _ in 0..3 {
        expand(&pic, id).await;
    }
    let grown = query(&pic, id, "observe").await;
    assert_eq!([word(&grown, 40), word(&grown, 48)], [B, 196608]);
    pic.upgrade_eop_canister(id, a.reopen.clone(), vec![], None)
        .await
        .unwrap();
    assert_eq!(query(&pic, id, "observe").await, grown);
    assert_eq!(query(&pic, id, "checkpoint_bytes").await, sealed);
    run(&pic, id, 5, 0).await;
    assert_eq!(query(&pic, id, "observe").await, grown);
    update(&pic, id, "pool_refuse").await;
    refused_update(&pic, id, "pool_grow", vec![]).await;
    assert!(
        query(&pic, id, "discarded_payload")
            .await
            .iter()
            .all(|byte| *byte == 0)
    );
    fs::write(
        dir.join("PAGED_NATIVE_GROWTH"),
        b"disjoint pool growth and reset passed\n",
    )
    .unwrap();
    pic.drop().await;
}

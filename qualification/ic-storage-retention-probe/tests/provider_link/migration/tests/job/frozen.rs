use super::*;
use ic_storage_retention_probe::artifact::{sha256, sha256_hex, validate_wasm};

#[test]
#[ignore = "native-job-test supplies the compiled job bridge"]
fn blocked_bootstrap_precedes_transient_reset_and_job_admission() {
    let (_, raw, bridge) = job_inputs();
    for bytes in [
        prepare_frozen_job(&raw, &bridge, false),
        prepare_staged_job(&raw, &bridge, false, true),
    ] {
        let mut imported = 0;
        let mut exports = std::collections::BTreeMap::new();
        let mut bodies = Vec::new();
        for payload in Parser::new(0).parse_all(&bytes) {
            match payload.unwrap() {
                Payload::ImportSection(r) => imported = r.into_imports().count(),
                Payload::ExportSection(r) => {
                    for export in r {
                        let export = export.unwrap();
                        assert!(export.name == "memory" || export.name.starts_with("canister_"));
                        exports.insert(export.name, export.index);
                    }
                }
                Payload::CodeSectionEntry(body) => bodies.push(body),
                _ => {}
            }
        }
        let calls = |index: u32| {
            bodies[index as usize - imported]
                .get_operators_reader()
                .unwrap()
                .into_iter()
                .filter_map(|op| match op.unwrap() {
                    Op::Call { function_index } => Some(function_index),
                    _ => None,
                })
                .collect::<Vec<_>>()
        };
        let post = calls(exports["canister_post_upgrade"]);
        let init = calls(exports["canister_init"]);
        assert_eq!(post.len(), 3);
        assert!(
            post[1] > post[0] && post[0] > post[2],
            "WAT bootstrap, reset helper, then checkpoint Rust"
        );
        // A valid same-signature swap to native initialization must not be admitted.
        let changed = crate::rewrite(&bytes, |module, id, data| {
            if id != 10 {
                module.section(&RawSection { id, data });
                return;
            }
            let mut code = CodeSection::new();
            for (index, body) in bodies.iter().enumerate() {
                let mut copy = RoundtripReencoder;
                let mut function = copy.new_function_with_parsed_locals(body).unwrap();
                for op in body.get_operators_reader().unwrap() {
                    let op = op.unwrap();
                    if index + imported == exports["canister_post_upgrade"] as usize
                        && matches!(op, Op::Call { function_index } if function_index == post[0])
                    {
                        function.instruction(&I::Call(init[0]));
                    } else {
                        function.instruction(&copy.instruction(op).unwrap());
                    }
                }
                code.function(&function);
            }
            module.section(&code);
        });
        validate_wasm(&changed, LIMIT).unwrap();
        assert_ne!(changed, bytes);
    }
}

pub(super) async fn refused_resume(
    pic: &PocketIc,
    id: CanisterId,
    target: &[u8],
    arguments: Vec<u8>,
    message: Option<&str>,
) {
    let native = query(pic, id, "observe").await;
    let stable = pic.get_stable_memory(id).await;
    let checkpoint = query(pic, id, "checkpoint_bytes").await;
    let hash = stopped_hash(pic, id).await;
    let error = pic
        .upgrade_eop_canister(id, target.to_vec(), arguments, None)
        .await
        .unwrap_err();
    assert!(
        matches!(
            error.error_code,
            ErrorCode::CanisterCalledTrap | ErrorCode::CanisterTrapped
        ),
        "{error:?}"
    );
    if let Some(message) = message {
        assert!(
            error.reject_message.contains(message),
            "expected {message}: {error:?}"
        );
    }
    assert_eq!(stopped_hash(pic, id).await, hash);
    assert_eq!(pic.get_stable_memory(id).await, stable);
    pic.start_canister(id, None).await.unwrap();
    assert_eq!(query(pic, id, "observe").await, native);
    // Exercise the old checkpoint runtime again after failed target reset/use.
    assert_eq!(query(pic, id, "checkpoint_bytes").await, checkpoint);
}

async fn resume(pic: &PocketIc, id: CanisterId, target: &[u8], epoch: u64, destination: u64) {
    let native = query(pic, id, "observe").await;
    let stable = pic.get_stable_memory(id).await;
    let checkpoint = query(pic, id, "checkpoint_bytes").await;
    pic.upgrade_eop_canister(id, target.to_vec(), args(epoch, destination, 0), None)
        .await
        .unwrap();
    assert_eq!(stopped_hash(pic, id).await, &sha256(target));
    pic.start_canister(id, None).await.unwrap();
    assert_eq!(query(pic, id, "observe").await, native);
    assert_eq!(pic.get_stable_memory(id).await, stable);
    assert_eq!(query(pic, id, "checkpoint_bytes").await, checkpoint);
    for method in [
        "vector_reuse",
        "vector_business_error",
        "vector_forget",
        "vector_error_trap",
        "vector_error_finalize_trap",
    ] {
        refused_update(pic, id, id, method, vec![]).await;
    }
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "native-job-test supplies fresh job inputs and the prepared server"]
async fn frozen_sources_survive_compatible_upgrades_without_releasing_writes() {
    let (directory, raw, bridge) = job_inputs();
    let save = |bytes: Vec<u8>| {
        let digest = sha256_hex(&bytes);
        fs::write(directory.join(format!("frozen-{digest}.wasm")), &bytes).unwrap();
        println!("frozen artifact bytes={} sha256={digest}", bytes.len());
        bytes
    };
    let frozen_a = save(prepare_frozen_job(&raw, &bridge, false));
    let frozen_b = save(prepare_frozen_job(&raw, &bridge, true));
    let reopen = save(prepare_job(&raw, &bridge, false, (0, 0), false));
    let recover_a = save(prepare_job(&raw, &bridge, false, (0, 0), true));
    let recover_b = save(prepare_job(&raw, &bridge, true, (0, 0), true));
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
    for root in [(3, 4), (0, 0), (512, 512)] {
        let id = pic.create_canister().await;
        pic.set_controllers(id, None, vec![CanisterId::anonymous(), id])
            .await
            .unwrap();
        pic.install_canister(
            id,
            save(prepare_job(&raw, &bridge, false, root, false)),
            vec![],
            None,
        )
        .await;
        refused_resume(pic, id, &frozen_a, args(1, 147456, 0), None).await; // Ready, not frozen.
        let sealed = publish(pic, id, 1, 131072, 147456).await;
        refused_import(pic, id, &frozen_a, args(1, 147456, 0)).await; // Fresh heap cannot resume.
        refused_resume(pic, id, &reopen, vec![], None).await; // Ordinary reopening stays strict.
        refused_resume(pic, id, &frozen_b, args(1, 147456, 0), None).await;
        for (epoch, target) in [(0, 147456), (2, 147456), (1, 131072)] {
            refused_resume(pic, id, &frozen_a, args(epoch, target, 0), None).await;
        }
        refused_resume(pic, id, &frozen_a, vec![], None).await;
        refused_resume(
            pic,
            id,
            &frozen_a,
            args(1, 147456, 1),
            Some("frozen resume after validation"),
        )
        .await;
        resume(pic, id, &frozen_a, 1, 147456).await;
        assert_eq!(publish(pic, id, 1, 131072, 147456).await, sealed);
        resume(pic, id, &frozen_a, 1, 147456).await;
        pic.update_call(id, id, "cancel_checkpoint", args(1, 147456, 0))
            .await
            .unwrap();
        refused_resume(pic, id, &frozen_a, args(1, 147456, 0), None).await;
        update(pic, id, "vector_reuse").await;
        publish(pic, id, 2, 131072, 147456).await;
        resume(pic, id, &frozen_a, 2, 147456).await;
        import(pic, id, &recover_b, 2, 131072, 147456).await;
        publish(pic, id, 3, 147456, 131072).await;
        resume(pic, id, &frozen_b, 3, 131072).await;
        import(pic, id, &recover_a, 3, 147456, 131072).await;
        assert_eq!(logical(&query(pic, id, "observe").await), [1]);
    }
    for mode in 0..=3 {
        let id = pic.create_canister().await;
        pic.set_controllers(id, None, vec![CanisterId::anonymous(), id])
            .await
            .unwrap();
        pic.install_canister(
            id,
            save(prepare_job(&raw, &bridge, false, (3, 4), false)),
            vec![],
            None,
        )
        .await;
        publish(pic, id, 1, 131072, 147456).await;
        let damage = if mode == 0 {
            args(0, 0, 0)
        } else {
            args(mode, 1, 147456)
        };
        pic.update_call(id, id, "damage_checkpoint", damage)
            .await
            .unwrap();
        let message = match mode {
            0 => None,
            1 => Some("frozen job counter mismatch"),
            2 => Some("frozen job values mismatch"),
            _ => Some("frozen job length mismatch"),
        };
        if mode != 0 {
            let bytes = query(pic, id, "checkpoint_bytes").await;
            let cp = envelope(&bytes, 1, 1, 131072, 147456);
            let end = cp.len() - 32;
            assert_eq!(&cp[end..], &sha256(&cp[..end]));
        }
        refused_resume(pic, id, &frozen_a, args(1, 147456, 0), message).await;
    }
    instance.drop().await;
}

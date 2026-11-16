use super::super::{artifact::Post, records::prepare};
use super::{
    installed::{query, replace_heap, words},
    *,
};
use crate::installed_tests::{stopped_hash, update};
use ic_storage_retention_probe::artifact::{sha256, sha256_hex, validate_wasm};
use ic_testkit::pocket_ic::{CanisterId, ErrorCode, PocketIcBuilder, nonblocking::PocketIc};

mod narrow;
mod portable_narrow;
mod portable_widen;
mod updates;
mod widen;

fn inputs() -> (PathBuf, Vec<u8>, Vec<u8>) {
    let (directory, _) = super::super::super::input();
    let raw = fs::read(directory.join("vector/provider_vector.wasm")).unwrap();
    let bridge = fs::read(directory.join("checkpoint-record.wasm")).unwrap();
    (directory, raw, bridge)
}

fn args(value: u64, fault: u64) -> Vec<u8> {
    [value, fault]
        .into_iter()
        .flat_map(u64::to_le_bytes)
        .collect()
}

fn logical(bytes: &[u8]) -> Vec<u64> {
    let w = words(bytes);
    assert_eq!((w[2], w[3]), (0x4e5650524f563031, 3));
    assert!(w[6] <= w[7] && w[7] <= 512);
    w[9..9 + w[6] as usize].to_vec()
}

// Also compare record fields, which lie outside the legacy vector observation.
async fn refused(pic: &PocketIc, id: CanisterId, target: &[u8], arguments: Vec<u8>, keep: bool) {
    let fault = arguments
        .get(8..16)
        .map(|bytes| u64::from_le_bytes(bytes.try_into().unwrap()));
    let message = match (keep, fault) {
        (true, Some(1)) => Some("record conversion before publication"),
        (true, Some(2)) => Some("record conversion after publication"),
        (false, Some(1)) => Some("native restore fault after stable write"),
        (false, Some(2)) => Some("native restore fault during construction"),
        _ => None,
    };
    refused_with_message(pic, id, target, arguments, keep, message).await;
}

async fn refused_with_message(
    pic: &PocketIc,
    id: CanisterId,
    target: &[u8],
    arguments: Vec<u8>,
    keep: bool,
    message: Option<&str>,
) {
    let native = query(pic, id, "observe").await;
    let record = query(pic, id, "observe_record").await;
    let stable = pic.get_stable_memory(id).await;
    let hash = stopped_hash(pic, id).await;
    let result = if keep {
        pic.upgrade_eop_canister(id, target.to_vec(), arguments, None)
            .await
    } else {
        replace_heap(pic, id, target, arguments).await
    };
    let error = result.unwrap_err();
    assert!(
        matches!(
            error.error_code,
            ErrorCode::CanisterTrapped | ErrorCode::CanisterCalledTrap
        ),
        "{error:?}"
    );
    if let Some(message) = message {
        assert!(error.reject_message.contains(message), "{error:?}");
    }
    assert_eq!(stopped_hash(pic, id).await, hash);
    assert_eq!(pic.get_stable_memory(id).await, stable);
    pic.start_canister(id, None).await.unwrap();
    assert_eq!(query(pic, id, "observe").await, native);
    assert_eq!(query(pic, id, "observe_record").await, record);
}

#[test]
#[ignore = "native-record-test supplies fresh compiler inputs"]
fn conversion_artifact_orders_source_admission_before_rust() {
    let (_, raw, bridge) = inputs();
    for (schema, mode) in [(2, Post::Convert), (3, Post::Narrow), (2, Post::Widen)] {
        let bytes = prepare(&raw, &bridge, false, (0, 0), schema, mode);
        let mut exports = std::collections::BTreeMap::new();
        let mut bodies = Vec::new();
        let mut imports = 0;
        for payload in Parser::new(0).parse_all(&bytes) {
            match payload.unwrap() {
                Payload::ImportSection(r) => {
                    for import in r.into_imports() {
                        assert_eq!(import.unwrap().module, "ic0");
                        imports += 1;
                    }
                }
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
            bodies[(index - imports) as usize]
                .get_operators_reader()
                .unwrap()
                .into_iter()
                .filter_map(|op| match op.unwrap() {
                    Op::Call { function_index } => Some(function_index),
                    _ => None,
                })
                .collect::<Vec<_>>()
        };
        let init = calls(exports["canister_init"]);
        let post = calls(exports["canister_post_upgrade"]);
        assert_eq!((init.len(), post.len()), (2, 4));
        assert!(post[0] > post[2] && post[3] > post[2]);
        assert!(post[1] > post[0] && post[1] > post[3]);
        assert_ne!(
            post[0], init[0],
            "source admission must not initialize retained state"
        );
        assert_ne!(
            post[2], init[1],
            "conversion must not create a checkpoint cell"
        );
        // Removing publication still produces valid Wasm; it is a different artifact.
        let changed = crate::rewrite(&bytes, |module: &mut Module, id, data| {
            if id != 10 {
                module.section(&RawSection { id, data });
                return;
            }
            let mut code = CodeSection::new();
            for body in &bodies {
                let mut copy = RoundtripReencoder;
                let mut function = copy.new_function_with_parsed_locals(body).unwrap();
                for op in body.get_operators_reader().unwrap() {
                    let op = op.unwrap();
                    if matches!(op, Op::Call { function_index } if function_index == post[2]) {
                        function.instruction(&I::Nop);
                    } else {
                        function.instruction(&copy.instruction(op).unwrap());
                    }
                }
                code.function(&function);
            }
            module.section(&code);
        });
        validate_wasm(&changed, LIMIT).unwrap();
        assert_ne!(changed, prepare(&raw, &bridge, false, (0, 0), schema, mode));
    }
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "native-record-test supplies fresh inputs and the prepared server"]
async fn installed_conversion_rollback_and_portable_recovery() {
    let (directory, raw, bridge) = inputs();
    let candidate = |relocated, root, schema, post| {
        let bytes = prepare(&raw, &bridge, relocated, root, schema, post);
        assert_eq!(bytes, prepare(&raw, &bridge, relocated, root, schema, post));
        let digest = sha256_hex(&bytes);
        fs::write(directory.join(format!("record-{digest}.wasm")), &bytes).unwrap();
        println!(
            "record schema={schema} relocated={relocated} root={root:?} post={post:?} sha256={digest}"
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
    let convert = candidate(false, (0, 0), 2, Post::Convert);
    let reopen = candidate(false, (0, 0), 2, Post::Reopen);
    let recover = candidate(true, (0, 0), 2, Post::Recover);
    let relocated_reopen = candidate(true, (0, 0), 2, Post::Reopen);
    let return_a = candidate(false, (0, 0), 2, Post::Recover);
    let downgrade = candidate(false, (0, 0), 1, Post::Reopen);
    let wrong_schema = candidate(true, (0, 0), 1, Post::Recover);
    for root in [(0, 0), (3, 4), (512, 512)] {
        let id = pic.create_canister().await;
        pic.install_canister(id, candidate(false, root, 1, Post::Reopen), vec![], None)
            .await;
        assert_eq!(words(&query(pic, id, "observe_record").await), [1, 41, 0]);
        refused(pic, id, &reopen, vec![], true).await;
        for arguments in [args(40, 0), args(100, 1), args(100, 2), vec![]] {
            refused(pic, id, &convert, arguments, true).await;
        }
        let before = query(pic, id, "observe").await;
        let stable = pic.get_stable_memory(id).await;
        pic.upgrade_eop_canister(id, convert.clone(), args(100, 0), None)
            .await
            .unwrap();
        assert_eq!(stopped_hash(pic, id).await, &sha256(&convert));
        pic.start_canister(id, None).await.unwrap();
        assert_eq!(query(pic, id, "observe").await, before);
        assert_eq!(pic.get_stable_memory(id).await, stable);
        assert_eq!(words(&query(pic, id, "observe_record").await), [2, 41, 100]);
        pic.upgrade_eop_canister(id, reopen.clone(), vec![], None)
            .await
            .unwrap();
        assert_eq!(query(pic, id, "observe").await, before);
        refused(pic, id, &convert, args(100, 0), true).await;
        refused(pic, id, &downgrade, vec![], true).await;
        let values = logical(&before);
        pic.update_call(id, id, "export_checkpoint", args(7, 0))
            .await
            .unwrap();
        let checkpoint = query(pic, id, "checkpoint_bytes").await;
        assert_eq!(&checkpoint[8..10], &3u16.to_le_bytes());
        assert_eq!(&checkpoint[24..28], &2u32.to_le_bytes());
        assert_eq!(&checkpoint[56..64], &41u64.to_le_bytes());
        assert_eq!(&checkpoint[72..80], &100u64.to_le_bytes());
        assert_eq!(checkpoint.len(), 120 + values.len() * 8);
        assert_eq!(
            &checkpoint[80..80 + values.len() * 8],
            values
                .iter()
                .copied()
                .flat_map(u64::to_le_bytes)
                .collect::<Vec<_>>()
        );
        let end = checkpoint.len() - 32;
        assert_eq!(&checkpoint[end..], &sha256(&checkpoint[..end]));
        refused(pic, id, &recover, args(7, 0), true).await;
        refused(pic, id, &wrong_schema, args(7, 0), false).await;
        for arguments in [args(8, 0), args(7, 1)] {
            refused(pic, id, &recover, arguments, false).await;
        }
        if root.0 > 0 {
            refused(pic, id, &recover, args(7, 2), false).await;
        }
        let stable = pic.get_stable_memory(id).await;
        stopped_hash(pic, id).await;
        replace_heap(pic, id, &recover, args(7, 0)).await.unwrap();
        assert_eq!(stopped_hash(pic, id).await, &sha256(&recover));
        pic.start_canister(id, None).await.unwrap();
        let restored = query(pic, id, "observe").await;
        assert_eq!(logical(&restored), values);
        let header = words(&restored);
        assert_eq!((header[4], header[6], header[7]), (147456, root.0, root.0));
        assert_eq!(words(&query(pic, id, "observe_record").await), [2, 41, 100]);
        assert_eq!(pic.get_stable_memory(id).await, stable);
        pic.upgrade_eop_canister(id, relocated_reopen.clone(), vec![], None)
            .await
            .unwrap();
        assert_eq!(query(pic, id, "observe").await, restored);
        if root.0 > 0 {
            update(pic, id, "vector_remove").await;
            update(pic, id, "vector_shrink").await;
        }
        update(pic, id, "vector_append").await;
        update(pic, id, "vector_reuse").await;
        assert_eq!(logical(&query(pic, id, "observe").await), [1]);
        pic.update_call(id, id, "export_checkpoint", args(8, 0))
            .await
            .unwrap();
        stopped_hash(pic, id).await;
        replace_heap(pic, id, &return_a, args(8, 0)).await.unwrap();
        pic.start_canister(id, None).await.unwrap();
        assert_eq!(logical(&query(pic, id, "observe").await), [1]);
        assert_eq!(words(&query(pic, id, "observe_record").await), [2, 41, 100]);
        update(pic, id, "vector_append").await;
        assert_eq!(logical(&query(pic, id, "observe").await), [1, 1]);
    }
    for mode in 0..3 {
        let id = pic.create_canister().await;
        pic.install_canister(id, reopen.clone(), vec![], None).await;
        pic.update_call(id, id, "export_checkpoint", args(9, 0))
            .await
            .unwrap();
        let before = query(pic, id, "checkpoint_bytes").await;
        pic.update_call(id, id, "damage_checkpoint", args(mode, 0))
            .await
            .unwrap();
        assert_ne!(query(pic, id, "checkpoint_bytes").await, before);
        refused(pic, id, &recover, args(9, 0), false).await;
    }
    instance.drop().await;
}

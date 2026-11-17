//! Generation-long native ownership under the controlled transient bootstrap.
use super::*;
use std::collections::BTreeMap;

fn inputs() -> PathBuf {
    let directory =
        PathBuf::from(std::env::var_os("NATIVE_SESSION_INPUTS").expect("session collector"));
    assert!(directory.is_absolute());
    directory
}

fn raw(variant: &str) -> Vec<u8> {
    ic_host_artifacts::read(
        fs::File::open(inputs().join(format!("session-{variant}.wasm"))).unwrap(),
        ic_host_artifacts::Length::AtMost(1024 * 1024),
    )
    .unwrap()
    .0
}

#[test]
#[ignore = "session collector supplies fresh compiler artifacts"]
fn artifact() {
    for variant in ["a", "b", "fail", "corrupt", "no-close", "pre-fail"] {
        let raw = raw(variant);
        assert!(prepare(&raw).is_err());
        assert!(artifact::prepare_graph(&raw).is_err());
        let prepared = artifact::prepare_session(&raw).expect("closed session profile");
        fs::write(
            inputs().join(format!("installed-{variant}.wasm")),
            &prepared.bytes,
        )
        .unwrap();
        fs::write(
            inputs().join(format!("installed-{variant}.txt")),
            prepared.report,
        )
        .unwrap();
        let outside = rewrite(&raw, |m, id, bytes| {
            if id == 11 {
                let mut data = DataSection::new();
                data.active(0, &ConstExpr::i32_const(BOUNDARY), [1]);
                m.section(&data);
            } else {
                m.section(&RawSection { id, data: bytes });
            }
        });
        assert_eq!(
            artifact::prepare_session(&outside).err(),
            Some(Refusal::Data)
        );
        let init = Parser::new(0)
            .parse_all(&raw)
            .find_map(|payload| {
                if let Payload::ExportSection(reader) = payload.unwrap() {
                    reader.into_iter().find_map(|export| {
                        let export = export.unwrap();
                        (export.name == "canister_init").then_some(export.index)
                    })
                } else {
                    None
                }
            })
            .unwrap();
        let unexpected_start = rewrite(&raw, |m, id, bytes| {
            m.section(&RawSection { id, data: bytes });
            if id == 7 {
                m.section(&StartSection {
                    function_index: init,
                });
            }
        });
        assert_eq!(
            artifact::prepare_session(&unexpected_start).err(),
            Some(Refusal::ExistingStart)
        );
    }
    fs::write(
        inputs().join("ADMITTED"),
        "six closed artifacts; transient data and start refusal passed\n",
    )
    .unwrap();
}

async fn records(pic: &PocketIc, id: CanisterId) -> [BTreeMap<u64, u64>; 2] {
    let bytes = pic
        .query_call(id, id, "session_state", vec![])
        .await
        .unwrap();
    assert_eq!(bytes.len() % 8, 0);
    let mut words = bytes
        .as_chunks::<8>()
        .0
        .iter()
        .map(|word| u64::from_le_bytes(*word));
    let result = core::array::from_fn(|_| {
        let count = words.next().unwrap();
        let mut map = BTreeMap::new();
        for _ in 0..count {
            assert_eq!(
                map.insert(words.next().unwrap(), words.next().unwrap()),
                None
            );
        }
        map
    });
    assert_eq!(words.next(), None);
    result
}

async fn refused(pic: &PocketIc, id: CanisterId, method: &str) {
    assert_eq!(
        pic.update_call(id, id, method, vec![])
            .await
            .unwrap_err()
            .error_code,
        ErrorCode::CanisterTrapped
    );
}

async fn retained(pic: &PocketIc, id: CanisterId) -> Vec<u8> {
    pic.query_call(id, id, "session_bytes", vec![])
        .await
        .unwrap()
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "prepared server and collector-owned artifacts"]
async fn installed() {
    let bytes = ["a", "b", "fail", "corrupt"].map(|variant| {
        let prepared = artifact::prepare_session(&raw(variant)).unwrap();
        let stored = fs::read(inputs().join(format!("installed-{variant}.wasm"))).unwrap();
        assert_eq!(stored, prepared.bytes);
        stored
    });
    let server = std::env::var("IC_TESTKIT_POCKET_IC_URL").expect("prepared server");
    let instance = PocketIcBuilder::new()
        .with_application_subnet()
        .with_max_request_time_ms(Some(30_000))
        .with_server_url(server.parse().unwrap())
        .build_async()
        .await;
    let pic = &instance;
    let id = pic.create_canister().await;
    pic.install_canister(id, bytes[0].clone(), vec![], None)
        .await;
    assert_eq!(
        &words::<5>(pic, id, "session_status").await[..4],
        &[101, 2, 1, 0]
    );
    update(pic, id, "session_fill").await;
    let expected = core::array::from_fn(|index| {
        (0..1536)
            .map(|key| (key, key + 1000 + index as u64))
            .collect::<BTreeMap<_, _>>()
    });
    assert_eq!(records(pic, id).await, expected);
    let mut costs = String::from("phase,admissions,calls,instructions\n");
    for call in 0..32 {
        update(pic, id, "session_lookup").await;
        let status = words::<5>(pic, id, "session_status").await;
        assert_eq!(status[2], 1);
        assert_eq!(status[3], call + 2);
        costs.push_str(&format!(
            "lookup,{},{},{}\n",
            status[2], status[3], status[4]
        ));
    }
    let before = words::<5>(pic, id, "session_status").await;
    let before_bytes = retained(pic, id).await;
    assert!(
        pic.query_call(id, id, "session_query_write", vec![])
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(words::<5>(pic, id, "session_status").await, before);
    assert_eq!(retained(pic, id).await, before_bytes);
    assert_eq!(records(pic, id).await, expected);
    for method in ["session_trap", "session_close_trap", "session_open"] {
        refused(pic, id, method).await;
        assert_eq!(words::<5>(pic, id, "session_status").await, before);
        assert_eq!(records(pic, id).await, expected);
        assert_eq!(retained(pic, id).await, before_bytes);
    }
    update(pic, id, "session_close").await;
    assert_eq!(words::<5>(pic, id, "session_status").await[1], 1);
    refused(pic, id, "session_lookup").await;
    update(pic, id, "session_open").await;
    let admitted = words::<5>(pic, id, "session_status").await;
    assert_eq!(admitted[2], 2);
    assert!(admitted[4] > before[4]);
    costs.push_str(&format!(
        "readmission,{},{},{}\n",
        admitted[2], admitted[3], admitted[4]
    ));
    assert_eq!(records(pic, id).await, expected);

    for target in &bytes[2..] {
        let before = words::<5>(pic, id, "session_status").await;
        let before_bytes = retained(pic, id).await;
        pic.stop_canister(id, None).await.unwrap();
        let error = pic
            .upgrade_eop_canister(id, target.clone(), vec![], None)
            .await
            .unwrap_err();
        assert_eq!(error.error_code, ErrorCode::CanisterTrapped);
        assert_eq!(
            pic.canister_status(id, None)
                .await
                .unwrap()
                .module_hash
                .unwrap(),
            sha256(&bytes[0])
        );
        pic.start_canister(id, None).await.unwrap();
        assert_eq!(words::<5>(pic, id, "session_status").await, before);
        assert_eq!(records(pic, id).await, expected);
        assert_eq!(retained(pic, id).await, before_bytes);
        update(pic, id, "session_lookup").await;
    }
    for variant in ["no-close", "pre-fail"] {
        let source = artifact::prepare_session(&raw(variant)).unwrap().bytes;
        let bad = pic.create_canister().await;
        pic.install_canister(bad, source.clone(), vec![], None)
            .await;
        update(pic, bad, "session_fill").await;
        let before = words::<5>(pic, bad, "session_status").await;
        let before_bytes = retained(pic, bad).await;
        pic.stop_canister(bad, None).await.unwrap();
        assert_eq!(
            pic.upgrade_eop_canister(bad, bytes[1].clone(), vec![], None)
                .await
                .unwrap_err()
                .error_code,
            ErrorCode::CanisterTrapped
        );
        assert_eq!(
            pic.canister_status(bad, None)
                .await
                .unwrap()
                .module_hash
                .unwrap(),
            sha256(&source)
        );
        pic.start_canister(bad, None).await.unwrap();
        assert_eq!(words::<5>(pic, bad, "session_status").await, before);
        assert_eq!(retained(pic, bad).await, before_bytes);
        assert_eq!(records(pic, bad).await, expected);
        update(pic, bad, "session_lookup").await;
        pic.stop_canister(bad, None).await.unwrap();
        pic.delete_canister(bad, None).await.unwrap();
    }
    for (target, version) in [(&bytes[1], 202), (&bytes[0], 101)] {
        upgrade(pic, id, target).await;
        let status = words::<5>(pic, id, "session_status").await;
        assert_eq!(&status[..4], &[version, 2, 1, 0]);
        assert_eq!(records(pic, id).await, expected);
        update(pic, id, "session_replace").await;
        let mut changed = expected.clone();
        for (index, map) in changed.iter_mut().enumerate() {
            map.insert(7, 9000 + index as u64);
        }
        assert_eq!(records(pic, id).await, changed);
        update(pic, id, "session_delete").await;
        for map in &mut changed {
            map.remove(&7);
        }
        assert_eq!(records(pic, id).await, changed);
        update(pic, id, "session_reinsert").await;
        assert_eq!(records(pic, id).await, expected);
        assert_eq!(words::<5>(pic, id, "session_status").await[2], 1);
    }
    fs::write(inputs().join("costs.csv"), costs).unwrap();
    pic.stop_canister(id, None).await.unwrap();
    pic.delete_canister(id, None).await.unwrap();
    instance.drop().await;
    fs::write(inputs().join("INSTALLED"), "live owners, explicit close/reopen, traps, corrupt target, failed and bidirectional compatible upgrades passed\n").unwrap();
}

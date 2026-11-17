//! Controlled Rust startup and installed retention qualification.
#![forbid(unsafe_code)]

mod graph_tests;
mod growth_tests;
mod session_tests;

use ic_storage_retention_probe::artifact::sha256;
use std::{fs, path::PathBuf};

use ic_testkit::pocket_ic::{CanisterId, ErrorCode, PocketIcBuilder, nonblocking::PocketIc};
use wasm_encoder::{
    ConstExpr, DataSection, MemorySection, MemoryType, Module, RawSection, StartSection,
};
use wasmparser::{Parser, Payload};

use ic_storage_retention_probe::startup as artifact;

use artifact::{BOUNDARY, MAGIC, Refusal, SENTINEL, admit, prepare};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn input(variant: &str) -> Vec<u8> {
    let path = root().join(format!(
        "target/qualification/rust-{variant}/wasm32-unknown-unknown/release/ic_storage_rust_startup_probe.wasm"
    ));
    let file = fs::File::open(path).expect("build the fixtures with make rust-startup-build");
    ic_host_artifacts::read(
        file,
        ic_host_artifacts::Length::AtMost((1024 * 1024) as u64),
    )
    .expect("bounded input artifact")
    .0
}

fn rewrite(raw: &[u8], mut section: impl FnMut(&mut Module, u8, &[u8])) -> Vec<u8> {
    let mut module = Module::new();
    for payload in Parser::new(0).parse_all(raw) {
        if let Some((id, range)) = payload.expect("valid source fixture").as_section() {
            section(
                &mut module,
                id,
                &raw[range.start as usize..range.end as usize],
            );
        }
    }
    module.finish()
}

#[test]
fn artifact_admission_rejects_unsafe_startup_layouts_and_tampering() {
    let raw = input("a");
    let valid = prepare(&raw).expect("admit Rust A");
    admit(&valid.bytes, &raw).expect("admit final bytes");

    let outside = rewrite(&raw, |m, id, bytes| {
        if id == 11 {
            let mut data = DataSection::new();
            data.active(0, &ConstExpr::i32_const(BOUNDARY), [42]);
            m.section(&data);
        } else {
            m.section(&RawSection { id, data: bytes });
        }
    });
    assert_eq!(prepare(&outside).err(), Some(Refusal::Data));

    let memory = rewrite(&raw, |m, id, bytes| {
        if id == 5 {
            let mut memory = MemorySection::new();
            memory.memory(MemoryType {
                minimum: 130,
                maximum: Some(256),
                memory64: false,
                shared: false,
                page_size_log2: None,
            });
            m.section(&memory);
        } else {
            m.section(&RawSection { id, data: bytes });
        }
    });
    assert_eq!(prepare(&memory).err(), Some(Refusal::Memory));

    let init = Parser::new(0)
        .parse_all(&raw)
        .find_map(|p| match p.expect("payload") {
            Payload::ExportSection(exports) => exports.into_iter().find_map(|e| {
                let e = e.expect("export");
                (e.name == "canister_init").then_some(e.index)
            }),
            _ => None,
        })
        .expect("init export");
    let unexpected_start = rewrite(&raw, |m, id, bytes| {
        m.section(&RawSection { id, data: bytes });
        if id == 7 {
            m.section(&StartSection {
                function_index: init,
            });
        }
    });
    assert_eq!(
        prepare(&unexpected_start).err(),
        Some(Refusal::ExistingStart)
    );

    // Removing bootstrap or marker leaves valid Wasm, but invalid retained-memory artifacts.
    for removed in [0, 8] {
        let altered = rewrite(&valid.bytes, |m, id, bytes| {
            if id != removed {
                m.section(&RawSection { id, data: bytes });
            }
        });
        assert_eq!(admit(&altered, &raw), Err(Refusal::FinalMismatch));
    }
    assert_eq!(
        admit(&valid.bytes[..valid.bytes.len() - 1], &raw),
        Err(Refusal::Malformed)
    );
    assert_eq!(
        admit(&valid.bytes, &input("b")),
        Err(Refusal::FinalMismatch)
    );
}

async fn words<const N: usize>(pic: &PocketIc, id: CanisterId, method: &str) -> [u64; N] {
    let bytes = pic.query_call(id, id, method, vec![]).await.expect("query");
    assert_eq!(bytes.len(), N * 8);
    std::array::from_fn(|i| u64::from_le_bytes(bytes[i * 8..i * 8 + 8].try_into().expect("word")))
}

async fn update(pic: &PocketIc, id: CanisterId, method: &str) {
    assert!(
        pic.update_call(id, id, method, vec![])
            .await
            .expect("update")
            .is_empty()
    );
}

async fn upgrade(pic: &PocketIc, id: CanisterId, bytes: &[u8]) {
    pic.stop_canister(id, None).await.expect("stop completes");
    pic.upgrade_eop_canister(id, bytes.to_vec(), vec![], None)
        .await
        .expect("keep upgrade");
    let hash = pic
        .canister_status(id, None)
        .await
        .expect("status")
        .module_hash
        .expect("hash");
    assert_eq!(hash, &sha256(bytes));
    pic.start_canister(id, None).await.expect("restart");
}

#[tokio::test(flavor = "current_thread")]
async fn rust_startup_restores_transients_without_touching_retained_bytes() {
    let raw_a = input("a");
    let raw_b = input("b");
    let a = prepare(&raw_a).expect("admit A");
    let b = prepare(&raw_b).expect("admit B");
    admit(&a.bytes, &raw_a).expect("final A");
    admit(&b.bytes, &raw_b).expect("final B");
    assert_ne!(a.bytes, b.bytes);
    let evidence = root().join("target/evidence/rust-startup");
    fs::create_dir_all(&evidence).expect("evidence directory");
    for (name, artifact) in [("a", &a), ("b", &b)] {
        fs::write(evidence.join(format!("{name}.wasm")), &artifact.bytes).expect("final artifact");
        fs::write(evidence.join(format!("{name}.txt")), &artifact.report).expect("artifact report");
        println!("{name}:\n{}", artifact.report);
    }
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
    let id = pic.create_canister().await;
    pic.install_canister(id, a.bytes.clone(), vec![], None)
        .await;
    assert_eq!(words::<4>(pic, id, "transient").await, [101, 0, 1, 2560]);

    // A zero anchor is not a fresh installation during upgrade. Startup has
    // already cleared volatile memory when the wrapper traps; rollback restores it.
    update(pic, id, "dirty").await;
    pic.stop_canister(id, None).await.expect("stop");
    let rejection = pic
        .upgrade_eop_canister(id, b.bytes.clone(), vec![], None)
        .await
        .expect_err("zero anchor");
    assert_eq!(rejection.error_code, ErrorCode::CanisterTrapped);
    assert_eq!(
        pic.canister_status(id, None)
            .await
            .expect("status")
            .module_hash
            .expect("hash"),
        &sha256(&a.bytes)
    );
    pic.start_canister(id, None).await.expect("restart A");
    assert_eq!(
        words::<4>(pic, id, "transient").await,
        [999, 888, 2560, 2560]
    );
    assert_eq!(words::<2>(pic, id, "retained").await, [0, 0]);

    update(pic, id, "retain").await;
    upgrade(pic, id, &b.bytes).await;
    assert_eq!(words::<4>(pic, id, "transient").await, [202, 0, 2, 2560]);
    assert_eq!(
        words::<2>(pic, id, "retained").await,
        [MAGIC as u64, SENTINEL as u64]
    );
    update(pic, id, "dirty").await;
    assert_eq!(
        words::<4>(pic, id, "transient").await,
        [999, 888, 2560, 2560]
    );
    upgrade(pic, id, &a.bytes).await;
    assert_eq!(words::<4>(pic, id, "transient").await, [101, 0, 2, 2560]);
    assert_eq!(
        words::<2>(pic, id, "retained").await,
        [MAGIC as u64, SENTINEL as u64]
    );
    instance.drop().await;
}

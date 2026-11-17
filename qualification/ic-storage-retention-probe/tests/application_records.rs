//! Installed admission and rollback through Storage's public byte-record owner.
use ic_storage_retention_probe::artifact::{sha256, sha256_hex, validate_wasm};
use ic_testkit::pocket_ic::{
    CanisterId, PocketIcBuilder, common::rest::BlobCompression, nonblocking::PocketIc,
};
use std::{fs, path::PathBuf};
use wasmparser::{Parser, Payload, TypeRef};

fn encode(words: &[u64]) -> Vec<u8> {
    words.iter().flat_map(|word| word.to_le_bytes()).collect()
}
fn artifact(variant: &str) -> Vec<u8> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let path = root.join(format!("target/qualification/application-records-{variant}/wasm32-unknown-unknown/release/ic_storage_checkpoint_provider_probe.wasm"));
    let bytes = ic_host_artifacts::read(
        fs::File::open(path).expect("make application-records-build"),
        ic_host_artifacts::Length::AtMost(2 * 1024 * 1024),
    )
    .unwrap()
    .0;
    validate_wasm(&bytes, 2 * 1024 * 1024).unwrap();
    let mut memories = 0;
    let mut exports = Vec::new();
    for payload in Parser::new(0).parse_all(&bytes) {
        match payload.unwrap() {
            Payload::MemorySection(reader) => {
                for memory in reader {
                    let memory = memory.unwrap();
                    assert!(!memory.memory64 && !memory.shared);
                    assert_eq!((memory.initial, memory.maximum), (32, Some(256)));
                    memories += 1;
                }
            }
            Payload::ImportSection(reader) => {
                for import in reader.into_imports() {
                    let import = import.unwrap();
                    assert_eq!(import.module, "ic0");
                    assert!(matches!(import.ty, TypeRef::Func(_)));
                }
            }
            Payload::ExportSection(reader) => {
                for export in reader {
                    let export = export.unwrap();
                    if export.name.starts_with("canister_") {
                        exports.push(export.name);
                    }
                }
            }
            Payload::CustomSection(section) => {
                assert!(!section.name().contains("enhanced-orthogonal-persistence"))
            }
            Payload::StartSection { .. } => panic!("ordinary application startup only"),
            _ => {}
        }
    }
    assert_eq!(memories, 1);
    exports.sort_unstable();
    assert_eq!(
        exports,
        [
            "canister_init",
            "canister_post_upgrade",
            "canister_query speculative",
            "canister_query state",
            "canister_update write"
        ]
    );
    let directory = PathBuf::from(
        std::env::var("IC_STORAGE_RECORDS_EVIDENCE").expect("selected attempt evidence"),
    );
    fs::write(directory.join(format!("{variant}.wasm")), &bytes).unwrap();
    println!(
        "application records {variant}: bytes={} sha256={}",
        bytes.len(),
        sha256_hex(&bytes)
    );
    bytes
}
async fn state(pic: &PocketIc, id: CanisterId) -> Vec<u8> {
    pic.query_call(id, id, "state", vec![]).await.unwrap()
}
fn values(bytes: &[u8]) -> Vec<&[u8]> {
    let mut input = &bytes[8..];
    let mut result = Vec::new();
    for _ in 0..3 {
        let len = u64::from_le_bytes(input[..8].try_into().unwrap()) as usize;
        input = &input[8..];
        result.push(&input[..len]);
        input = &input[len..];
    }
    assert!(input.is_empty());
    result
}
async fn update(pic: &PocketIc, id: CanisterId, lengths: [u64; 2], refused: bool) {
    let reply = pic
        .update_call(id, id, "write", encode(&[lengths[0], lengths[1], 0]))
        .await
        .unwrap();
    assert_eq!(reply.len(), 16);
    assert_eq!(&reply[..8], &u64::from(refused).to_le_bytes());
    println!(
        "record write lengths={lengths:?} refused={refused} body_instructions={}",
        u64::from_le_bytes(reply[8..].try_into().unwrap())
    );
}

#[tokio::test(flavor = "current_thread")]
async fn public_records_admit_once_commit_synchronously_and_roll_back_installed_failures() {
    let a = artifact("a");
    let b = artifact("b");
    assert_ne!(a, b);
    let server = std::env::var("IC_TESTKIT_POCKET_IC_URL").expect("prepared testkit server");
    let pic = PocketIcBuilder::new()
        .with_application_subnet()
        .with_server_url(server.parse().unwrap())
        .with_max_request_time_ms(Some(30_000))
        .build_async()
        .await;
    let id = pic.create_canister().await;
    pic.install_canister(id, a.clone(), vec![], None).await;
    let initial = state(&pic, id).await;
    assert_eq!(values(&initial), [b"intent".as_slice(), b"custody", b""]);
    assert_eq!(pic.get_stable_memory(id).await.len(), 17 * 65536);
    update(&pic, id, [16_384, 2 * 524_288 + 24], false).await;
    let full = state(&pic, id).await;
    let v = values(&full);
    assert_eq!(v[0], vec![7; 16_384]);
    assert_eq!(v[1], vec![9; 2 * 524_288 + 24]);
    assert_eq!(v[2], b"validated worker binding");
    for fault in 1..=4 {
        let before = state(&pic, id).await;
        let stable = pic.get_stable_memory(id).await;
        let error = pic
            .update_call(id, id, "write", encode(&[19, 29, fault]))
            .await
            .expect_err("late failure");
        let expected = [
            "after first record",
            "after second record",
            "after participant record",
            "reply encoder refuses",
        ][(fault - 1) as usize];
        assert!(error.reject_message.contains(expected), "{error:?}");
        assert_eq!(state(&pic, id).await, before);
        assert_eq!(pic.get_stable_memory(id).await, stable);
    }
    let stable = pic.get_stable_memory(id).await;
    let speculative = pic.query_call(id, id, "speculative", vec![]).await.unwrap();
    assert_eq!(values(&speculative)[0].len(), 31);
    assert_eq!(state(&pic, id).await, full);
    assert_eq!(pic.get_stable_memory(id).await, stable);
    // First-record refusal writes nothing; ordinary late Err preserves prior writes.
    update(&pic, id, [16_385, 0], true).await;
    assert_eq!(state(&pic, id).await, full);
    update(&pic, id, [23, 2 * 524_288 + 25], true).await;
    let partial = state(&pic, id).await;
    assert_eq!(values(&partial)[0], vec![7; 23]);
    assert_eq!(values(&partial)[1], v[1]);
    // Ordinary upgrade discards every Rust header; no pre-upgrade flush is exported.
    pic.stop_canister(id, None).await.unwrap();
    pic.upgrade_canister(id, b.clone(), encode(&[1, 0]), None)
        .await
        .unwrap();
    pic.start_canister(id, None).await.unwrap();
    let upgraded = state(&pic, id).await;
    assert_eq!(&upgraded[..8], &202u64.to_le_bytes());
    assert_eq!(values(&upgraded), values(&partial));
    for args in [[2, 0], [1, 1]] {
        let stable = pic.get_stable_memory(id).await;
        pic.stop_canister(id, None).await.unwrap();
        pic.upgrade_canister(id, a.clone(), encode(&args), None)
            .await
            .expect_err("target refusal");
        assert_eq!(
            pic.canister_status(id, None)
                .await
                .unwrap()
                .module_hash
                .unwrap(),
            sha256(&b)
        );
        assert_eq!(pic.get_stable_memory(id).await, stable);
        pic.start_canister(id, None).await.unwrap();
        assert_eq!(state(&pic, id).await, upgraded);
    }
    // External corruption is test orchestration, never an application memory API.
    let good = pic.get_stable_memory(id).await;
    for bad in [Vec::new(), {
        let mut bytes = good.clone();
        bytes[160] ^= 1;
        bytes
    }] {
        pic.stop_canister(id, None).await.unwrap();
        pic.set_stable_memory(id, bad.clone(), BlobCompression::NoCompression)
            .await;
        pic.upgrade_canister(id, a.clone(), encode(&[1, 0]), None)
            .await
            .expect_err("no reset");
        assert_eq!(pic.get_stable_memory(id).await, bad);
        pic.set_stable_memory(id, good.clone(), BlobCompression::NoCompression)
            .await; // explicit fixture restoration
        pic.start_canister(id, None).await.unwrap();
        assert_eq!(state(&pic, id).await, upgraded);
    }
    update(&pic, id, [37, 47], false).await;
    pic.stop_canister(id, None).await.unwrap();
    pic.upgrade_canister(id, a.clone(), encode(&[1, 0]), None)
        .await
        .unwrap();
    pic.start_canister(id, None).await.unwrap();
    assert_eq!(values(&state(&pic, id).await)[0], vec![7; 37]);
    assert_eq!(pic.get_stable_memory(id).await.len(), 17 * 65536);
    pic.drop().await;
}

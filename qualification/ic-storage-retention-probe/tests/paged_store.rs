//! Installed governed page publication/rollback, with ordinary heap replacement.
//! The probe reconstructs transient maps; this is not retained native recovery.
#![forbid(unsafe_code)]
use ic_storage_retention_probe::artifact::{sha256, sha256_hex, validate_wasm};
use ic_testkit::pocket_ic::{
    CanisterId, ErrorCode, PocketIcBuilder, common::rest::BlobCompression, nonblocking::PocketIc,
};
use std::{fs, path::PathBuf};
use wasmparser::{Parser, Payload};

fn input(name: &str) -> Vec<u8> {
    let directory =
        PathBuf::from(std::env::var("PAGED_STORE_INPUTS").expect("make paged-store-test"));
    let bytes = ic_host_artifacts::read(
        fs::File::open(directory.join(format!("{name}.wasm"))).unwrap(),
        ic_host_artifacts::Length::AtMost(1024 * 1024),
    )
    .unwrap()
    .0;
    validate_wasm(&bytes, 1024 * 1024).unwrap();
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
                    assert!(matches!(import.ty, wasmparser::TypeRef::Func(_)));
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
            _ => {}
        }
    }
    exports.sort_unstable();
    assert_eq!(memories, 1);
    assert_eq!(
        exports,
        [
            "canister_init",
            "canister_post_upgrade",
            "canister_query page",
            "canister_query state",
            "canister_update advance",
            "canister_update begin",
            "canister_update damage",
            "canister_update publish"
        ]
    );
    println!(
        "paged store {name}: {} bytes, sha256={}",
        bytes.len(),
        sha256_hex(&bytes)
    );
    bytes
}
fn args(words: [u64; 3]) -> Vec<u8> {
    words.into_iter().flat_map(u64::to_le_bytes).collect()
}
fn word(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap())
}
async fn server() -> PocketIc {
    PocketIcBuilder::new()
        .with_application_subnet()
        .with_max_request_time_ms(Some(30_000))
        .with_server_url(
            std::env::var("IC_TESTKIT_POCKET_IC_URL")
                .unwrap()
                .parse()
                .unwrap(),
        )
        .build_async()
        .await
}
async fn install(pic: &PocketIc, wasm: &[u8]) -> CanisterId {
    let id = pic.create_canister().await;
    pic.install_canister(id, wasm.to_vec(), vec![], None).await;
    pic.set_controllers(id, None, vec![id, CanisterId::anonymous()])
        .await
        .unwrap();
    id
}
async fn state(pic: &PocketIc, id: CanisterId) -> Vec<u8> {
    pic.query_call(id, id, "state", vec![]).await.unwrap()
}
async fn update(pic: &PocketIc, id: CanisterId, method: &str, words: [u64; 3]) {
    assert!(
        pic.update_call(id, id, method, args(words))
            .await
            .unwrap()
            .is_empty()
    );
}
async fn rejected(
    pic: &PocketIc,
    id: CanisterId,
    method: &str,
    words: [u64; 3],
    after_write: bool,
) {
    let before = state(pic, id).await;
    let stable = pic.get_stable_memory(id).await;
    let error = pic
        .update_call(id, id, method, args(words))
        .await
        .unwrap_err();
    assert!(
        matches!(
            error.error_code,
            ErrorCode::CanisterTrapped | ErrorCode::CanisterCalledTrap
        ),
        "{error:?}"
    );
    if after_write {
        assert!(
            error
                .reject_message
                .contains("paged store after governed write"),
            "{error:?}"
        );
    }
    assert_eq!(state(pic, id).await, before);
    assert_eq!(pic.get_stable_memory(id).await, stable);
}
async fn rejected_upgrade(
    pic: &PocketIc,
    id: CanisterId,
    wasm: &[u8],
    words: Vec<u8>,
    after_write: bool,
) {
    let before = state(pic, id).await;
    let stable = pic.get_stable_memory(id).await;
    pic.stop_canister(id, None).await.unwrap();
    let hash = pic.canister_status(id, None).await.unwrap().module_hash;
    let error = pic
        .upgrade_canister(id, wasm.to_vec(), words, None)
        .await
        .unwrap_err();
    assert!(
        matches!(
            error.error_code,
            ErrorCode::CanisterTrapped | ErrorCode::CanisterCalledTrap
        ),
        "{error:?}"
    );
    if after_write {
        assert!(
            error
                .reject_message
                .contains("paged store after governed write"),
            "{error:?}"
        );
    }
    assert_eq!(
        pic.canister_status(id, None).await.unwrap().module_hash,
        hash
    );
    assert_eq!(pic.get_stable_memory(id).await, stable);
    pic.start_canister(id, None).await.unwrap();
    assert_eq!(state(pic, id).await, before);
}
async fn upgrade(pic: &PocketIc, id: CanisterId, wasm: &[u8]) {
    let before = state(pic, id).await;
    let stable = pic.get_stable_memory(id).await;
    pic.stop_canister(id, None).await.unwrap();
    pic.upgrade_canister(id, wasm.to_vec(), vec![], None)
        .await
        .unwrap();
    assert_eq!(
        pic.canister_status(id, None)
            .await
            .unwrap()
            .module_hash
            .unwrap(),
        sha256(wasm)
    );
    pic.start_canister(id, None).await.unwrap();
    let after = state(pic, id).await;
    assert_ne!(&after[..8], &before[..8]);
    assert_eq!(&after[8..], &before[8..]);
    assert_eq!(pic.get_stable_memory(id).await, stable);
}

// Independent byte checks; never use the fixture's encoder/decoder as the oracle.
fn check_state(bytes: &[u8], phase: u16, next: u32, pages: u64) {
    assert_eq!(bytes.len(), 960);
    let metadata = &bytes[8..112];
    let manifest = &bytes[112..440];
    assert_eq!(&metadata[..8], b"ICJPG001");
    assert_eq!(&metadata[10..12], &phase.to_le_bytes());
    assert_eq!(word(metadata, 16), 1);
    assert_eq!((word(metadata, 24), word(metadata, 32)), (131072, 393216));
    assert_eq!(&metadata[44..48], &next.to_le_bytes());
    assert_eq!(&manifest[..8], b"NKPPAIR1");
    assert_eq!(&manifest[288..296], b"PAIRDONE");
    assert_eq!(&manifest[296..], sha256(&manifest[..296]));
    let mut bound = metadata[..64].to_vec();
    bound.extend_from_slice(manifest);
    bound.extend_from_slice(b"JOBDONE1");
    assert_eq!(&metadata[64..72], b"JOBDONE1");
    assert_eq!(&metadata[72..], sha256(&bound));
    assert_eq!(word(bytes, 952), pages);
    if phase == 1 {
        assert_eq!(&bytes[440..952], &[0; 512]);
    } else {
        let continuation = &bytes[440..952];
        assert_eq!(&continuation[..8], b"NKPCNT01");
        assert_eq!(&continuation[16..120], metadata);
        assert_eq!(&continuation[480..], sha256(&continuation[..480]));
    }
}
async fn check_pages(pic: &PocketIc, id: CanisterId, counts: [u64; 2]) {
    let bytes = state(pic, id).await;
    for (root, count) in counts.into_iter().enumerate() {
        let manifest = &bytes[144 + root * 128..272 + root * 128];
        assert_eq!(&manifest[..8], b"NKPAGED1");
        assert_eq!(&manifest[96..], sha256(&manifest[..96]));
        assert_eq!(
            (word(manifest, 16), word(manifest, 24), word(manifest, 32)),
            (1, root as u64 + 1, count)
        );
        let mut expected = manifest[56..88].to_vec();
        for index in 0..count.div_ceil(64) {
            let page = pic
                .query_call(id, id, "page", args([root as u64, index, 0]))
                .await
                .unwrap();
            assert_eq!(page.len(), 1104);
            let mut hashed = b"NKPAGEH1".to_vec();
            hashed.extend_from_slice(&page);
            assert_eq!(sha256(&hashed).as_slice(), expected);
            assert_eq!(&page[..8], b"NKPAGE01");
            assert_eq!(
                (word(&page, 8), word(&page, 16), word(&page, 24)),
                (1, root as u64 + 1, index)
            );
            let length = (count - index * 64).min(64);
            assert_eq!(word(&page, 32), length);
            for record in 0..length {
                let key = u64::MAX - count + 1 + index * 64 + record;
                let at = 80 + record as usize * 16;
                assert_eq!(
                    (word(&page, at), word(&page, at + 8)),
                    (key, (u64::MAX - key) ^ count)
                );
            }
            expected = page[48..80].to_vec();
        }
        assert_eq!(expected, [0; 32]);
    }
}

#[tokio::test(flavor = "current_thread")]
async fn governed_pages_and_progress_survive_reopening_and_atomic_rollback() {
    let a = input("a");
    let b = input("b");
    assert_ne!(a, b);
    let pic = server().await;
    for counts in [[0u64, 0], [0, 65], [65, 0], [129, 65], [2304, 2304]] {
        let id = install(&pic, &a).await;
        rejected_upgrade(&pic, id, &b, vec![], false).await; // unsealed namespace
        if counts == [129, 65] {
            for fault in [1, 5, 2, 4] {
                rejected(&pic, id, "publish", [counts[0], counts[1], fault], true).await;
            }
            pic.set_controllers(id, None, vec![CanisterId::anonymous()])
                .await
                .unwrap();
            rejected(&pic, id, "publish", [counts[0], counts[1], 0], false).await;
            pic.set_controllers(id, None, vec![id, CanisterId::anonymous()])
                .await
                .unwrap();
        }
        update(&pic, id, "publish", [counts[0], counts[1], 0]).await;
        let pages = counts
            .into_iter()
            .map(|count| count.div_ceil(64))
            .sum::<u64>();
        check_state(&state(&pic, id).await, 1, 0, pages);
        check_pages(&pic, id, counts).await;
        rejected(&pic, id, "publish", [counts[0], counts[1], 0], false).await;
        upgrade(&pic, id, &b).await;
        for fault in [3, 4] {
            rejected(&pic, id, "begin", [fault, 0, 0], true).await;
        }
        update(&pic, id, "begin", [0, 0, 0]).await;
        check_state(&state(&pic, id).await, 4, 0, pages);
        upgrade(&pic, id, &a).await;
        let mut next = 0;
        loop {
            let limit = if pages > 5 { 32 } else { 2 };
            for fault in [3, 4] {
                rejected(&pic, id, "advance", [next, limit, fault], true).await;
                if counts == [129, 65] {
                    rejected_upgrade(&pic, id, &b, args([next, limit, fault]), true).await;
                }
            }
            rejected(&pic, id, "advance", [next + 1, limit, 0], false).await;
            update(&pic, id, "advance", [next, limit, 0]).await;
            let end = (next + limit).min(pages);
            check_state(
                &state(&pic, id).await,
                if end == pages { 2 } else { 4 },
                end as u32,
                pages,
            );
            let stable = pic.get_stable_memory(id).await;
            let before = state(&pic, id).await;
            update(&pic, id, "advance", [next, limit, 4]).await; // retry never writes/traps
            assert_eq!(state(&pic, id).await, before);
            assert_eq!(pic.get_stable_memory(id).await, stable);
            upgrade(&pic, id, &b).await;
            upgrade(&pic, id, &a).await;
            if end == pages {
                break;
            }
            next = end;
        }
        check_pages(&pic, id, counts).await;
    }
    pic.drop().await;
    fs::write(
        PathBuf::from(std::env::var("PAGED_STORE_INPUTS").unwrap()).join("TRANSACTIONS"),
        "passed\n",
    )
    .unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn corrupt_storage_refuses_without_replacement_writes() {
    let a = input("a");
    let b = input("b");
    let pic = server().await;
    for part in 0..4 {
        let id = install(&pic, &a).await;
        update(&pic, id, "publish", [129, 65, 0]).await;
        update(&pic, id, "begin", [0, 0, 0]).await;
        update(&pic, id, "advance", [0, 2, 0]).await;
        update(&pic, id, "damage", [part, 0, 0]).await;
        rejected(&pic, id, "advance", [2, 2, 0], false).await;
        if part != 3 {
            rejected_upgrade(&pic, id, &b, vec![], false).await;
        }
    }
    // Infrastructure removal must not turn reopening into fresh initialization.
    let id = install(&pic, &a).await;
    update(&pic, id, "publish", [129, 65, 0]).await;
    pic.set_stable_memory(id, vec![], BlobCompression::NoCompression)
        .await;
    let stable = pic.get_stable_memory(id).await;
    pic.stop_canister(id, None).await.unwrap();
    let old_hash = pic.canister_status(id, None).await.unwrap().module_hash;
    let error = pic.upgrade_canister(id, b, vec![], None).await.unwrap_err();
    assert_eq!(error.error_code, ErrorCode::CanisterTrapped);
    assert_eq!(
        pic.canister_status(id, None).await.unwrap().module_hash,
        old_hash
    );
    assert_eq!(pic.get_stable_memory(id).await, stable);
    pic.drop().await;
    fs::write(
        PathBuf::from(std::env::var("PAGED_STORE_INPUTS").unwrap()).join("REFUSALS"),
        "passed\n",
    )
    .unwrap();
}

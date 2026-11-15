//! Installed custody protocol only; no Rust code executes in these canisters.
#![forbid(unsafe_code)]

use std::time::Duration;

use ic_host_artifacts::artifact::Sha256Digest;
use ic_testkit::pic::PocketIcStartupConfig;
use ic_testkit::pocket_ic::{CanisterId, ErrorCode, PocketIcBuilder, nonblocking::PocketIc};
use wasmparser::{Operator, Parser, Payload, TypeRef};

use super::{module, stopped_hash};

const SOURCE: &str = include_str!("provider.wat");

fn variant_source(version: u64, extent: u64, trap: bool) -> String {
    SOURCE
        .replace("__VERSION__", &version.to_string())
        .replace("__EXTENT__", &extent.to_string())
        .replace("__TRAP__", if trap { "1" } else { "0" })
}

fn variant(version: u64, extent: u64, trap: bool) -> Vec<u8> {
    let bytes = module(&variant_source(version, extent, trap));
    admit(&bytes).expect("fixed-domain provider protocol");
    bytes
}

fn record(label: &str, bytes: &[u8]) {
    let digest = Sha256Digest::compute(bytes);
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/evidence/provider-custody");
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join(format!("{digest}.wasm"));
    if path.exists() {
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
    } else {
        std::fs::write(&path, bytes).unwrap();
    }
    println!(
        "provider custody {label}: sha256={digest} bytes={}",
        bytes.len()
    );
}

// Structural constraints on our trusted WAT fixture, not a validator for hostile
// code or the final Rust/provider artifact. Source review owns store addresses.
fn admit(bytes: &[u8]) -> Result<(), &'static str> {
    let mut memories = 0;
    let mut imports = Vec::new();
    for payload in Parser::new(0).parse_all(bytes) {
        match payload.map_err(|_| "parse")? {
            Payload::MemorySection(reader) => {
                for memory in reader {
                    let memory = memory.map_err(|_| "memory")?;
                    memories += 1;
                    if memory.memory64
                        || memory.shared
                        || memory.initial != 3
                        || memory.maximum != Some(3)
                        || memory.page_size_log2.is_some()
                    {
                        return Err("memory geometry");
                    }
                }
            }
            Payload::ImportSection(reader) => {
                for import in reader.into_imports() {
                    let import = import.map_err(|_| "import")?;
                    if import.module != "ic0" || !matches!(import.ty, TypeRef::Func(_)) {
                        return Err("import");
                    }
                    imports.push(import.name);
                }
            }
            Payload::StartSection { .. } | Payload::DataSection(_) | Payload::TableSection(_) => {
                return Err("initialization or table");
            }
            Payload::CodeSectionEntry(body) => {
                for op in body.get_operators_reader().map_err(|_| "body")? {
                    if matches!(op.map_err(|_| "operator")?, Operator::MemoryGrow { .. }) {
                        return Err("growth");
                    }
                }
            }
            _ => {}
        }
    }
    if memories != 1 || imports != ["msg_reply_data_append", "msg_reply"] {
        return Err("memory or import roster");
    }
    Ok(())
}

async fn observe(pic: &PocketIc, id: CanisterId) -> [u64; 5] {
    let bytes = pic.query_call(id, id, "observe", vec![]).await.unwrap();
    assert_eq!(bytes.len(), 40);
    std::array::from_fn(|i| u64::from_le_bytes(bytes[i * 8..(i + 1) * 8].try_into().unwrap()))
}

async fn update(pic: &PocketIc, id: CanisterId, method: &str) {
    assert!(
        pic.update_call(id, id, method, vec![])
            .await
            .unwrap()
            .is_empty()
    );
}

async fn rejected_update(pic: &PocketIc, id: CanisterId, method: &str) {
    let before = observe(pic, id).await;
    let error = pic.update_call(id, id, method, vec![]).await.unwrap_err();
    assert_eq!(error.error_code, ErrorCode::CanisterTrapped);
    assert_eq!(observe(pic, id).await, before);
}

async fn rejected_upgrade(pic: &PocketIc, id: CanisterId, target: Vec<u8>) {
    let before = observe(pic, id).await;
    let hash = stopped_hash(pic, id).await;
    let error = pic
        .upgrade_eop_canister(id, target, vec![], None)
        .await
        .unwrap_err();
    assert_eq!(error.error_code, ErrorCode::CanisterTrapped);
    assert_eq!(stopped_hash(pic, id).await, hash);
    pic.start_canister(id, None).await.unwrap();
    assert_eq!(observe(pic, id).await, before);
}

#[test]
fn fixed_domain_artifacts_reject_growth_initialization_and_foreign_geometry() {
    variant(1, 4096, false);
    let base = variant_source(1, 4096, false);
    for (from, to, error) in [
        ("3 3)", "3 4)", "memory geometry"),
        (
            "(call $settle)",
            "(drop (memory.grow (i32.const 0)))",
            "growth",
        ),
        (
            "(global $version",
            "(data (i32.const 131072) \"bad\") (global $version",
            "initialization or table",
        ),
        (
            "\"ic0\" \"msg_reply\"",
            "\"unexpected\" \"msg_reply\"",
            "import",
        ),
    ] {
        assert_eq!(base.matches(from).count(), 1);
        assert_eq!(admit(&module(&base.replace(from, to))), Err(error));
    }
}

#[tokio::test(flavor = "current_thread")]
async fn installed_claims_require_settlement_and_refusals_preserve_custody() {
    for (label, bytes) in [
        ("A", variant(1, 4096, false)),
        ("B", variant(2, 4096, false)),
        ("incompatible", variant(2, 8192, false)),
        ("trapping", variant(2, 4096, true)),
    ] {
        record(label, &bytes);
    }
    let config = PocketIcStartupConfig::from_env(Duration::from_secs(30))
        .expect("prepare the server explicitly with make retention-setup");
    let owner = config
        .build_instance(
            PocketIcBuilder::new()
                .with_max_request_time_ms(Some(30_000))
                .with_application_subnet(),
        )
        .await
        .unwrap();
    let pic = owner.instance();
    let id = pic.create_canister().await;
    pic.install_canister(id, variant(1, 4096, false), vec![], None)
        .await;
    assert_eq!(observe(pic, id).await, [1, 1, 1, 17, 4096]);
    update(pic, id, "advance").await;
    assert_eq!(observe(pic, id).await, [1, 1, 1, 18, 4096]);
    rejected_update(pic, id, "duplicate_claim").await;
    rejected_update(pic, id, "trap_after_write").await;
    rejected_upgrade(pic, id, variant(2, 8192, false)).await;
    rejected_upgrade(pic, id, variant(2, 4096, true)).await;

    let a_hash = stopped_hash(pic, id).await;
    pic.upgrade_eop_canister(id, variant(2, 4096, false), vec![], None)
        .await
        .unwrap();
    assert_ne!(stopped_hash(pic, id).await, a_hash);
    pic.start_canister(id, None).await.unwrap();
    assert_eq!(observe(pic, id).await, [2, 1, 1, 18, 4096]);
    update(pic, id, "advance").await;
    stopped_hash(pic, id).await;
    pic.upgrade_eop_canister(id, variant(1, 4096, false), vec![], None)
        .await
        .unwrap();
    assert_eq!(stopped_hash(pic, id).await, a_hash);
    pic.start_canister(id, None).await.unwrap();
    assert_eq!(observe(pic, id).await, [1, 1, 1, 19, 4096]);

    update(pic, id, "abandon_claim").await;
    assert_eq!(observe(pic, id).await, [1, 2, 0, 19, 4096]);
    rejected_update(pic, id, "advance").await;
    rejected_upgrade(pic, id, variant(2, 4096, false)).await;

    // A destination cannot rely only on the source's pre-upgrade hook. This
    // deliberately deficient source has no such export, yet commits a claim.
    let no_guard =
        module(&variant_source(1, 4096, false).replace("(export \"canister_pre_upgrade\")", ""));
    admit(&no_guard).unwrap();
    record("source without pre-upgrade guard", &no_guard);
    let unsealed = pic.create_canister().await;
    pic.install_canister(unsealed, no_guard, vec![], None).await;
    update(pic, unsealed, "abandon_claim").await;
    assert_eq!(observe(pic, unsealed).await, [1, 2, 0, 17, 4096]);
    rejected_upgrade(pic, unsealed, variant(2, 4096, false)).await;

    let corrupt = pic.create_canister().await;
    pic.install_canister(corrupt, variant(1, 4096, false), vec![], None)
        .await;
    update(pic, corrupt, "corrupt_extent").await;
    assert_eq!(observe(pic, corrupt).await, [1, 1, 1, 17, 0]);
    rejected_update(pic, corrupt, "advance").await;
    rejected_upgrade(pic, corrupt, variant(2, 4096, false)).await;
    owner.close().await;
}

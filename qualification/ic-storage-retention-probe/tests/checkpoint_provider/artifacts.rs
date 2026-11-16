//! Bounded artifact validation for ordinary (heap-discarding) upgrades.

use ic_storage_retention_probe::artifact::{sha256_hex, validate_wasm};
use std::{fs, path::PathBuf};

use wasmparser::{Parser, Payload};

pub(super) fn input(variant: &str) -> Vec<u8> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let path = root.join(format!("target/qualification/checkpoint-{variant}/wasm32-unknown-unknown/release/ic_storage_checkpoint_provider_probe.wasm"));
    let bytes = ic_host_artifacts::read(
        fs::File::open(path).expect("make checkpoint-provider-build"),
        ic_host_artifacts::Length::AtMost((1024 * 1024) as u64),
    )
    .expect("bounded artifact")
    .0;
    validate_wasm(&bytes, 1024 * 1024).expect("valid Wasm");
    let mut memories = 0;
    let mut exports = Vec::new();
    for payload in Parser::new(0).parse_all(&bytes) {
        match payload.expect("valid payload") {
            Payload::MemorySection(reader) => {
                for memory in reader {
                    let memory = memory.expect("memory");
                    assert!(!memory.memory64 && !memory.shared);
                    assert_eq!(memory.initial, 32);
                    assert_eq!(memory.maximum, Some(256));
                    memories += 1;
                }
            }
            Payload::ImportSection(reader) => {
                for import in reader.into_imports() {
                    let import = import.expect("import");
                    assert_eq!(import.module, "ic0");
                    assert!(matches!(import.ty, wasmparser::TypeRef::Func(_)));
                }
            }
            Payload::ExportSection(reader) => {
                for export in reader {
                    let export = export.expect("export");
                    if export.name.starts_with("canister_") {
                        exports.push(export.name);
                    }
                }
            }
            Payload::CustomSection(section) => {
                assert!(!section.name().contains("enhanced-orthogonal-persistence"))
            }
            Payload::StartSection { .. } => panic!("ordinary fixture has no Wasm start section"),
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
            "canister_query checkpoint_bytes",
            "canister_query state",
            "canister_update append",
            "canister_update damage_checkpoint",
            "canister_update export_checkpoint",
            "canister_update fill",
            "canister_update pop"
        ]
    );
    let evidence = root.join("target/evidence/checkpoint-provider");
    fs::create_dir_all(&evidence).expect("evidence directory");
    let digest = sha256_hex(&bytes);
    fs::write(evidence.join(format!("{digest}.wasm")), &bytes).expect("artifact evidence");
    let report = format!(
        "ordinary-upgrade fixture {variant}\nsha256={}\nbytes={}\n",
        sha256_hex(&bytes),
        bytes.len()
    );
    fs::write(evidence.join(format!("{digest}.txt")), &report).expect("artifact identity");
    println!("{report}");
    bytes
}

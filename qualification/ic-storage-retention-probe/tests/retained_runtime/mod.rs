//! Public runtime artifact and installed adoption witness.
#![forbid(unsafe_code)]
use ic_storage::retained_build::{admit, prepare};
use ic_storage_retention_probe::artifact::sha256_hex;
use std::{fs, path::PathBuf};
mod installed_tests;
mod negative;
fn directory() -> PathBuf {
    PathBuf::from(std::env::var_os("RETAINED_RUNTIME_INPUTS").unwrap())
}
fn artifact(name: &str) -> Vec<u8> {
    let raw = fs::read(directory().join(format!("{name}.wasm"))).unwrap();
    let bytes = prepare(&raw).unwrap();
    admit(&bytes, &raw).unwrap();
    fs::write(directory().join(format!("{name}-installed.wasm")), &bytes).unwrap();
    println!(
        "retained {name} bytes={} sha256={}",
        bytes.len(),
        sha256_hex(&bytes)
    );
    bytes
}
#[test]
fn artifact_contract() {
    for name in ["a", "b", "no-close", "fail-close"] {
        let bytes = artifact(name);
        assert!(prepare(&bytes).is_err());
        let raw = fs::read(directory().join(format!("{name}.wasm"))).unwrap();
        let mut bad = bytes;
        *bad.last_mut().unwrap() ^= 1;
        assert!(admit(&bad, &raw).is_err());
    }
    let old = wat::parse_str("(module (memory 640 2048))").unwrap();
    assert!(prepare(&old).is_err());
    fs::write(
        directory().join("ADMITTED"),
        "public retained artifact contract\n",
    )
    .unwrap();
}

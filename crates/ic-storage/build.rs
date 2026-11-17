//! Retained native custody is qualified for exactly one compiler contract.
use std::{env, process::Command};
fn main() {
    println!("cargo:rerun-if-env-changed=RUSTC");
    if env::var_os("CARGO_FEATURE_RETAINED").is_none() {
        return;
    }
    let compiler = env::var_os("RUSTC").expect("Cargo supplies RUSTC");
    let output = Command::new(compiler)
        .arg("--version")
        .output()
        .expect("read compiler identity");
    assert!(output.status.success(), "compiler identity refused");
    let version = std::str::from_utf8(&output.stdout).expect("compiler identity must be UTF-8");
    assert_eq!(
        version.split_whitespace().nth(1),
        Some("1.100.0"),
        "retained runtime supports only the qualified Rust 1.100.0 contract; incompatible compilers require portable recovery qualification"
    );
}

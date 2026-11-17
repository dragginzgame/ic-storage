#!/usr/bin/env bash
# Capture the built ordinary probes and exercise the shared governed page store.
set -euo pipefail
[[ $# == 0 ]] || { echo 'usage: run-paged-store-probe.sh' >&2; exit 1; }
root="$(CDPATH='' cd -- "$(dirname -- "$0")/../../.." && pwd)"
cd "$root"
export CARGO_NET_OFFLINE=true RUSTUP_AUTO_INSTALL=0
mkdir -p target/evidence/paged-store
output="$(mktemp -d "$root/target/evidence/paged-store/attempt.XXXXXX")"
printf 'Paged store diagnostics: %s\n' "$output"
rustc --version --verbose > "$output/compiler.txt"
cp "$0" Cargo.toml Cargo.lock rust-toolchain.toml Makefile "$output/"
mkdir "$output/qualification-workspace"
cp qualification/Cargo.toml qualification/Cargo.lock "$output/qualification-workspace/"
cp -R crates/ic-storage "$output/native-core"
cp -R qualification/ic-storage-native-feasibility/src "$output/feasibility-source"
cp qualification/ic-storage-native-feasibility/Cargo.toml "$output/feasibility-source/"
cp -R qualification/ic-storage-checkpoint-provider-probe "$output/checkpoint-source"
cp qualification/ic-storage-retention-probe/tests/paged_store.rs "$output/"
for variant in a b; do
    cp "target/qualification/paged-store-$variant/wasm32-unknown-unknown/release/ic_storage_checkpoint_provider_probe.wasm" "$output/$variant.wasm"
done
PAGED_STORE_INPUTS="$output" cargo test --manifest-path qualification/Cargo.toml \
    -p ic-storage-retention-probe --target-dir target/qualification --locked --offline \
    --test paged_store -- --nocapture 2>&1 | tee "$output/installed.log"
test -f "$output/TRANSACTIONS"
test -f "$output/REFUSALS"
printf 'Governed paged storage transactions passed.\n' > "$output/COMPLETE"

#!/usr/bin/env bash
# The private graph owns native runners and separately selected canister backends.
set -euo pipefail
[[ $# == 1 && -n "$1" ]] || { echo 'usage: check-qualification-lints.sh TOOLCHAIN' >&2; exit 2; }
export RUSTUP_TOOLCHAIN="$1" RUSTUP_AUTO_INSTALL=0 CARGO_NET_OFFLINE=true
common=(--manifest-path qualification/Cargo.toml --locked --offline)
cargo clippy "${common[@]}" --workspace --all-targets \
    --features ic-storage-benchmarks/owned,ic-storage-benchmarks/dfinity,ic-storage-benchmarks/metrics-checkpoints -- -D warnings
wasm=(--target wasm32-unknown-unknown)
cargo clippy "${common[@]}" "${wasm[@]}" --workspace --lib \
    --features ic-storage-benchmarks/owned -- -D warnings
cargo clippy "${common[@]}" "${wasm[@]}" -p ic-storage-test-fixtures --examples -- -D warnings
for features in owned dfinity dfinity-governed dfinity-head owned,returning-mutations owned,footprint dfinity,footprint; do
    cargo clippy "${common[@]}" "${wasm[@]}" -p ic-storage-benchmarks \
        --example storage-comparison-wasm --features "$features" -- -D warnings
done
cargo clippy "${common[@]}" "${wasm[@]}" -p ic-storage-benchmarks \
    --example admission-wasm --example metrics-checkpoints-wasm --features metrics-checkpoints -- -D warnings
for package in storage-size-consumer icss-size-consumer; do
    for features in '' map set vector cell region map,region map,set,vector,cell,region; do
        cargo clippy "${common[@]}" "${wasm[@]}" -p "$package" --lib \
            --no-default-features --features "$features" -- -D warnings
    done
done
cargo clippy "${common[@]}" "${wasm[@]}" -p storage-size-consumer --lib \
    --features discard-mutations -- -D warnings

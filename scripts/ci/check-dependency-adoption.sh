#!/usr/bin/env bash
# Read-only destination admission before release planning or artifact work.
set -euo pipefail
[[ $# == 1 && -n "$1" ]] || { echo 'usage: check-dependency-adoption.sh TOOLCHAIN' >&2; exit 2; }
export RUSTUP_TOOLCHAIN="$1" RUSTUP_AUTO_INSTALL=0 CARGO_NET_OFFLINE=true
shared=vendor/toko-tooling/scripts/ci
mkdir -p "$PWD/target/dependency-adoption"
evidence="$(mktemp -d "$PWD/target/dependency-adoption/attempt.XXXXXX")"
printf 'Dependency adoption evidence: %s\n' "$evidence"
inputs=(Cargo.toml Cargo.lock crates/*/Cargo.toml qualification/Cargo.toml \
    qualification/Cargo.lock qualification/*/Cargo.toml)
checkpoint() {
    perl "$shared/validation-input-checkpoint.pl" "$PWD" "$1" "$2" "$shared/verify-file-checksum.sh" "${inputs[@]}"
}
checkpoint '' "$evidence/before.json"
host="$(rustc -vV | sed -n 's/^host: //p')"
[[ "$host" =~ ^[A-Za-z0-9_.-]+$ ]] || { echo 'invalid native compiler target' >&2; exit 1; }
# Observe every independent lock before policy or compilation. Never unlock,
# update a version, refresh artifacts or authenticate plausible failed stdout.
cargo metadata --locked --offline --format-version 1 --manifest-path Cargo.toml > "$evidence/library.json"
cargo metadata --locked --offline --format-version 1 --manifest-path qualification/Cargo.toml > "$evidence/runtime.json"
perl "$shared/check-cargo-graph.pl" --consumer "$PWD" \
    --package ic-storage --package ic-metrics-storage --package ic-logging-storage \
    --target wasm32-unknown-unknown --edges normal --explain \
    --forbid pocket-ic --forbid-feature ic-storage/test-support \
    --require ic-storage=1 --forbid ic-platform --require ic-platform-runtime=1 --require ic-wire=1 \
    --forbid-feature ic-platform-runtime/runtime --forbid-feature ic-platform-runtime/executor --forbid-feature ic-platform-runtime/cdk \
    --forbid ic-platform-runtime-guard --forbid ic-platform-types --forbid candid --forbid-pattern '^ic-cdk'
perl "$shared/check-cargo-graph.pl" --consumer "$PWD" \
    --package ic-storage --package ic-metrics-storage --package ic-logging-storage \
    --target "$host" --edges normal --explain \
    --require ic-wire=1 --forbid-feature ic-metrics/native-wire \
    --forbid ic-platform --forbid ic-platform-runtime --forbid ic-platform-types --forbid ic-platform-runtime-guard
# The native qualifier exchanges Testkit/Wire types and opaque Wasm bytes;
# it deliberately has no normal dependency on the local Memory/Storage crates.
perl "$shared/check-cargo-graph.pl" --consumer "$PWD/qualification" \
    --package ic-storage-runtime-qualification --target "$host" --edges normal --explain \
    --require ic-testkit=1 --require ic-wire=1
# Observe default Storage independently: selecting a runtime-enabled root alongside
# it would unify features and conceal the production boundary.
for target in "$host" wasm32-unknown-unknown; do
    runtime_policy=(--forbid ic-platform-runtime)
    if [[ "$target" == wasm32-unknown-unknown ]]; then
        runtime_policy=(--require ic-platform-runtime=1)
    fi
    perl "$shared/check-cargo-graph.pl" --consumer "$PWD/qualification" \
        --package ic-storage-default-qualification --target "$target" --edges normal --explain \
        --require ic-storage=1 "${runtime_policy[@]}" --forbid ic-platform \
        --forbid-feature ic-storage/test-support \
        --forbid-feature ic-platform-runtime/runtime --forbid-feature ic-platform-runtime/executor --forbid-feature ic-platform-runtime/cdk \
        --forbid ic-platform-runtime-guard --forbid ic-platform-types --forbid candid --forbid-pattern '^ic-cdk'
done
# A separate consumer selects the published canonical runtime as an application would.
# Keep this out of production-library graphs and the opaque-Wasm native driver.
for target in "$host" wasm32-unknown-unknown; do
    perl "$shared/check-cargo-graph.pl" --consumer "$PWD/qualification" \
        --package ic-storage-platform-qualification --target "$target" --edges normal --explain \
        --require ic-storage=1 --require ic-platform=1 --require ic-platform-runtime=1 \
        --require ic-platform-runtime-guard=1 --require ic-platform-types=1 \
        --forbid ic-stable-structures --forbid pocket-ic
done
checkpoint "$evidence/before.json" "$evidence/graphs.json"
cargo clippy --locked --offline --manifest-path Cargo.toml --target wasm32-unknown-unknown \
    --package ic-storage --package ic-metrics-storage --package ic-logging-storage --lib -- -D warnings
cargo clippy --locked --offline --manifest-path qualification/Cargo.toml \
    --package ic-storage-runtime-qualification --target "$host" --all-targets -- -D warnings
for target in "$host" wasm32-unknown-unknown; do
    cargo clippy --locked --offline --manifest-path qualification/Cargo.toml \
        --package ic-storage-default-qualification --target "$target" --all-targets -- -D warnings
    cargo clippy --locked --offline --manifest-path qualification/Cargo.toml \
        --package ic-storage-platform-qualification --target "$target" --all-targets -- -D warnings
done
checkpoint "$evidence/before.json" "$evidence/final.json"
echo 'Destination graphs and selected consumer compilation admitted; artifact/runtime/publication qualification remains separate.'

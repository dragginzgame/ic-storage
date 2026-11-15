#!/usr/bin/env bash
# Artifact checks by default; installed mode requires the admitted local server.
set -euo pipefail

mode="${1:-artifact}"
if [[ $# -gt 1 || ( "$mode" != artifact && "$mode" != installed ) ]]; then
    echo 'usage: run-provider-probe.sh [artifact|installed]' >&2
    exit 1
fi

root="$(CDPATH='' cd -- "$(dirname -- "$0")/../../.." && pwd)"
cd "$root"
export CARGO_NET_OFFLINE=true RUSTUP_AUTO_INSTALL=0 CARGO_INCREMENTAL=0
if [[ -n "${RUSTFLAGS:-}${CARGO_ENCODED_RUSTFLAGS:-}${CARGO_BUILD_TARGET:-}${RUSTC_WRAPPER:-}${RUSTC_WORKSPACE_WRAPPER:-}${RUSTC:-}${RUSTUP_TOOLCHAIN:-}${CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUSTFLAGS:-}${CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_LINKER:-}" ]]; then
    echo 'Provider diagnostics require the repository compiler and no compiler/target overrides.' >&2
    exit 1
fi

mkdir -p target/evidence/provider-link
output="$(mktemp -d "$root/target/evidence/provider-link/attempt.XXXXXX")"
printf 'Provider artifact diagnostics: %s\n' "$output"
rustc --version --verbose > "$output/compiler.txt"
cp qualification/ic-storage-native-feasibility/tests/fixtures/provider_abi.rs "$output/"
cp qualification/ic-storage-retention-probe/tests/provider_custody/provider.wat "$output/"
cp -R qualification/ic-storage-retention-probe/tests/provider_link "$output/admission-source"
cp qualification/Cargo.toml qualification/Cargo.lock rust-toolchain.toml "$output/"
mkdir "$output/vector"
cp qualification/ic-storage-native-feasibility/tests/fixtures/provider_vector.rs \
    qualification/ic-storage-native-feasibility/tests/fixtures/region.rs "$output/vector/"
cp -R qualification/ic-storage-native-feasibility/tests/fixtures/region "$output/vector/region"

rustc --edition 2024 --crate-type cdylib --target wasm32-unknown-unknown \
    -C panic=abort -C codegen-units=1 -C lto=fat -C opt-level=3 \
    -C link-arg=--stack-first -C link-arg=-zstack-size=65536 \
    -C link-arg=--initial-memory=196608 -C link-arg=--max-memory=196608 \
    -C link-arg=--export=__stack_pointer -C link-arg=--export=__data_end \
    -C link-arg=--export=__heap_base -D warnings --emit=link,llvm-ir \
    --out-dir "$output" "$output/provider_abi.rs" \
    > "$output/compiler.log" 2>&1

# This larger profile is only inventoried; the scalar combiner must refuse it.
# Reserve the lower 32 KiB for stack, leaving transient data below provider metadata.
rustc --edition 2024 --crate-type cdylib --target wasm32-unknown-unknown \
    -C panic=abort -C codegen-units=1 -C lto=fat -C opt-level=3 \
    -C link-arg=--stack-first -C link-arg=-zstack-size=32768 \
    -C link-arg=--initial-memory=196608 -C link-arg=--max-memory=196608 \
    -C link-arg=--export=__stack_pointer -C link-arg=--export=__data_end \
    -C link-arg=--export=__heap_base -D warnings --emit=link,llvm-ir \
    --out-dir "$output/vector" "$output/vector/provider_vector.rs" \
    > "$output/vector/compiler.log" 2>&1

NATIVE_PROVIDER_INPUTS="$output" cargo test --manifest-path qualification/Cargo.toml \
    -p ic-storage-retention-probe --target-dir target/qualification --locked --offline \
    --test provider_link -- --include-ignored --skip installed_tests --nocapture 2>&1 | tee "$output/report.txt"
if [[ "$mode" == installed ]]; then
    NATIVE_PROVIDER_INPUTS="$output" cargo test --manifest-path qualification/Cargo.toml \
        -p ic-storage-retention-probe --target-dir target/qualification --locked --offline \
        --test provider_link installed_tests -- --ignored --nocapture 2>&1 | tee "$output/installed.txt"
fi
printf 'Scalar provider %s checks passed; vector persistence is NOT qualified.\n' "$mode" > "$output/COMPLETE"

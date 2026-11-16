#!/usr/bin/env bash
# Closed map artifacts and optional installed execution, using prepared tools.
set -euo pipefail
mode="${1:-artifact}"
if [[ $# -gt 1 || ( "$mode" != artifact && "$mode" != installed ) ]]; then
    echo 'usage: run-map-probe.sh [artifact|installed]' >&2
    exit 1
fi
root="$(CDPATH='' cd -- "$(dirname -- "$0")/../../.." && pwd)"
cd "$root"
export CARGO_NET_OFFLINE=true RUSTUP_AUTO_INSTALL=0 CARGO_INCREMENTAL=0
if [[ -n "${RUSTFLAGS:-}${CARGO_ENCODED_RUSTFLAGS:-}${CARGO_BUILD_TARGET:-}${RUSTC_WRAPPER:-}${RUSTC_WORKSPACE_WRAPPER:-}${RUSTC:-}${RUSTUP_TOOLCHAIN:-}${CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUSTFLAGS:-}${CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_LINKER:-}" ]]; then
    echo 'Map diagnostics require the repository compiler without overrides.' >&2
    exit 1
fi
mkdir -p target/evidence/native-map-provider
output="$(mktemp -d "$root/target/evidence/native-map-provider/attempt.XXXXXX")"
printf 'Map provider diagnostics: %s\n' "$output"
rustc --version --verbose > "$output/compiler.txt"
cp "$0" Cargo.toml Cargo.lock rust-toolchain.toml Makefile "$output/"
cp -R crates/ic-storage-native "$output/native-core"
cp qualification/ic-storage-native-feasibility/tests/fixtures/provider_map.rs qualification/ic-storage-native-feasibility/tests/fixtures/runtime.rs "$output/"
cp -R qualification/ic-storage-native-feasibility/tests/fixtures/region "$output/region"
cp -R qualification/ic-storage-retention-probe/tests/provider_link "$output/admission-source"
cargo build -p ic-storage-native --features map --lib --release --target wasm32-unknown-unknown \
    --config 'profile.release.lto=true' --config 'profile.release.codegen-units=1' --config 'profile.release.panic="abort"' \
    --target-dir target/qualification/map-deps --locked --offline --message-format=json \
    > "$output/dependencies.json" 2> "$output/dependencies.log"
deps="$root/target/qualification/map-deps/wasm32-unknown-unknown/release"
jq -e -r -s '[.[] | select(.reason == "compiler-artifact") | .filenames[] | sub("/[^/]+$"; "")] | unique | if length == 0 then error("no compiler artifacts") else .[] end' \
    "$output/dependencies.json" > "$output/search-paths.txt"
search=()
while IFS= read -r directory; do search+=("-L" "dependency=$directory"); done < "$output/search-paths.txt"
rustc --edition 2024 --crate-type cdylib --target wasm32-unknown-unknown \
    -C panic=abort -C codegen-units=1 -C lto=fat -C opt-level=3 \
    -C link-arg=--stack-first -C link-arg=-zstack-size=32768 \
    -C link-arg=--initial-memory=196608 -C link-arg=--max-memory=196608 \
    -C link-arg=--export=__stack_pointer -C link-arg=--export=__data_end \
    -C link-arg=--export=__heap_base -D warnings --emit=link,llvm-ir \
    --extern "ic_storage_native=$deps/libic_storage_native.rlib" "${search[@]}" \
    --out-dir "$output" "$output/provider_map.rs" > "$output/compiler.log" 2>&1
NATIVE_MAP_INPUTS="$output" cargo test --manifest-path qualification/Cargo.toml \
    -p ic-storage-retention-probe --target-dir target/qualification --locked --offline \
    --test provider_link map -- --include-ignored --skip installed_tests --nocapture 2>&1 | tee "$output/artifacts.log"
test -f "$output/ADMITTED"
if [[ "$mode" == installed ]]; then
    NATIVE_MAP_INPUTS="$output" cargo test --manifest-path qualification/Cargo.toml \
        -p ic-storage-retention-probe --target-dir target/qualification --locked --offline \
        --test provider_link map::installed_tests -- --ignored --nocapture 2>&1 | tee "$output/installed.log"
    test -f "$output/INSTALLED"
fi
printf 'Closed native map %s checks passed.\n' "$mode" > "$output/COMPLETE"

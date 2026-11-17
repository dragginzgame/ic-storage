#!/usr/bin/env bash
# Closed map artifacts and optional installed execution, using prepared tools.
set -euo pipefail
mode="${1:-artifact}"
if [[ $# -gt 1 || ( "$mode" != artifact && "$mode" != installed && "$mode" != recovery && "$mode" != job && "$mode" != paged ) ]]; then
    echo 'usage: run-map-probe.sh [artifact|installed|recovery|job|paged]' >&2
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
if [[ "$mode" == recovery || "$mode" == job || "$mode" == paged ]]; then
    profile=map
    [[ "$mode" != job ]] || profile=map-job
    [[ "$mode" != paged ]] || profile=map-paged
    cp "target/qualification/checkpoint-$profile/wasm32-unknown-unknown/release/ic_storage_checkpoint_provider_probe.wasm" "$output/checkpoint-$profile.wasm"
    cp -R qualification/ic-storage-checkpoint-provider-probe "$output/checkpoint-source"
    cp -R qualification/ic-storage-native-feasibility/src "$output/feasibility-source"
    mkdir "$output/qualification-workspace"
    cp qualification/Cargo.toml qualification/Cargo.lock "$output/qualification-workspace/"
    cp qualification/ic-storage-native-feasibility/Cargo.toml "$output/feasibility-source/"
fi
cargo build -p ic-storage-native --features map --lib --release --target wasm32-unknown-unknown \
    --config 'profile.release.lto=true' --config 'profile.release.codegen-units=1' --config 'profile.release.panic="abort"' \
    --target-dir target/qualification/map-deps --locked --offline --message-format=json \
    > "$output/dependencies.json" 2> "$output/dependencies.log"
deps="$root/target/qualification/map-deps/wasm32-unknown-unknown/release"
jq -e -r -s '[.[] | select(.reason == "compiler-artifact") | .filenames[] | sub("/[^/]+$"; "")] | unique | if length == 0 then error("no compiler artifacts") else .[] end' \
    "$output/dependencies.json" > "$output/search-paths.txt"
search=()
while IFS= read -r directory; do search+=("-L" "dependency=$directory"); done < "$output/search-paths.txt"
rustc --edition 2024 --crate-type cdylib --target wasm32-unknown-unknown --cfg 'feature="provider-pool"' \
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
if [[ "$mode" == recovery ]]; then
    NATIVE_MAP_INPUTS="$output" cargo test --manifest-path qualification/Cargo.toml \
        -p ic-storage-retention-probe --target-dir target/qualification --locked --offline \
        --test provider_link map::recovery::installed_tests -- --ignored --nocapture 2>&1 | tee "$output/recovery.log"
    test -f "$output/RECOVERED"
fi
if [[ "$mode" == job ]]; then
    NATIVE_MAP_INPUTS="$output" cargo test --manifest-path qualification/Cargo.toml \
        -p ic-storage-retention-probe --target-dir target/qualification --locked --offline \
        --test provider_link map::recovery::job_installed_tests -- --ignored --nocapture 2>&1 | tee "$output/job.log"
    test -f "$output/JOB_RECOVERED"
    test -f "$output/STAGED_MAP_RECOVERED"
    test -f "$output/STAGED_MAP_RESTARTED"
fi
if [[ "$mode" == paged ]]; then
    NATIVE_MAP_INPUTS="$output" cargo test --manifest-path qualification/Cargo.toml \
        -p ic-storage-retention-probe --target-dir target/qualification --locked --offline \
        --test provider_link map::recovery::paged_installed_tests -- --ignored --nocapture 2>&1 | tee "$output/paged.log"
    test -f "$output/PAGED_NATIVE_RECOVERED"
    test -f "$output/PAGED_NATIVE_REFUSED"
    test -f "$output/PAGED_NATIVE_MULTIPAGE"
    test -f "$output/PAGED_NATIVE_GROWTH"
fi
printf 'Closed native map %s checks passed.\n' "$mode" > "$output/COMPLETE"

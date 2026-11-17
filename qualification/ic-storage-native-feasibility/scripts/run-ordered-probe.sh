#!/usr/bin/env bash
# Closed ordered-root artifacts and installed lifecycle under the existing pool.
set -euo pipefail
mode="${1:-artifact}"
[[ $# -le 1 && ( "$mode" == artifact || "$mode" == installed ) ]] || { echo 'usage: run-ordered-probe.sh [artifact|installed]' >&2; exit 1; }
root="$(CDPATH='' cd -- "$(dirname -- "$0")/../../.." && pwd)"
cd "$root"
export CARGO_NET_OFFLINE=true RUSTUP_AUTO_INSTALL=0 CARGO_INCREMENTAL=0
if [[ -n "${RUSTFLAGS:-}${CARGO_ENCODED_RUSTFLAGS:-}${CARGO_BUILD_TARGET:-}${RUSTC_WRAPPER:-}${RUSTC_WORKSPACE_WRAPPER:-}${RUSTC:-}${RUSTUP_TOOLCHAIN:-}${CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUSTFLAGS:-}${CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_LINKER:-}" ]]; then
    echo 'Ordered diagnostics require the repository compiler without overrides.' >&2
    exit 1
fi
mkdir -p target/evidence/ordered-native
output="$(mktemp -d "$root/target/evidence/ordered-native/attempt.XXXXXX")"
printf 'Ordered provider diagnostics: %s\n' "$output"
rustc --version --verbose > "$output/compiler.txt"
cp "$0" Cargo.toml Cargo.lock rust-toolchain.toml Makefile "$output/"
mkdir "$output/qualification-workspace"
cp qualification/Cargo.toml qualification/Cargo.lock "$output/qualification-workspace/"
cp -R crates/ic-storage "$output/native-core"
cp qualification/ic-storage-native-feasibility/tests/fixtures/provider_ordered.rs qualification/ic-storage-native-feasibility/tests/fixtures/ordered_runtime.rs qualification/ic-storage-native-feasibility/tests/fixtures/ordered_session.rs "$output/"
cp -R qualification/ic-storage-native-feasibility/tests/fixtures/region "$output/region"
cp -R qualification/ic-storage-retention-probe/src/startup "$output/"
cp -R qualification/ic-storage-retention-probe/tests/ordered_native "$output/admission-source"
cargo build --manifest-path qualification/Cargo.toml -p ic-storage-checkpoint-provider-probe \
    --lib --release --target wasm32-unknown-unknown --target-dir target/qualification/ordered-deps \
    --locked --offline --message-format=json > "$output/dependencies.json" 2> "$output/dependencies.log"
jq -e -r -s '[.[] | select(.reason == "compiler-artifact") | .filenames[] | sub("/[^/]+$"; "")] | unique | if length == 0 then error("no compiler artifacts") else .[] end' \
    "$output/dependencies.json" > "$output/search-paths.txt"
search=()
while IFS= read -r directory; do search+=("-L" "dependency=$directory"); done < "$output/search-paths.txt"
storage="$(jq -e -r -s '[.[] | select(.reason == "compiler-artifact" and .target.name == "ic_storage") | .filenames[] | select(endswith(".rlib"))] | if length == 1 then .[0] else error("Storage artifact ambiguity") end' "$output/dependencies.json")"
runtime="$(jq -e -r -s '[.[] | select(.reason == "compiler-artifact" and .target.name == "ic_platform_runtime") | .filenames[] | select(endswith(".rlib"))] | if length == 1 then .[0] else error("Runtime artifact ambiguity") end' "$output/dependencies.json")"
for variant in a b recovery no-close pre-fail; do
    flags=()
    [[ "$variant" == a ]] || flags+=(--cfg ordered_b)
    [[ "$variant" != recovery ]] || flags+=(--cfg ordered_recovery)
    [[ "$variant" != no-close ]] || flags+=(--cfg ordered_no_close)
    [[ "$variant" != pre-fail ]] || flags+=(--cfg ordered_fail_pre)
    rustc --edition 2024 --crate-type cdylib --target wasm32-unknown-unknown --cfg 'feature="provider-pool"' \
        --check-cfg 'cfg(ordered_b,ordered_no_close,ordered_fail_pre)' --check-cfg 'cfg(ordered_recovery)' --check-cfg 'cfg(test)' --check-cfg 'cfg(feature, values("provider-pool"))' \
        -C panic=abort -C codegen-units=1 -C lto=fat -C opt-level=3 \
        -C link-arg=--stack-first -C link-arg=-zstack-size=1048576 \
        -C link-arg=--initial-memory=8388608 -C link-arg=--max-memory=8388608 \
        -C link-arg=--export=__stack_pointer -C link-arg=--export=__data_end -C link-arg=--export=__heap_base \
        -D warnings --emit="link=$output/$variant.wasm,llvm-ir=$output/$variant.ll" --extern "ic_storage=$storage" --extern "ic_platform_runtime=$runtime" \
        "${search[@]}" "${flags[@]}" "$output/provider_ordered.rs" \
        > "$output/compiler-$variant.log" 2>&1
done
ORDERED_NATIVE_INPUTS="$output" cargo test --manifest-path qualification/Cargo.toml \
    -p ic-storage-retention-probe --target-dir target/qualification --locked --offline \
    --test ordered_native artifact -- --nocapture 2>&1 | tee "$output/artifacts.log"
test -f "$output/ADMITTED"
if [[ "$mode" == installed ]]; then
    ORDERED_NATIVE_INPUTS="$output" cargo test --manifest-path qualification/Cargo.toml \
        -p ic-storage-retention-probe --target-dir target/qualification --locked --offline \
        --test ordered_native installed -- --ignored --nocapture 2>&1 | tee "$output/installed.log"
    test -f "$output/LIFECYCLE"
    test -f "$output/REFUSALS"
    test -f "$output/SESSION"
fi

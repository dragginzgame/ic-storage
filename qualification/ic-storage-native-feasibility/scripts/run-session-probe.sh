#!/usr/bin/env bash
# Generation-owned map service under the controlled transient startup boundary.
set -euo pipefail
mode="${1:-artifact}"
if [[ $# -gt 1 || ( "$mode" != artifact && "$mode" != installed && "$mode" != matrix ) ]]; then
    echo 'usage: run-session-probe.sh [artifact|installed|matrix]' >&2
    exit 1
fi
if [[ "$mode" == matrix && "${STORAGE_MATRIX_PROFILE:-full}" != full && "${STORAGE_MATRIX_PROFILE:-full}" != smoke ]]; then
    echo 'STORAGE_MATRIX_PROFILE must be full or smoke' >&2
    exit 1
fi
root="$(CDPATH='' cd -- "$(dirname -- "$0")/../../.." && pwd)"
cd "$root"
export CARGO_NET_OFFLINE=true RUSTUP_AUTO_INSTALL=0 CARGO_INCREMENTAL=0
if [[ -n "${RUSTFLAGS:-}${CARGO_ENCODED_RUSTFLAGS:-}${CARGO_BUILD_TARGET:-}${RUSTC_WRAPPER:-}${RUSTC_WORKSPACE_WRAPPER:-}${RUSTC:-}${RUSTUP_TOOLCHAIN:-}${CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUSTFLAGS:-}${CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_LINKER:-}" ]]; then
    echo 'Session diagnostics require the repository compiler without overrides.' >&2
    exit 1
fi
mkdir -p target/evidence/native-session
output="$(mktemp -d "$root/target/evidence/native-session/attempt.XXXXXX")"
printf 'Native session diagnostics: %s\n' "$output"
rustc --version --verbose > "$output/compiler.txt"
cp "$0" Cargo.toml Cargo.lock rust-toolchain.toml Makefile "$output/"
cp -R crates/ic-storage "$output/native-core"
cp qualification/ic-storage-native-feasibility/tests/fixtures/provider_session.rs qualification/ic-storage-native-feasibility/tests/fixtures/session_benchmark.rs qualification/ic-storage-native-feasibility/tests/fixtures/runtime.rs "$output/"
cp -R qualification/ic-storage-native-feasibility/tests/fixtures/region "$output/region"
cp -R qualification/ic-storage-retention-probe/src/startup qualification/ic-storage-retention-probe/tests/rust_startup "$output/"
mkdir "$output/qualification-workspace"
cp qualification/Cargo.toml qualification/Cargo.lock "$output/qualification-workspace/"
if [[ "$mode" == matrix ]]; then
    cp target/qualification/bench-stable-matrix/wasm32-unknown-unknown/release/examples/stable_matrix_wasm.wasm "$output/stable-matrix.wasm"
    cp -R qualification/ic-storage-simple-benchmarks "$output/benchmark-source"
    cp -R qualification/ic-storage-retention-probe/tests/provider_link "$output/comparison-source"
    perl vendor/toko-tooling/scripts/ci/check-cargo-graph.pl --consumer "$root/qualification" \
        --package ic-storage-simple-benchmarks --features stable-map --target wasm32-unknown-unknown --edges normal \
        --require ic-stable-structures=1 --require ic-platform-runtime=1 --forbid ic-storage --forbid toko-tooling \
        --observations --json > "$output/stable-dependencies.json"
    jq -e 'any(.observations[]; .name == "ic-stable-structures" and .identity == "ic-stable-structures v0.7.2")' "$output/stable-dependencies.json" > /dev/null || {
        echo 'Matrix requires the reviewed ic-stable-structures 0.7.2 selection.' >&2
        exit 1
    }
fi
cargo build -p ic-storage --features map --lib --release --target wasm32-unknown-unknown \
    --config 'profile.release.lto=true' --config 'profile.release.codegen-units=1' --config 'profile.release.panic="abort"' \
    --target-dir target/qualification/map-deps --locked --offline --message-format=json \
    > "$output/dependencies.json" 2> "$output/dependencies.log"
deps="$root/target/qualification/map-deps/wasm32-unknown-unknown/release"
jq -e -r -s '[.[] | select(.reason == "compiler-artifact") | .filenames[] | sub("/[^/]+$"; "")] | unique | if length == 0 then error("no compiler artifacts") else .[] end' \
    "$output/dependencies.json" > "$output/search-paths.txt"
search=()
while IFS= read -r directory; do search+=("-L" "dependency=$directory"); done < "$output/search-paths.txt"
compile=(--edition 2024 --crate-type cdylib --target wasm32-unknown-unknown
    --cfg 'feature="provider-pool"' --check-cfg 'cfg(feature, values("provider-pool"))'
    --check-cfg 'cfg(test,session_b,session_fail_post,session_corrupt_post,session_no_close,session_fail_pre,session_bench)'
    -C panic=abort -C codegen-units=1 -C lto=fat -C opt-level=3
    -C link-arg=--stack-first -C link-arg=-zstack-size=1048576
    -C link-arg=--initial-memory=8454144 -C link-arg=--max-memory=16777216
    -C link-arg=--export=__stack_pointer -C link-arg=--export=__data_end
    -C link-arg=--export=__heap_base -D warnings
    --extern "ic_storage=$deps/libic_storage.rlib" "${search[@]}")
variants=(a b fail corrupt no-close pre-fail)
[[ "$mode" != matrix ]] || variants+=(bench)
for variant in "${variants[@]}"; do
    flags=()
    [[ "$variant" == a || "$variant" == bench ]] || flags+=(--cfg session_b)
    [[ "$variant" != bench ]] || flags+=(--cfg session_bench)
    [[ "$variant" != fail ]] || flags+=(--cfg session_fail_post)
    [[ "$variant" != corrupt ]] || flags+=(--cfg session_corrupt_post)
    [[ "$variant" != no-close ]] || flags+=(--cfg session_no_close)
    [[ "$variant" != pre-fail ]] || flags+=(--cfg session_fail_pre)
    clippy-driver "${compile[@]}" "${flags[@]}" --emit=metadata \
        -o "$output/session-$variant.rmeta" "$output/provider_session.rs" > "$output/clippy-$variant.log" 2>&1
    rustc "${compile[@]}" "${flags[@]}" --emit="link=$output/session-$variant.wasm,llvm-ir=$output/session-$variant.ll" \
        "$output/provider_session.rs" > "$output/compiler-$variant.log" 2>&1
done
NATIVE_SESSION_INPUTS="$output" cargo test --manifest-path qualification/Cargo.toml \
    -p ic-storage-retention-probe --target-dir target/qualification --locked --offline \
    --test rust_startup session_tests::artifact -- --ignored --nocapture 2>&1 | tee "$output/artifact.log"
test -f "$output/ADMITTED"
if [[ "$mode" == installed ]]; then
    NATIVE_SESSION_INPUTS="$output" cargo test --manifest-path qualification/Cargo.toml \
        -p ic-storage-retention-probe --target-dir target/qualification --locked --offline \
        --test rust_startup session_tests::installed -- --ignored --nocapture 2>&1 | tee "$output/installed.log"
    test -f "$output/INSTALLED"
fi
if [[ "$mode" == matrix ]]; then
    NATIVE_SESSION_INPUTS="$output" cargo test --manifest-path qualification/Cargo.toml \
        -p ic-storage-retention-probe --target-dir target/qualification --locked --offline \
        --test provider_link map::comparison_tests::matrix::installed_tests_matrix \
        -- --ignored --nocapture 2>&1 | tee "$output/matrix.log"
    test -f "$output/MATRIX"
    printf 'Matrix report: %s/summary.md\n' "$output"
fi
printf 'Native session %s checks passed.\n' "$mode" > "$output/COMPLETE"

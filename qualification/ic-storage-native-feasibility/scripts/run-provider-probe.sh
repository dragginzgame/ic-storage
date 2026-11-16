#!/usr/bin/env bash
# Artifact checks by default; installed mode requires the admitted local server.
set -euo pipefail

mode="${1:-artifact}"
if [[ $# -gt 1 || ( "$mode" != artifact && "$mode" != installed && "$mode" != migration && "$mode" != job && "$mode" != record && "$mode" != record-vector && "$mode" != record-job ) ]]; then
    echo 'usage: run-provider-probe.sh [artifact|installed|migration|job|record|record-vector|record-job]' >&2
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
cp "$0" "$output/run-provider-probe.sh"
cp Makefile "$output/build-Makefile"
cp qualification/ic-storage-native-feasibility/tests/fixtures/provider_abi.rs "$output/"
cp qualification/ic-storage-retention-probe/tests/provider_custody/provider.wat "$output/"
cp -R qualification/ic-storage-retention-probe/tests/provider_link "$output/admission-source"
cp -R qualification/ic-storage-retention-probe/src "$output/host-source"
cp qualification/ic-storage-retention-probe/Cargo.toml "$output/host-source/"
cp qualification/Cargo.toml qualification/Cargo.lock rust-toolchain.toml "$output/"
mkdir "$output/vector"
cp -R crates/ic-storage-native "$output/vector/native-core"
cp Cargo.toml "$output/vector/native-core/workspace-Cargo.toml"
cp Cargo.lock "$output/vector/native-core/workspace-Cargo.lock"
cp qualification/ic-storage-native-feasibility/tests/fixtures/provider_vector.rs "$output/vector/"
cp qualification/ic-storage-native-feasibility/tests/fixtures/provider_record_vector.rs "$output/vector/"
cp qualification/ic-storage-native-feasibility/tests/fixtures/runtime.rs "$output/vector/"
cp -R qualification/ic-storage-native-feasibility/tests/fixtures/region "$output/vector/region"
cp -R qualification/ic-storage-retention-probe/tests/provider_custody/vector "$output/vector/provider-source"
cp qualification/ic-storage-native-feasibility/vector-provider-contract.md "$output/vector/"
cp qualification/ic-storage-retention-probe/record-vector-provider.md "$output/vector/"
cp qualification/ic-storage-retention-probe/record-vector-recovery.md "$output/vector/"
cp -R qualification/ic-storage-native-feasibility/src "$output/feasibility-source"
cp qualification/ic-storage-native-feasibility/Cargo.toml "$output/feasibility-source/"
cp -R crates/ic-storage-native-derive "$output/derive-source"
if [[ "$mode" == migration || "$mode" == job || "$mode" == record || "$mode" == record-vector || "$mode" == record-job ]]; then
    cp -R qualification/ic-storage-checkpoint-provider-probe/src "$output/checkpoint-source"
    cp qualification/ic-storage-checkpoint-provider-probe/Cargo.toml "$output/checkpoint-source/"
    cp qualification/ic-storage-native-feasibility/native-checkpoint-contract.md "$output/"
    if [[ "$mode" == job ]]; then
        cp qualification/ic-storage-native-feasibility/migration-job.md "$output/"
        cp qualification/ic-storage-native-feasibility/staged-import.md "$output/"
        cp target/qualification/checkpoint-job/wasm32-unknown-unknown/release/ic_storage_checkpoint_provider_probe.wasm "$output/checkpoint-job.wasm"
    elif [[ "$mode" == record-job ]]; then
        cp qualification/ic-storage-native-feasibility/migration-job.md qualification/ic-storage-native-feasibility/staged-import.md "$output/"
        cp target/qualification/checkpoint-record-vector-job/wasm32-unknown-unknown/release/ic_storage_checkpoint_provider_probe.wasm "$output/checkpoint-record-vector-job.wasm"
    elif [[ "$mode" == record-vector ]]; then
        cp target/qualification/checkpoint-record-vector/wasm32-unknown-unknown/release/ic_storage_checkpoint_provider_probe.wasm "$output/checkpoint-record-vector.wasm"
        cp target/qualification/checkpoint-record-vector-source/wasm32-unknown-unknown/release/ic_storage_checkpoint_provider_probe.wasm "$output/checkpoint-record-vector-source.wasm"
        cp target/qualification/checkpoint-record-vector-v2/wasm32-unknown-unknown/release/ic_storage_checkpoint_provider_probe.wasm "$output/checkpoint-record-vector-v2.wasm"
        cp target/qualification/checkpoint-record-vector-convert/wasm32-unknown-unknown/release/ic_storage_checkpoint_provider_probe.wasm "$output/checkpoint-record-vector-convert.wasm"
    elif [[ "$mode" == record ]]; then
        cp qualification/ic-storage-native-feasibility/records.md "$output/"
        cp target/qualification/checkpoint-record/wasm32-unknown-unknown/release/ic_storage_checkpoint_provider_probe.wasm "$output/checkpoint-record.wasm"
    else
        cp target/qualification/checkpoint-native/wasm32-unknown-unknown/release/ic_storage_checkpoint_provider_probe.wasm "$output/checkpoint-bridge.wasm"
    fi
fi

rustc --edition 2024 --crate-type cdylib --target wasm32-unknown-unknown \
    -C panic=abort -C codegen-units=1 -C lto=fat -C opt-level=3 \
    -C link-arg=--stack-first -C link-arg=-zstack-size=65536 \
    -C link-arg=--initial-memory=196608 -C link-arg=--max-memory=196608 \
    -C link-arg=--export=__stack_pointer -C link-arg=--export=__data_end \
    -C link-arg=--export=__heap_base -D warnings --emit=link,llvm-ir \
    --out-dir "$output" "$output/provider_abi.rs" \
    > "$output/compiler.log" 2>&1

# This larger profile has separate artifact checks; the scalar combiner refuses it.
# Installed mode executes only the documented bounded scalar/vector contracts.
# Reserve the lower 32 KiB for stack, leaving transient data below provider metadata.
rustc --edition 2024 --crate-type rlib --crate-name ic_storage_native \
    --target wasm32-unknown-unknown -C panic=abort -C opt-level=3 \
    -C embed-bitcode=yes -D warnings \
    --out-dir "$output/vector" "$output/vector/native-core/src/lib.rs" \
    > "$output/vector/native-core-compiler.log" 2>&1
rustc --edition 2024 --crate-type cdylib --target wasm32-unknown-unknown \
    -C panic=abort -C codegen-units=1 -C lto=fat -C opt-level=3 \
    -C link-arg=--stack-first -C link-arg=-zstack-size=32768 \
    -C link-arg=--initial-memory=196608 -C link-arg=--max-memory=196608 \
    -C link-arg=--export=__stack_pointer -C link-arg=--export=__data_end \
    -C link-arg=--export=__heap_base -D warnings --emit=link,llvm-ir \
    --extern "ic_storage_native=$output/vector/libic_storage_native.rlib" \
    --out-dir "$output/vector" "$output/vector/provider_vector.rs" \
    > "$output/vector/compiler.log" 2>&1

# Reuse the exact V2/V3 declarations and derive without std on wasm32. This
# compiler artifact has its own diagnostic; the u64 admission profile refuses it.
cargo build --manifest-path qualification/Cargo.toml -p ic-storage-native-feasibility \
    --lib --release --target wasm32-unknown-unknown \
    --target-dir target/qualification/record-vector-deps --locked --offline --message-format=json \
    > "$output/vector/record-dependencies.json" 2> "$output/vector/record-dependencies.log"
record_deps="$root/target/qualification/record-vector-deps/wasm32-unknown-unknown/release"
# Cargo owns artifact placement (including its per-package build directories).
# Admit successful structured output before passing the actual dependency paths.
jq -e -r -s '[.[] | select(.reason == "compiler-artifact") | .filenames[] | sub("/[^/]+$"; "")] | unique | if length == 0 then error("no compiler artifacts") else .[] end' \
    "$output/vector/record-dependencies.json" > "$output/vector/record-search-paths.txt"
record_search=()
while IFS= read -r record_directory; do
    record_search+=("-L" "dependency=$record_directory")
done < "$output/vector/record-search-paths.txt"
for record_schema in v3 v2; do
    record_flags=(--check-cfg 'cfg(record_v2,test)' --check-cfg 'cfg(feature, values("provider-pool"))')
    record_output="$output/vector"
    if [[ "$record_schema" == v2 ]]; then
        record_flags+=(--cfg record_v2)
        record_output="$output/vector/v2"
        mkdir "$record_output"
    fi
    rustc --edition 2024 --crate-type cdylib --target wasm32-unknown-unknown \
        -C panic=abort -C codegen-units=1 -C lto=fat -C opt-level=3 \
        -C link-arg=--stack-first -C link-arg=-zstack-size=32768 \
        -C link-arg=--initial-memory=196608 -C link-arg=--max-memory=196608 \
        -C link-arg=--export=__stack_pointer -C link-arg=--export=__data_end \
        -C link-arg=--export=__heap_base -D warnings --emit=link,llvm-ir \
        --extern "ic_storage_native_feasibility=$record_deps/libic_storage_native_feasibility.rlib" \
        "${record_search[@]}" \
        "${record_flags[@]}" \
        --out-dir "$record_output" "$output/vector/provider_record_vector.rs" \
        > "$record_output/record-compiler.log" 2>&1
done

NATIVE_PROVIDER_INPUTS="$output" cargo test --manifest-path qualification/Cargo.toml \
    -p ic-storage-retention-probe --target-dir target/qualification --locked --offline \
    --test provider_link -- --include-ignored --skip installed_tests --skip migration --skip map:: --nocapture 2>&1 | tee "$output/report.txt"
if [[ "$mode" == installed ]]; then
    NATIVE_PROVIDER_INPUTS="$output" cargo test --manifest-path qualification/Cargo.toml \
        -p ic-storage-retention-probe --target-dir target/qualification --locked --offline \
        --test provider_link installed_tests -- --ignored --skip map:: --nocapture 2>&1 | tee "$output/installed.txt"
fi
if [[ "$mode" == migration ]]; then
    NATIVE_PROVIDER_INPUTS="$output" cargo test --manifest-path qualification/Cargo.toml \
        -p ic-storage-retention-probe --target-dir target/qualification --locked --offline \
        --test provider_link migration -- --ignored --skip migration::tests::job --skip migration::tests::records --skip migration::record_vector --nocapture 2>&1 | tee "$output/migration.txt"
fi
if [[ "$mode" == job ]]; then
    NATIVE_PROVIDER_INPUTS="$output" cargo test --manifest-path qualification/Cargo.toml \
        -p ic-storage-retention-probe --target-dir target/qualification --locked --offline \
        --test provider_link migration::tests::job -- --ignored --nocapture 2>&1 | tee "$output/job.txt"
fi
if [[ "$mode" == record ]]; then
    NATIVE_PROVIDER_INPUTS="$output" cargo test --manifest-path qualification/Cargo.toml \
        -p ic-storage-retention-probe --target-dir target/qualification --locked --offline \
        --test provider_link migration::tests::records -- --ignored --nocapture 2>&1 | tee "$output/records.txt"
fi
if [[ "$mode" == record-vector ]]; then
    NATIVE_PROVIDER_INPUTS="$output" cargo test --manifest-path qualification/Cargo.toml \
        -p ic-storage-retention-probe --target-dir target/qualification --locked --offline \
        --test provider_link migration::record_vector -- --ignored --skip migration::record_vector::tests::job --nocapture 2>&1 | tee "$output/record-vector.txt"
fi
if [[ "$mode" == record-job ]]; then
    NATIVE_PROVIDER_INPUTS="$output" cargo test --manifest-path qualification/Cargo.toml \
        -p ic-storage-retention-probe --target-dir target/qualification --locked --offline \
        --test provider_link migration::record_vector::tests::job -- --ignored --nocapture 2>&1 | tee "$output/record-job.txt"
fi
printf 'Bounded provider %s checks passed; only this mode and its recorded inputs are covered, not general native persistence.\n' "$mode" > "$output/COMPLETE"

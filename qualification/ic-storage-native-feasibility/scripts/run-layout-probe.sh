#!/usr/bin/env bash
# Diagnostic compilation only. Never emits a retained-layout admission receipt.
set -euo pipefail

root="$(CDPATH='' cd -- "$(dirname -- "$0")/../../.." && pwd)"
cd "$root"
export CARGO_NET_OFFLINE=true RUSTUP_AUTO_INSTALL=0 CARGO_INCREMENTAL=0
if [[ -n "${RUSTFLAGS:-}${CARGO_ENCODED_RUSTFLAGS:-}${CARGO_BUILD_TARGET:-}${RUSTC_WRAPPER:-}${RUSTC_WORKSPACE_WRAPPER:-}${RUSTC:-}${RUSTUP_TOOLCHAIN:-}${CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUSTFLAGS:-}${CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_LINKER:-}" ]]; then
    echo 'Layout diagnostics require the repository compiler and no compiler/target overrides.' >&2
    exit 1
fi

mkdir -p target/evidence/native-layout
output="$(mktemp -d "$root/target/evidence/native-layout/attempt.XXXXXX")"
printf 'Diagnostic inputs and outputs: %s\n' "$output"
rustc --version --verbose > "$output/compiler.txt"
host="$(sed -n 's/^host: //p' "$output/compiler.txt")"
[[ -n "$host" && "$host" != *$'\n'* ]] || { echo 'Cannot identify compiler host' >&2; exit 1; }
cp -R qualification/ic-storage-native-feasibility "$output/source"
cp qualification/Cargo.toml "$output/workspace.toml"
cp qualification/Cargo.lock rust-toolchain.toml "$output/"

for target_name in host wasm; do
    target="$host"
    [[ "$target_name" != wasm ]] || target=wasm32-unknown-unknown
    for variant in base swapped; do
        build="$output/build-$target_name-$variant"
        features=''
        [[ "$variant" != swapped ]] || features=layout-swapped
        cargo rustc --manifest-path qualification/Cargo.toml \
            -p ic-storage-native-feasibility --example layout_probe \
            --target "$target" --target-dir "$build" --locked --offline \
            --no-default-features --features "$features" -- \
            -C debuginfo=2 -C codegen-units=1 -C opt-level=0 -C strip=none --emit=llvm-ir \
            > "$output/$target_name-$variant-build.log" 2>&1
        # The fresh directory and disabled incremental compilation prevent stale
        # artifacts. Cargo output directories vary by version: require one file.
        find "$build" -type f -name layout_probe.ll -print0 > "$output/$target_name-$variant-paths"
        candidates=()
        while IFS= read -r -d '' file; do candidates+=("$file"); done \
            < "$output/$target_name-$variant-paths"
        [[ "${#candidates[@]}" == 1 ]] || { echo 'Expected exactly one emitted LLVM IR file' >&2; exit 1; }
        cp "${candidates[0]}" "$output/$target_name-$variant.ll"
        if [[ "$target_name" == host ]]; then
            "$build/$target/debug/examples/layout_probe" > "$output/host-$variant-observation.txt"
        fi
    done
done

NATIVE_LAYOUT_INPUTS="$output" cargo test --manifest-path qualification/Cargo.toml \
    -p ic-storage-native-feasibility --target-dir target/qualification --locked --offline \
    --test layout_metadata_tests compiled_layouts -- --ignored --nocapture \
    2>&1 | tee "$output/report.txt"
printf '%s\n' 'Diagnostic experiment completed; native persistence is NOT admitted.' > "$output/COMPLETE"

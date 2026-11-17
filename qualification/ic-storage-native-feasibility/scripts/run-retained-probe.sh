#!/usr/bin/env bash
set -euo pipefail
mode="${1:-artifact}"
[[ $# -le 1 && ( "$mode" == artifact || "$mode" == installed ) ]] || { echo 'usage: run-retained-probe.sh [artifact|installed]' >&2; exit 1; }
root="$(CDPATH='' cd -- "$(dirname -- "$0")/../../.." && pwd)"
cd "$root"
export CARGO_NET_OFFLINE=true RUSTUP_AUTO_INSTALL=0 CARGO_INCREMENTAL=0
[[ -z "${RUSTFLAGS:-}${CARGO_ENCODED_RUSTFLAGS:-}${RUSTC_WRAPPER:-}${RUSTC_WORKSPACE_WRAPPER:-}${RUSTC:-}${RUSTUP_TOOLCHAIN:-}" ]] || { echo 'retained qualification requires the pinned compiler without overrides' >&2; exit 1; }
mkdir -p target/evidence/retained-runtime
output="$(mktemp -d "$root/target/evidence/retained-runtime/attempt.XXXXXX")"
printf 'Public retained runtime diagnostics: %s\n' "$output"
rustc --version --verbose > "$output/compiler.txt"
cp "$0" Cargo.toml Cargo.lock rust-toolchain.toml "$output/"
cp -R crates/ic-storage "$output/product-source"
cp qualification/ic-storage-native-feasibility/tests/fixtures/provider_retained.rs "$output/"
cp -R qualification/ic-storage-retention-probe/tests/retained_runtime "$output/admission-source"
cargo build --config 'profile.release.lto=true' --config 'profile.release.panic="abort"' --config 'profile.release.codegen-units=1' -p ic-storage --features retained,checkpoint,platform --lib --release --target wasm32-unknown-unknown --target-dir target/qualification/retained-deps --locked --offline --message-format=json > "$output/dependencies.json" 2> "$output/dependencies.log"
jq -e -r -s '[.[] | select(.reason == "compiler-artifact") | .filenames[] | sub("/[^/]+$"; "")] | unique | if length == 0 then error("no compiler artifacts") else .[] end' "$output/dependencies.json" > "$output/search-paths.txt"
search=()
while IFS= read -r directory; do search+=("-L" "dependency=$directory"); done < "$output/search-paths.txt"
storage="$(jq -e -r -s '[.[] | select(.reason == "compiler-artifact" and .target.name == "ic_storage") | .filenames[] | select(endswith(".rlib"))] | if length == 1 then .[0] else error("Storage artifact ambiguity") end' "$output/dependencies.json")"
runtime="$(jq -e -r -s '[.[] | select(.reason == "compiler-artifact" and .target.name == "ic_platform_runtime") | .filenames[] | select(endswith(".rlib"))] | if length == 1 then .[0] else error("Runtime artifact ambiguity") end' "$output/dependencies.json")"
for variant in a b no-close fail-close; do
    flags=()
    [[ "$variant" != b ]] || flags+=(--cfg retained_b)
    [[ "$variant" != no-close ]] || flags+=(--cfg retained_no_close)
    [[ "$variant" != fail-close ]] || flags+=(--cfg retained_fail_close)
    compile=(--edition 2024 --crate-type cdylib --target wasm32-unknown-unknown --check-cfg 'cfg(retained_b,retained_no_close,retained_fail_close)' -C panic=abort -C codegen-units=1 -C lto=fat -C opt-level=3 \
        -C link-arg=--stack-first -C link-arg=-zstack-size=1048576 -C link-arg=--global-base=37748736 \
        -C link-arg=--initial-memory=41943040 -C link-arg=--max-memory=134217728 \
        -C link-arg=--export=__stack_pointer -C link-arg=--export=__data_end -C link-arg=--export=__heap_base \
        -D warnings --extern "ic_storage=$storage" --extern "ic_platform_runtime=$runtime" "${search[@]}" "${flags[@]}")
    clippy-driver "${compile[@]}" --emit=metadata -o "$output/$variant.rmeta" "$output/provider_retained.rs" > "$output/clippy-$variant.log" 2>&1
    rustc "${compile[@]}" -o "$output/$variant.wasm" "$output/provider_retained.rs" > "$output/compiler-$variant.log" 2>&1
done
RETAINED_RUNTIME_INPUTS="$output" cargo test --manifest-path qualification/Cargo.toml -p ic-storage-retention-probe --target-dir target/qualification --locked --offline --test retained_runtime artifact -- --nocapture 2>&1 | tee "$output/artifacts.log"
test -f "$output/ADMITTED"
if [[ "$mode" == installed ]]; then
    RETAINED_RUNTIME_INPUTS="$output" cargo test --manifest-path qualification/Cargo.toml -p ic-storage-retention-probe --target-dir target/qualification --locked --offline --test retained_runtime installed -- --ignored --nocapture 2>&1 | tee "$output/installed.log"
    test -f "$output/INSTALLED"
fi

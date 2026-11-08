#!/usr/bin/env bash
# Caller admits Testkit; this comparison never installs tools or updates locks.
set -euo pipefail
[[ $# == 5 ]] || exit 2
root="$1" cli="$2" server_directory="$3" rows="$4" samples="$5"
[[ "$rows" =~ ^[1-9][0-9]*$ && ${#rows} -le 7 && "$rows" -le 1000000 ]]
[[ "$samples" =~ ^[1-9][0-9]*$ && ${#samples} -le 2 && "$samples" -le 20 ]]
cd "$root"
export RUSTUP_AUTO_INSTALL=0
server="$("$cli" check --directory "$server_directory")"
mkdir -p target/evidence/storage-comparison
output="$(mktemp -d "$root/target/evidence/storage-comparison/attempt.XXXXXX")"
printf 'Storage comparison evidence: %s\n' "$output"
manifest=testing/runtime-qualification/Cargo.toml
reference="${BENCH_REFERENCE:-dfinity}"
[[ "$reference" == dfinity || "$reference" == dfinity-head || "$reference" == dfinity-governed ]]
if [[ -n "${BENCH_WORKLOAD:-}" ]]; then
    # Named workloads own the knobs below. Refuse silently mixed experiments.
    for variable in BENCH_CASES BENCH_PAYLOADS BENCH_GROWTH_CYCLES BENCH_PATTERNS BENCH_ROUNDS BENCH_SHUFFLED BENCH_UPSTREAM_CACHE_SLOTS BENCH_COLD_CACHE; do
        if [[ -n "${!variable:-}" ]]; then
            echo "$variable cannot be combined with BENCH_WORKLOAD" >&2; exit 2
        fi
    done
    arguments=(--workload "$BENCH_WORKLOAD")
else
    arguments=(--rows "$rows" --patterns "${BENCH_PATTERNS:-uniform,hot16}" --rounds "${BENCH_ROUNDS:-1}"
        --upstream-cache-slots "${BENCH_UPSTREAM_CACHE_SLOTS:-0}")
    [[ -z "${BENCH_CASES:-}" ]] || arguments+=(--cases "$BENCH_CASES")
    [[ -z "${BENCH_PAYLOADS:-}" ]] || arguments+=(--payloads "$BENCH_PAYLOADS")
    [[ "${BENCH_GROWTH_CYCLES:-0}" != 1 ]] || arguments+=(--growth-cycles)
    [[ "${BENCH_SHUFFLED:-0}" != 1 ]] || arguments+=(--shuffled)
    [[ "${BENCH_COLD_CACHE:-0}" != 1 ]] || arguments+=(--cold-cache)
fi
arguments+=(--upstream-bucket-pages "${BENCH_UPSTREAM_BUCKET_PAGES:-8}")
if [[ "$reference" == dfinity-head ]]; then
    arguments+=(--reference-revision b5d1650463f477affb3d358e86b1113953fc9937)
fi
if [[ "$reference" == dfinity-governed ]]; then
    arguments+=(--reference-revision 0.7.2-governed)
fi
rustc -vV > "$output/compiler.txt"
cargo -V > "$output/cargo.txt"
cp "$manifest" testing/runtime-qualification/Cargo.lock "$output/"
cp Cargo.lock "$output/production.lock"
git rev-parse HEAD > "$output/source-commit"
git diff --binary > "$output/pending.patch"
# Hash actual working source, including untracked files from the pending hard cut.
rg --files crates/ic-storage testing/runtime-qualification/crates/ic-storage-benchmarks scripts/ci/compare-storage.sh | LC_ALL=C sort > "$output/source-files"
while IFS= read -r path; do shasum -a 256 "$path"; done < "$output/source-files" > "$output/source.sha256"
tar -czf "$output/source.tar.gz" -T "$output/source-files" Cargo.toml rust-toolchain.toml "$manifest" scripts/ci/compare-storage.sh
shasum -a 256 "$cli" "$server" Cargo.toml rust-toolchain.toml "$manifest" testing/runtime-qualification/Cargo.lock > "$output/inputs.sha256"
# Large inline ICSS test types need more stack than the ordinary Rust/Wasm
# default. This selects the synthetic comparison only, identically on both sides.
comparison_rustflags="${RUSTFLAGS:+$RUSTFLAGS }-C link-arg=-zstack-size=8388608"
printf '%s\n' "$comparison_rustflags" > "$output/wasm-rustflags.txt"
for backend in owned "$reference"; do
    if ! RUSTFLAGS="$comparison_rustflags" cargo build --locked --offline --manifest-path "$manifest" -p ic-storage-benchmarks \
        --example storage-comparison-wasm --target wasm32-unknown-unknown --profile comparison \
        --features "$backend" --target-dir "target/storage-comparison-build/$backend" > "$output/$backend-build.log" 2>&1; then
        cat "$output/$backend-build.log" >&2; exit 1
    fi
    cp "target/storage-comparison-build/$backend/wasm32-unknown-unknown/comparison/examples/storage_comparison_wasm.wasm" "$output/$backend.wasm"
    # Preserve original modules; gzip changes installation transport only.
    gzip -n -c "$output/$backend.wasm" > "$output/$backend.wasm.gz"
done
if ! cargo build --locked --offline --manifest-path "$manifest" -p ic-storage-benchmarks \
    --bin ic-storage-benchmarks --target-dir target/runtime-qualification > "$output/driver-build.log" 2>&1; then
    cat "$output/driver-build.log" >&2; exit 1
fi
shasum -a 256 "$output/owned.wasm" "$output/$reference.wasm" "$output/owned.wasm.gz" "$output/$reference.wasm.gz" "$output/wasm-rustflags.txt" target/runtime-qualification/debug/ic-storage-benchmarks >> "$output/inputs.sha256"
shasum -a 256 -c "$output/source.sha256" > "$output/source-check.log"
shasum -a 256 -c "$output/inputs.sha256" > "$output/inputs-check.log"
cmp testing/runtime-qualification/Cargo.lock "$output/Cargo.lock"
cmp Cargo.lock "$output/production.lock"
status=0
selections=(all)
[[ "${BENCH_WORKLOAD:-}" != types ]] || selections=(map set vector cell region)
server_ttl=900
# The complete type matrix includes full-width 64 KiB payloads and independent
# final-state checks. Keep its bounded lifetime separate from ordinary runs.
[[ "${BENCH_WORKLOAD:-}" != types ]] || server_ttl=1800
printf '%s\n' "$server_ttl" > "$output/server-ttl-seconds.txt"
for selection in "${selections[@]}"; do
    shard_arguments=()
    log_prefix=""
    [[ "$selection" == all ]] || log_prefix="$selection-"
    [[ "$selection" == all ]] || shard_arguments=(--collection "$selection")
    env -u POCKET_IC_BIN -u IC_TESTKIT_POCKET_IC_URL "$cli" run --directory "$server_directory" --ttl "$server_ttl" \
        --server-stdout "$output/${log_prefix}server.stdout" --server-stderr "$output/${log_prefix}server.stderr" \
        -- "$root/target/runtime-qualification/debug/ic-storage-benchmarks" \
        --owned-wasm "$output/owned.wasm.gz" --dfinity-wasm "$output/$reference.wasm.gz" \
        --output "$output/report" --samples "$samples" "${arguments[@]}" "${shard_arguments[@]}" >> "$output/runtime.log" 2>&1 || status=$?
    [[ "$status" == 0 ]] || break
done
cat "$output/runtime.log"
[[ "$status" == 0 ]] || exit "$status"
shasum -a 256 -c "$output/source.sha256" >> "$output/source-check.log"
shasum -a 256 -c "$output/inputs.sha256" >> "$output/inputs-check.log"
cmp testing/runtime-qualification/Cargo.lock "$output/Cargo.lock"
cmp Cargo.lock "$output/production.lock"
printf 'Comparison summary: %s/report/summary.csv\n' "$output"

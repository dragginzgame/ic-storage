#!/usr/bin/env bash
# Matched production-shaped consumers, independent of the broad type catalogue.
set -euo pipefail
[[ $# == 3 ]] || { echo 'usage: compare-storage-size.sh ROOT TESTKIT_CLI SERVER_DIRECTORY' >&2; exit 2; }
root="$1" cli="$2" server_directory="$3"
cd "$root"
export RUSTUP_AUTO_INSTALL=0
server="$("$cli" check --directory "$server_directory")"
manifest=testing/runtime-qualification/Cargo.toml
# Keep the independently owned application endpoints matched across backends.
if ! cmp -s testing/runtime-qualification/crates/storage-size-consumer/src/app.rs \
    testing/runtime-qualification/crates/icss-size-consumer/src/app.rs; then
    echo 'Size consumer application endpoints differ; restore the matched workload before comparing.' >&2
    exit 1
fi
mkdir -p target/evidence/storage-consumer-size
output="$(mktemp -d "$root/target/evidence/storage-consumer-size/attempt.XXXXXX")"
printf 'Consumer size evidence: %s\n' "$output"
rustc -Vv > "$output/compiler.txt"
cargo -V > "$output/cargo.txt"
printf 'profile=wasm-size\nRUSTFLAGS=%s\nCARGO_ENCODED_RUSTFLAGS=%s\n' "${RUSTFLAGS:-}" "${CARGO_ENCODED_RUSTFLAGS:-}" > "$output/build-settings.txt"
cp "$manifest" testing/runtime-qualification/Cargo.lock "$output/"
git rev-parse HEAD > "$output/source-commit"
git diff --binary > "$output/pending.patch"
rg --files crates/ic-storage testing/runtime-qualification/crates/{storage-size-consumer,icss-size-consumer,ic-storage-benchmarks} \
    scripts/ci/compare-storage-size.sh | LC_ALL=C sort > "$output/source-files"
printf '%s\n' Cargo.toml rust-toolchain.toml "$manifest" testing/runtime-qualification/Cargo.lock >> "$output/source-files"
while IFS= read -r path; do shasum -a 256 "$path"; done < "$output/source-files" > "$output/source.sha256"
tar -czf "$output/source.tar.gz" -T "$output/source-files"
for selection in base map set vector cell region map-region all; do
    features=()
    case "$selection" in
        base) ;;
        map-region) features=(--features "map,region") ;;
        all) features=(--features "map,set,vector,cell,region") ;;
        *) features=(--features "$selection") ;;
    esac
    for backend in storage icss; do
        package="$backend-size-consumer"
        cargo tree --locked --offline --manifest-path "$manifest" -p "$package" \
            --target wasm32-unknown-unknown --no-default-features "${features[@]}" -e normal \
            > "$output/$selection-$backend.graph.txt"
        if [[ "$backend" == storage ]]; then forbidden=ic-stable-structures; else forbidden=ic-storage; fi
        if grep -Eq "(^|[[:space:]])$forbidden v" "$output/$selection-$backend.graph.txt"; then
            echo "Unexpected $forbidden in $backend consumer graph" >&2; exit 1
        fi
        cargo build --locked --offline --manifest-path "$manifest" -p "$package" --lib \
            --target wasm32-unknown-unknown --profile wasm-size --no-default-features "${features[@]}" \
            --target-dir target/storage-size-build > "$output/$selection-$backend.build.log" 2>&1 || {
                cat "$output/$selection-$backend.build.log" >&2; exit 1;
            }
        cp "target/storage-size-build/wasm32-unknown-unknown/wasm-size/${package//-/_}.wasm" "$output/$selection-$backend.wasm"
    done
done
cargo build --locked --offline --manifest-path "$manifest" -p ic-storage-benchmarks \
    --example size-consumers --target-dir target/runtime-qualification > "$output/driver-build.log" 2>&1 || {
        cat "$output/driver-build.log" >&2; exit 1;
    }
shasum -a 256 "$output/"*.wasm "$cli" "$server" target/runtime-qualification/debug/examples/size-consumers > "$output/artifacts.sha256"
shasum -a 256 -c "$output/source.sha256" > "$output/source-check.log"
env -u POCKET_IC_BIN -u IC_TESTKIT_POCKET_IC_URL "$cli" run --directory "$server_directory" --ttl 900 \
    --server-stdout "$output/server.stdout" --server-stderr "$output/server.stderr" \
    -- "$root/target/runtime-qualification/debug/examples/size-consumers" "$output" > "$output/runtime.log" 2>&1 || {
        cat "$output/runtime.log" >&2; exit 1;
    }
shasum -a 256 -c "$output/source.sha256" >> "$output/source-check.log"
shasum -a 256 -c "$output/artifacts.sha256" > "$output/artifacts-check.log"
cat "$output/sizes.csv"

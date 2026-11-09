#!/usr/bin/env bash
# The real caller's command order and failure propagation; Cargo is substituted.
set -euo pipefail
root="$(pwd -P)"
mkdir -p "$root/target/fixtures"
fixture="$(mktemp -d "$root/target/fixtures/storage-adoption-fixture.XXXXXX")"
echo "Retained adoption caller fixture: $fixture"
mkdir -p "$fixture/source/scripts/ci" "$fixture/source/ci" "$fixture/bin"
cp Makefile rust-toolchain.toml "$fixture/source/"
cp -R make "$fixture/source/"
cp ci/release-tools.env ci/runtime-tools.env "$fixture/source/ci/"
cp scripts/ci/check-dependency-adoption.sh "$fixture/source/scripts/ci/"
cp scripts/ci/check-make-execution.sh "$fixture/source/scripts/ci/"
bash scripts/ci/prepare-script-fixture.sh "$root" "$fixture/source" scripts/ci/check-cargo-graph.pl scripts/ci/validation-input-checkpoint.pl scripts/ci/verify-file-checksum.sh
for path in Cargo.toml Cargo.lock crates/*/Cargo.toml testing/runtime-qualification/Cargo.toml \
    testing/runtime-qualification/Cargo.lock testing/runtime-qualification/crates/*/Cargo.toml; do
    if [[ "$path" == */* ]]; then mkdir -p "$fixture/source/${path%/*}"; fi
    cp "$path" "$fixture/source/$path"
done
printf 'artifact must remain untouched\n' > "$fixture/source/existing-artifact"
cp "$fixture/source/existing-artifact" "$fixture/artifact.before"
export ADOPTION_CALLS="$fixture/calls"
cat > "$fixture/bin/cargo" <<'SH'
#!/usr/bin/env bash
set -euo pipefail
printf '%s\n' "$*" >> "$ADOPTION_CALLS"
case "$1" in
    metadata)
        printf '{}\n'
        if [[ "${ADOPTION_FAILURE:-}" == root-metadata && "$*" != *testing/runtime-qualification* ]]; then exit 23; fi
        if [[ "${ADOPTION_FAILURE:-}" == independent-metadata && "$*" == *testing/runtime-qualification* ]]; then exit 24; fi
        ;;
    tree)
        if [[ "$*" == *ic-storage-platform-qualification* ]]; then
            printf 'ic-storage-platform-qualification v0.0.0\nic-storage v0.64.0\nic-platform v0.18.0\nic-platform-runtime v0.18.0\nic-platform-runtime-guard v0.18.0\nic-platform-types v0.17.0\n'
            [[ "${ADOPTION_FAILURE:-}" != composition-policy ]] || printf 'ic-platform-runtime-guard v0.17.1\n'
        elif [[ "$*" == *ic-storage-default-qualification* ]]; then
            printf 'ic-storage-default-qualification v0.0.0\nic-storage v0.65.1\nic-wire v0.15.0\n'
            if [[ "$*" == *wasm32-unknown-unknown* ]]; then
                printf 'ic-platform-runtime v0.18.1\n'
                [[ "${ADOPTION_FAILURE:-}" != default-executor ]] || printf 'ic-platform-runtime feature "executor"\n'
            else
                [[ "${ADOPTION_FAILURE:-}" != default-native ]] || printf 'ic-platform-runtime v0.18.1\n'
            fi
        elif [[ "$PWD" == */testing/runtime-qualification ]]; then
            printf 'ic-storage-runtime-qualification v0.0.0\nic-testkit v0.95.0\nic-wire v0.18.0\n'
            [[ "${ADOPTION_FAILURE:-}" != independent-policy ]] || printf 'ic-testkit v0.94.0\n'
        else
            printf 'ic-storage v0.60.7\nic-metrics-storage v0.61.5\nic-logging-storage v0.60.5\nic-wire v0.13.3\n'
            if [[ "$*" == *wasm32-unknown-unknown* ]]; then
                printf 'ic-platform-runtime v0.18.0\n'
                [[ "${ADOPTION_FAILURE:-}" != runtime-policy ]] || printf 'ic-platform-runtime v0.17.1\n'
            else
                [[ "${ADOPTION_FAILURE:-}" != native-runtime ]] || printf 'ic-platform-runtime v0.18.0\n'
                [[ "${ADOPTION_FAILURE:-}" != native-wire-family ]] || printf 'ic-wire v0.15.0\n'
                [[ "${ADOPTION_FAILURE:-}" != native-metrics-wire ]] || printf 'ic-metrics feature "native-wire"\n'
            fi
        fi
        [[ "${ADOPTION_FAILURE:-}" != plausible-tree ]] || exit 25
        ;;
    clippy)
        printf 'plausible passing lint output\n'
        if [[ "${ADOPTION_FAILURE:-}" == root-lint && "$*" != *testing/runtime-qualification* ]]; then exit 26; fi
        if [[ "${ADOPTION_FAILURE:-}" == composition-lint && "$*" == *ic-storage-platform-qualification* ]]; then exit 28; fi
        if [[ "${ADOPTION_FAILURE:-}" == default-lint && "$*" == *ic-storage-default-qualification* ]]; then exit 29; fi
        if [[ "${ADOPTION_FAILURE:-}" == independent-lint && "$*" == *testing/runtime-qualification* ]]; then exit 27; fi
        ;;
    *) exit 99 ;;
esac
SH
chmod +x "$fixture/bin/cargo"
export PATH="$fixture/bin:$PATH"
cd "$fixture/source"
: > "$ADOPTION_CALLS"
make -j4 --no-print-directory check-dependency-adoption > "$fixture/success.log" 2>&1
grep -F "Dependency adoption evidence: $fixture/source/target/dependency-adoption/attempt." "$fixture/success.log" >/dev/null
[[ "$(wc -l < "$ADOPTION_CALLS")" == 15 ]]
awk 'NR<=2 && $1!="metadata" {exit 1} NR>2 && NR<=9 && $1!="tree" {exit 1} NR>9 && $1!="clippy" {exit 1}' "$ADOPTION_CALLS"
for failure in root-metadata independent-metadata runtime-policy native-runtime native-wire-family native-metrics-wire independent-policy default-native default-executor composition-policy plausible-tree root-lint independent-lint default-lint composition-lint; do
    : > "$ADOPTION_CALLS"
    if ADOPTION_FAILURE="$failure" make -j4 --no-print-directory check-dependency-adoption > "$fixture/$failure.log" 2>&1; then exit 1; fi
    case "$failure" in
        root-lint) [[ "$(rg -c '^clippy ' "$ADOPTION_CALLS")" == 1 ]] ;;
        default-lint) [[ "$(rg -c '^clippy ' "$ADOPTION_CALLS")" == 3 ]] ;;
        composition-lint) [[ "$(rg -c '^clippy ' "$ADOPTION_CALLS")" == 4 ]] ;;
        independent-lint) [[ "$(rg -c '^clippy ' "$ADOPTION_CALLS")" == 2 ]] ;;
        *) if rg -q '^clippy ' "$ADOPTION_CALLS"; then exit 1; fi ;;
    esac
    cmp existing-artifact "$fixture/artifact.before"
    cmp Cargo.lock "$root/Cargo.lock"
    cmp testing/runtime-qualification/Cargo.lock "$root/testing/runtime-qualification/Cargo.lock"
done
echo 'Actual parallel Make dispatch admits every graph before lint and preserves inputs/artifacts on failure.'

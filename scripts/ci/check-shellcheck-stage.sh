#!/usr/bin/env bash
# Opt-in exact-input reuse of the existing shell tooling check only.
set -euo pipefail
if [[ -z "${VALIDATION_STAGE_STATE:-}" ]]; then
    exec bash scripts/ci/qualify-shellcheck-stage.sh
fi
[[ $# == 1 && -n "${RELEASE_GROUP:-}" ]] || { echo 'stage reuse requires the selected compiler and RELEASE_GROUP' >&2; exit 2; }
export RUSTUP_TOOLCHAIN="$1" RUSTUP_AUTO_INSTALL=0 CARGO_NET_OFFLINE=true
root="$(pwd -P)"
record="$(mktemp -d "${TMPDIR:-/tmp}/storage-shellcheck-admission.XXXXXX")"
printf 'Retained ShellCheck admission: %s\n' "$record"
# Fresh policy/metadata and source/index identity are consumer admission, not an
# assertion that a prior whole gate or different source passed.
base="$(git rev-parse HEAD)"
git diff --cached --quiet
git ls-files --stage -z | shasum -a 256 > "$record/index.before"
bash vendor/toko-tooling/scripts/ci/cargo-release-group.sh plan "$RELEASE_GROUP" > "$record/group.json"
inputs=(Cargo.toml Cargo.lock ci/cargo-release-groups.json crates/*/Cargo.toml)
perl vendor/toko-tooling/scripts/ci/validation-input-checkpoint.pl "$root" '' "$record/inputs.json" vendor/toko-tooling/scripts/ci/verify-file-checksum.sh "${inputs[@]}"
source_id="$(printf '%s\n' "$base"; cat "$record/index.before" "$record/inputs.json")"
source_id="$(printf '%s' "$source_id" | shasum -a 256)"
source_id="${source_id%% *}"
shellcheck_bin="${SHELLCHECK_BIN:-$(command -v shellcheck)}"
jq -cnS --arg path "$PATH" --arg home "$HOME" --arg shellcheck "$shellcheck_bin" \
    '{PATH:$path,HOME:$home,LC_ALL:"C",SHELLCHECK_BIN:$shellcheck}' > "$record/environment.json"
# ShellCheck 0.11.0's native macOS payloads exceed 32 MiB (ARM64: 61,551,911
# bytes; Intel: 35,176,792). Admit their complete bytes within a fixed 64 MiB cap.
perl vendor/toko-tooling/scripts/ci/run-validation-stage.pl --state "$VALIDATION_STAGE_STATE" --stage shellcheck \
    --contract storage-shellcheck/1 --source "$source_id" --group "$RELEASE_GROUP" \
    --observe scripts/ci/observe-shellcheck-stage.sh --environment "$record/environment.json" --max-bytes 67108864 \
    -- bash scripts/ci/qualify-shellcheck-stage.sh
perl vendor/toko-tooling/scripts/ci/validation-input-checkpoint.pl "$root" "$record/inputs.json" "$record/after.json" vendor/toko-tooling/scripts/ci/verify-file-checksum.sh
git ls-files --stage -z | shasum -a 256 > "$record/index.after"
cmp "$record/index.before" "$record/index.after"
[[ "$(git rev-parse HEAD)" == "$base" ]]

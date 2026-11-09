#!/usr/bin/env bash
# Real consumer tooling qualification in a retained source copy; no release effects.
set -euo pipefail
root="$(pwd -P)"
[[ -f scripts/ci/run-validation-stage.pl ]] || { echo 'Adopt the reviewed Shared stage interface first.' >&2; exit 1; }
pin="$(sed -n 's/^channel = "\([^"]*\)"$/\1/p' rust-toolchain.toml)"
# shellcheck source=ci/release-tools.env
source ci/release-tools.env
editor="$(RUSTUP_TOOLCHAIN="$pin" bash scripts/dev/install-rust-tools.sh --consumer "$root" \
    --package ic-build --version "$IC_BUILD_RELEASE_METADATA_VERSION" --bin ic-build-release-metadata --profile debug --features cli --check)"
shellcheck_bin="${SHELLCHECK_BIN:-$(command -v shellcheck)}"
mkdir -p "$root/target/fixtures"
fixture="$(mktemp -d "$root/target/fixtures/storage-stage-qualification.XXXXXX")"
fixture="$(cd -P "$fixture" && pwd -P)"
echo "Retained stage qualification: $fixture"
parent_inputs=(Cargo.toml Cargo.lock ci/cargo-release-groups.json crates/*/Cargo.toml)
perl scripts/ci/validation-input-checkpoint.pl "$root" '' "$fixture/parent-inputs.json" scripts/ci/verify-file-checksum.sh "${parent_inputs[@]}"
git clone --quiet --shared "$root" "$fixture/source with spaces"
for file in Cargo.toml Cargo.lock Makefile rust-toolchain.toml .toko-tooling.snapshot; do cp "$file" "$fixture/source with spaces/"; done
for directory in ci crates releases scripts make .githooks testing; do cp -Rp "$directory" "$fixture/source with spaces/"; done
base_source="$(git rev-parse HEAD)"
git diff --cached --quiet
git ls-files --stage -z | shasum -a 256 > "$fixture/index.before"
cd "$fixture/source with spaces"
export RUSTUP_TOOLCHAIN="$pin" RUSTUP_AUTO_INSTALL=0 CARGO_NET_OFFLINE=true
inputs=(Cargo.toml Cargo.lock ci/cargo-release-groups.json crates/*/Cargo.toml)
for group in storage metrics-storage logging-storage; do
    bash scripts/ci/cargo-release-group.sh plan "$group" > "$fixture/$group.plan.json"
    perl scripts/ci/validation-input-checkpoint.pl "$PWD" '' "$fixture/$group.inputs.json" \
        scripts/ci/verify-file-checksum.sh "${inputs[@]}"
    status=0
    SHELLCHECK_BIN="$shellcheck_bin" VALIDATION_STAGE_STATE="$fixture/state" RELEASE_GROUP="$group" \
        make -j4 --no-print-directory check-shellcheck-stage > "$fixture/$group.log" 2>&1 || status=$?
    if [[ "$status" != 0 ]]; then
        cat "$fixture/$group.log" >&2 || :
        exit "$status"
    fi
    perl scripts/ci/validation-input-checkpoint.pl "$PWD" "$fixture/$group.inputs.json" "$fixture/$group.after.json" \
        scripts/ci/verify-file-checksum.sh
    if [[ "$group" == logging-storage ]]; then continue; fi
    previous="$(jq -r .previous "$fixture/$group.plan.json")"
    next="$(bash scripts/ci/next-release-version.sh "$previous" patch)"
    RELEASE_VERSION="$next" bash scripts/ci/cargo-release-group.sh plan "$group" > "$fixture/project.json"
    while IFS= read -r path; do
        jq --arg path "$path" '.manifests[] | select(.path == $path) | {version,dependencies}' "$fixture/project.json" > "$fixture/edit.json"
        "$editor" --max-input-bytes "$CARGO_RELEASE_METADATA_MAX_BYTES" group-manifest "$path" "$previous" "$next" "$fixture/edit.json" > "$fixture/candidate"
        cp "$fixture/candidate" "$path"
    done < <(jq -r '.manifests[].path' "$fixture/project.json")
    packages=()
    while IFS= read -r package; do packages+=("$package"); done < <(jq -r '.members[]' "$fixture/project.json")
    "$editor" --max-input-bytes "$CARGO_RELEASE_METADATA_MAX_BYTES" lock Cargo.lock "$previous" "$next" "${packages[@]}" > "$fixture/candidate.lock"
    cp "$fixture/candidate.lock" Cargo.lock
    bash scripts/ci/cargo-release-group.sh plan "$group" > "$fixture/projected.json"
    jq -n -e --slurpfile before "$fixture/$group.plan.json" --slurpfile after "$fixture/projected.json" '
        def unrelated($m): $m.versions | to_entries | map(select(.key as $n | $before[0].members | index($n) | not));
        unrelated($before[0]) == unrelated($after[0])' > /dev/null
done
find "$fixture/state/attempts" -name admission.json -exec cat {} + | jq -s '
    {schema:1,executed:map(select(.mode=="executed"))|length,reused:map(select(.mode=="reused"))|length,
     sources:map(.source)|unique,groups:map(.group)|sort,proofs:map(.execution_proof_sha256)|unique,
     timings:map({group,mode,elapsed_seconds})}' > "$fixture/result.json"
jq -e '.executed==1 and .reused==2 and (.sources|length)==3 and (.proofs|length)==1 and
    .groups==["logging-storage","metrics-storage","storage"]' "$fixture/result.json" > /dev/null
git -C "$root" ls-files --stage -z | shasum -a 256 > "$fixture/index.after"
cmp "$fixture/index.before" "$fixture/index.after"
[[ "$(git -C "$root" rev-parse HEAD)" == "$base_source" ]]
perl "$root/scripts/ci/validation-input-checkpoint.pl" "$root" "$fixture/parent-inputs.json" \
    "$fixture/parent-after.json" "$root/scripts/ci/verify-file-checksum.sh"
cat "$fixture/result.json"

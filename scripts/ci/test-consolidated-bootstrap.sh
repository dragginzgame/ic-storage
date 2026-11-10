#!/usr/bin/env bash
# Real offline Cargo/editor, with interrupted metadata only in disposable copies.
set -euo pipefail
unset RELEASE_CACHE_PREPARE MAKEFLAGS MFLAGS MAKEOVERRIDES GNUMAKEFLAGS MAKEFILES
root="$(pwd -P)"
pin="$(sed -n 's/^channel = "\([^"]*\)"$/\1/p' rust-toolchain.toml)"
# shellcheck source=ci/release-tools.env
source ci/release-tools.env
editor="$(RUSTUP_TOOLCHAIN="$pin" bash scripts/dev/install-rust-tools.sh --consumer "$root" \
    --package ic-build --version "$IC_BUILD_RELEASE_METADATA_VERSION" --bin ic-build-release-metadata --profile debug --features cli --check)"
mkdir -p "$root/target/fixtures"
fixture="$(mktemp -d "$root/target/fixtures/ic-storage-group-bootstrap.XXXXXX")"
echo "Retained group bootstrap fixture: $fixture"
cp Cargo.toml Cargo.lock LICENSE.md rust-toolchain.toml CHANGELOG.md "$fixture/"
mkdir -p "$fixture/docs"
cp -R docs/changelog "$fixture/docs/"
cp -R releases "$fixture/"
cp -R crates "$fixture/"
# Group metadata includes the independent benchmark's catalog and lockfile.
# Copy the declared roster so Shared can admit the same files as a real release.
while IFS= read -r path; do
    mkdir -p "$(dirname "$fixture/$path")"
    cp "$path" "$fixture/$path"
done < <(jq -r '.groups[].metadata_files[]?, .shared_metadata_files[]?' ci/cargo-release-groups.json)
mkdir -p "$fixture/scripts/dev" "$fixture/scripts/ci" "$fixture/ci"
cp scripts/dev/run-repo-tool.sh "$fixture/scripts/dev/"
bash scripts/ci/prepare-script-fixture.sh "$root" "$fixture" scripts/ci/observe-cargo-release-bootstrap.sh
cp scripts/ci/read-storage-packages.sh scripts/ci/read-cargo-workspace-version.sh scripts/ci/validate-release-inputs.pl scripts/ci/StableVersion.pm "$fixture/scripts/ci/"
cp ci/release-tools.env ci/cargo-release-groups.json "$fixture/ci/"
cat > "$fixture/scripts/dev/install-rust-tools.sh" <<'STUB'
#!/usr/bin/env bash
set -euo pipefail
[[ "${*: -1}" == --check ]] || exit 1
printf '%s\n' "$BOOTSTRAP_QUALIFIED_EDITOR"
STUB
export BOOTSTRAP_QUALIFIED_EDITOR="$editor"
export CARGO_NET_OFFLINE=true RUSTUP_AUTO_INSTALL=0
export CARGO_TARGET_DIR="$fixture/target"
cd "$fixture"
cp Cargo.toml original.toml
cp Cargo.lock original.lock
cargo "+$pin" metadata --locked --offline --format-version 1 > baseline-metadata.json
while IFS= read -r group; do
    export RELEASE_GROUP="$group"
    cp original.toml Cargo.toml
    cp original.lock Cargo.lock
    previous="$(bash scripts/dev/run-repo-tool.sh "$pin" version)"
    next="$(bash "$root/scripts/ci/next-release-version.sh" "$previous" minor)"
    RELEASE_VERSION="$next" RUSTUP_TOOLCHAIN="$pin" bash "$root/scripts/ci/cargo-release-group.sh" plan "$group" > "$group.plan.json"
    cp Cargo.lock "$group.lock"
    while IFS= read -r package; do
        "$editor" --max-input-bytes "$CARGO_RELEASE_METADATA_MAX_BYTES" lock "$group.lock" "$previous" "$next" "$package" > next.lock
        mv next.lock "$group.lock"
    done < <(jq -r '.members[]' "$group.plan.json")
    while IFS= read -r path; do
        cp "$path" "$path.original"
        jq --arg path "$path" '.manifests[] | select(.path == $path) | {version,dependencies}' "$group.plan.json" > edit.json
        "$editor" --max-input-bytes "$CARGO_RELEASE_METADATA_MAX_BYTES" group-manifest "$path.original" "$previous" "$next" edit.json > "$path.candidate"
    done < <(jq -r '.manifests[].path' "$group.plan.json")
    for state in lock-first manifest-first; do
        cp original.lock Cargo.lock
        while IFS= read -r path; do cp "$path.original" "$path"; done < <(jq -r '.manifests[].path' "$group.plan.json")
        expected="$previous"
        if [[ "$state" == lock-first ]]; then cp "$group.lock" Cargo.lock
        else
            while IFS= read -r path; do cp "$path.candidate" "$path"; done < <(jq -r '.manifests[].path' "$group.plan.json")
            expected="$next"
        fi
        shasum -a 256 Cargo.toml Cargo.lock crates/*/Cargo.toml > "$group.$state.inputs.sha256"
        bash scripts/dev/run-repo-tool.sh "$pin" version > "$group.$state.stdout" 2> "$group.$state.stderr"
        [[ "$(cat "$group.$state.stdout")" == "$expected" ]]
        shasum -a 256 -c "$group.$state.inputs.sha256"
    done
    # Actual coherent group projection: resolve every member, then compile the
    # unaffected core consumer while retaining other groups' exact identities.
    while IFS= read -r path; do cp "$path.candidate" "$path"; done < <(jq -r '.manifests[].path' "$group.plan.json")
    cp "$group.lock" Cargo.lock
    shasum -a 256 Cargo.toml Cargo.lock crates/*/Cargo.toml > "$group.admission.inputs.sha256"
    cargo "+$pin" fetch --locked --offline
    shasum -a 256 -c "$group.admission.inputs.sha256"
    cargo "+$pin" metadata --locked --offline --format-version 1 > "$group.metadata.json"
    jq -e --slurpfile before baseline-metadata.json --slurpfile plan "$group.plan.json" '
      def unrelated($m): [$m.packages[] | select(.name as $n | $plan[0].members | index($n) | not) | {id,version}] | sort_by(.id);
      unrelated(.) == unrelated($before[0])' "$group.metadata.json" > /dev/null
    cargo "+$pin" check -p ic-storage --lib --locked --offline > "$group.consumer.log" 2>&1
    while IFS= read -r path; do cp "$path.original" "$path"; done < <(jq -r '.manifests[].path' "$group.plan.json")
    cp original.lock Cargo.lock
 done < <(jq -r '.groups[].name' ci/cargo-release-groups.json)
echo 'All three groups bootstrap both interrupted metadata states without modifying source inputs.'

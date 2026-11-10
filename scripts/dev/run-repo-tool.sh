#!/usr/bin/env bash
# Keep the Rust adapter reachable between atomic manifest/lockfile replacements.
set -euo pipefail
export RUSTUP_AUTO_INSTALL=0
# Cargo's --offline flags scope helper compilation/discovery. Preserve the
# caller's network setting for commands the helper dispatches, such as publish.
if [[ $# -lt 2 ]]; then
    echo 'usage: run-repo-tool.sh <toolchain> <command> [arguments]' >&2
    exit 2
fi
toolchain="$1"
shift
check_dependency_cache() {
    local status
    local fetch=(fetch --locked --offline)
    # Only the documented release entry points select network preparation.
    # Cargo still honours explicit offline environment/configuration settings.
    if [[ "${RELEASE_CACHE_PREPARE:-0}" == 1 ]]; then
        fetch=(fetch --locked)
    fi
    if cargo "+$toolchain" "${fetch[@]}" "$@"; then
        # The compiled adapter and its validation children stay offline.
        unset RELEASE_CACHE_PREPARE
        return
    else
        status=$?
    fi
    echo 'Dependency preparation failed. Run make fetch-dependencies to populate the selected cache; resolve any reported lockfile or network errors first. Explicit offline settings remain in effect.' >&2
    exit "$status"
}
case "$1" in
    version|release-*|qualify-release|publish)
        [[ -n "${RELEASE_GROUP:-}" ]] || { echo 'Select RELEASE_GROUP explicitly.' >&2; exit 2; }
        jq -e --arg group "$RELEASE_GROUP" 'any(.groups[]; .name == $group)' ci/cargo-release-groups.json > /dev/null || exit 2
        ;;
esac
# Shared observes every group before bootstrap/cache effects.
observer=scripts/ci/observe-cargo-release-bootstrap.sh
observe_bootstrap() { RUSTUP_TOOLCHAIN="$toolchain" bash "$observer" observe; }
observation="$(observe_bootstrap)"
check_observation() {
    local current
    current="$(observe_bootstrap)" || return
    [[ "$current" == "$observation" ]] || { echo 'bootstrap inputs changed during helper dispatch' >&2; return 1; }
}
if [[ "$(printf '%s\n' "$observation" | jq -er '.coherent')" == true ]]; then
    check_dependency_cache
    check_observation
    exec cargo "+$toolchain" run --locked --offline --quiet --example repo-tool -- "$@"
fi
target="$(printf '%s\n' "$observation" | jq -er '.target_directory' && printf '.')"
target="${target%$'\n.'}"
[[ "$target" == /* && "$target" != *$'\n'* && "$target" != *$'\r'* ]] || { echo 'invalid bootstrap target directory' >&2; exit 1; }
mkdir -p "$target/repo-tool-bootstrap"
scratch="$(mktemp -d "$target/repo-tool-bootstrap/attempt.XXXXXX")"
echo "Retained repo-tool bootstrap metadata: $scratch" >&2
cp Cargo.lock LICENSE.md Cargo.toml "$scratch/"
cp -R crates "$scratch/"
mkdir -p "$scratch/ci"
cp ci/cargo-release-groups.json "$scratch/ci/"
# shellcheck source=ci/release-tools.env
source ci/release-tools.env
editor_admission=(--check)
if [[ "${RELEASE_CACHE_PREPARE:-0}" == 1 ]]; then editor_admission=(); fi
editor="$(RUSTUP_TOOLCHAIN="$toolchain" bash scripts/dev/install-rust-tools.sh \
    --consumer "$PWD" --package ic-build --version "$IC_BUILD_RELEASE_METADATA_VERSION" \
    --bin ic-build-release-metadata --profile debug --features cli ${editor_admission[@]+"${editor_admission[@]}"})"
check_observation
printf '%s\n' "$observation" | jq '.inputs' > "$scratch/observed-inputs.json"
perl "${observer%/*}/validation-input-checkpoint.pl" "$scratch" "$scratch/observed-inputs.json" \
    "$scratch/copied-inputs.json" "${observer%/*}/verify-file-checksum.sh"
printf '%s\n' "$observation" | jq '.catalog_edit' > "$scratch/root-edit.json"
"$editor" --max-input-bytes "$CARGO_RELEASE_METADATA_MAX_BYTES" group-manifest "$scratch/Cargo.toml" \
    0.0.0 0.0.0 "$scratch/root-edit.json" > "$scratch/root.candidate"
mv "$scratch/root.candidate" "$scratch/Cargo.toml"
printf '{"version":"package","dependencies":[]}\n' > "$scratch/member-edit.json"
printf '%s\n' "$observation" | jq -er '.workspace_root as $root | .groups[].members[] |
    [(.manifest | ltrimstr($root + "/")),.declared,.locked] | @tsv' > "$scratch/members"
while IFS=$'\t' read -r manifest declared locked; do
    "$editor" --max-input-bytes "$CARGO_RELEASE_METADATA_MAX_BYTES" group-manifest "$scratch/$manifest" \
        "$declared" "$locked" "$scratch/member-edit.json" > "$scratch/member.candidate"
    mv "$scratch/member.candidate" "$scratch/$manifest"
done < "$scratch/members"
check_dependency_cache --manifest-path "$scratch/Cargo.toml"
check_observation
exec cargo "+$toolchain" run --locked --offline --quiet \
    --manifest-path "$scratch/Cargo.toml" --target-dir "$target/repo-tool-bootstrap/build" \
    --example repo-tool -- "$@"

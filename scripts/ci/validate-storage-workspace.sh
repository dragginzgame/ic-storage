#!/usr/bin/env bash
# Consumer-owned full-gate dispatch over the same policy used for group plans.
set -euo pipefail
[[ -z "${VALIDATION_PLAN:-}" ]] || { echo "Full validation refuses an inherited group plan" >&2; exit 1; }
mkdir -p target/validation-rosters
selection="$(mktemp -d target/validation-rosters/selection.XXXXXX)"
jq -cS '[.groups[].name]' ci/cargo-release-groups.json > "$selection/groups.json"
roster="$(perl vendor/toko-tooling/scripts/ci/validation-evidence.pl roster "$PWD" ci/release-validation.json "$selection/groups.json")"
targets=()
while IFS= read -r target; do targets+=("$target"); done <<< "$roster"
VALIDATION_REPOSITORY_ROOT="$PWD" bash vendor/toko-tooling/scripts/ci/run-validation-targets.sh --fail-fast "${targets[@]}"

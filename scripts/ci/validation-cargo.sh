#!/usr/bin/env bash
# Consumer-owned Cargo scope; only an immutable runner-admitted plan narrows it.
set -euo pipefail
[[ $# == 3 ]] || exit 2
stage=$1 pin=$2 plan=$3
scope=(--workspace)
if [[ -n "$plan" ]]; then
    [[ -n "${VALIDATION_INPUT_BASELINE:-}" && -n "${RELEASE_SOURCE:-}" && -n "${RELEASE_GROUP:-}" ]] || {
        echo 'Cargo validation requires an admitted plan checkpoint, source and group' >&2
        exit 1
    }
    expected="$(jq -er --arg path "$plan" '.[$path] | strings' "$VALIDATION_INPUT_BASELINE")"
    actual="$(bash vendor/toko-tooling/scripts/ci/verify-file-checksum.sh --print sha256 "$plan")"
    [[ "$actual" == "$expected" ]] || {
        echo 'Cargo validation plan differs from its admitted checkpoint' >&2
        exit 1
    }
    selection="$(jq -er --arg source "$RELEASE_SOURCE" --arg group "$RELEASE_GROUP" \
        'select(.schema == 1 and .source == $source and .group == $group and .policy == "ci/release-validation.json") | .scope' "$plan")"
    case "$selection:$RELEASE_GROUP" in
        group:metrics-storage|group:logging-storage) scope=(--package "ic-$RELEASE_GROUP") ;;
        workspace:*) ;;
        *) echo 'Unknown Cargo validation scope' >&2; exit 1 ;;
    esac
fi
common=("${scope[@]}" --locked --offline)
case "$stage" in
    lint) cargo "+$pin" clippy "${common[@]}" --all-targets -- -D warnings ;;
    features) cargo "+$pin" check "${common[@]}" --all-targets --features ic-storage/test-support ;;
    docs) RUSTDOCFLAGS='-D warnings' cargo "+$pin" doc "${common[@]}" --no-deps ;;
    wasm) cargo "+$pin" check "${common[@]}" --target wasm32-unknown-unknown --tests ;;
    *) echo 'Unknown Cargo validation stage' >&2; exit 2 ;;
esac

#!/usr/bin/env bash
# Exercise the actual stage Make recipes with captured Cargo effects.
set -euo pipefail
unset MAKEFLAGS MFLAGS MAKEOVERRIDES GNUMAKEFLAGS MAKEFILES VALIDATION_PLAN
mkdir -p target/fixtures
fixture="$(mktemp -d "$PWD/target/fixtures/validation-cargo.XXXXXX")"
echo "Retained Cargo scope fixture: $fixture"
mkdir "$fixture/bin"
cat > "$fixture/bin/cargo" <<'STUB'
#!/usr/bin/env bash
set -euo pipefail
printf '%s\n' "$*" >> "$CARGO_SCOPE_EVENTS"
[[ "${RUSTDOCFLAGS:-}" == '-D warnings' || "${2:-}" != doc ]]
exit "${CARGO_SCOPE_STATUS:-0}"
STUB
chmod +x "$fixture/bin/cargo"
export CARGO_SCOPE_EVENTS="$fixture/calls"
export PATH="$fixture/bin:$PATH"
plan="${fixture#"$PWD/"}/plan.json"
export VALIDATION_INPUT_BASELINE="$fixture/checkpoint.json"
source_id=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
pin="$(sed -n 's/^channel = "\([^"]*\)"$/\1/p' rust-toolchain.toml)"
for group in workspace storage metrics-storage logging-storage; do
    set --
    scope=workspace
    owner=storage
    expected=--workspace
    if [[ "$group" != workspace ]]; then
        owner=$group
        if [[ "$group" != storage ]]; then scope=group; expected="--package ic-$group"; fi
        jq -cnS --arg source "$source_id" --arg group "$owner" --arg scope "$scope" \
            '{schema:1,source:$source,group:$group,scope:$scope,policy:"ci/release-validation.json"}' > "$plan"
        hash="$(bash scripts/ci/verify-file-checksum.sh --print sha256 "$plan")"
        jq -cnS --arg path "$plan" --arg hash "$hash" '{($path):$hash}' > "$VALIDATION_INPUT_BASELINE"
        set -- "VALIDATION_PLAN=$plan" "RELEASE_SOURCE=$source_id" "RELEASE_GROUP=$owner"
    fi
    : > "$CARGO_SCOPE_EVENTS"
    for stage in lint features docs wasm; do
        make --no-print-directory "validate-$stage" "$@" > "$fixture/$group-$stage.log" 2>&1
    done
    printf '%s\n' \
        "+$pin clippy $expected --locked --offline --all-targets -- -D warnings" \
        "+$pin check $expected --locked --offline --all-targets --features ic-storage/test-support" \
        "+$pin doc $expected --locked --offline --no-deps" \
        "+$pin check $expected --locked --offline --target wasm32-unknown-unknown --tests" > "$fixture/expected"
    cmp "$fixture/expected" "$CARGO_SCOPE_EVENTS"
    if CARGO_SCOPE_STATUS=19 make --no-print-directory validate-lint "$@" > "$fixture/$group-failure.log" 2>&1; then
        echo 'Cargo failure was ignored' >&2; exit 1
    fi
done
# Inherited selections must never narrow an explicitly full gate.
if make --no-print-directory validate-toolchain "VALIDATION_PLAN=$plan" > "$fixture/full-refusal.log" 2>&1; then exit 1; fi
: > "$CARGO_SCOPE_EVENTS"
printf ' ' >> "$plan"
if make --no-print-directory validate-lint "$@" > "$fixture/drift-refusal.log" 2>&1; then exit 1; fi
[[ ! -s "$CARGO_SCOPE_EVENTS" ]]
# A plan without the runner checkpoint cannot authorize a package selection.
if env -u VALIDATION_INPUT_BASELINE make --no-print-directory validate-lint "$@" > "$fixture/no-checkpoint.log" 2>&1; then exit 1; fi
[[ ! -s "$CARGO_SCOPE_EVENTS" ]]
cat > "$fixture/bin/repo-tool" <<'STUB'
#!/usr/bin/env bash
printf '%s\n' "$*" > "$CARGO_SCOPE_EVENTS"
STUB
chmod +x "$fixture/bin/repo-tool"
make --no-print-directory validate-ci "TOOL=$fixture/bin/repo-tool" \
    CHANGE_EVENT=pull_request CHANGE_REF=refs/heads/main "CHANGE_HEAD=$source_id" \
    CHANGE_BASE=bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb > "$fixture/ci-dispatch.log" 2>&1
printf '%s\n' "validate-ci --event pull_request --base bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb --head $source_id --reference refs/heads/main" > "$fixture/ci-expected"
cmp "$fixture/ci-expected" "$CARGO_SCOPE_EVENTS"
echo 'Workspace/group Cargo scopes, failure propagation and stale/unadmitted plans pass.'

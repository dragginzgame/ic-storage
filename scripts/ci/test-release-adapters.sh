#!/usr/bin/env bash
# Exercise common dispatch and consumer-specific release adapters without effects.
set -euo pipefail
# This independent fixture owns its selections, not the invoking release's.
unset MAKEFLAGS MFLAGS MAKEOVERRIDES GNUMAKEFLAGS MAKEFILES
unset VALIDATION_REPOSITORY_ROOT VALIDATION_RUNNER_SNAPSHOT_PATH
unset RELEASE_CACHE_PREPARE VALIDATION_PLAN
unset RELEASE_GROUPS RELEASE_PUBLISH_TARGET RELEASE_SETUP_TARGET RELEASE_SETUP_MODE RELEASE_BATCH_ACTIVE
export RELEASE_DELIVERY=direct
export RELEASE_GROUP=storage
# Keep Make status/warning assertions independent of the caller's locale.
export LC_ALL=C
# shellcheck source=ci/release-tools.env
source ci/release-tools.env
# Resolve relative script entry points without CDPATH output or newline loss.
ROOT="${BASH_SOURCE[0]}"
[[ "$ROOT" == /* ]] || ROOT="$PWD/$ROOT"
ROOT="$(cd -P "${ROOT%/*}/../.." && printf '%s/.' "$PWD")"
ROOT="${ROOT%/.}"
FIXTURE_TOOLCHAIN="$(sed -n 's/^channel = "\([^"]*\)"$/\1/p' "$ROOT/rust-toolchain.toml")"
[[ "$FIXTURE_TOOLCHAIN" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || exit 1
export FIXTURE_TOOLCHAIN
RELEASE_BATCH_ACTIVE=1 bash "$ROOT/scripts/ci/check-release-commands.sh" "$ROOT" rust-toolchain.toml ci/release-tools.env ci/runtime-tools.env ci/tool-versions.env make/consumer.mk make/tools.mk make/release.mk make/rust-format.mk make/execution.mk scripts/ci/check-make-execution.sh scripts/ci/check-release-delivery.sh
mkdir -p "$ROOT/target/fixtures"
FIXTURE="$(mktemp -d "$ROOT/target/fixtures/ic-storage-release-adapters.XXXXXX")"
FIXTURE="$(CDPATH="" cd -P -- "$FIXTURE" && printf '%s/.' "$PWD")"
FIXTURE="${FIXTURE%/.}"
# shellcheck source=scripts/ci/fixture-lifecycle.sh
source "$ROOT/scripts/ci/fixture-lifecycle.sh"
# shellcheck source=scripts/ci/make-adapter-probes.sh
source "$ROOT/scripts/ci/make-adapter-probes.sh"
fixture_complete=false
finish_fixture() {
    local status="$1" command="$2" path
    if [[ "$status" != 0 || "$fixture_complete" != true ]]; then
        printf 'Release-adapter fixture failed in %s: %s\n' "$PWD" "$command" >&2
        for path in "$FIXTURE/jobserver"/compiler-*.log "$FIXTURE/bootstrap/bootstrap.log" \
            "$FIXTURE/bootstrap"/release-cache-*.log; do
            if [[ -f "$path" ]]; then
                printf 'Retained diagnostic: %s\n' "$path" >&2
                tail -n 40 "$path" >&2
            fi
        done
    fi
    fixture_finish "$status" "$FIXTURE" "Consumer release-adapter fixture retained"
}
trap 'finish_fixture "$?" "$BASH_COMMAND"' EXIT
readonly suite_root="$FIXTURE"

prepare_make_fixture() {
    local directory="$1"
mkdir -p "$directory/ci" "$directory/make"
cp "$ROOT/Makefile" "$ROOT/rust-toolchain.toml" "$directory/"
cp "$ROOT/ci/release-tools.env" "$ROOT/ci/runtime-tools.env" "$ROOT/ci/tool-versions.env" "$directory/ci/"
cp "$ROOT/make/consumer.mk" "$ROOT/make/tools.mk" "$ROOT/make/release.mk" "$ROOT/make/rust-format.mk" "$ROOT/make/execution.mk" "$directory/make/"
cp "$ROOT/ci/release-validation.json" "$ROOT/ci/cargo-release-groups.json" "$directory/ci/"
mkdir -p "$directory/scripts/ci"
cp "$ROOT/scripts/ci/check-make-execution.sh" "$ROOT/scripts/ci/check-release-delivery.sh" "$directory/scripts/ci/"
cp "$ROOT/scripts/ci/validation-cargo.sh" "$ROOT/scripts/ci/validate-storage-workspace.sh" "$ROOT/scripts/ci/validation-evidence.pl" "$ROOT/scripts/ci/run-validation-targets.sh" "$ROOT/scripts/ci/verify-file-checksum.sh" "$ROOT/scripts/ci/validation-input-checkpoint.pl" "$ROOT/scripts/ci/release-validation-plan.pl" "$ROOT/scripts/ci/ValidationPlan.pm" "$directory/scripts/ci/"
}

test_make_admission() (
    # Keep child exit handling and environment local; the outer trap owns retention.
    trap - EXIT
    # Exercise hooks inside the admitted batch; outer dispatch is checked separately.
    export RELEASE_BATCH_ACTIVE=1
    local FIXTURE="$suite_root/make"
    prepare_make_fixture "$FIXTURE"
cat > "$FIXTURE/scripts/ci/run-release.sh" <<'STUB'
#!/usr/bin/env bash
set -euo pipefail
[[ "$RELEASE_DELIVERY" == direct ]] || exit 1
[[ "$RELEASE_CACHE_PREPARE" == 1 ]] || exit 1
printf '%s\n' "$*" >> runner-events
exit "${RELEASE_TEST_STATUS:-0}"
STUB
for target in release-patch release-minor release-major release-resume qualify-release publish publish-dry-run; do
    if (cd "$FIXTURE"; env -u RELEASE_GROUP make --no-print-directory "$target") > "$FIXTURE/no-group-$target.log" 2>&1; then
        echo 'missing group selection was admitted' >&2; exit 1
    fi
    [[ ! -e "$FIXTURE/runner-events" ]] || exit 1
done
# Shared's selected test-release-commands.sh owns exhaustive flag spellings,
# combinations and replacement permutations. Here qualify the real consumer
# Makefile: all aliases refuse -i, and patch represents every forbidden family.
for target in release-patch release-minor release-major release-resume; do
    modes=(-i)
    if [[ "$target" == release-patch ]]; then modes=(-i -n -t -q); fi
    for mode in "${modes[@]}"; do
        for source in direct inherited; do
            status=0
            if [[ "$source" == direct ]]; then
                (cd "$FIXTURE"; RELEASE_TEST_STATUS=23 make "$mode" "$target" --no-print-directory) \
                    > "$FIXTURE/mode-$target-$mode-$source.log" 2>&1 || status=$?
            else
                (cd "$FIXTURE"; MAKEFLAGS="$mode" RELEASE_TEST_STATUS=23 make "$target" --no-print-directory) \
                    > "$FIXTURE/mode-$target-$mode-$source.log" 2>&1 || status=$?
            fi
            [[ "$status" == 2 && ! -e "$FIXTURE/runner-events" ]] || exit 1
        done
    done
done
for target in release-patch release-minor release-major release-resume; do
    for source in command-line makefile; do
        status=0
        if [[ "$source" == command-line ]]; then
            (cd "$FIXTURE"; make -i --no-print-directory "$target" MAKEFLAGS= MFLAGS=) \
                > "$FIXTURE/mflags-$target-$source.log" 2>&1 || status=$?
        else
            printf 'MFLAGS :=\n' > "$FIXTURE/erased-mflags.mk"
            (cd "$FIXTURE"; make -i --no-print-directory -f erased-mflags.mk -f Makefile "$target" MAKEFLAGS=) \
                > "$FIXTURE/mflags-$target-$source.log" 2>&1 || status=$?
        fi
        [[ "$status" == 2 && ! -e "$FIXTURE/runner-events" ]] || exit 1
    done
done
for target in release-patch release-minor release-major release-resume; do
    for delivery in pr invalid; do
        if (cd "$FIXTURE"; make --no-print-directory "$target" RELEASE_DELIVERY="$delivery") \
            > "$FIXTURE/command-$target-$delivery.log" 2>&1; then
            echo 'unsupported release delivery accepted' >&2; exit 1
        fi
        if (cd "$FIXTURE"; RELEASE_DELIVERY="$delivery" make --no-print-directory "$target") \
            > "$FIXTURE/environment-$target-$delivery.log" 2>&1; then
            echo 'inherited unsupported release delivery accepted' >&2; exit 1
        fi
        [[ ! -e "$FIXTURE/runner-events" ]] || exit 1
    done
done
# Parallel qualification must refuse missing tools before any build/test phase.
# Substitute effects where the recursive production recipes actually dispatch.
# Replacing recipes only in the parent would leave the child using missing scripts.
mkdir -p "$FIXTURE/scripts/dev"
cat > "$FIXTURE/scripts/ci/verify-shared-tooling-snapshot.sh" <<'STUB'
#!/usr/bin/env bash
root="$(pwd -P)"
[[ "$#" == 4 && "$*" == "--consumer $root --manifest .shared-tooling.snapshot" ]] || exit 1
STUB
cat > "$FIXTURE/scripts/ci/check-format-tools.sh" <<'STUB'
#!/usr/bin/env bash
[[ "$#" == 2 && "$*" == '2.1.4 cargo' && "$RUSTUP_TOOLCHAIN" == "$FIXTURE_TOOLCHAIN" ]] || exit 1
STUB
cat > "$FIXTURE/scripts/dev/install-host-tools.sh" <<'STUB'
#!/usr/bin/env bash
root="$(pwd -P)"
[[ "$#" == 5 && "$*" == "--consumer $root --versions $root/ci/tool-versions.env --check" ]] || exit 1
printf '%s\n' "$*" > host-admission-events
echo 'fixture host admission failure' >&2
exit 23
STUB
cat > "$FIXTURE/scripts/ci/check-dependency-pins.sh" <<'STUB'
#!/usr/bin/env bash
printf 'reached\n' > build-events
exit 23
STUB
status=0
(cd "$FIXTURE"; make -j2 --no-print-directory validate-toolchain) \
    > "$FIXTURE/tool-ordering.log" 2>&1 || status=$?
[[ "$status" == 2 && ! -e "$FIXTURE/build-events" ]] || exit 1
(cd -P "$FIXTURE"; [[ "$(cat host-admission-events)" == "--consumer $PWD --versions $PWD/ci/tool-versions.env --check" ]]) || exit 1
grep -F 'fixture host admission failure' "$FIXTURE/tool-ordering.log"
grep -E 'host-tools-check.*Error 23' "$FIXTURE/tool-ordering.log"
# Memory explicitly selects IC tools after the common sets. Exercise the actual
# aggregate with substitute leaves, including its product failure propagation.
for kind in host rust ic; do
    cat > "$FIXTURE/scripts/dev/install-$kind-tools.sh" <<'STUB'
#!/usr/bin/env bash
set -euo pipefail
kind="${0##*/install-}"
kind="${kind%-tools.sh}"
[[ "$1" == --consumer && "$2" == "$(pwd -P)" ]] || exit 1
if [[ "$kind" == rust && "$3" == --package ]]; then
    [[ "$*" == "--consumer $PWD --package ic-build --version $IC_BUILD_RELEASE_METADATA_VERSION --bin ic-build-release-metadata --profile debug --features cli" ||
       "$*" == "--consumer $PWD --package ic-build --version $IC_BUILD_RELEASE_METADATA_VERSION --bin ic-build-release-metadata --profile debug --features cli --check" ]] || exit 1
    printf 'editor %s\n' "${13:-install}" >> aggregate-events
    exit 0
fi
case "$kind" in
    host|rust) [[ "$3" == --versions && "$4" == "$PWD/ci/tool-versions.env" ]] || exit 1 ;;
    ic)
        [[ "$3" == --pins && "$4" == "$PWD/ci/ic-tools.tsv" &&
            "$5" == --tool && "$6" == wasm-opt ]] || exit 1
        shift 2 ;;
esac
printf '%s %s\n' "$kind" "${5:-install}" >> aggregate-events
[[ "$kind" != ic || "${IC_ADMISSION_FAIL:-0}" != 1 ]] || exit 23
STUB
done
for target in install-tools tools-check; do
    rm -f "$FIXTURE/aggregate-events"
    (cd "$FIXTURE"; make -j2 --no-print-directory "$target") > "$FIXTURE/$target.log" 2>&1
    if [[ "$target" == install-tools ]]; then
        printf 'ic --preflight\nrust --preflight\nhost install\nrust install\nic install\neditor install\n' > "$FIXTURE/aggregate-expected"
    else
        printf 'host --check\nrust --check\nic --check\neditor --check\n' > "$FIXTURE/aggregate-expected"
    fi
    cmp "$FIXTURE/aggregate-expected" "$FIXTURE/aggregate-events"
    status=0
    (cd "$FIXTURE"; IC_ADMISSION_FAIL=1 make -j2 --no-print-directory "$target") \
        > "$FIXTURE/$target-ic-failure.log" 2>&1 || status=$?
    [[ "$status" == 2 ]] || exit 1
done
# Runtime admission must preserve busy status without suggesting installation.
cat > "$FIXTURE/runtime-cli" <<'STUB'
#!/usr/bin/env bash
set -euo pipefail
printf '%s\n' "$*" >> "$RUNTIME_EVENTS"
if [[ "$1" == --check ]]; then status="$RUNTIME_CLI_STATUS"
else status="$RUNTIME_SERVER_STATUS"; fi
if [[ "$status" != 0 ]]; then
    printf 'runtime admission refused: %s\n' "$status" >&2
    exit "$status"
fi
printf '%s\n' "$0"
STUB
chmod +x "$FIXTURE/runtime-cli"
for selection in ready busy-cli busy-server missing-cli missing-server; do
    cli_status=0 server_status=0
    case "$selection" in
        busy-cli) cli_status=75 ;;
        busy-server) server_status=75 ;;
        missing-cli) cli_status=1 ;;
        missing-server) server_status=1 ;;
    esac
    status=0
    (cd "$FIXTURE"; RUNTIME_CLI_STATUS="$cli_status" RUNTIME_SERVER_STATUS="$server_status" \
        RUNTIME_EVENTS="$FIXTURE/runtime-$selection.events" \
        make --no-print-directory runtime-server-check TESTKIT_CLI="$FIXTURE/runtime-cli") \
        > "$FIXTURE/runtime-$selection.log" 2>&1 || status=$?
    if [[ "$selection" == ready ]]; then
        [[ "$status" == 0 ]] || exit 1
    else
        [[ "$status" == 2 ]] || exit 1
        if [[ "$selection" == busy-* ]]; then
            rg -q 'Error 75' "$FIXTURE/runtime-$selection.log" || exit 1
            if rg -q 'Prepare the selected CLI' "$FIXTURE/runtime-$selection.log"; then exit 1; fi
        else
            rg -q 'Prepare the selected CLI' "$FIXTURE/runtime-$selection.log" || exit 1
        fi
    fi
    expected_calls=2
    if [[ "$selection" == *-cli ]]; then expected_calls=1; fi
    [[ "$(wc -l < "$FIXTURE/runtime-$selection.events" | tr -d ' ')" == "$expected_calls" ]] || exit 1
done
# The actual validation recipe must finish Clippy before later stages, even with
# parallel Make. Substitute recursive leaves and Cargo, never the parent recipe.
mkdir "$FIXTURE/lint-bin"
cat > "$FIXTURE/lint-gate" <<'STUB'
#!/usr/bin/env bash
printf 'make %s\n' "$*" >> "$MAKE_PROBE_EVENTS"
STUB
cat > "$FIXTURE/lint-bin/cargo" <<'STUB'
#!/usr/bin/env bash
set -euo pipefail
printf 'cargo %s\n' "$*" >> "$MAKE_PROBE_EVENTS"
if [[ "${2:-}" == clippy ]]; then
    case "$*" in
        "+$FIXTURE_TOOLCHAIN clippy --workspace --locked --offline --all-targets -- -D warnings"|\
        "+$FIXTURE_TOOLCHAIN clippy --package ic-metrics-storage --locked --offline --all-targets -- -D warnings"|\
        "+$FIXTURE_TOOLCHAIN clippy --package ic-logging-storage --locked --offline --all-targets -- -D warnings") ;;
        *) exit 1 ;;
    esac
    exit "$MAKE_PROBE_LINT_STATUS"
fi
STUB
chmod +x "$FIXTURE/lint-gate" "$FIXTURE/lint-bin/cargo"
cat > "$FIXTURE/scripts/ci/package-workspace.sh" <<'STUB'
#!/usr/bin/env bash
printf 'package %s\n' "$*" >> "$MAKE_PROBE_EVENTS"
STUB
printf '%s\n' \
    'make --no-print-directory shared-tooling-check tools-check format-tools-check validation-compilers-check' \
    "cargo +$FIXTURE_TOOLCHAIN clippy --workspace --locked --offline --all-targets -- -D warnings" \
    > "$FIXTURE/lint-failure-expected"
cp "$FIXTURE/lint-failure-expected" "$FIXTURE/lint-success-expected"
printf '%s\n' \
    "cargo +$FIXTURE_TOOLCHAIN check --workspace --locked --offline --all-targets --features ic-storage/test-support" \
    'make --no-print-directory check-production-graph check-pins check-storage-boundary check-candid-boundary check-license check-doc-links fmt-check' \
    'make --no-print-directory test-release-adapters test-tools test-tooling test-hooks test-package-admission' \
    'make --no-print-directory test-fixture-cell' \
    "cargo +$FIXTURE_TOOLCHAIN test -p ic-storage --locked --offline -- --test-threads=1 --skip repo_tool::" \
    "cargo +$FIXTURE_TOOLCHAIN test -p ic-metrics-storage --locked --offline -- --test-threads=1" \
    "cargo +$FIXTURE_TOOLCHAIN test -p ic-logging-storage --locked --offline -- --test-threads=1" \
    "cargo +$FIXTURE_TOOLCHAIN doc --workspace --locked --offline --no-deps" \
    "cargo +$FIXTURE_TOOLCHAIN check --workspace --locked --offline --target wasm32-unknown-unknown --tests" \
    'make --no-print-directory wasm-size' \
    "package $FIXTURE_TOOLCHAIN --allow-dirty" \
    >> "$FIXTURE/lint-success-expected"
(cd "$FIXTURE"; PATH="$FIXTURE/lint-bin:$PATH" MAKE=./lint-gate \
    make_probe_lint_order "$(command -v make)" "$FIXTURE/Makefile" "$FIXTURE" \
    "$FIXTURE/lint-events" validate-toolchain \
    "$FIXTURE/lint-failure-expected" "$FIXTURE/lint-success-expected")
# Exercise the actual plan runner -> recursive Make -> scoped Cargo recipes.
# Read-only Git identity is substituted; no fixture commit/tag is needed.
cat > "$FIXTURE/lint-bin/git" <<'STUB'
#!/usr/bin/env bash
printf '%s\n' aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
STUB
chmod +x "$FIXTURE/lint-bin/git"
for group in metrics-storage logging-storage; do
    jq -cS --arg group "$group" '{schema:1,source:"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        group:$group,scope:"group",policy:"ci/release-validation.json",
        inputs:["Makefile","scripts/ci/validation-cargo.sh"],
        stages:[.stages[] | {target:.,required:(. == "validate-lint" or . == "validate-features" or . == "validate-docs" or . == "validate-wasm"),reason:"Focused Cargo caller qualification"}]}' \
        "$FIXTURE/ci/release-validation.json" > "$FIXTURE/$group.plan.json"
    : > "$FIXTURE/plan-cargo-events"
    (cd "$FIXTURE"; PATH="$FIXTURE/lint-bin:$PATH" MAKE_PROBE_EVENTS="$FIXTURE/plan-cargo-events" \
        MAKE_PROBE_LINT_STATUS=0 make --no-print-directory validate-release-plan \
        "VALIDATION_PLAN=$group.plan.json" "RELEASE_GROUP=$group" \
        RELEASE_SOURCE=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa \
        "VALIDATION_LOG_DIR=$FIXTURE/$group.plan-logs") > "$FIXTURE/$group.plan-run.log" 2>&1
    printf '%s\n' \
        "cargo +$FIXTURE_TOOLCHAIN clippy --package ic-$group --locked --offline --all-targets -- -D warnings" \
        "cargo +$FIXTURE_TOOLCHAIN check --package ic-$group --locked --offline --all-targets --features ic-storage/test-support" \
        "cargo +$FIXTURE_TOOLCHAIN doc --package ic-$group --locked --offline --no-deps" \
        "cargo +$FIXTURE_TOOLCHAIN check --package ic-$group --locked --offline --target wasm32-unknown-unknown --tests" > "$FIXTURE/plan-cargo-expected"
    cmp "$FIXTURE/plan-cargo-expected" "$FIXTURE/plan-cargo-events"
done
# The consolidated validate target delegates once and preserves gate failure.
cat > "$FIXTURE/compiler-gate" <<'STUB'
#!/usr/bin/env bash
[[ "$#" == 2 && "$*" == '--no-print-directory validate-toolchain' ]] || exit 1
printf '%s\n' "$*" > compiler-gate-events
exit "$COMPILER_GATE_STATUS"
STUB
chmod +x "$FIXTURE/compiler-gate"
for gate in 0 27; do
    status=0
    (cd "$FIXTURE"; COMPILER_GATE_STATUS="$gate" make --no-print-directory validate MAKE=./compiler-gate) > "$FIXTURE/compiler-$gate.log" 2>&1 || status=$?
    [[ "$(cat "$FIXTURE/compiler-gate-events")" == '--no-print-directory validate-toolchain' ]] || exit 1
    if [[ "$gate" == 0 ]]; then [[ "$status" == 0 ]] || exit 1
    else [[ "$status" == 2 ]] || exit 1; fi
done
# Diagnostics stay intact; unintended duplicate recipes must still be visible.
if grep -E 'warning: (overriding|ignoring old) recipe' "$FIXTURE/tool-ordering.log" "$FIXTURE"/lint-*.log "$FIXTURE"/compiler-*.log; then
    echo 'consumer admission fixtures replaced a production recipe' >&2; exit 1
fi
(cd "$FIXTURE"; make --no-print-directory release-patch)
[[ "$(cat "$FIXTURE/runner-events")" == 'patch origin main' ]] || exit 1
# The previous inline assignment forced preparation for every release entrypoint.
# Preserve that contract even when a caller supplies a conflicting selection.
for target in release-patch release-minor release-major release-resume; do
    (cd "$FIXTURE"; RELEASE_CACHE_PREPARE=0 make --no-print-directory "$target" RELEASE_CACHE_PREPARE=0 VERSION=0.1.1)
done
# Memory's standard release entrypoints always select this checkout's runner.
# An external substitute makes a redirected dispatch harmless and observable.
external_root="$FIXTURE/external runner"
mkdir -p "$external_root/scripts/ci"
cat > "$external_root/scripts/ci/check-make-execution.sh" <<'STUB'
#!/usr/bin/env bash
echo escaped >> external-probe-events
exit 23
STUB
cat > "$external_root/scripts/ci/run-release.sh" <<'STUB'
#!/usr/bin/env bash
printf '%s\n' "$*" >> external-runner-events
exit 23
STUB
rm "$FIXTURE/runner-events"
for target in release-patch release-minor release-major release-resume; do
    (cd "$FIXTURE"; SHARED_TOOLING_ROOT="$external_root" make --no-print-directory "$target" VERSION=0.1.1 RELEASE_REMOTE=fixture RELEASE_BRANCH=reviewed)
    (cd "$FIXTURE"; make --no-print-directory "$target" SHARED_TOOLING_ROOT="$external_root" VERSION=0.1.1 RELEASE_REMOTE=fixture RELEASE_BRANCH=reviewed)
done
printf '%s\n' 'patch fixture reviewed' 'patch fixture reviewed' \
    'minor fixture reviewed' 'minor fixture reviewed' \
    'major fixture reviewed' 'major fixture reviewed' \
    'resume 0.1.1 fixture reviewed' 'resume 0.1.1 fixture reviewed' > "$FIXTURE/expected-runner-events"
cmp "$FIXTURE/expected-runner-events" "$FIXTURE/runner-events"
[[ ! -e "$FIXTURE/external-runner-events" ]] || exit 1
[[ ! -e "$FIXTURE/external-probe-events" ]] || exit 1
cat > "$FIXTURE/overrides.mk" <<'MAKE'
RELEASE_REMOTE := recursive
.PHONY: nested
nested:
	+@$(MAKE) release-patch
MAKE
recursive_make="$(command -v make) --no-print-directory -f Makefile -f overrides.mk"
rm "$FIXTURE/runner-events"
(cd "$FIXTURE"; make -j2 --no-print-directory -f Makefile -f overrides.mk nested \
    "MAKE=$recursive_make" "SHARED_TOOLING_ROOT=$external_root" RELEASE_CACHE_PREPARE=0)
[[ "$(cat "$FIXTURE/runner-events")" == 'patch recursive main' ]] || exit 1
[[ ! -e "$FIXTURE/external-runner-events" && ! -e "$FIXTURE/external-probe-events" ]] || exit 1
)

test_jobserver_handoff() (
    # Keep child exit handling and environment local; the outer trap owns retention.
    trap - EXIT
    local FIXTURE="$suite_root/jobserver"
    prepare_make_fixture "$FIXTURE"
# Qualify descriptor handoff through actual consumer Cargo, formatter and helper
# recipes. All executable effects below are substitutes, including publication.
jobserver="$FIXTURE"
mkdir -p "$jobserver/bin"
# Follow the formatter's declared closure, including catalog admission helpers.
bash "$ROOT/scripts/ci/prepare-script-fixture.sh" "$ROOT" "$jobserver" \
    make/rust-format.mk scripts/ci/check-release-delivery.sh
cmp "$ROOT/scripts/ci/run-rust-formatting.pl" "$jobserver/scripts/ci/run-rust-formatting.pl"
cmp "$ROOT/scripts/ci/RustSource.pm" "$jobserver/scripts/ci/RustSource.pm"
mkdir -p "$jobserver/testing/runtime-qualification"
cp "$ROOT/testing/package-consumer.rs" "$jobserver/testing/"
cat > "$jobserver/scripts/ci/package-workspace.sh" <<'STUB'
#!/usr/bin/env bash
exec cargo package "$@"
STUB
export JOBSERVER_EVENTS="$jobserver/events"
cat > "$jobserver/bin/cargo" <<'STUB'
#!/usr/bin/env bash
set -euo pipefail
[[ "${MAKEFLAGS:-}" =~ --jobserver-(auth|fds)=([0-9]+),([0-9]+) ]] || exit 1
reader="${BASH_REMATCH[2]}"; writer="${BASH_REMATCH[3]}"
: <&"$reader"
: >&"$writer"
printf '%s [%s] (%s) %s\n' "${0##*/}" "$PWD" "${RUSTUP_TOOLCHAIN:-}" "$*" >> "$JOBSERVER_EVENTS"
if [[ "${0##*/}" == cargo && "${1:-}" == metadata ]]; then
    printf '{"workspace_root":"%s","workspace_members":["a","b"],"workspace_default_members":["a"],"packages":[{"id":"a","name":"fixture-a","manifest_path":"%s/Cargo.toml"},{"id":"b","name":"fixture-b","manifest_path":"%s/Cargo.toml"}]}\n' "$PWD" "$PWD" "$PWD"
    exit 0
fi
if [[ "${0##*/}" == rustfmt ]]; then
    if [[ "$*" == --version && "${RUSTFMT_TEST_FAIL_VERSION:-0}" == 1 ]]; then
        echo 'fixture standalone formatter admission failure' >&2; exit 37
    fi
    if [[ "$*" == '--edition '* && "${RUSTFMT_TEST_FAIL_PASS:-0}" == 1 ]]; then
        echo 'fixture standalone formatting failure' >&2; exit 39
    fi
fi
[[ "$*" != 'sort --version' ]] || echo 'cargo-sort 2.1.4'
exit 0
STUB
cp "$jobserver/bin/cargo" "$jobserver/bin/rustfmt"
cat > "$jobserver/helper" <<'STUB'
#!/usr/bin/env bash
set -euo pipefail
cargo helper "$@"
case "$1" in target) echo "$PWD/target" ;; version) echo 0.35.1 ;; esac
STUB
chmod +x "$jobserver/bin/cargo" "$jobserver/bin/rustfmt" "$jobserver/helper"
parallel=(-j4)
if make --help | grep -q -- --jobserver-style; then parallel+=(--jobserver-style=pipe); fi
for target in fmt fmt-check test-tooling test fetch-dependencies wasm-size version \
    release-version release-preflight release-verify qualify-release package publish publish-dry-run; do
    (cd "$jobserver"; PATH="$jobserver/bin:$PATH" make --no-print-directory "${parallel[@]}" \
        "$target" TOOL=./helper) > "$jobserver/$target.log" 2>&1
done
grep -Fx "cargo [$jobserver] ($FIXTURE_TOOLCHAIN) sort --workspace --grouped" "$JOBSERVER_EVENTS"
grep -Fx "cargo [$jobserver] ($FIXTURE_TOOLCHAIN) fmt --package fixture-a --package fixture-b" "$JOBSERVER_EVENTS"
grep -Fx "cargo [$jobserver/testing/runtime-qualification] ($FIXTURE_TOOLCHAIN) sort --workspace --grouped" "$JOBSERVER_EVENTS"
grep -Fx "cargo [$jobserver/testing/runtime-qualification] ($FIXTURE_TOOLCHAIN) fmt --package fixture-a --package fixture-b" "$JOBSERVER_EVENTS"
grep -Fx "rustfmt [$jobserver] ($FIXTURE_TOOLCHAIN) --edition 2024 $jobserver/testing/package-consumer.rs" "$JOBSERVER_EVENTS"
grep -Fx "rustfmt [$jobserver] ($FIXTURE_TOOLCHAIN) --edition 2024 --check $jobserver/testing/package-consumer.rs" "$JOBSERVER_EVENTS"
awk '/ (sort --workspace --grouped|fmt --package fixture-a --package fixture-b|--edition 2024)( |$)/ { print; if (++passes == 5) exit }' \
    "$JOBSERVER_EVENTS" > "$jobserver/observed-order"
printf '%s\n' \
    "cargo [$jobserver] ($FIXTURE_TOOLCHAIN) sort --workspace --grouped" \
    "cargo [$jobserver] ($FIXTURE_TOOLCHAIN) fmt --package fixture-a --package fixture-b" \
    "cargo [$jobserver/testing/runtime-qualification] ($FIXTURE_TOOLCHAIN) sort --workspace --grouped" \
    "cargo [$jobserver/testing/runtime-qualification] ($FIXTURE_TOOLCHAIN) fmt --package fixture-a --package fixture-b" \
    "rustfmt [$jobserver] ($FIXTURE_TOOLCHAIN) --edition 2024 $jobserver/testing/package-consumer.rs" \
    > "$jobserver/expected-order"
cmp "$jobserver/expected-order" "$jobserver/observed-order"
grep -F 'helper release-verify' "$JOBSERVER_EVENTS"
grep -F 'helper publish --dry-run' "$JOBSERVER_EVENTS"

# Admission fails before workspace mutation; a later standalone failure retains
# its exact child status/log after workspace passes. Both use the actual driver.
for phase in admission pass; do
    : > "$JOBSERVER_EVENTS"
    status=0
    if [[ "$phase" == admission ]]; then
        (cd "$jobserver"; PATH="$jobserver/bin:$PATH" RUSTFMT_TEST_FAIL_VERSION=1 \
            make --no-print-directory "${parallel[@]}" fmt) > "$jobserver/$phase.log" 2>&1 || status=$?
        expected=37
        if grep -qF 'sort --workspace' "$JOBSERVER_EVENTS"; then exit 1; fi
    else
        (cd "$jobserver"; PATH="$jobserver/bin:$PATH" RUSTFMT_TEST_FAIL_PASS=1 \
            make --no-print-directory "${parallel[@]}" fmt-check) > "$jobserver/$phase.log" 2>&1 || status=$?
        expected=39
        grep -qF 'fmt --package fixture-a --package fixture-b -- --check' "$JOBSERVER_EVENTS"
    fi
    [[ "$status" == 2 ]] || exit 1
    grep -qF "FAILED (exit $expected," "$jobserver/$phase.log"
    retained="$(sed -n 's/^Details: //p' "$jobserver/$phase.log")"
    [[ -f "$retained" ]] || exit 1
    grep -qF 'fixture standalone' "$retained"
    printf 'Standalone %s failure retained with exit %s: %s\n' "$phase" "$expected" "$retained"
done
# Force-run retains the same consumer roster; standalone selections run uncached.
printf '\n.PHONY: fixture-force\nfixture-force: FORMAT_RUST_ARGS += --no-cache\nfixture-force: fmt-check\n' >> "$jobserver/Makefile"
(cd "$jobserver"; PATH="$jobserver/bin:$PATH" make --no-print-directory "${parallel[@]}" fixture-force) > "$jobserver/force.log" 2>&1
grep -qF 'Standalone Rust sources selected; running without shortcut' "$jobserver/force.log"
grep -qF 'Standalone Rust formatting... ok (' "$jobserver/force.log"
# The consumer's validation compiler owns every formatter pass, including a
# caller that supplies a different compiler through the environment or command
# line. Keep all passes selected, including with the macOS GNU Make 3.81 parser.
for compiler_origin in environment command; do
    : > "$JOBSERVER_EVENTS"
    compiler_arguments=()
    if [[ "$compiler_origin" == command ]]; then compiler_arguments+=(RUSTUP_TOOLCHAIN=other-compiler); fi
    # Bash 3.2 treats an empty array as unset under nounset.
    (cd "$jobserver"; PATH="$jobserver/bin:$PATH" RUSTUP_TOOLCHAIN=other-compiler \
        make --no-print-directory "${parallel[@]}" fmt-check \
            ${compiler_arguments[@]+"${compiler_arguments[@]}"}) \
        > "$jobserver/compiler-$compiler_origin.log" 2>&1
    sed -e 's/--edition 2024 /--edition 2024 --check /' \
        -e 's/sort --workspace --grouped$/sort --workspace --grouped --check/' \
        -e 's/fmt --package fixture-a --package fixture-b$/fmt --package fixture-a --package fixture-b -- --check/' \
        "$jobserver/expected-order" > "$jobserver/expected-check-order"
    awk '/ (sort --workspace --grouped|fmt --package fixture-a --package fixture-b|--edition 2024)( |$)/ { print }' \
        "$JOBSERVER_EVENTS" > "$jobserver/compiler-order"
    cmp "$jobserver/expected-check-order" "$jobserver/compiler-order"
done
)

test_release_receipts() (
    # Keep child exit handling and environment local; the outer trap owns retention.
    trap - EXIT
    local FIXTURE="$suite_root/receipts"
    prepare_make_fixture "$FIXTURE"
mkdir -p "$FIXTURE/custom target"
cat > "$FIXTURE/helper" <<'STUB'
#!/usr/bin/env bash
set -euo pipefail
case "$1" in
    target) printf '%s/custom target\n' "$PWD" ;;
    version) echo 0.25.14 ;;
    release-*)
        [[ "$RELEASE_DELIVERY" == direct ]] || exit 1
        [[ "$RELEASE_KIND" == minor && "$RELEASE_PREVIOUS" == 0.25.14 && "$RELEASE_VERSION" == 0.26.0 ]] || exit 1
        [[ "$RELEASE_DATE" == 2026-10-05 && "$RELEASE_SOURCE" == aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa ]] || exit 1
        [[ "$RELEASE_REMOTE" == fixture && "$RELEASE_BRANCH" == reviewed ]] || exit 1
        case "$1" in
            release-committed-check|release-tagged-check|release-push-check)
                [[ "$RELEASE_COMMIT" == bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb ]] || exit 1
                ;;
            *) [[ -z "$RELEASE_COMMIT" ]] || exit 1 ;;
        esac
        printf '%s\n' "$1" >> events
        if [[ "$1" == release-verify ]]; then
            printf 'gate stdout %s\n' "${GATE_STATUS:-0}"
            printf 'gate stderr %s\n' "${GATE_STATUS:-0}" >&2
            exit "${GATE_STATUS:-0}"
        fi
        ;;
    *) exit 2 ;;
esac
STUB
chmod +x "$FIXTURE/helper"
cd "$FIXTURE"
selection=(RELEASE_KIND=minor RELEASE_PREVIOUS=0.25.14 RELEASE_VERSION=0.26.0
    RELEASE_DATE=2026-10-05 RELEASE_SOURCE=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
    RELEASE_REMOTE=fixture RELEASE_BRANCH=reviewed RELEASE_COMMIT= TOOL=./helper)
[[ "$(make --no-print-directory -s release-version TOOL=./helper)" == 0.25.14 ]] || exit 1
targets=(release-preflight release-prepare-version release-prepared-check release-files
    release-commit-check release-committed-check release-tagged-check release-push-check)
for target in "${targets[@]}"; do
    case "$target" in
        release-committed-check|release-tagged-check|release-push-check)
            make --no-print-directory "$target" "${selection[@]}" RELEASE_COMMIT=bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
            ;;
        *) make --no-print-directory "$target" "${selection[@]}" ;;
    esac
done
printf '%s\n' "${targets[@]}" > expected
cmp expected events
if make --no-print-directory release-verify "${selection[@]}" GATE_STATUS=7 > failed.log 2>&1; then
    echo 'failed verification accepted' >&2; exit 1
fi
logs=("custom target/release-validation/attempts/"verify.*)
[[ "${#logs[@]}" == 1 ]] || exit 1
cp "${logs[0]}" saved-failure
make --no-print-directory release-verify "${selection[@]}" GATE_STATUS=0 > success.log 2>&1
logs=("custom target/release-validation/attempts/"verify.*)
[[ "${#logs[@]}" == 2 ]] || exit 1
failed=0
passed=0
for log in "${logs[@]}"; do
    if cmp -s saved-failure "$log"; then failed=$((failed + 1)); fi
    if [[ "$(cat "$log")" == $'gate stdout 0\ngate stderr 0' ]]; then passed=$((passed + 1)); fi
done
[[ "$failed" == 1 && "$passed" == 1 ]] || exit 1
)

test_bootstrap_recovery() (
    # Keep child exit handling and environment local; the outer trap owns retention.
    trap - EXIT
    local FIXTURE="$suite_root/bootstrap"
    prepare_make_fixture "$FIXTURE"
# The actual Make launcher must reach the adapter while only the root lock is
# prepared. Cargo/parser substitutes prove isolation and argument propagation.
mkdir -p "$FIXTURE/scripts/dev" "$FIXTURE/bin" \
    "$FIXTURE/crates/ic-storage/src" "$FIXTURE/crates/ic-storage/examples" "$FIXTURE/ci" "$FIXTURE/make"
cp "$ROOT/scripts/ci/read-cargo-workspace-version.sh" \
    "$ROOT/scripts/ci/validate-release-inputs.pl" "$ROOT/scripts/ci/StableVersion.pm" "$FIXTURE/scripts/ci/"
cp "$ROOT/scripts/dev/run-repo-tool.sh" "$FIXTURE/scripts/dev/"
bash "$ROOT/scripts/ci/prepare-script-fixture.sh" "$ROOT" "$FIXTURE" scripts/ci/observe-cargo-release-bootstrap.sh
cp "$ROOT/scripts/ci/read-storage-packages.sh" "$FIXTURE/scripts/ci/"
mkdir -p "$FIXTURE/crates/ic-storage/tools/repo_tool"
cp "$ROOT/ci/cargo-release-groups.json" "$FIXTURE/ci/"
# Exercise the real published renderer; substitute only tool-store selection.
fixture_editor="$(RUSTUP_TOOLCHAIN="$FIXTURE_TOOLCHAIN" bash "$ROOT/scripts/dev/install-rust-tools.sh" \
    --consumer "$ROOT" --package ic-build --version "$IC_BUILD_RELEASE_METADATA_VERSION" \
    --bin ic-build-release-metadata --profile debug --features cli --check)"
export FIXTURE_RELEASE_EDITOR="$fixture_editor"
cat > "$FIXTURE/scripts/dev/install-rust-tools.sh" <<'STUB'
#!/usr/bin/env bash
set -euo pipefail
[[ "$*" == "--consumer $PWD --package ic-build --version $IC_BUILD_RELEASE_METADATA_VERSION --bin ic-build-release-metadata --profile debug --features cli" ||
   "$*" == "--consumer $PWD --package ic-build --version $IC_BUILD_RELEASE_METADATA_VERSION --bin ic-build-release-metadata --profile debug --features cli --check" ]] || exit 1
[[ "${CARGO_NET_OFFLINE-unset}" == "$CALLER_OFFLINE" ]] || exit 1
exit_status="${EDITOR_STATUS:-0}"
[[ "$exit_status" == 0 ]] || exit "$exit_status"
[[ "${EDITOR_MUTATE_INPUT:-0}" != 1 ]] || printf '\n# changed during editor admission\n' >> Cargo.toml
printf '%s\n' "$FIXTURE_RELEASE_EDITOR"
STUB
cd "$FIXTURE"
cat > Cargo.toml <<'MANIFEST'
[workspace]
members = ["crates/ic-storage", "crates/ic-metrics-storage", "crates/ic-logging-storage"]
[workspace.package]
rust-version = "1.99.1"
[workspace.dependencies]
ic-storage = { path = "crates/ic-storage", version = "0.12.3" }
ic-metrics-storage = { path = "crates/ic-metrics-storage", version = "0.12.3" }
ic-logging-storage = { path = "crates/ic-logging-storage", version = "0.12.3" }
other = "0.12.3"
MANIFEST
cat > Cargo.lock <<'LOCK'
version = 4
[[package]]
name = "ic-storage"
version = "0.12.3"
[[package]]
name = "ic-metrics-storage"
version = "0.12.3"
[[package]]
name = "ic-logging-storage"
version = "0.12.3"
LOCK
echo fixture > crates/ic-storage/README.md
touch crates/ic-storage/src/lib.rs
echo fixture-license > LICENSE
cat > crates/ic-storage/Cargo.toml <<'MANIFEST'
[package]
name = "ic-storage"
version = "0.12.3"
MANIFEST
mkdir -p crates/ic-storage/src
printf '[package]\nname = "ic-storage"\nversion = "0.12.3"\n' > crates/ic-storage/Cargo.toml
echo fixture > crates/ic-storage/README.md
touch crates/ic-storage/src/lib.rs
mkdir -p crates/ic-metrics-storage/src
printf '[package]\nname = "ic-metrics-storage"\nversion = "0.12.3"\n' > crates/ic-metrics-storage/Cargo.toml
echo fixture > crates/ic-metrics-storage/README.md
touch crates/ic-metrics-storage/src/lib.rs
mkdir -p crates/ic-logging-storage/src
printf '[package]\nname = "ic-logging-storage"\nversion = "0.12.3"\n' > crates/ic-logging-storage/Cargo.toml
echo scaffold > crates/ic-logging-storage/README.md
touch crates/ic-logging-storage/src/lib.rs
cp Cargo.toml original-manifest
cat > bin/cargo <<'STUB'
#!/usr/bin/env bash
set -euo pipefail
if [[ "$1" == metadata ]]; then
    [[ "$RUSTUP_TOOLCHAIN" == "$FIXTURE_TOOLCHAIN" && "$CARGO_NET_OFFLINE" == true && "$RUSTUP_AUTO_INSTALL" == 0 ]] || exit 1
    [[ "$*" == 'metadata --locked --no-deps --offline --format-version 1' ]] || exit 1
    "$FIXTURE_REAL_CARGO" "$@" > metadata-output.json
    # Keep the real workspace discovery, with a controlled target observation
    # for the launcher's malformed-target refusal cases.
    jq --arg target "$BOOTSTRAP_TARGET" '.target_directory=$target' metadata-output.json
    exit 0
fi
[[ "$1" == +"$FIXTURE_TOOLCHAIN" ]] || exit 1
[[ "$RUSTUP_AUTO_INSTALL" == 0 && "${CARGO_NET_OFFLINE-unset}" == "$CALLER_OFFLINE" ]] || exit 1
case "$2" in
    fetch)
        [[ "$3" == --locked ]] || exit 1
        offset=4
        if [[ "${RELEASE_CACHE_PREPARE:-0}" != 1 ]]; then
            [[ "$4" == --offline ]] || exit 1
            offset=5
        fi
        if [[ $# -ge "$offset" ]]; then
            [[ "${!offset}" == --manifest-path ]] || exit 1
            offset=$((offset + 1))
            cmp expected-manifest "${!offset}"
            cmp Cargo.lock "$(dirname "${!offset}")/Cargo.lock"
            echo bootstrap-cache >> calls
        else
            [[ $# == $((offset - 1)) ]] || exit 1
            echo normal-cache >> calls
        fi
        if [[ "${CACHE_STATUS:-0}" != 0 ]]; then
            echo 'simulated offline cache failure' >&2
            exit "$CACHE_STATUS"
        fi
        if [[ "${CACHE_MISSING:-0}" == 1 ]]; then
            if [[ "${RELEASE_CACHE_PREPARE:-0}" != 1 || "$CALLER_OFFLINE" == true ]]; then
                echo 'simulated offline cache failure' >&2
                exit 101
            fi
            echo prepared >> preparation-events
        fi
        [[ "${CACHE_MUTATE_INPUT:-0}" != 1 ]] || printf '\n# changed during cache preparation\n' >> Cargo.lock
        ;;
    run)
        [[ "${RELEASE_CACHE_PREPARE:-0}" == 0 ]] || exit 1
        [[ "$3" == --locked && "$4" == --offline && "$5" == --quiet ]] || exit 1
        if [[ "$6" == --manifest-path ]]; then
            [[ "$8" == --target-dir && "$9" == "$BOOTSTRAP_TARGET/repo-tool-bootstrap/build" ]] || exit 1
            [[ "${10}" == --example && "${11}" == repo-tool && "${12}" == -- ]] || exit 1
            [[ "${13}" == version || "${13}" == publish ]] || exit 1
            cmp expected-manifest "$7"
            cmp Cargo.lock "$(dirname "$7")/Cargo.lock"
            [[ "$(yq -p toml -o json '.package.version' "$(dirname "$7")/crates/ic-storage/Cargo.toml")" == '"0.13.0"' ]] || exit 1
            cmp crates/ic-storage/README.md "$(dirname "$7")/crates/ic-storage/README.md"
            cmp LICENSE "$(dirname "$7")/LICENSE"
            [[ -d "$(dirname "$7")/crates/ic-storage/src" && ! -L "$(dirname "$7")/crates/ic-storage/src" ]] || exit 1
            [[ -d "$(dirname "$7")/crates/ic-storage/examples" && ! -L "$(dirname "$7")/crates/ic-storage/examples" ]] || exit 1
            cmp crates/ic-storage/src/lib.rs "$(dirname "$7")/crates/ic-storage/src/lib.rs"
            cmp crates/ic-storage/src/lib.rs "$(dirname "$7")/crates/ic-storage/src/lib.rs"
            cmp crates/ic-storage/README.md "$(dirname "$7")/crates/ic-storage/README.md"
            [[ "$(yq -p toml -o json '.package.version' "$(dirname "$7")/crates/ic-storage/Cargo.toml")" == '"0.13.0"' ]] || exit 1
            [[ "$(yq -p toml -o json ' .package.version ' "$(dirname "$7")/crates/ic-metrics-storage/Cargo.toml")" == '"0.13.0"' ]] || exit 1
            cmp crates/ic-metrics-storage/src/lib.rs "$(dirname "$7")/crates/ic-metrics-storage/src/lib.rs"
            [[ "$(yq -p toml -o json ' .package.version ' "$(dirname "$7")/crates/ic-logging-storage/Cargo.toml")" == '"0.13.0"' ]] || exit 1
            cmp crates/ic-logging-storage/src/lib.rs "$(dirname "$7")/crates/ic-logging-storage/src/lib.rs"
            echo bootstrap >> calls
        else
            [[ "$6" == --example && "$7" == repo-tool && "$8" == -- ]] || exit 1
            [[ "$9" == version || "$9" == publish ]] || exit 1
            echo normal >> calls
        fi
        printf '%s\n' "${CARGO_NET_OFFLINE-unset}" > child-offline
        exit_code="${BOOTSTRAP_STATUS:-0}"
        [[ "$exit_code" == 0 ]] || exit "$exit_code"
        echo 0.12.3
        ;;
    *) exit 2 ;;
esac
STUB
cat > bin/yq <<'STUB'
#!/usr/bin/env bash
set -euo pipefail
"$FIXTURE_TOML_YQ" "$@"
case "${!#}" in */"${TOML_PARSER_FAILURE:-no-failure}") exit 23 ;; esac
STUB
chmod +x bin/cargo bin/yq
FIXTURE_REAL_CARGO="$(command -v cargo)"
export FIXTURE_REAL_CARGO FIXTURE_TOML_YQ="$ROOT/.tools/host/bin/yq"
export PATH="$PWD/bin:$PATH" YQ="$PWD/bin/yq" BOOTSTRAP_TARGET="$PWD/custom target"
unset CARGO_NET_OFFLINE
export CALLER_OFFLINE=unset

check_publication_environment() {
    local offline
    cp calls saved-version-calls
    for offline in unset false true; do
        export CALLER_OFFLINE="$offline"
        if [[ "$offline" == unset ]]; then unset CARGO_NET_OFFLINE
        else export CARGO_NET_OFFLINE="$offline"; fi
        make --no-print-directory publish > "publish-$offline.log" 2>&1
        [[ "$(cat child-offline)" == "$offline" ]] || exit 1
    done
    unset CARGO_NET_OFFLINE
    export CALLER_OFFLINE=unset
    cp saved-version-calls calls
}

check_release_cache_preparation() {
    local offline status
    cp calls saved-cache-calls
    for offline in unset false true; do
        export CALLER_OFFLINE="$offline"
        if [[ "$offline" == unset ]]; then unset CARGO_NET_OFFLINE
        else export CARGO_NET_OFFLINE="$offline"; fi
        : > calls
        : > preparation-events
        status=0
        CACHE_MISSING=1 RELEASE_CACHE_PREPARE=1 bash scripts/dev/run-repo-tool.sh "$FIXTURE_TOOLCHAIN" version \
            > "release-cache-$offline.log" 2>&1 || status=$?
        if [[ "$offline" == true ]]; then
            [[ "$status" == 101 && ! -s preparation-events ]] || exit 1
            grep -F 'Run make fetch-dependencies' "release-cache-$offline.log"
            if grep -E '^(normal|bootstrap)$' calls; then exit 1; fi
        else
            [[ "$status" == 0 && "$(cat preparation-events)" == prepared ]] || exit 1
            grep -Fx 0.12.3 "release-cache-$offline.log"
            grep -E '^(normal|bootstrap)$' calls
        fi
    done
    unset CARGO_NET_OFFLINE
    export CALLER_OFFLINE=unset
    : > calls
    status=0
    CACHE_STATUS=9 RELEASE_CACHE_PREPARE=1 bash scripts/dev/run-repo-tool.sh "$FIXTURE_TOOLCHAIN" version \
        > release-network-failure.log 2>&1 || status=$?
    [[ "$status" == 9 ]] || exit 1
    if grep -E '^(normal|bootstrap)$' calls; then exit 1; fi
    cp saved-cache-calls calls
}

# A failed or malformed roster cannot dispatch cache preparation or the helper.
roster=ci/cargo-release-groups.json
cp "$roster" original-roster
for invalid in 'not-json' '["ic-storage"]' '["ic-storage","ic-storage","ic-metrics-storage"]' '["ic-storage","ic-storage","ic-metrics-storage","ic-metrics-storage"]' '["ic-storage","ic-storage","ic-metrics-storage","../escape"]'; do
    printf '%s\n' "$invalid" > "$roster"
    status=0
    bash scripts/dev/run-repo-tool.sh "$FIXTURE_TOOLCHAIN" version > roster-refusal.log 2>&1 || status=$?
    [[ "$status" != 0 && ! -e calls ]] || exit 1
done
cp original-roster "$roster"
[[ "$(make --no-print-directory -s release-version)" == 0.12.3 ]] || exit 1
[[ "$(cat calls)" == $'normal-cache\nnormal' ]] || exit 1
check_publication_environment
check_release_cache_preparation
cp calls successful-calls
status=0
CACHE_STATUS=101 bash scripts/dev/run-repo-tool.sh "$FIXTURE_TOOLCHAIN" version > cache-refusal.log 2>&1 || status=$?
[[ "$status" == 101 ]] || exit 1
grep -F 'simulated offline cache failure' cache-refusal.log
grep -F 'Run make fetch-dependencies' cache-refusal.log
printf '%s\n' normal-cache >> successful-calls
cmp successful-calls calls
cmp original-manifest Cargo.toml
cp calls saved-calls
cat > expected-manifest <<'MANIFEST'
[workspace]
members = ["crates/ic-storage", "crates/ic-metrics-storage", "crates/ic-logging-storage"]
[workspace.package]
rust-version = "1.99.1"
[workspace.dependencies]
ic-storage = { path = "crates/ic-storage", version = "0.13.0" }
ic-metrics-storage = { path = "crates/ic-metrics-storage", version = "0.13.0" }
ic-logging-storage = { path = "crates/ic-logging-storage", version = "0.13.0" }
other = "0.12.3"
MANIFEST
"$FIXTURE_RELEASE_EDITOR" --max-input-bytes "$CARGO_RELEASE_METADATA_MAX_BYTES" lock Cargo.lock 0.12.3 0.13.0 ic-storage > split-lock
# Each group now has one package; a partial multi-member core lock is impossible.
cp split-lock storage-lock
"$FIXTURE_RELEASE_EDITOR" --max-input-bytes "$CARGO_RELEASE_METADATA_MAX_BYTES" lock storage-lock 0.12.3 0.13.0 ic-metrics-storage > metrics-lock
"$FIXTURE_RELEASE_EDITOR" --max-input-bytes "$CARGO_RELEASE_METADATA_MAX_BYTES" lock metrics-lock 0.12.3 0.13.0 ic-logging-storage > prepared-lock
cp prepared-lock Cargo.lock
[[ "$(make --no-print-directory -s release-version 2> bootstrap.log)" == 0.12.3 ]] || exit 1
printf '%s\n' bootstrap-cache bootstrap >> saved-calls
cmp saved-calls calls
check_publication_environment
check_release_cache_preparation
cmp original-manifest Cargo.toml
cmp prepared-lock Cargo.lock
cp calls successful-calls
status=0
CACHE_STATUS=101 bash scripts/dev/run-repo-tool.sh "$FIXTURE_TOOLCHAIN" version > bootstrap-cache-refusal.log 2>&1 || status=$?
[[ "$status" == 101 ]] || exit 1
grep -F 'simulated offline cache failure' bootstrap-cache-refusal.log
grep -F 'Run make fetch-dependencies' bootstrap-cache-refusal.log
printf '%s\n' bootstrap-cache >> successful-calls
cmp successful-calls calls
cmp original-manifest Cargo.toml
cmp prepared-lock Cargo.lock
if BOOTSTRAP_STATUS=7 make --no-print-directory -s release-version > refusal.log 2>&1; then
    echo 'launcher ignored a failed locked bootstrap' >&2; exit 1
fi
cmp original-manifest Cargo.toml
cmp prepared-lock Cargo.lock
# Shared path admission rejects CR/LF before creating recovery tooling state.
normal_target="$BOOTSTRAP_TARGET"
for suffix in $'\n\n' $'\r'; do
    export BOOTSTRAP_TARGET="$PWD/invalid target$suffix"
    if make --no-print-directory -s release-version > invalid-target.log 2>&1; then
        echo 'launcher admitted a forbidden target directory' >&2; exit 1
    fi
    grep -F 'invalid bootstrap target directory' invalid-target.log
    [[ ! -e "$BOOTSTRAP_TARGET" && ! -e "$PWD/invalid target" ]] || exit 1
    cmp original-manifest Cargo.toml
    cmp prepared-lock Cargo.lock
done
export BOOTSTRAP_TARGET="$normal_target"
# Failed admission must stop before helper compilation or source writes.
cp calls saved-editor-calls
status=0
EDITOR_STATUS=19 bash scripts/dev/run-repo-tool.sh "$FIXTURE_TOOLCHAIN" version > editor-refusal.log 2>&1 || status=$?
[[ "$status" == 19 ]] || exit 1
cmp saved-editor-calls calls
cmp original-manifest Cargo.toml
cmp prepared-lock Cargo.lock
# Plausible parser output never overrides its failed observation status.
cp bin/yq bin/failed-yq
printf '%s\n' 'exit 23' >> bin/failed-yq
cp calls failed-parser-calls
status=0
YQ="$PWD/bin/failed-yq" bash scripts/dev/run-repo-tool.sh "$FIXTURE_TOOLCHAIN" version > failed-parser.log 2>&1 || status=$?
[[ "$status" == 23 ]] || exit 1
cmp failed-parser-calls calls
cmp original-manifest Cargo.toml
cmp prepared-lock Cargo.lock
export BOOTSTRAP_TARGET="$normal_target"
# Common helpers preserve supported TOML formatting and leave dependency text intact.
cp calls saved-format-calls
cat > Cargo.toml <<'MANIFEST'
[workspace]
members = ["crates/ic-storage", "crates/ic-metrics-storage", "crates/ic-logging-storage"]
[ workspace.package ] # selected metadata
  rust-version   = "1.99.1" # keep this comment
[workspace.dependencies]
ic-storage = { path = "crates/ic-storage", version = "0.12.3" }
ic-metrics-storage = { path = "crates/ic-metrics-storage", version = "0.12.3" }
ic-logging-storage = { path = "crates/ic-logging-storage", version = "0.12.3" }
other = "0.12.3"
MANIFEST
cp Cargo.toml formatted-manifest
cat > expected-manifest <<'MANIFEST'
[workspace]
members = ["crates/ic-storage", "crates/ic-metrics-storage", "crates/ic-logging-storage"]
[ workspace.package ] # selected metadata
  rust-version   = "1.99.1" # keep this comment
[workspace.dependencies]
ic-storage = { path = "crates/ic-storage", version = "0.13.0" }
ic-metrics-storage = { path = "crates/ic-metrics-storage", version = "0.13.0" }
ic-logging-storage = { path = "crates/ic-logging-storage", version = "0.13.0" }
other = "0.12.3"
MANIFEST
[[ "$(make --no-print-directory -s release-version 2> formatted.log)" == 0.12.3 ]] || exit 1
cmp formatted-manifest Cargo.toml
cmp prepared-lock Cargo.lock
# Derived versions alone cannot authenticate an observation after helpers.
cp calls before-mutation-calls
status=0
EDITOR_MUTATE_INPUT=1 bash scripts/dev/run-repo-tool.sh "$FIXTURE_TOOLCHAIN" version > mutation-editor.log 2>&1 || status=$?
[[ "$status" != 0 ]] || exit 1
grep -F 'bootstrap inputs changed during helper dispatch' mutation-editor.log
cmp before-mutation-calls calls
cp Cargo.toml mutated-manifest.retained
cp formatted-manifest Cargo.toml
status=0
CACHE_MUTATE_INPUT=1 bash scripts/dev/run-repo-tool.sh "$FIXTURE_TOOLCHAIN" version > mutation-cache.log 2>&1 || status=$?
[[ "$status" != 0 ]] || exit 1
grep -F 'bootstrap inputs changed during helper dispatch' mutation-cache.log
printf '%s\n' bootstrap-cache >> before-mutation-calls
cmp before-mutation-calls calls
cp Cargo.lock mutated-lock.retained
cp prepared-lock Cargo.lock
# A parser emitting plausible facts before failing cannot start Cargo preparation.
for input in Cargo.toml Cargo.lock crates/ic-storage/Cargo.toml crates/ic-metrics-storage/Cargo.toml crates/ic-logging-storage/Cargo.toml; do
    cp calls saved-parser-calls
    status=0
    TOML_PARSER_FAILURE="$input" bash scripts/dev/run-repo-tool.sh "$FIXTURE_TOOLCHAIN" version \
        > "failed-toml-${input//\//_}.log" 2>&1 || status=$?
    [[ "$status" != 0 ]] || exit 1
    cmp saved-parser-calls calls
    cmp formatted-manifest Cargo.toml
    cmp prepared-lock Cargo.lock
done
# Cargo admits manifest syntax before projection, including duplicate-key refusal.
sed '/keep this comment/a\
  rust-version = "1.99.1"' formatted-manifest > Cargo.toml
cp Cargo.toml duplicate-manifest
cp calls saved-parser-calls
if bash scripts/dev/run-repo-tool.sh "$FIXTURE_TOOLCHAIN" version > duplicate-manifest.log 2>&1; then
    echo 'launcher accepted duplicate manifest metadata' >&2; exit 1
fi
cmp saved-parser-calls calls
cmp duplicate-manifest Cargo.toml
cmp prepared-lock Cargo.lock
cp original-manifest Cargo.toml
# Reject malformed lock values before shell capture can trim or discard bytes.
for package in ic-storage ic-metrics-storage ic-logging-storage; do
    for version in '13' '"0.13.0\n"' '"0.13.0\u0000"'; do
        printf 'version = 4\n' > Cargo.lock
        for other in ic-storage ic-metrics-storage ic-logging-storage; do
            value='"0.12.3"'
            if [[ "$other" == "$package" ]]; then value="$version"; fi
            printf '[[package]]\nname = "%s"\nversion = %s\n' "$other" "$value" >> Cargo.lock
        done
        cp Cargo.lock malformed-lock
        if bash scripts/dev/run-repo-tool.sh "$FIXTURE_TOOLCHAIN" version > malformed-lock.log 2>&1; then
            echo 'launcher accepted malformed locked version' >&2; exit 1
        fi
        cmp saved-parser-calls calls
        cmp original-manifest Cargo.toml
        cmp malformed-lock Cargo.lock
    done
done
cp prepared-lock Cargo.lock
cp saved-format-calls calls
cp calls saved-calls
cat >> Cargo.lock <<'LOCK'
[[package]]
name = "ic-storage"
version = "0.14.0"
LOCK
cp Cargo.lock conflicting-lock
if make --no-print-directory -s release-version > conflicting.log 2>&1; then
    echo 'launcher accepted multiple root lock versions' >&2; exit 1
fi
cmp saved-calls calls
cmp original-manifest Cargo.toml
cmp conflicting-lock Cargo.lock
cp prepared-lock Cargo.lock
for package in ic-storage ic-metrics-storage ic-logging-storage; do
    manifest="crates/$package/Cargo.toml"
    cp "$manifest" saved-member
    for refusal in missing explicit-version; do
        if [[ "$refusal" == missing ]]; then rm "$manifest"
        else printf '[package]\nname = "%s"\nversion.workspace = true\n' "$package" > "$manifest"; fi
        cp calls saved-member-calls
        if bash scripts/dev/run-repo-tool.sh "$FIXTURE_TOOLCHAIN" version > member-refusal.log 2>&1; then
            echo 'launcher admitted invalid required package metadata' >&2; exit 1
        fi
        cmp saved-member-calls calls
        cmp original-manifest Cargo.toml
        cmp prepared-lock Cargo.lock
        cp saved-member "$manifest"
    done
done
)

test_release_batch_adoption() (
    trap - EXIT
    local FIXTURE="$suite_root/batch"
    prepare_make_fixture "$FIXTURE"
    cat > "$FIXTURE/scripts/ci/run-release-batch.pl" <<'STUB'
use strict;
use warnings;
open my $out, '>', 'batch-argv' or die $!;
print {$out} join("\n", @ARGV), "\n";
close $out or die $!;
STUB
    for group in storage metrics-storage logging-storage; do
        for kind in patch minor major; do
            (cd "$FIXTURE"; env -u RELEASE_BATCH_ACTIVE make --no-print-directory \
                "release-$kind" RELEASE_GROUP="$group" RELEASE_PUBLISH_TARGET=publish)
            printf '%s\n' --root "$FIXTURE" --groups "$group" --kind "$kind" \
                --remote origin --branch main --delivery direct --publish-target publish \
                --setup-target release-candidate-setup > "$FIXTURE/expected-argv"
            cmp "$FIXTURE/expected-argv" "$FIXTURE/batch-argv"
        done
    done
    (cd "$FIXTURE"; env -u RELEASE_BATCH_ACTIVE make --no-print-directory release-patch \
        RELEASE_GROUP= RELEASE_GROUPS=storage,metrics-storage,logging-storage)
    printf '%s\n' --root "$FIXTURE" --groups storage,metrics-storage,logging-storage \
        --kind patch --remote origin --branch main --delivery direct \
        --setup-target release-candidate-setup > "$FIXTURE/expected-argv"
    cmp "$FIXTURE/expected-argv" "$FIXTURE/batch-argv"
    cat > "$FIXTURE/setup-make" <<'STUB'
#!/usr/bin/env bash
printf '%s\n' "$*" >> setup-events
exit "${SETUP_STATUS:-0}"
STUB
    chmod +x "$FIXTURE/setup-make"
    for mode in prepare check; do
        (cd "$FIXTURE"; make --no-print-directory release-candidate-setup \
            RELEASE_SETUP_MODE="$mode" MAKE=./setup-make)
    done
    printf '%s\n' '--no-print-directory install-tools fetch-dependencies' \
        '--no-print-directory tools-check format-tools-check' > "$FIXTURE/setup-expected"
    cmp "$FIXTURE/setup-expected" "$FIXTURE/setup-events"
    if (cd "$FIXTURE"; SETUP_STATUS=23 make --no-print-directory release-candidate-setup \
        RELEASE_SETUP_MODE=prepare MAKE=./setup-make) > "$FIXTURE/setup-failure.log" 2>&1; then
        echo 'batch setup swallowed prerequisite failure' >&2; exit 1
    fi
    cp "$FIXTURE/setup-events" "$FIXTURE/setup-before"
    if (cd "$FIXTURE"; make --no-print-directory release-candidate-setup \
        RELEASE_SETUP_MODE=unknown MAKE=./setup-make) > "$FIXTURE/setup-mode.log" 2>&1; then
        echo 'batch setup accepted unknown recovery mode' >&2; exit 1
    fi
    cmp "$FIXTURE/setup-before" "$FIXTURE/setup-events"
)

if [[ "${1:-}" == --bootstrap-only && $# == 1 ]]; then
    test_bootstrap_recovery
elif [[ "${1:-}" == --make-only && $# == 1 ]]; then
    test_make_admission
    test_release_batch_adoption
    test_jobserver_handoff
else
    [[ $# == 0 ]] || exit 2
    test_make_admission
    test_release_batch_adoption
    test_jobserver_handoff
    test_release_receipts
    test_bootstrap_recovery
fi
echo 'Consumer selection forwarding, attempt retention and launcher checks passed (substitutes only).'
fixture_complete=true

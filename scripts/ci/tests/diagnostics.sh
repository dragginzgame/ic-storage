#!/usr/bin/env bash
# Offline failure-path fixture. No Cargo, downloads, hosted upload or real secrets.
set -euo pipefail
root="$(CDPATH='' cd -- "$(dirname -- "$0")/../../.." && pwd)"
fixture="$(mktemp -d "${TMPDIR:-/tmp}/storage-ci-diagnostics.XXXXXX")"
fixture="$(cd -P "$fixture" && pwd)"
trap 'rm -rf "$fixture"' EXIT
mkdir -p "$fixture/scripts/ci" "$fixture/runner" "$fixture/store/rust-fixture/.attempt.123" "$fixture/target/toko-tooling/.attempt.early"
cp "$root/scripts/ci/diagnostics.pl" "$root/scripts/ci/setup-step.sh" "$fixture/scripts/ci/"
printf 'compiler fixture: missing header\n' > "$fixture/store/rust-fixture/.attempt.123/install.log"
printf 'attempt: early preparation\nTOKEN=fixture-secret\n' > "$fixture/target/toko-tooling/.attempt.early/install.log"
printf 'outside selected boundary\n' > "$fixture/outside.log"
ln -s "$fixture/outside.log" "$fixture/target/toko-tooling/escape.log"
ln -s "$fixture/store" "$fixture/target/toko-tooling/escape-directory"
status=0
FIXTURE_TOKEN=fixture-secret bash "$fixture/scripts/ci/setup-step.sh" compiler bash -c 'echo "underlying compiler failed: missing fixture header"; echo "Authorization: Bearer fixture-secret"; exit 47' > "$fixture/console" 2>&1 || status=$?
[[ "$status" == 47 ]]
grep -q 'original_status=47' "$fixture/console"
grep -q 'missing fixture header' "$fixture/console"
if grep -q 'fixture-secret' "$fixture/console"; then exit 1; fi
perl "$fixture/scripts/ci/diagnostics.pl" collect "$fixture" "$fixture/runner" "$fixture/store" "$fixture/collected" > "$fixture/index"
grep -q '.attempt.early/install.log' "$fixture/index"
grep -q '.attempt.123/install.log' "$fixture/index"
grep -q 'Missing evidence' "$fixture/index"
if grep -R -q 'fixture-secret\|outside selected boundary' "$fixture/collected"; then exit 1; fi
# Finite log bound and PEM omission.
perl -e 'print "x" x 120, "\n" for 1..10000' > "$fixture/large.log"
perl "$fixture/scripts/ci/diagnostics.pl" excerpt "$fixture/large.log" > "$fixture/bounded"
grep -q 'truncated' "$fixture/bounded"
[[ $(wc -c < "$fixture/bounded") -lt 5000 ]]
printf '%s\n' '-----BEGIN PRIVATE KEY-----' 'private fixture contents' > "$fixture/key.log"
perl "$fixture/scripts/ci/diagnostics.pl" excerpt "$fixture/key.log" | grep -q 'omitted in full'
# Missing evidence is explicit and a collector failure remains visible.
mkdir "$fixture/empty"
status=0
perl "$fixture/scripts/ci/diagnostics.pl" collect "$fixture/empty" "$fixture/empty" "$fixture/empty" "$fixture/missing" > "$fixture/missing-console" || status=$?
[[ "$status" == 3 ]]
grep -q 'No selected diagnostics' "$fixture/missing-console"
# Excerpt failure must not replace the original setup status.
rm "$fixture/scripts/ci/diagnostics.pl"
status=0
bash "$fixture/scripts/ci/setup-step.sh" broken-collector bash -c 'exit 47' > "$fixture/collector-console" 2>&1 || status=$?
[[ "$status" == 47 ]]
grep -q 'Diagnostic excerpt failed' "$fixture/collector-console"
printf 'Offline setup diagnostics failure-path checks passed.\n'

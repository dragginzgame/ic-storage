#!/usr/bin/env bash
# Capture setup before tests exist, preserve its status, and print sanitized detail.
set -euo pipefail
phase="${1:?setup phase required}"
shift
[[ "$phase" =~ ^[a-z0-9-]+$ && $# -gt 0 ]] || exit 2
root="$(CDPATH='' cd -- "$(dirname -- "$0")/../.." && pwd)"
mkdir -p "$root/target/ci-setup"
umask 077
log="$root/target/ci-setup/$phase.log"
status=0
"$@" > "$log" 2>&1 || status=$?
printf 'tool=%s phase=%s status=%s\n' "$(basename -- "$1")" "$phase" "$status" > "$root/target/ci-setup/$phase.status"
printf 'Setup phase=%s original_status=%s evidence=%s\n' "$phase" "$status" "$log"
if ! perl "$root/scripts/ci/diagnostics.pl" excerpt "$log"; then
    echo 'Diagnostic excerpt failed; original setup status is preserved.' >&2
    [[ "$status" != 0 ]] || status=1
fi
exit "$status"

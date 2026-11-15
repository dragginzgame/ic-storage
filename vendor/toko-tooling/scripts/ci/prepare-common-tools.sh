#!/usr/bin/env bash
set -euo pipefail
# The caller selects its checkout; setup and checks remain consumer-owned Make targets.
: "${RUNNER_TEMP:?}" "${GITHUB_WORKSPACE:?}" "${GITHUB_PATH:?}"
make install-tools 2>&1 | tee "$RUNNER_TEMP/tools-install.log"
make tools-check 2>&1 | tee "$RUNNER_TEMP/tools-check.log"
{
    echo "$GITHUB_WORKSPACE/.tools/host/bin"
    echo "$GITHUB_WORKSPACE/.tools/rust/common/bin"
} >> "$GITHUB_PATH"

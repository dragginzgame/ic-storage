#!/usr/bin/env bash
set -euo pipefail
files="$(printf '%s\n' /usr/bin/env Makefile ci/*.env vendor/toko-tooling/ci/tool-versions.env \
    scripts/ci/*.sh scripts/dev/*.sh vendor/toko-tooling/scripts/ci/*.sh \
    vendor/toko-tooling/scripts/dev/*.sh vendor/toko-tooling/.githooks/pre-commit \
    "${SHELLCHECK_BIN:-$(command -v shellcheck)}" "$(command -v jq)" | jq -Rsc 'split("\n")[:-1]')"
jq -cnS --argjson files "$files" '{schema:1,files:$files,configuration:{shell:"bash",config:"none",source_path:"vendor/toko-tooling",scope:"consumer-tooling"}}'

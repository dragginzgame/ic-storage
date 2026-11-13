#!/usr/bin/env bash
set -euo pipefail
exec "${SHELLCHECK_BIN:-shellcheck}" --norc --shell=bash -P vendor/toko-tooling \
    ci/release-tools.env ci/runtime-tools.env vendor/toko-tooling/ci/tool-versions.env \
    scripts/ci/*.sh scripts/dev/*.sh vendor/toko-tooling/scripts/ci/*.sh \
    vendor/toko-tooling/scripts/dev/*.sh vendor/toko-tooling/.githooks/pre-commit

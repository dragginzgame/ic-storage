#!/usr/bin/env bash
# Consumer reuse proof; generic installer matrices remain upstream.
set -euo pipefail
[[ $# == 0 ]] || exit 2
# Check prepared selections first: missing tools must not turn this probe into setup.
make --no-print-directory host-tools-check ic-tools-check rust-tools-check
source vendor/toko-tooling/scripts/ci/fixture-lifecycle.sh
mkdir -p "$PWD/target/fixtures"
sharing="$(fixture_create "$PWD/target/fixtures/ic-storage-tool-reuse.XXXXXX")"
echo "Retained offline tool-reuse qualification: $sharing"
mkdir -p "$sharing/consumer" "$sharing/bin"
printf '#!/bin/sh\necho unexpected-download >&2\nexit 91\n' > "$sharing/bin/curl"
chmod +x "$sharing/bin/curl"
for kind in host ic; do
  for mode in install check; do
    options=()
    if [[ "$kind" == ic ]]; then options+=(--tool wasm-opt); fi
    if [[ "$mode" == check ]]; then options+=(--check); fi
    PATH="$sharing/bin:$PATH" bash "vendor/toko-tooling/scripts/dev/install-$kind-tools.sh" \
      --consumer "$sharing/consumer" ${options[@]+"${options[@]}"} \
      > "$sharing/$kind-$mode.log" 2>&1
  done
  test "$(cd ".tools/$kind" && pwd -P)" = "$(cd "$sharing/consumer/.tools/$kind" && pwd -P)"
done
cp rust-toolchain.toml "$sharing/consumer/"
for mode in install check; do
  options=()
  if [[ "$mode" == check ]]; then options=(--check); fi
  CARGO_NET_OFFLINE=true RUSTUP_AUTO_INSTALL=0 \
    bash vendor/toko-tooling/scripts/dev/install-rust-tools.sh --consumer "$sharing/consumer" \
    ${options[@]+"${options[@]}"} \
    > "$sharing/rust-$mode.log" 2>&1
done
# Equal physical executables prove reuse of the prepared generation.
for tool in cargo-sort cargo-sort-derives; do
  perl -MCwd=abs_path -e 'my ($a,$b)=map {abs_path($_)} @ARGV; exit(defined($a) && defined($b) && $a eq $b ? 0 : 1)' \
    ".tools/rust/common/bin/$tool" "$sharing/consumer/.tools/rust/common/bin/$tool"
done
